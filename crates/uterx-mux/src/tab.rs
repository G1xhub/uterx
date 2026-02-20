//! Tab management — each tab contains one or more panes in a layout.

use serde::{Deserialize, Serialize};
use crate::pane::{Pane, PaneId};
use crate::layout::Layout;

/// Unique identifier for a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u64);

/// A tab groups panes together with a layout.
pub struct Tab {
    pub id: TabId,
    pub name: String,
    pub panes: Vec<Pane>,
    pub layout: Layout,
    pub active_pane: Option<PaneId>,
    /// If true, keyboard input is broadcast to all panes.
    pub broadcast: bool,
}

impl Tab {
    pub fn new(id: TabId, name: String) -> Self {
        Self {
            id,
            name,
            panes: Vec::new(),
            layout: Layout::Single,
            active_pane: None,
            broadcast: false,
        }
    }

    /// Add a pane to this tab.
    pub fn add_pane(&mut self, pane: Pane) {
        if self.active_pane.is_none() {
            self.active_pane = Some(pane.id);
        }
        self.panes.push(pane);
    }

    /// Get the currently focused pane.
    pub fn focused_pane(&self) -> Option<&Pane> {
        let id = self.active_pane?;
        self.panes.iter().find(|p| p.id == id)
    }

    /// Get the currently focused pane mutably.
    pub fn focused_pane_mut(&mut self) -> Option<&mut Pane> {
        let id = self.active_pane?;
        self.panes.iter_mut().find(|p| p.id == id)
    }
}
