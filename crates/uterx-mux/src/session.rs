//! Session persistence — save and restore terminal sessions.

use serde::{Deserialize, Serialize};
use crate::tab::TabId;

/// Serializable session state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub name: String,
    pub tabs: Vec<TabState>,
    pub active_tab: Option<u64>,
}

/// Serializable tab state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabState {
    pub id: u64,
    pub name: String,
    pub layout: String, // serialized layout type
    pub pane_count: usize,
}

/// A live session that holds the current workspace state.
pub struct Session {
    pub name: String,
    pub active_tab: Option<TabId>,
    next_id: u64,
}

impl Session {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            active_tab: None,
            next_id: 1,
        }
    }

    /// Generate a unique ID for a new pane or tab.
    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Serialize the session for persistence.
    pub fn save(&self) -> SessionState {
        SessionState {
            name: self.name.clone(),
            tabs: Vec::new(), // TODO: collect from live tabs
            active_tab: self.active_tab.map(|t| t.0),
        }
    }

    /// Restore a session from saved state.
    pub fn restore(_state: SessionState) -> Self {
        // TODO: recreate tabs, panes, and PTYs
        Self::new("restored")
    }
}
