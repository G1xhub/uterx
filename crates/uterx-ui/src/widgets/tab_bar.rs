//! Tab bar widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

/// A tab in the tab bar.
pub struct TabInfo {
    pub name: String,
    pub active: bool,
}

/// Renders a horizontal tab bar.
pub struct TabBar<'a> {
    tabs: &'a [TabInfo],
}

impl<'a> TabBar<'a> {
    pub fn new(tabs: &'a [TabInfo]) -> Self {
        Self { tabs }
    }
}

impl<'a> Widget for TabBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut x = area.x;
        for tab in self.tabs {
            let label = format!(" {} ", tab.name);
            let width = label.len() as u16;
            if x + width > area.x + area.width {
                break;
            }

            let style = if tab.active {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray).bg(Color::DarkGray)
            };

            buf.set_string(x, area.y, &label, style);
            x += width;
        }
    }
}
