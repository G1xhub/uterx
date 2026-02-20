//! Terminal view: renders the uterx-core grid into a ratatui frame.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};
use uterx_core::Grid;

/// A ratatui widget that renders a terminal grid.
pub struct TerminalView<'a> {
    grid: &'a Grid,
    show_cursor: bool,
}

impl<'a> TerminalView<'a> {
    pub fn new(grid: &'a Grid) -> Self {
        Self {
            grid,
            show_cursor: true,
        }
    }

    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }
}

impl<'a> Widget for TerminalView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let rows = (area.height as usize).min(self.grid.rows());
        let cols = (area.width as usize).min(self.grid.cols());

        for row in 0..rows {
            for col in 0..cols {
                if let Some(cell) = self.grid.cell(row, col) {
                    let ch = cell.content.chars().next().unwrap_or(' ');
                    let style = convert_style(&cell.attrs);

                    let x = area.x + col as u16;
                    let y = area.y + row as u16;
                    if let Some(buf_cell) = buf.cell_mut((x, y)) {
                        buf_cell.set_char(ch);
                        buf_cell.set_style(style);
                    }
                }
            }
        }

        // Draw cursor
        if self.show_cursor {
            let cx = area.x + self.grid.cursor_col as u16;
            let cy = area.y + self.grid.cursor_row as u16;
            if cx < area.x + area.width && cy < area.y + area.height {
                if let Some(buf_cell) = buf.cell_mut((cx, cy)) {
                    buf_cell.set_style(Style::default().fg(Color::Black).bg(Color::White));
                }
            }
        }
    }
}

fn convert_style(attrs: &uterx_core::CellAttributes) -> Style {
    let mut style = Style::default();

    style = match attrs.fg {
        uterx_core::cell::Color::Default => style.fg(Color::Reset),
        uterx_core::cell::Color::Indexed(i) => style.fg(Color::Indexed(i)),
        uterx_core::cell::Color::Rgb(r, g, b) => style.fg(Color::Rgb(r, g, b)),
    };

    style = match attrs.bg {
        uterx_core::cell::Color::Default => style.bg(Color::Reset),
        uterx_core::cell::Color::Indexed(i) => style.bg(Color::Indexed(i)),
        uterx_core::cell::Color::Rgb(r, g, b) => style.bg(Color::Rgb(r, g, b)),
    };

    if attrs.bold {
        style = style.add_modifier(ratatui::style::Modifier::BOLD);
    }
    if attrs.italic {
        style = style.add_modifier(ratatui::style::Modifier::ITALIC);
    }
    if attrs.underline {
        style = style.add_modifier(ratatui::style::Modifier::UNDERLINED);
    }

    style
}
