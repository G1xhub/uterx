//! Status bar widget — rich info bar at the bottom of the screen.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

/// The status bar shows session info, pane details, and keybinding hints.
pub struct StatusBar<'a> {
    pub session_name: &'a str,
    pub pane_title: &'a str,
    pub pane_count: usize,
    pub tab_count: usize,
    pub broadcast: bool,
    pub focused_index: usize,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = Color::Rgb(30, 30, 46);       // Catppuccin base
        let fg = Color::Rgb(166, 173, 200);     // Catppuccin subtext0
        let accent = Color::Rgb(137, 180, 250); // Catppuccin blue
        let green = Color::Rgb(166, 227, 161);  // Catppuccin green
        let yellow = Color::Rgb(249, 226, 175); // Catppuccin yellow
        let dim = Color::Rgb(88, 91, 112);      // Catppuccin overlay0

        let base_style = Style::default().fg(fg).bg(bg);
        let accent_style = Style::default().fg(accent).bg(bg).add_modifier(Modifier::BOLD);
        let sep_style = Style::default().fg(dim).bg(bg);
        let hint_style = Style::default().fg(dim).bg(bg);
        let hint_key_style = Style::default().fg(green).bg(bg);

        // Fill background
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(base_style);
            }
        }

        let mut x = area.x;

        // Session name badge
        let session_badge = format!(" {} ", self.session_name);
        let badge_style = Style::default()
            .fg(Color::Rgb(30, 30, 46))
            .bg(accent)
            .add_modifier(Modifier::BOLD);
        buf.set_string(x, area.y, &session_badge, badge_style);
        x += session_badge.len() as u16;

        // Separator
        buf.set_string(x, area.y, " \u{2502} ", sep_style);
        x += 3;

        // Pane title
        let title_str = format!("{}", self.pane_title);
        buf.set_string(x, area.y, &title_str, accent_style);
        x += title_str.len() as u16;

        // Separator
        buf.set_string(x, area.y, " \u{2502} ", sep_style);
        x += 3;

        // Pane count info
        let pane_info = format!(
            "{}/{} panes",
            self.focused_index + 1,
            self.pane_count
        );
        buf.set_string(x, area.y, &pane_info, base_style);
        x += pane_info.len() as u16;

        // Broadcast indicator
        if self.broadcast {
            buf.set_string(x, area.y, " \u{2502} ", sep_style);
            x += 3;
            let bc_style = Style::default()
                .fg(Color::Rgb(30, 30, 46))
                .bg(yellow)
                .add_modifier(Modifier::BOLD);
            buf.set_string(x, area.y, " BROADCAST ", bc_style);
            x += 11;
        }

        // Right-side keybinding hints
        let hints = [
            ("^T", "Tab"),
            ("^P", "Cmd"),
            ("A-V", "Split"),
            ("F1", "Help"),
        ];

        // Calculate total width of hints
        let mut total_hint_width: u16 = 0;
        for (key, label) in &hints {
            total_hint_width += (key.len() + 1 + label.len() + 2) as u16;
        }

        let hint_start = area.x + area.width.saturating_sub(total_hint_width + 1);
        if hint_start > x {
            let mut hx = hint_start;
            for (key, label) in &hints {
                buf.set_string(hx, area.y, key, hint_key_style);
                hx += key.len() as u16;
                let lbl = format!(":{} ", label);
                buf.set_string(hx, area.y, &lbl, hint_style);
                hx += lbl.len() as u16;
            }
        }
    }
}
