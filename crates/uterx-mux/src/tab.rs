//! Tab management — each tab contains one or more panes in a layout.

use crate::layout::Layout;
use crate::pane::{Pane, PaneId, Rect};
use serde::{Deserialize, Serialize};

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
    /// If set, this pane is maximized and fills the entire terminal area.
    /// Other panes are hidden while this pane is maximized.
    pub maximized_pane: Option<PaneId>,
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
            maximized_pane: None,
        }
    }

    /// Create a tab with an initial pane running the given shell.
    pub fn with_pane(id: TabId, name: String, pane: Pane) -> Self {
        let pane_id = pane.id;
        Self {
            id,
            name,
            panes: vec![pane],
            layout: Layout::Single,
            active_pane: Some(pane_id),
            broadcast: false,
            maximized_pane: None,
        }
    }

    /// Add a pane to this tab.
    pub fn add_pane(&mut self, pane: Pane) {
        if self.active_pane.is_none() {
            self.active_pane = Some(pane.id);
        }
        self.panes.push(pane);
    }

    /// Remove a pane by id. Returns true if removed.
    pub fn remove_pane(&mut self, id: PaneId) -> bool {
        let before = self.panes.len();
        self.panes.retain(|p| p.id != id);
        let removed = self.panes.len() < before;

        if removed {
            if self.active_pane == Some(id) {
                // Focus the first remaining pane
                self.active_pane = self.panes.first().map(|p| p.id);
            }
            // If the removed pane was maximized, clear the maximized state
            if self.maximized_pane == Some(id) {
                self.maximized_pane = None;
            }
        }
        removed
    }

    /// Split the active pane horizontally (top/bottom).
    /// Returns the new pane ID, or None if no active pane.
    pub fn split_horizontal(&mut self, new_pane: Pane) -> Option<PaneId> {
        if self.active_pane.is_none() {
            return None;
        }
        let new_id = new_pane.id;
        self.panes.push(new_pane);
        self.update_layout_for_split(true);
        Some(new_id)
    }

    /// Split the active pane vertically (left/right).
    /// Returns the new pane ID, or None if no active pane.
    pub fn split_vertical(&mut self, new_pane: Pane) -> Option<PaneId> {
        if self.active_pane.is_none() {
            return None;
        }
        let new_id = new_pane.id;
        self.panes.push(new_pane);
        self.update_layout_for_split(false);
        Some(new_id)
    }

    /// Update layout after adding a pane via split.
    fn update_layout_for_split(&mut self, horizontal: bool) {
        let count = self.panes.len();
        if count <= 1 {
            self.layout = Layout::Single;
        } else if horizontal {
            self.layout = Layout::HorizontalSplit {
                ratios: vec![1.0 / count as f32; count],
            };
        } else {
            self.layout = Layout::VerticalSplit {
                ratios: vec![1.0 / count as f32; count],
            };
        }
    }

    /// Recalculate pane rects based on the current layout and available area.
    /// Only relayouts tiled (non-floating) panes.
    pub fn relayout(&mut self, area: Rect) {
        let tiled_count = self.panes.iter().filter(|p| !p.is_floating).count();
        let rects = self.layout.compute_rects(area, tiled_count);
        let mut rect_iter = rects.into_iter();
        for pane in self.panes.iter_mut() {
            if !pane.is_floating {
                if let Some(rect) = rect_iter.next() {
                    pane.resize(rect);
                }
            }
        }
    }

    /// Focus the next pane (wraps around).
    pub fn focus_next(&mut self) {
        if self.panes.is_empty() {
            return;
        }
        let current_idx = self
            .active_pane
            .and_then(|id| self.panes.iter().position(|p| p.id == id))
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.panes.len();

        // Update focused flags
        if let Some(old) = self.panes.get_mut(current_idx) {
            old.focused = false;
        }
        self.panes[next_idx].focused = true;
        self.active_pane = Some(self.panes[next_idx].id);
    }

    /// Focus the previous pane (wraps around).
    pub fn focus_prev(&mut self) {
        if self.panes.is_empty() {
            return;
        }
        let current_idx = self
            .active_pane
            .and_then(|id| self.panes.iter().position(|p| p.id == id))
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            self.panes.len() - 1
        } else {
            current_idx - 1
        };

        if let Some(old) = self.panes.get_mut(current_idx) {
            old.focused = false;
        }
        self.panes[prev_idx].focused = true;
        self.active_pane = Some(self.panes[prev_idx].id);
    }

    /// Toggle broadcast mode.
    pub fn toggle_broadcast(&mut self) {
        self.broadcast = !self.broadcast;
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

    /// Process PTY output for all panes. Returns true if any pane had data.
    pub fn process_all_pty_output(&mut self) -> bool {
        let mut any = false;
        for pane in &mut self.panes {
            if pane.process_pty_output() {
                any = true;
            }
        }
        any
    }

    /// Write input to the focused pane (or all panes in broadcast mode).
    pub fn write_input(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if self.broadcast {
            for pane in &mut self.panes {
                pane.write_to_pty(data)?;
                // Reset scrollback when user types
                pane.scrollback_offset = 0;
            }
        } else if let Some(pane) = self.focused_pane_mut() {
            pane.write_to_pty(data)?;
            // Reset scrollback when user types
            pane.scrollback_offset = 0;
        }
        Ok(())
    }

    /// Number of panes in this tab.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Check if this tab has no panes left.
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    /// Toggle the focused pane between tiled and floating.
    /// When a pane becomes floating, it gets a centered default position.
    /// When it becomes tiled, it re-joins the layout.
    pub fn toggle_float(&mut self, area: Rect) {
        if let Some(pane) = self.focused_pane_mut() {
            pane.is_floating = !pane.is_floating;
            if pane.is_floating {
                // Set a centered floating rect (60% of area)
                let fw = (area.width as f32 * 0.6) as u16;
                let fh = (area.height as f32 * 0.6) as u16;
                let fx = area.x + (area.width.saturating_sub(fw)) / 2;
                let fy = area.y + (area.height.saturating_sub(fh)) / 2;
                let float_rect = Rect {
                    x: fx,
                    y: fy,
                    width: fw,
                    height: fh,
                };
                pane.resize(float_rect);
            }
        }
        // Re-layout tiled panes
        self.relayout(area);
    }

    /// Get tiled (non-floating) panes.
    pub fn tiled_panes(&self) -> impl Iterator<Item = &Pane> {
        self.panes.iter().filter(|p| !p.is_floating)
    }

    /// Get floating panes.
    pub fn floating_panes(&self) -> impl Iterator<Item = &Pane> {
        self.panes.iter().filter(|p| p.is_floating)
    }

    /// Move a floating pane to an absolute position.
    pub fn move_floating_pane(&mut self, pane_id: PaneId, x: u16, y: u16) {
        if let Some(pane) = self
            .panes
            .iter_mut()
            .find(|p| p.id == pane_id && p.is_floating)
        {
            pane.rect.x = x;
            pane.rect.y = y;
        }
    }

    /// Toggle maximized state for the focused pane.
    /// If the focused pane is already maximized, restore it.
    /// If another pane is maximized, switch to the focused pane.
    /// Returns true if a pane is now maximized, false otherwise.
    pub fn toggle_maximize(&mut self) -> bool {
        if let Some(focused_id) = self.active_pane {
            // If currently maximized pane is the focused one, restore it
            if self.maximized_pane == Some(focused_id) {
                self.maximized_pane = None;
                false
            } else {
                // Maximize the focused pane
                self.maximized_pane = Some(focused_id);
                true
            }
        } else {
            // No focused pane, just clear any maximized state
            self.maximized_pane = None;
            false
        }
    }

    /// Check if any pane is currently maximized.
    pub fn has_maximized_pane(&self) -> bool {
        self.maximized_pane.is_some()
    }

    /// Get the currently maximized pane if any.
    pub fn maximized_pane(&self) -> Option<&Pane> {
        self.maximized_pane
            .and_then(|id| self.panes.iter().find(|p| p.id == id))
    }

    /// Get a mutable reference to the maximized pane if any.
    pub fn maximized_pane_mut(&mut self) -> Option<&mut Pane> {
        let id = self.maximized_pane?;
        self.panes.iter_mut().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::Pane;

    fn make_rect() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        }
    }

    #[test]
    fn test_add_and_focus() {
        let mut tab = Tab::new(TabId(1), "test".into());
        let p1 = Pane::new_bare(PaneId(1), make_rect());
        let p2 = Pane::new_bare(PaneId(2), make_rect());
        tab.add_pane(p1);
        tab.add_pane(p2);

        assert_eq!(tab.pane_count(), 2);
        assert_eq!(tab.active_pane, Some(PaneId(1)));
    }

    #[test]
    fn test_focus_cycling() {
        let mut tab = Tab::new(TabId(1), "test".into());
        let mut p1 = Pane::new_bare(PaneId(1), make_rect());
        p1.focused = true;
        tab.add_pane(p1);
        tab.add_pane(Pane::new_bare(PaneId(2), make_rect()));
        tab.add_pane(Pane::new_bare(PaneId(3), make_rect()));

        tab.focus_next();
        assert_eq!(tab.active_pane, Some(PaneId(2)));

        tab.focus_next();
        assert_eq!(tab.active_pane, Some(PaneId(3)));

        tab.focus_next(); // wraps
        assert_eq!(tab.active_pane, Some(PaneId(1)));

        tab.focus_prev(); // wraps back
        assert_eq!(tab.active_pane, Some(PaneId(3)));
    }

    #[test]
    fn test_remove_pane() {
        let mut tab = Tab::new(TabId(1), "test".into());
        tab.add_pane(Pane::new_bare(PaneId(1), make_rect()));
        tab.add_pane(Pane::new_bare(PaneId(2), make_rect()));

        assert!(tab.remove_pane(PaneId(1)));
        assert_eq!(tab.pane_count(), 1);
        // Active pane should switch to the remaining one
        assert_eq!(tab.active_pane, Some(PaneId(2)));
    }

    #[test]
    fn test_split_vertical() {
        let mut tab = Tab::new(TabId(1), "test".into());
        tab.add_pane(Pane::new_bare(PaneId(1), make_rect()));

        let new_pane = Pane::new_bare(PaneId(2), make_rect());
        let new_id = tab.split_vertical(new_pane);
        assert_eq!(new_id, Some(PaneId(2)));
        assert_eq!(tab.pane_count(), 2);

        match &tab.layout {
            Layout::VerticalSplit { ratios } => {
                assert_eq!(ratios.len(), 2);
            }
            other => panic!("Expected VerticalSplit, got {:?}", other),
        }
    }

    #[test]
    fn test_broadcast_write() {
        let mut tab = Tab::new(TabId(1), "test".into());
        // Bare panes have no PTY, so write_input will silently succeed
        tab.add_pane(Pane::new_bare(PaneId(1), make_rect()));
        tab.add_pane(Pane::new_bare(PaneId(2), make_rect()));
        tab.broadcast = true;

        // Should not error even with no PTY
        tab.write_input(b"hello").unwrap();
    }

    #[test]
    fn test_relayout() {
        let mut tab = Tab::new(TabId(1), "test".into());
        tab.add_pane(Pane::new_bare(
            PaneId(1),
            Rect {
                x: 0,
                y: 0,
                width: 40,
                height: 12,
            },
        ));
        tab.add_pane(Pane::new_bare(
            PaneId(2),
            Rect {
                x: 0,
                y: 0,
                width: 40,
                height: 12,
            },
        ));
        tab.layout = Layout::VerticalSplit {
            ratios: vec![0.5, 0.5],
        };

        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        tab.relayout(area);

        assert_eq!(tab.panes[0].rect.width, 50);
        assert_eq!(tab.panes[1].rect.width, 50);
        assert_eq!(tab.panes[0].rect.height, 30);
    }
}
