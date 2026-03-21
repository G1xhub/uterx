//! Pane management — each pane owns a PTY, parser, and grid.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uterx_core::{Grid, Parser};
use uterx_core::parser::apply_actions;
use uterx_platform::pty::PtyProcess;

/// Unique identifier for a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u64);

/// Rectangular region on screen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// A single terminal pane with its own PTY process.
pub struct Pane {
    pub id: PaneId,
    pub title: String,
    pub rect: Rect,
    pub grid: Grid,
    pub parser: Parser,
    pub focused: bool,
    /// If true, this pane is floating (absolute position, rendered on top).
    pub is_floating: bool,
    /// Scrollback offset — 0 means show current screen, >0 means scroll up that many lines.
    pub scrollback_offset: usize,
    /// PTY process — owns the writer and master handle.
    pty: Option<PtyProcess>,
    /// Channel receiving output from the PTY reader task.
    pty_rx: Option<mpsc::Receiver<Vec<u8>>>,
}

impl Pane {
    /// Create a new pane with a PTY running the given shell.
    pub fn new(id: PaneId, rect: Rect, shell: &str) -> anyhow::Result<Self> {
        let cols = rect.width;
        let rows = rect.height;
        let grid = Grid::new(cols as usize, rows as usize);
        let parser = Parser::new();

        let mut pty = PtyProcess::spawn(shell, cols, rows)?;

        // Spawn async reader task
        let mut pty_reader = pty.take_reader();
        let (tx, rx) = mpsc::channel::<Vec<u8>>(64);

        tokio::spawn(async move {
            let mut buf = [0u8; 8192];
            loop {
                match pty_reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            id,
            title: String::from("shell"),
            rect,
            grid,
            parser,
            focused: false,
            is_floating: false,
            scrollback_offset: 0,
            pty: Some(pty),
            pty_rx: Some(rx),
        })
    }

    /// Create a pane without a PTY (for testing or placeholder use).
    pub fn new_bare(id: PaneId, rect: Rect) -> Self {
        let grid = Grid::new(rect.width as usize, rect.height as usize);
        Self {
            id,
            title: String::from("shell"),
            rect,
            grid,
            parser: Parser::new(),
            focused: false,
            is_floating: false,
            scrollback_offset: 0,
            pty: None,
            pty_rx: None,
        }
    }

    /// Drain all available PTY output and apply to the grid.
    /// Returns true if any data was processed.
    pub fn process_pty_output(&mut self) -> bool {
        let rx = match self.pty_rx.as_mut() {
            Some(rx) => rx,
            None => return false,
        };

        let mut had_data = false;
        while let Ok(data) = rx.try_recv() {
            let actions = self.parser.advance(&data);
            apply_actions(&mut self.grid, &actions);
            had_data = true;
        }
        had_data
    }

    /// Write bytes to the PTY (send input to the shell).
    pub fn write_to_pty(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if let Some(pty) = self.pty.as_mut() {
            pty.write(data)?;
        }
        Ok(())
    }

    /// Resize this pane to a new rect. Also resizes the PTY.
    pub fn resize(&mut self, rect: Rect) {
        self.rect = rect;
        self.grid.resize(rect.width as usize, rect.height as usize);
        if let Some(pty) = self.pty.as_ref() {
            let _ = pty.resize(rect.width, rect.height);
        }
    }

    /// Toggle floating state.
    pub fn toggle_float(&mut self) {
        self.is_floating = !self.is_floating;
    }

    /// Check if the PTY reader channel is still open.
    pub fn is_alive(&self) -> bool {
        self.pty_rx.is_some()
    }

    /// Take the PTY data receiver (for integration into a select! loop).
    pub fn take_pty_rx(&mut self) -> Option<mpsc::Receiver<Vec<u8>>> {
        self.pty_rx.take()
    }

    /// Put the PTY data receiver back.
    pub fn set_pty_rx(&mut self, rx: mpsc::Receiver<Vec<u8>>) {
        self.pty_rx = Some(rx);
    }
}
