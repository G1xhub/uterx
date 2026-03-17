//! Custom ratatui widgets: tab bar, status bar, command palette, help overlay, file browser, editor, ai_sidebar.

pub mod ai_sidebar;
pub mod command_palette;
pub mod editor;
pub mod file_browser;
pub mod help_overlay;
pub mod status_bar;
pub mod tab_bar;

pub use ai_sidebar::{AiProvider, AiSidebarField, AiSidebarState, AiSidebarWidget};
pub use command_palette::{CommandEntry, CommandPalette};
pub use editor::{EditorState, EditorWidget};
pub use file_browser::{FileBrowserState, FileBrowserWidget};
pub use help_overlay::HelpOverlay;
pub use status_bar::StatusBar;
pub use tab_bar::TabBar;
