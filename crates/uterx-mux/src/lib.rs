//! uterx-mux — Terminal multiplexer.
//!
//! Manages panes, tabs, sessions, and layouts.
//! Provides broadcast mode and session persistence.

pub mod layout;
pub mod pane;
pub mod session;
pub mod tab;

pub use layout::Layout;
pub use pane::{Pane, PaneId, Rect};
pub use session::Session;
pub use tab::{Tab, TabId};
