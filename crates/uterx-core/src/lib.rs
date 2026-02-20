//! uterx-core — Terminal emulator core
//!
//! Provides VTE/ANSI parsing, terminal cell grid, scrollback buffer,
//! and Unicode 17 support.

pub mod cell;
pub mod grid;
pub mod parser;
pub mod scrollback;

pub use cell::{Cell, CellAttributes};
pub use grid::Grid;
pub use parser::Parser;
pub use scrollback::Scrollback;
