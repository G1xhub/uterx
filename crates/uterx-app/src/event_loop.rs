//! Main event loop: terminal setup, input handling, PTY IO, rendering.
//!
//! Uses the uterx-mux Session/Tab/Pane architecture.
//! Each pane owns its own PTY, parser, and grid.
//! Tokio async with crossterm event stream.
//! Features:
//!   - Catppuccin Mocha themed UI
//!   - Tab bar with branding, numbered tabs, action buttons
//!   - Status bar with session info, pane count, keybinding hints
//!   - Help overlay (F1) with full keybinding reference
//!   - Command palette (Ctrl+P) with fuzzy search
//!   - File browser sidebar (Ctrl+E) — desktop-like directory navigation
//!   - Open folders in new panes for a "Terminal Desktop" experience

use crate::config::AppConfig;
use crossterm::{
    event::{
        Event, EventStream, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
        EnableMouseCapture, DisableMouseCapture,
    },
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders},
    Terminal,
};
use std::io;
use uterx_mux::{Rect as MuxRect, Session};
use uterx_ui::input::{Action, InputHandler};
use uterx_ui::widgets::command_palette::{default_commands, CommandEntry, CommandPalette};
use uterx_ui::widgets::file_browser::{FileBrowserState, FileBrowserWidget};
use uterx_ui::widgets::help_overlay::HelpOverlay;
use uterx_ui::widgets::status_bar::StatusBar;
use uterx_ui::widgets::tab_bar::{TabBar, TabInfo};
use uterx_ui::TerminalView;

/// UI overlay state.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Overlay {
    None,
    Help,
    CommandPalette,
}

/// Where keyboard input is currently directed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Focus {
    Terminal,
    FileBrowser,
}

/// Run the main terminal event loop.
pub async fn run(mut cfg: AppConfig) -> anyhow::Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Show help on first launch
    let mut overlay = if cfg.is_first_launch() {
        cfg.mark_first_launch_done()?;
        Overlay::Help
    } else {
        Overlay::None
    };

    let size = terminal.size()?;
    let cols = size.width;
    let rows = size.height.saturating_sub(2); // Reserve for tab bar + status bar

    let shell = cfg.shell();
    let show_tab_bar = cfg.ui.show_tab_bar;
    let show_status_bar = cfg.ui.show_status_bar;

    // Create session with an initial tab + pane
    let mut session = Session::new("main");
    let pane_area = MuxRect {
        x: 0,
        y: 0,
        width: cols,
        height: rows,
    };
    session.create_tab(&shell, pane_area)?;

    // Set the first pane as focused
    if let Some(tab) = session.active_tab_mut() {
        if let Some(pane) = tab.focused_pane_mut() {
            pane.focused = true;
        }
    }

    let input_handler = InputHandler::new();

    // Command palette state
    let commands = default_commands();
    let mut palette_selected: usize = 0;
    let mut palette_filter = String::new();
    let mut filtered_commands: Vec<CommandEntry> = commands.clone();

    // File browser state
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let mut file_browser = FileBrowserState::new(&home);
    file_browser.width = 32;

    // Focus state
    let mut focus = Focus::Terminal;

    // crossterm event stream
    let mut event_stream = EventStream::new();

    tracing::info!(
        "event loop started ({}x{}, shell={}, session={})",
        cols,
        rows,
        shell,
        session.name
    );

    loop {
        // Process PTY output for all panes
        session.process_all_pty_output();

        // Render
        let fb_visible = file_browser.visible;
        let fb_width = file_browser.width;
        let fb_focused = focus == Focus::FileBrowser;

        terminal.draw(|frame| {
            let area = frame.area();

            // Vertical layout: tab bar + main area + status bar
            let mut v_constraints = Vec::new();
            if show_tab_bar {
                v_constraints.push(Constraint::Length(1));
            }
            v_constraints.push(Constraint::Min(1));
            if show_status_bar {
                v_constraints.push(Constraint::Length(1));
            }

            let v_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(v_constraints)
                .split(area);

            let mut v_idx = 0;

            // ── Tab bar ──
            if show_tab_bar {
                let broadcast = session
                    .active_tab()
                    .map(|t| t.broadcast)
                    .unwrap_or(false);
                let tabs: Vec<TabInfo> = session
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let name = if let Some(pane) = t.focused_pane() {
                            if pane.grid.title.is_empty() {
                                t.name.clone()
                            } else {
                                pane.grid.title.clone()
                            }
                        } else {
                            t.name.clone()
                        };
                        TabInfo {
                            name,
                            active: session.active_tab == Some(t.id),
                            index: i,
                        }
                    })
                    .collect();
                let tab_bar = TabBar::new(&tabs).broadcast(broadcast);
                frame.render_widget(tab_bar, v_chunks[v_idx]);
                v_idx += 1;
            }

            // ── Main area (sidebar + terminal) ──
            let main_area = v_chunks[v_idx];
            v_idx += 1;

            // Horizontal split: file browser sidebar + terminal area
            let (sidebar_area, term_area) = if fb_visible {
                let h_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Length(fb_width),
                        Constraint::Min(1),
                    ])
                    .split(main_area);
                (Some(h_chunks[0]), h_chunks[1])
            } else {
                (None, main_area)
            };

            // ── File browser sidebar ──
            if let Some(sb_area) = sidebar_area {
                let fb_widget = FileBrowserWidget::new(&file_browser, fb_focused);
                frame.render_widget(fb_widget, sb_area);
            }

            // ── Terminal panes ──
            if let Some(tab) = session.active_tab() {
                if tab.panes.len() == 1 {
                    if let Some(pane) = tab.panes.first() {
                        let view = TerminalView::new(&pane.grid);
                        frame.render_widget(view, term_area);
                    }
                } else {
                    let mux_area = MuxRect {
                        x: term_area.x,
                        y: term_area.y,
                        width: term_area.width,
                        height: term_area.height,
                    };
                    let rects = tab.layout.compute_rects(mux_area, tab.panes.len());

                    for (pane, mux_rect) in tab.panes.iter().zip(rects.iter()) {
                        let pane_area = ratatui::layout::Rect {
                            x: mux_rect.x,
                            y: mux_rect.y,
                            width: mux_rect.width,
                            height: mux_rect.height,
                        };

                        let (border_style, title_style) = if pane.focused {
                            (
                                Style::default()
                                    .fg(Color::Rgb(137, 180, 250))
                                    .add_modifier(Modifier::BOLD),
                                Style::default()
                                    .fg(Color::Rgb(205, 214, 244))
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            (
                                Style::default().fg(Color::Rgb(69, 71, 90)),
                                Style::default().fg(Color::Rgb(108, 112, 134)),
                            )
                        };

                        let pane_title = if pane.grid.title.is_empty() {
                            "shell".to_string()
                        } else {
                            pane.grid.title.clone()
                        };

                        let block = Block::default()
                            .borders(Borders::ALL)
                            .border_style(border_style)
                            .title(Span::styled(
                                format!(" {} ", pane_title),
                                title_style,
                            ));
                        let inner = block.inner(pane_area);
                        frame.render_widget(block, pane_area);

                        let view = TerminalView::new(&pane.grid)
                            .show_cursor(pane.focused);
                        frame.render_widget(view, inner);
                    }
                }
            }

            // ── Status bar ──
            if show_status_bar && v_idx < v_chunks.len() {
                let (pane_title, broadcast, pane_count, focused_idx) = session
                    .active_tab()
                    .map(|t| {
                        let title = t
                            .focused_pane()
                            .map(|p| {
                                if p.grid.title.is_empty() {
                                    "shell"
                                } else {
                                    p.grid.title.as_str()
                                }
                            })
                            .unwrap_or("shell");
                        let fi = t.panes.iter().position(|p| p.focused).unwrap_or(0);
                        (title, t.broadcast, t.panes.len(), fi)
                    })
                    .unwrap_or(("shell", false, 0, 0));
                let status = StatusBar {
                    session_name: &session.name,
                    pane_title,
                    pane_count,
                    tab_count: session.tabs.len(),
                    broadcast,
                    focused_index: focused_idx,
                };
                frame.render_widget(status, v_chunks[v_idx]);
            }

            // ── Overlays ──
            match &overlay {
                Overlay::Help => {
                    let help = HelpOverlay::new();
                    frame.render_widget(help, area);
                }
                Overlay::CommandPalette => {
                    let palette = CommandPalette::new(
                        &filtered_commands,
                        palette_selected,
                        &palette_filter,
                    );
                    frame.render_widget(palette, area);
                }
                Overlay::None => {}
            }
        })?;

        // ── Input handling ──
        tokio::select! {
            Some(Ok(ev)) = event_stream.next() => {
                match ev {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        // ── Overlay input (takes priority) ──
                        match &overlay {
                            Overlay::Help => {
                                overlay = Overlay::None;
                                continue;
                            }
                            Overlay::CommandPalette => {
                                match key.code {
                                    KeyCode::Esc => {
                                        overlay = Overlay::None;
                                        palette_filter.clear();
                                        palette_selected = 0;
                                        filtered_commands = commands.clone();
                                    }
                                    KeyCode::Up => {
                                        if palette_selected > 0 {
                                            palette_selected -= 1;
                                        }
                                    }
                                    KeyCode::Down => {
                                        if palette_selected + 1 < filtered_commands.len() {
                                            palette_selected += 1;
                                        }
                                    }
                                    KeyCode::Enter => {
                                        if let Some(cmd) = filtered_commands.get(palette_selected) {
                                            let action = palette_action_for(cmd);
                                            overlay = Overlay::None;
                                            palette_filter.clear();
                                            palette_selected = 0;
                                            filtered_commands = commands.clone();

                                            if let Some(a) = action {
                                                if execute_palette_action(
                                                    a,
                                                    &mut session,
                                                    &mut overlay,
                                                    &mut file_browser,
                                                    &mut focus,
                                                    &terminal,
                                                    &shell,
                                                    show_tab_bar,
                                                    show_status_bar,
                                                ) {
                                                    break; // Quit
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Backspace => {
                                        palette_filter.pop();
                                        palette_selected = 0;
                                        filtered_commands = filter_commands(&commands, &palette_filter);
                                    }
                                    KeyCode::Char(c) => {
                                        palette_filter.push(c);
                                        palette_selected = 0;
                                        filtered_commands = filter_commands(&commands, &palette_filter);
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                            Overlay::None => {}
                        }

                        // ── Global keybindings (always processed) ──
                        if let Some(action) = input_handler.resolve(&key) {
                            match action {
                                Action::Quit => break,
                                Action::ShowHelp => {
                                    overlay = Overlay::Help;
                                    continue;
                                }
                                Action::CommandPalette => {
                                    overlay = Overlay::CommandPalette;
                                    palette_filter.clear();
                                    palette_selected = 0;
                                    filtered_commands = commands.clone();
                                    continue;
                                }
                                Action::ToggleFileBrowser => {
                                    file_browser.toggle();
                                    if file_browser.visible {
                                        focus = Focus::FileBrowser;
                                    } else {
                                        focus = Focus::Terminal;
                                    }
                                    // Relayout panes for new available width
                                    let area = compute_pane_area(
                                        &terminal,
                                        show_tab_bar,
                                        show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                    );
                                    session.relayout_all(area);
                                    continue;
                                }
                                _ => {
                                    // Fall through to focus-specific handling below
                                }
                            }
                        }

                        // ── File browser focused input ──
                        if focus == Focus::FileBrowser && file_browser.visible {
                            match key.code {
                                KeyCode::Up => {
                                    file_browser.cursor_up();
                                }
                                KeyCode::Down => {
                                    file_browser.cursor_down();
                                    let h = terminal.size().map(|s| s.height.saturating_sub(4) as usize).unwrap_or(20);
                                    file_browser.adjust_scroll(h);
                                }
                                KeyCode::Enter => {
                                    if let Some(entry) = file_browser.selected_entry() {
                                        if entry.is_dir {
                                            let path = entry.path.clone();
                                            let entry_name = entry.name.clone();
                                            if entry_name == ".." {
                                                // Navigate up
                                                file_browser.navigate_to(path);
                                            } else if entry.expanded {
                                                // Collapse
                                                file_browser.toggle_expand();
                                            } else {
                                                // Expand directory in tree
                                                file_browser.toggle_expand();
                                            }
                                        }
                                    }
                                }
                                KeyCode::Right => {
                                    // Open folder in new pane
                                    if let Some(entry) = file_browser.selected_entry() {
                                        if entry.is_dir && entry.name != ".." {
                                            let path = entry.path.clone();
                                            let area = compute_pane_area(
                                                &terminal,
                                                show_tab_bar,
                                                show_status_bar,
                                                file_browser.width,
                                            );
                                            open_folder_in_pane(
                                                &mut session,
                                                &shell,
                                                &path,
                                                area,
                                            );
                                            focus = Focus::Terminal;
                                        }
                                    }
                                }
                                KeyCode::Left => {
                                    // Navigate to parent in tree
                                    if let Some(parent) = file_browser.root.parent() {
                                        let parent = parent.to_path_buf();
                                        file_browser.navigate_to(parent);
                                    }
                                }
                                KeyCode::Esc | KeyCode::Tab => {
                                    // Return focus to terminal
                                    focus = Focus::Terminal;
                                }
                                KeyCode::Char('r') => {
                                    // Refresh
                                    file_browser.refresh_root();
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // ── Terminal focused input ──
                        if let Some(action) = input_handler.resolve(&key) {
                            match action {
                                Action::NewTab => {
                                    let area = compute_pane_area(
                                        &terminal, show_tab_bar, show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                    );
                                    if let Err(e) = session.create_tab(&shell, area) {
                                        tracing::error!("failed to create tab: {}", e);
                                    }
                                }
                                Action::CloseTab => {
                                    session.close_active_tab();
                                    if session.tabs.is_empty() {
                                        break;
                                    }
                                }
                                Action::NextTab => session.next_tab(),
                                Action::PrevTab => session.prev_tab(),
                                Action::NewPane | Action::SplitVertical => {
                                    let area = compute_pane_area(
                                        &terminal, show_tab_bar, show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                    );
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.relayout(area);
                                    }
                                    if let Err(e) = session.split_vertical(&shell) {
                                        tracing::error!("failed to split vertical: {}", e);
                                    }
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.relayout(area);
                                    }
                                }
                                Action::SplitHorizontal => {
                                    let area = compute_pane_area(
                                        &terminal, show_tab_bar, show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                    );
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.relayout(area);
                                    }
                                    if let Err(e) = session.split_horizontal(&shell) {
                                        tracing::error!("failed to split horizontal: {}", e);
                                    }
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.relayout(area);
                                    }
                                }
                                Action::ClosePane => {
                                    session.close_active_pane();
                                    let tab_empty = session
                                        .active_tab()
                                        .map(|t| t.is_empty())
                                        .unwrap_or(false);
                                    if tab_empty {
                                        session.close_active_tab();
                                        if session.tabs.is_empty() {
                                            break;
                                        }
                                    } else {
                                        let area = compute_pane_area(
                                            &terminal, show_tab_bar, show_status_bar,
                                            if file_browser.visible { file_browser.width } else { 0 },
                                        );
                                        if let Some(tab) = session.active_tab_mut() {
                                            tab.relayout(area);
                                        }
                                    }
                                }
                                Action::NextPane => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.focus_next();
                                    }
                                }
                                Action::PrevPane => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.focus_prev();
                                    }
                                }
                                Action::ToggleBroadcast => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.toggle_broadcast();
                                    }
                                }
                                Action::Search | Action::Fullscreen => {
                                    tracing::debug!("action {:?} not yet implemented", action);
                                }
                                Action::RawInput(data) => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        let _ = tab.write_input(data);
                                    }
                                }
                                // Already handled above
                                Action::Quit | Action::ShowHelp | Action::CommandPalette
                                | Action::ToggleFileBrowser | Action::OpenFolder(_) => {}
                            }
                        } else {
                            // Forward key to active pane
                            let bytes = key_to_bytes(&key);
                            if !bytes.is_empty() {
                                if let Some(tab) = session.active_tab_mut() {
                                    let _ = tab.write_input(&bytes);
                                }
                            }
                        }
                    }
                    Event::Resize(w, h) => {
                        let sidebar_w = if file_browser.visible { file_browser.width } else { 0 };
                        let chrome_rows: u16 =
                            if show_tab_bar { 1 } else { 0 } + if show_status_bar { 1 } else { 0 };
                        let area = MuxRect {
                            x: 0,
                            y: 0,
                            width: w.saturating_sub(sidebar_w),
                            height: h.saturating_sub(chrome_rows),
                        };
                        session.relayout_all(area);
                    }
                    Event::Mouse(mouse) => {
                        if overlay == Overlay::None {
                            // Check if click is in file browser area
                            if file_browser.visible && mouse.column < file_browser.width {
                                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                                    focus = Focus::FileBrowser;
                                    // Click on a specific entry
                                    let header_offset = if show_tab_bar { 1u16 } else { 0 } + 2; // tab bar + header + path
                                    if mouse.row >= header_offset {
                                        let clicked_idx = file_browser.scroll_offset
                                            + (mouse.row - header_offset) as usize;
                                        if clicked_idx < file_browser.entries.len() {
                                            file_browser.cursor = clicked_idx;
                                        }
                                    }
                                }
                            } else {
                                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                                    focus = Focus::Terminal;
                                }
                                handle_mouse_event(
                                    &mut session,
                                    mouse,
                                    show_tab_bar,
                                    if file_browser.visible { file_browser.width } else { 0 },
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(4)) => {}
        }
    }

    // Save session
    let save_path = session.save_path();
    if let Err(e) = session.save_to_file(&save_path) {
        tracing::warn!("failed to save session: {}", e);
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    io::stdout().execute(DisableMouseCapture)?;
    io::stdout().execute(LeaveAlternateScreen)?;
    tracing::info!("uterx exited");
    Ok(())
}

// ── Helper types and functions ──────────────────────────────────────────

/// Actions triggered from the command palette.
enum PaletteAction {
    Quit,
    Help,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    SplitVertical,
    SplitHorizontal,
    NextPane,
    PrevPane,
    Broadcast,
    FileBrowser,
}

/// Map a command palette entry to a PaletteAction.
fn palette_action_for(cmd: &CommandEntry) -> Option<PaletteAction> {
    match cmd.label.as_str() {
        "New Tab" => Some(PaletteAction::NewTab),
        "Close Tab" => Some(PaletteAction::CloseTab),
        "Next Tab" => Some(PaletteAction::NextTab),
        "Previous Tab" => Some(PaletteAction::PrevTab),
        "Split Vertical" => Some(PaletteAction::SplitVertical),
        "Split Horizontal" => Some(PaletteAction::SplitHorizontal),
        "Focus Next Pane" => Some(PaletteAction::NextPane),
        "Focus Previous Pane" => Some(PaletteAction::PrevPane),
        "Toggle Broadcast" => Some(PaletteAction::Broadcast),
        "Help" => Some(PaletteAction::Help),
        "File Browser" => Some(PaletteAction::FileBrowser),
        "Quit" => Some(PaletteAction::Quit),
        _ => None,
    }
}

/// Execute a command palette action. Returns true if the app should quit.
fn execute_palette_action(
    action: PaletteAction,
    session: &mut Session,
    overlay: &mut Overlay,
    file_browser: &mut FileBrowserState,
    focus: &mut Focus,
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    shell: &str,
    show_tab_bar: bool,
    show_status_bar: bool,
) -> bool {
    let sidebar_w = if file_browser.visible { file_browser.width } else { 0 };
    match action {
        PaletteAction::Quit => return true,
        PaletteAction::Help => {
            *overlay = Overlay::Help;
        }
        PaletteAction::FileBrowser => {
            file_browser.toggle();
            if file_browser.visible {
                *focus = Focus::FileBrowser;
            } else {
                *focus = Focus::Terminal;
            }
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar,
                if file_browser.visible { file_browser.width } else { 0 });
            session.relayout_all(area);
        }
        PaletteAction::NewTab => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, sidebar_w);
            let _ = session.create_tab(shell, area);
        }
        PaletteAction::CloseTab => {
            session.close_active_tab();
            if session.tabs.is_empty() {
                return true;
            }
        }
        PaletteAction::NextTab => session.next_tab(),
        PaletteAction::PrevTab => session.prev_tab(),
        PaletteAction::SplitVertical => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, sidebar_w);
            if let Some(tab) = session.active_tab_mut() { tab.relayout(area); }
            let _ = session.split_vertical(shell);
            if let Some(tab) = session.active_tab_mut() { tab.relayout(area); }
        }
        PaletteAction::SplitHorizontal => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, sidebar_w);
            if let Some(tab) = session.active_tab_mut() { tab.relayout(area); }
            let _ = session.split_horizontal(shell);
            if let Some(tab) = session.active_tab_mut() { tab.relayout(area); }
        }
        PaletteAction::NextPane => {
            if let Some(tab) = session.active_tab_mut() { tab.focus_next(); }
        }
        PaletteAction::PrevPane => {
            if let Some(tab) = session.active_tab_mut() { tab.focus_prev(); }
        }
        PaletteAction::Broadcast => {
            if let Some(tab) = session.active_tab_mut() { tab.toggle_broadcast(); }
        }
    }
    false
}

/// Filter commands by substring match.
fn filter_commands(all: &[CommandEntry], filter: &str) -> Vec<CommandEntry> {
    if filter.is_empty() {
        return all.to_vec();
    }
    let f = filter.to_lowercase();
    all.iter()
        .filter(|c| {
            c.label.to_lowercase().contains(&f)
                || c.shortcut.to_lowercase().contains(&f)
                || c.description.to_lowercase().contains(&f)
        })
        .cloned()
        .collect()
}

/// Open a folder in a new terminal pane (cd + shell).
fn open_folder_in_pane(
    session: &mut Session,
    shell: &str,
    folder: &std::path::Path,
    area: MuxRect,
) {
    // Create a new tab with the pane cd'd into the folder
    if let Ok(tab_id) = session.create_tab(shell, area) {
        // Send a cd command to the newly created pane
        if let Some(tab) = session.tabs.iter_mut().find(|t| t.id == tab_id) {
            if let Some(pane) = tab.focused_pane_mut() {
                // Set title to folder name
                let folder_name = folder
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| folder.to_string_lossy().to_string());
                pane.grid.title = folder_name;

                // Send cd command
                let cd_cmd = format!("cd \"{}\"\r", folder.display());
                let _ = pane.write_to_pty(cd_cmd.as_bytes());
            }
        }
        // Switch to the new tab
        session.active_tab = Some(tab_id);
    }
}

/// Compute the pane area accounting for chrome and sidebar.
fn compute_pane_area(
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    show_tab_bar: bool,
    show_status_bar: bool,
    sidebar_width: u16,
) -> MuxRect {
    let size = terminal.size().unwrap_or_default();
    let chrome_rows: u16 =
        if show_tab_bar { 1 } else { 0 } + if show_status_bar { 1 } else { 0 };
    MuxRect {
        x: 0,
        y: 0,
        width: size.width.saturating_sub(sidebar_width),
        height: size.height.saturating_sub(chrome_rows),
    }
}

/// Convert a crossterm key event into bytes to send to the PTY.
fn key_to_bytes(key: &crossterm::event::KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
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

/// Handle a mouse event — focus pane on click, scroll wheel.
fn handle_mouse_event(
    session: &mut Session,
    mouse: crossterm::event::MouseEvent,
    show_tab_bar: bool,
    sidebar_offset: u16,
) {
    let tab = match session.active_tab_mut() {
        Some(t) => t,
        None => return,
    };

    let offset_y = if show_tab_bar { 1u16 } else { 0 };
    let mx = mouse.column.saturating_sub(sidebar_offset);
    let my = mouse.row.saturating_sub(offset_y);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let mut target_pane_id = None;
            for pane in &tab.panes {
                let r = &pane.rect;
                if mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height {
                    target_pane_id = Some(pane.id);
                    break;
                }
            }
            if let Some(pid) = target_pane_id {
                for pane in &mut tab.panes {
                    pane.focused = pane.id == pid;
                }
                tab.active_pane = Some(pid);
            }
        }
        MouseEventKind::ScrollUp => {}
        MouseEventKind::ScrollDown => {}
        _ => {}
    }
}
