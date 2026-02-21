//! Custom ratatui widgets: tab bar, status bar, command palette, help overlay, file browser.

pub mod command_palette;
pub mod file_browser;
pub mod help_overlay;
pub mod status_bar;
pub mod tab_bar;

pub use command_palette::{CommandEntry, CommandPalette};
pub use file_browser::{FileBrowserState, FileBrowserWidget};
pub use help_overlay::HelpOverlay;
pub use status_bar::StatusBar;
pub use tab_bar::TabBar;
