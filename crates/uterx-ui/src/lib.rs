//! uterx-ui — TUI layer for the terminal.
//!
//! Ratatui-based chrome: tab bar, status bar, borders, search overlay.
//! Crossterm backend for terminal IO and input handling.

pub mod input;
pub mod terminal_view;
pub mod widgets;

pub use terminal_view::TerminalView;
