//! Status bar widget (bottom of screen).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

/// The status bar shows session info, active pane, and hints.
pub struct StatusBar<'a> {
    pub session_name: &'a str,
    pub pane_title: &'a str,
    pub broadcast: bool,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = Style::default().fg(Color::White).bg(Color::DarkGray);
        // Fill background
        for x in area.x..area.x + area.width {
            buf.set_string(x, area.y, " ", bg);
        }

        let left = format!(" {} | {} ", self.session_name, self.pane_title);
        buf.set_string(area.x, area.y, &left, bg);

        if self.broadcast {
            let tag = " [BROADCAST] ";
            let x = area.x + area.width - tag.len() as u16;
            let broadcast_style = Style::default().fg(Color::Yellow).bg(Color::DarkGray);
            buf.set_string(x, area.y, tag, broadcast_style);
        }
    }
}
