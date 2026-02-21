//! Tab bar widget — app branding, styled tabs, action buttons.

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
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabBarHover {
    Tab(usize),
    Plus,
    Help,
}

/// Renders a horizontal tab bar with branding and controls.
///
/// Layout: `⚡uterx │ 1:Tab1 │ 2:Tab2 │ ...            [+] [?]`
pub struct TabBar<'a> {
    tabs: &'a [TabInfo],
    pub broadcast: bool,
    hover: Option<TabBarHover>,
}

impl<'a> TabBar<'a> {
    pub fn new(tabs: &'a [TabInfo]) -> Self {
        Self {
            tabs,
            broadcast: false,
            hover: None,
        }
    }

    pub fn broadcast(mut self, b: bool) -> Self {
        self.broadcast = b;
        self
    }

    pub fn hover(mut self, hover: Option<TabBarHover>) -> Self {
        self.hover = hover;
        self
    }
}

impl<'a> Widget for TabBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill the entire bar with the background
        let bar_bg = Style::default().fg(Color::Gray).bg(Color::Rgb(30, 30, 46));
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(bar_bg);
            }
        }

        let mut x = area.x;

        // Branding
        let brand = " uterx ";
        let brand_style = Style::default()
            .fg(Color::Rgb(137, 180, 250)) // Catppuccin blue
            .bg(Color::Rgb(30, 30, 46))
            .add_modifier(Modifier::BOLD);
        buf.set_string(x, area.y, brand, brand_style);
        x += brand.len() as u16;

        // Separator
        let sep_style = Style::default()
            .fg(Color::Rgb(88, 91, 112))
            .bg(Color::Rgb(30, 30, 46));
        buf.set_string(x, area.y, "\u{2502}", sep_style); // │
        x += 1;

        // Tabs
        for tab in self.tabs {
            let label = format!(" {}:{} ", tab.index + 1, tab.name);
            let width = label.len() as u16;
            if x + width > area.x + area.width.saturating_sub(12) {
                // Leave room for right-side buttons
                break;
            }

            let hovered = self.hover == Some(TabBarHover::Tab(tab.index));

            let style = if tab.active {
                Style::default()
                    .fg(Color::Rgb(205, 214, 244)) // Catppuccin text
                    .bg(if hovered {
                        Color::Rgb(69, 71, 90) // surface1
                    } else {
                        Color::Rgb(49, 50, 68) // surface0
                    })
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(if hovered { Modifier::UNDERLINED } else { Modifier::empty() })
            } else {
                Style::default()
                    .fg(if hovered {
                        Color::Rgb(205, 214, 244) // text
                    } else {
                        Color::Rgb(108, 112, 134) // overlay0
                    })
                    .bg(if hovered {
                        Color::Rgb(49, 50, 68) // surface0
                    } else {
                        Color::Rgb(30, 30, 46) // base
                    })
                    .add_modifier(if hovered { Modifier::BOLD } else { Modifier::empty() })
            };

            buf.set_string(x, area.y, &label, style);
            x += width;

            // Tab separator
            buf.set_string(x, area.y, "\u{2502}", sep_style);
            x += 1;
        }

        // Right-side controls
        let right_section_width: u16 = 12;
        let right_x = area.x + area.width.saturating_sub(right_section_width);

        // Broadcast indicator
        if self.broadcast {
            let bc_style = Style::default()
                .fg(Color::Rgb(249, 226, 175)) // Catppuccin yellow
                .bg(Color::Rgb(30, 30, 46))
                .add_modifier(Modifier::BOLD);
            let bc_x = right_x.saturating_sub(6);
            buf.set_string(bc_x, area.y, " BC ", bc_style);
        }

        // [+] New tab button
        let plus_hovered = self.hover == Some(TabBarHover::Plus);
        let plus_style = Style::default()
            .fg(Color::Rgb(166, 227, 161)) // Catppuccin green
            .bg(if plus_hovered {
                Color::Rgb(49, 50, 68)
            } else {
                Color::Rgb(30, 30, 46)
            })
            .add_modifier(if plus_hovered { Modifier::BOLD } else { Modifier::empty() });
        buf.set_string(right_x, area.y, " [+] ", plus_style);

        // [?] Help button
        let help_hovered = self.hover == Some(TabBarHover::Help);
        let help_style = Style::default()
            .fg(Color::Rgb(180, 190, 254)) // Catppuccin lavender
            .bg(if help_hovered {
                Color::Rgb(49, 50, 68)
            } else {
                Color::Rgb(30, 30, 46)
            })
            .add_modifier(if help_hovered { Modifier::BOLD } else { Modifier::empty() });
        buf.set_string(right_x + 5, area.y, " [?] ", help_style);
    }
}
