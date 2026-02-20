//! Terminal cell representation.

use serde::{Deserialize, Serialize};

/// ANSI color (index or RGB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl Default for Color {
    fn default() -> Self {
        Self::Default
    }
}

/// Visual attributes for a cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellAttributes {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
    pub hidden: bool,
}

/// A single terminal cell (one character slot).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// The character(s) in this cell. May be empty for wide-char continuations.
    pub content: String,
    /// Number of columns this cell occupies (1 for normal, 2 for wide).
    pub width: u8,
    /// Visual attributes.
    pub attrs: CellAttributes,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            content: String::from(" "),
            width: 1,
            attrs: CellAttributes::default(),
        }
    }
}

impl Cell {
    /// Create a cell with a single character and default attributes.
    pub fn new(ch: char) -> Self {
        Self {
            content: ch.to_string(),
            width: 1,
            attrs: CellAttributes::default(),
        }
    }

    /// Reset this cell to a blank space with default attributes.
    pub fn clear(&mut self) {
        self.content.clear();
        self.content.push(' ');
        self.width = 1;
        self.attrs = CellAttributes::default();
    }
}
