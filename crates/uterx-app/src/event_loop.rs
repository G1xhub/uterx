//! Main event loop: terminal setup, input handling, PTY IO, rendering.
//!
//! Uses Tokio for async PTY reads and crossterm's event stream.
//! Renders TabBar + TerminalView + StatusBar via ratatui.

use crate::config::AppConfig;
use crossterm::{
    event::{self, Event, EventStream, KeyCode, KeyModifiers},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::io;
use tokio::sync::mpsc;
use uterx_core::{Grid, Parser};
use uterx_core::parser::apply_actions;
use uterx_platform::pty::PtyProcess;
use uterx_ui::input::{Action, InputHandler};
use uterx_ui::widgets::status_bar::StatusBar;
use uterx_ui::widgets::tab_bar::{TabBar, TabInfo};
use uterx_ui::TerminalView;

/// Run the main terminal event loop.
pub async fn run(cfg: AppConfig) -> anyhow::Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let size = terminal.size()?;
    let cols = size.width;
    let rows = size.height.saturating_sub(2); // Reserve for tab bar + status bar

    // Initialize core components
    let mut grid = Grid::new(cols as usize, rows as usize);
    let mut parser = Parser::new();
    let input_handler = InputHandler::new();

    // Spawn PTY
    let shell = cfg.shell();
    let mut pty = PtyProcess::spawn(&shell, cols, rows)?;

    // Move PTY reader into an async task → channel
    let mut pty_reader = pty.take_reader();
    let (pty_tx, mut pty_rx) = mpsc::channel::<Vec<u8>>(64);

    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;

        let mut buf = [0u8; 8192];
        loop {
            match pty_reader.read(&mut buf).await {
                Ok(0) => break, // EOF — PTY closed
                Ok(n) => {
                    if pty_tx.send(buf[..n].to_vec()).await.is_err() {
                        break; // receiver dropped
                    }
                }
                Err(_) => break,
            }
        }
    });

    // crossterm event stream
    let mut event_stream = EventStream::new();

    // Tab info for the tab bar (single tab for now)
    let session_name = "main";
    let show_tab_bar = cfg.ui.show_tab_bar;
    let show_status_bar = cfg.ui.show_status_bar;

    tracing::info!("event loop started ({}x{}, shell={})", cols, rows, shell);

    loop {
        // Render
        terminal.draw(|frame| {
            let area = frame.area();

            // Layout: optional tab bar (1 row) + terminal + optional status bar (1 row)
            let mut constraints = Vec::new();
            if show_tab_bar {
                constraints.push(Constraint::Length(1));
            }
            constraints.push(Constraint::Min(1));
            if show_status_bar {
                constraints.push(Constraint::Length(1));
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(area);

            let mut idx = 0;

            // Tab bar
            if show_tab_bar {
                let tabs = vec![TabInfo {
                    name: grid.title.clone().unwrap_or_else(|| "shell".to_string()),
                    active: true,
                }];
                let tab_bar = TabBar::new(&tabs);
                frame.render_widget(tab_bar, chunks[idx]);
                idx += 1;
            }

            // Terminal view
            let view = TerminalView::new(&grid);
            frame.render_widget(view, chunks[idx]);
            idx += 1;

            // Status bar
            if show_status_bar && idx < chunks.len() {
                let pane_title = if grid.title.is_empty() {
                    "shell"
                } else {
                    &grid.title
                };
                let status = StatusBar {
                    session_name,
                    pane_title,
                    broadcast: false,
                };
                frame.render_widget(status, chunks[idx]);
            }
        })?;

        // Select on PTY data or user input
        tokio::select! {
            // PTY output arrived
            Some(data) = pty_rx.recv() => {
                let actions = parser.advance(&data);
                apply_actions(&mut grid, &actions);
            }
            // crossterm event
            Some(Ok(ev)) = event_stream.next() => {
                match ev {
                    Event::Key(key) => {
                        // Check against keybinding engine first
                        if let Some(action) = input_handler.resolve(&key) {
                            match action {
                                Action::Quit => break,
                                // Other actions are placeholders for Phase 2
                                _ => {
                                    tracing::debug!("action: {:?}", action);
                                }
                            }
                        } else {
                            // Forward key to PTY
                            let bytes = key_to_bytes(&key);
                            if !bytes.is_empty() {
                                let _ = pty.write(&bytes);
                            }
                        }
                    }
                    Event::Resize(w, h) => {
                        let new_rows = h.saturating_sub(2);
                        grid.resize(w as usize, new_rows as usize);
                        let _ = pty.resize(w, new_rows);
                    }
                    _ => {}
                }
            }
            else => break,
        }
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    tracing::info!("uterx exited");
    Ok(())
}

/// Helper trait extension to get an optional title.
trait TitleExt {
    fn unwrap_or_else<F: FnOnce() -> String>(&self, f: F) -> String;
}

impl TitleExt for String {
    fn unwrap_or_else<F: FnOnce() -> String>(&self, f: F) -> String {
        if self.is_empty() { f() } else { self.clone() }
    }
}

/// Convert a crossterm key event into bytes to send to the PTY.
fn key_to_bytes(key: &crossterm::event::KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+letter → control character
                let ctrl = (c as u8).wrapping_sub(b'a').wrapping_add(1);
                vec![ctrl]
            } else {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n) => match n {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            12 => b"\x1b[24~".to_vec(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}
