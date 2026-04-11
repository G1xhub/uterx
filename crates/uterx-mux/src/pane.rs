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

/// Text selection within a pane (row, col coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

impl Selection {
    /// Create a new selection from start to end coordinates.
    pub fn new(start_row: usize, start_col: usize, end_row: usize, end_col: usize) -> Self {
        Self {
            start_row,
            start_col,
            end_row,
            end_col,
        }
    }

    /// Normalize selection so start comes before end.
    pub fn normalized(&self) -> Self {
        if self.start_row < self.end_row
            || (self.start_row == self.end_row && self.start_col <= self.end_col)
        {
            *self
        } else {
            Self {
                start_row: self.end_row,
                start_col: self.end_col,
                end_row: self.start_row,
                end_col: self.start_col,
            }
        }
    }

    /// Check if a given cell is within the selection.
    pub fn contains(&self, row: usize, col: usize) -> bool {
        let norm = self.normalized();
        if row < norm.start_row || row > norm.end_row {
            return false;
        }
        if row == norm.start_row && col < norm.start_col {
            return false;
        }
        if row == norm.end_row && col > norm.end_col {
            return false;
        }
        true
    }

    /// Get the selected text from a grid.
    pub fn get_text(&self, grid: &Grid) -> String {
        let norm = self.normalized();
        let mut result = String::new();

        for row in norm.start_row..=norm.end_row {
            let start_col = if row == norm.start_row { norm.start_col } else { 0 };
            let end_col = if row == norm.end_row {
                norm.end_col
            } else {
                grid.cols().saturating_sub(1)
            };

            for col in start_col..=end_col {
                if let Some(cell) = grid.cell(row, col) {
                    result.push(cell.content.chars().next().unwrap_or(' '));
                }
            }

            // Add newline between rows (except last)
            if row < norm.end_row {
                result.push('\n');
            }
        }

        result
    }
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
    /// Current text selection (if any).
    pub selection: Option<Selection>,
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
            selection: None,
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
            selection: None,
            pty: None,
            pty_rx: None,
        }
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Start a new selection at the given coordinates.
    pub fn start_selection(&mut self, row: usize, col: usize) {
        self.selection = Some(Selection::new(row, col, row, col));
    }

    /// Update the end of the current selection.
    pub fn update_selection(&mut self, row: usize, col: usize) {
        if let Some(ref mut sel) = self.selection {
            sel.end_row = row;
            sel.end_col = col;
        }
    }

    /// Get the selected text if there is a selection.
    pub fn selected_text(&self) -> Option<String> {
        self.selection.map(|sel| sel.get_text(&self.grid))
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
