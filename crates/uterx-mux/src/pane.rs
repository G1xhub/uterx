//! Pane management.

use serde::{Deserialize, Serialize};
use uterx_core::Grid;

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

/// A single terminal pane.
pub struct Pane {
    pub id: PaneId,
    pub title: String,
    pub rect: Rect,
    pub grid: Grid,
    pub focused: bool,
    // TODO: PTY handle, scrollback, parser state
}

impl Pane {
    /// Create a new pane with the given ID and screen region.
    pub fn new(id: PaneId, rect: Rect) -> Self {
        let grid = Grid::new(rect.width as usize, rect.height as usize);
        Self {
            id,
            title: String::from("shell"),
            rect,
            grid,
            focused: false,
        }
    }

    /// Resize this pane to a new rect.
    pub fn resize(&mut self, rect: Rect) {
        self.rect = rect;
        self.grid.resize(rect.width as usize, rect.height as usize);
    }
}
