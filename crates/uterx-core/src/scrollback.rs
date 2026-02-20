//! Scrollback buffer for terminal history.

use crate::cell::Cell;

/// A ring-buffer style scrollback that stores rows that have scrolled
/// off the top of the visible grid.
#[derive(Debug, Clone)]
pub struct Scrollback {
    lines: Vec<Vec<Cell>>,
    max_lines: usize,
}

impl Scrollback {
    /// Create a new scrollback buffer with the given maximum line capacity.
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
        }
    }

    /// Push a row into the scrollback buffer.
    /// If the buffer is full, the oldest line is dropped.
    pub fn push(&mut self, row: Vec<Cell>) {
        if self.lines.len() >= self.max_lines {
            self.lines.remove(0);
        }
        self.lines.push(row);
    }

    /// Number of lines currently stored.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get a line by index (0 = oldest).
    pub fn line(&self, index: usize) -> Option<&[Cell]> {
        self.lines.get(index).map(|l| l.as_slice())
    }

    /// Clear the scrollback buffer.
    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

impl Default for Scrollback {
    fn default() -> Self {
        Self::new(10_000)
    }
}
