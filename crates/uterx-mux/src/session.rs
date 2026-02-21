//! Session — owns tabs and manages the multiplexer state.
//! Supports save/restore to TOML files for session persistence.

use serde::{Deserialize, Serialize};
use crate::pane::{Pane, PaneId, Rect};
use crate::tab::{Tab, TabId};
use std::path::{Path, PathBuf};

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
    pub layout: String,
    pub pane_count: usize,
}

/// A live session that holds the current multiplexer state.
pub struct Session {
    pub name: String,
    pub tabs: Vec<Tab>,
    pub active_tab: Option<TabId>,
    next_id: u64,
}

impl Session {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tabs: Vec::new(),
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

    /// Create a new tab with an initial pane running the shell.
    /// Returns the TabId.
    pub fn create_tab(&mut self, shell: &str, area: Rect) -> anyhow::Result<TabId> {
        let tab_id = TabId(self.next_id());
        let pane_id = PaneId(self.next_id());
        let pane = Pane::new(pane_id, area, shell)?;
        let tab = Tab::with_pane(tab_id, format!("Tab {}", tab_id.0), pane);
        self.tabs.push(tab);
        if self.active_tab.is_none() {
            self.active_tab = Some(tab_id);
        }
        Ok(tab_id)
    }

    /// Close a tab by ID. Returns true if removed.
    pub fn close_tab(&mut self, id: TabId) -> bool {
        let before = self.tabs.len();
        self.tabs.retain(|t| t.id != id);
        let removed = self.tabs.len() < before;

        if removed && self.active_tab == Some(id) {
            self.active_tab = self.tabs.first().map(|t| t.id);
        }
        removed
    }

    /// Close the currently active tab. Returns the new active tab, if any.
    pub fn close_active_tab(&mut self) -> Option<TabId> {
        if let Some(id) = self.active_tab {
            self.close_tab(id);
        }
        self.active_tab
    }

    /// Switch to the next tab (wraps around).
    pub fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let idx = self.active_tab
            .and_then(|id| self.tabs.iter().position(|t| t.id == id))
            .unwrap_or(0);
        let next = (idx + 1) % self.tabs.len();
        self.active_tab = Some(self.tabs[next].id);
    }

    /// Switch to the previous tab (wraps around).
    pub fn prev_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let idx = self.active_tab
            .and_then(|id| self.tabs.iter().position(|t| t.id == id))
            .unwrap_or(0);
        let prev = if idx == 0 { self.tabs.len() - 1 } else { idx - 1 };
        self.active_tab = Some(self.tabs[prev].id);
    }

    /// Get the active tab.
    pub fn active_tab(&self) -> Option<&Tab> {
        let id = self.active_tab?;
        self.tabs.iter().find(|t| t.id == id)
    }

    /// Get the active tab mutably.
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        let id = self.active_tab?;
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Split the focused pane in the active tab horizontally.
    pub fn split_horizontal(&mut self, shell: &str) -> anyhow::Result<Option<PaneId>> {
        let next_id = self.next_id();
        let tab = match self.active_tab_mut() {
            Some(t) => t,
            None => return Ok(None),
        };
        let area = tab
            .focused_pane()
            .map(|p| p.rect)
            .unwrap_or(Rect { x: 0, y: 0, width: 80, height: 24 });

        let pane = Pane::new(PaneId(next_id), area, shell)?;
        let new_id = tab.split_horizontal(pane);
        Ok(new_id)
    }

    /// Split the focused pane in the active tab vertically.
    pub fn split_vertical(&mut self, shell: &str) -> anyhow::Result<Option<PaneId>> {
        let next_id = self.next_id();
        let tab = match self.active_tab_mut() {
            Some(t) => t,
            None => return Ok(None),
        };
        let area = tab
            .focused_pane()
            .map(|p| p.rect)
            .unwrap_or(Rect { x: 0, y: 0, width: 80, height: 24 });

        let pane = Pane::new(PaneId(next_id), area, shell)?;
        let new_id = tab.split_vertical(pane);
        Ok(new_id)
    }

    /// Close the focused pane in the active tab.
    pub fn close_active_pane(&mut self) -> bool {
        let tab = match self.active_tab_mut() {
            Some(t) => t,
            None => return false,
        };
        let pane_id = match tab.active_pane {
            Some(id) => id,
            None => return false,
        };
        tab.remove_pane(pane_id)
    }

    /// Process PTY output for all panes in all tabs. Returns true if any had data.
    pub fn process_all_pty_output(&mut self) -> bool {
        let mut any = false;
        for tab in &mut self.tabs {
            if tab.process_all_pty_output() {
                any = true;
            }
        }
        any
    }

    /// Relayout all tabs with the given terminal area (excluding chrome).
    pub fn relayout_all(&mut self, area: Rect) {
        for tab in &mut self.tabs {
            tab.relayout(area);
        }
    }

    /// Number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    // ── Persistence ────────────────────────────────────────────────────

    /// Serialize the session for persistence.
    pub fn save_state(&self) -> SessionState {
        SessionState {
            name: self.name.clone(),
            tabs: self.tabs.iter().map(|t| TabState {
                id: t.id.0,
                name: t.name.clone(),
                layout: format!("{:?}", t.layout),
                pane_count: t.pane_count(),
            }).collect(),
            active_tab: self.active_tab.map(|t| t.0),
        }
    }

    /// Save session state to a TOML file.
    pub fn save_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let state = self.save_state();
        let toml_str = toml::to_string_pretty(&state)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml_str)?;
        tracing::info!("session saved to {}", path.display());
        Ok(())
    }

    /// Load session state from a TOML file.
    pub fn load_state(path: &Path) -> anyhow::Result<SessionState> {
        let content = std::fs::read_to_string(path)?;
        let state: SessionState = toml::from_str(&content)?;
        Ok(state)
    }

    /// Restore a session from saved state by re-spawning PTYs.
    /// Each tab gets recreated with fresh panes running the given shell.
    pub fn restore(state: &SessionState, shell: &str, area: Rect) -> anyhow::Result<Self> {
        let mut session = Self::new(&state.name);

        for tab_state in &state.tabs {
            // Recreate each tab with the number of panes it had
            let tab_id = TabId(session.next_id());
            let mut tab = Tab::new(tab_id, tab_state.name.clone());

            for _ in 0..tab_state.pane_count.max(1) {
                let pane_id = PaneId(session.next_id());
                let pane = Pane::new(pane_id, area, shell)?;
                tab.add_pane(pane);
            }

            // Restore layout type heuristic
            if tab.pane_count() > 1 {
                if tab_state.layout.contains("Horizontal") {
                    tab.layout = crate::layout::Layout::HorizontalSplit {
                        ratios: vec![1.0 / tab.pane_count() as f32; tab.pane_count()],
                    };
                } else {
                    tab.layout = crate::layout::Layout::VerticalSplit {
                        ratios: vec![1.0 / tab.pane_count() as f32; tab.pane_count()],
                    };
                }
            }
            tab.relayout(area);
            session.tabs.push(tab);
        }

        // Restore active tab
        if let Some(active_id) = state.active_tab {
            session.active_tab = session.tabs.iter().find(|t| t.id.0 == active_id).map(|t| t.id);
        }
        if session.active_tab.is_none() {
            session.active_tab = session.tabs.first().map(|t| t.id);
        }

        tracing::info!("session '{}' restored with {} tabs", session.name, session.tabs.len());
        Ok(session)
    }

    /// Default sessions directory: ~/.uterx/sessions/
    pub fn sessions_dir() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".uterx").join("sessions")
    }

    /// Path for this session's save file.
    pub fn save_path(&self) -> PathBuf {
        Self::sessions_dir().join(format!("{}.toml", self.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_roundtrip() {
        let state = SessionState {
            name: "test-session".into(),
            tabs: vec![
                TabState {
                    id: 1,
                    name: "Tab 1".into(),
                    layout: "Single".into(),
                    pane_count: 1,
                },
                TabState {
                    id: 2,
                    name: "Tab 2".into(),
                    layout: "VerticalSplit { ratios: [0.5, 0.5] }".into(),
                    pane_count: 2,
                },
            ],
            active_tab: Some(1),
        };

        // Serialize to TOML
        let toml_str = toml::to_string_pretty(&state).unwrap();
        assert!(toml_str.contains("test-session"));
        assert!(toml_str.contains("Tab 1"));
        assert!(toml_str.contains("Tab 2"));

        // Deserialize back
        let restored: SessionState = toml::from_str(&toml_str).unwrap();
        assert_eq!(restored.name, "test-session");
        assert_eq!(restored.tabs.len(), 2);
        assert_eq!(restored.active_tab, Some(1));
        assert_eq!(restored.tabs[1].pane_count, 2);
    }

    #[test]
    fn test_save_to_file_and_load() {
        let tmp = std::env::temp_dir().join("uterx-test-session.toml");

        let mut session = Session::new("test-persist");
        // Add bare tabs directly (no PTY needed)
        let tab = Tab::new(TabId(1), "Tab 1".into());
        session.tabs.push(tab);
        session.active_tab = Some(TabId(1));

        // Save
        session.save_to_file(&tmp).unwrap();
        assert!(tmp.exists());

        // Load
        let state = Session::load_state(&tmp).unwrap();
        assert_eq!(state.name, "test-persist");
        assert_eq!(state.tabs.len(), 1);

        // Cleanup
        let _ = std::fs::remove_file(&tmp);
    }
}
