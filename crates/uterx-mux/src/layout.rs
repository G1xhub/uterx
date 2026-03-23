//! Layout engine for arranging panes within a tab.

use crate::pane::Rect;
use serde::{Deserialize, Serialize};

/// Layout modes for panes within a tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Layout {
    /// Single pane fills the entire tab area.
    Single,
    /// Horizontal split (panes stacked top-to-bottom).
    HorizontalSplit { ratios: Vec<f32> },
    /// Vertical split (panes side by side).
    VerticalSplit { ratios: Vec<f32> },
    /// Tiled layout — TODO: tree-based splitting.
    Tiled,
    /// Floating panes with absolute positions.
    Floating,
}

impl Default for Layout {
    fn default() -> Self {
        Self::Single
    }
}

impl Layout {
    /// Compute pane rects for the given number of panes within a bounding area.
    pub fn compute_rects(&self, area: Rect, count: usize) -> Vec<Rect> {
        if count == 0 {
            return Vec::new();
        }
        match self {
            Layout::Single => vec![area],
            Layout::HorizontalSplit { ratios } => {
                split_rects(area, ratios, count, SplitDirection::Horizontal)
            }
            Layout::VerticalSplit { ratios } => {
                split_rects(area, ratios, count, SplitDirection::Vertical)
            }
            Layout::Tiled | Layout::Floating => {
                // Fallback: equal vertical split
                let ratios = vec![1.0 / count as f32; count];
                split_rects(area, &ratios, count, SplitDirection::Vertical)
            }
        }
    }

    /// Get the split direction if this is a split layout.
    pub fn split_direction(&self) -> Option<SplitDirection> {
        match self {
            Layout::HorizontalSplit { .. } => Some(SplitDirection::Horizontal),
            Layout::VerticalSplit { .. } => Some(SplitDirection::Vertical),
            _ => None,
        }
    }

    /// Get the ratios if this is a split layout.
    pub fn ratios(&self) -> Option<&[f32]> {
        match self {
            Layout::HorizontalSplit { ratios } | Layout::VerticalSplit { ratios } => Some(ratios),
            _ => None,
        }
    }

    /// Get mutable access to ratios if this is a split layout.
    pub fn ratios_mut(&mut self) -> Option<&mut Vec<f32>> {
        match self {
            Layout::HorizontalSplit { ratios } | Layout::VerticalSplit { ratios } => Some(ratios),
            _ => None,
        }
    }

    /// Update the ratio between two adjacent panes at the given border index.
    /// The border index is the index of the first pane (between pane i and i+1).
    /// delta is the change in ratio (positive = increase first pane, decrease second).
    pub fn adjust_ratio(&mut self, border_idx: usize, delta: f32) {
        if let Some(ratios) = self.ratios_mut() {
            if border_idx + 1 >= ratios.len() {
                return;
            }

            let new_first = (ratios[border_idx] + delta).max(0.1);
            let new_second = (ratios[border_idx + 1] - delta).max(0.1);

            ratios[border_idx] = new_first;
            ratios[border_idx + 1] = new_second;
        }
    }
}

/// Direction of a split layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

fn split_rects(area: Rect, ratios: &[f32], count: usize, dir: SplitDirection) -> Vec<Rect> {
    let total: f32 = ratios.iter().sum();
    let mut rects = Vec::with_capacity(count);
    let mut offset = 0u16;

    for i in 0..count {
        let ratio = ratios.get(i).copied().unwrap_or(1.0 / count as f32) / total;
        let (w, h, x, y) = match dir {
            SplitDirection::Horizontal => {
                let h = (area.height as f32 * ratio) as u16;
                (area.width, h, area.x, area.y + offset)
            }
            SplitDirection::Vertical => {
                let w = (area.width as f32 * ratio) as u16;
                (w, area.height, area.x + offset, area.y)
            }
        };
        rects.push(Rect {
            x,
            y,
            width: w,
            height: h,
        });
        offset += match dir {
            SplitDirection::Horizontal => h,
            SplitDirection::Vertical => w,
        };
    }
    rects
}
