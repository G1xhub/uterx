//! Custom ratatui widgets: tab bar, status bar, command palette, help overlay, file browser, editor, context menu.

pub mod command_palette;
pub mod context_menu;
pub mod editor;
pub mod file_browser;
pub mod help_overlay;
pub mod status_bar;
pub mod tab_bar;

pub use command_palette::{CommandCategory, CommandEntry, CommandPalette, default_commands};
pub use context_menu::{ContextMenuAction, ContextMenuItem, ContextMenuState, ContextMenuWidget};
pub use editor::{EditorState, EditorWidget};
pub use file_browser::{FileBrowserState, FileBrowserWidget};
pub use help_overlay::HelpOverlay;
pub use status_bar::StatusBar;
pub use tab_bar::TabBar;
