//! Help overlay — full-screen keybinding reference and feature guide.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

/// Help overlay content sections.
struct HelpSection {
    title: &'static str,
    entries: Vec<(&'static str, &'static str)>,
}

/// Full-screen help overlay widget.
pub struct HelpOverlay;

impl HelpOverlay {
    pub fn new() -> Self {
        Self
    }

    fn sections(&self) -> Vec<HelpSection> {
        vec![
            HelpSection {
                title: "General",
                entries: vec![
                    ("Ctrl+Q", "Quit uterx"),
                    ("F1", "Toggle this help screen"),
                    ("Ctrl+P", "Open command palette"),
                    ("Ctrl+E", "Toggle file browser sidebar"),
                ],
            },
            HelpSection {
                title: "Tabs",
                entries: vec![
                    ("Ctrl+T", "New tab"),
                    ("Ctrl+W", "Close active pane/tab"),
                    ("Ctrl+Tab", "Next tab"),
                    ("Ctrl+Shift+Tab", "Previous tab"),
                ],
            },
            HelpSection {
                title: "Panes & Splits",
                entries: vec![
                    ("Alt+V", "Split vertical (left | right)"),
                    ("Alt+H", "Split horizontal (top / bottom)"),
                    ("Alt+Right", "Focus next pane"),
                    ("Alt+Left", "Focus previous pane"),
                    ("Alt+F", "Toggle floating pane (drag to move)"),
                    ("Click", "Click pane to focus"),
                    ("Click tab", "Switch tab / [+] new / [?] help"),
                    ("Drag float", "Move floating pane with mouse"),
                ],
            },
            HelpSection {
                title: "Broadcast",
                entries: vec![
                    ("Alt+B", "Toggle broadcast mode"),
                    ("", "Type in ALL panes simultaneously"),
                ],
            },
            HelpSection {
                title: "File Browser (Ctrl+E)",
                entries: vec![
                    ("Up/Down", "Navigate entries"),
                    ("Enter (dir)", "Expand/collapse folder"),
                    ("Enter (file)", "Open file in editor"),
                    ("Right (dir)", "Open folder in new terminal pane"),
                    ("/", "Start search (fuzzy, live)"),
                    ("Tab (search)", "Toggle recursive search"),
                    ("Esc (search)", "Exit search mode"),
                    ("r", "Refresh directory listing"),
                    ("Esc / Tab", "Return focus to terminal"),
                ],
            },
            HelpSection {
                title: "Editor (Normal mode)",
                entries: vec![
                    ("i / a / o", "Enter Insert mode"),
                    ("h j k l", "Move cursor (←↓↑→)"),
                    ("0 / $", "Start / end of line"),
                    ("gg / G", "First / last line"),
                    ("x", "Delete char at cursor"),
                    ("dd", "Delete current line"),
                    ("u", "Undo"),
                    ("Ctrl+S", "Save file"),
                    ("Ctrl+W", "Close (twice if unsaved)"),
                ],
            },
            HelpSection {
                title: "Editor (Insert mode)",
                entries: vec![
                    ("Esc", "Return to Normal mode"),
                    ("Ctrl+S", "Save file"),
                    ("Ctrl+W", "Close (twice if unsaved)"),
                    ("Backspace / Delete", "Delete character"),
                    ("Arrow keys", "Move cursor"),
                ],
            },
            HelpSection {
                title: "Configuration",
                entries: vec![
                    ("~/.uterx/config.toml", "Settings file"),
                    ("~/.uterx/sessions/", "Saved sessions"),
                ],
            },
        ]
    }
}

impl Widget for HelpOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = Color::Rgb(24, 24, 37);          // Catppuccin mantle
        let border_color = Color::Rgb(137, 180, 250); // Catppuccin blue
        let title_color = Color::Rgb(203, 166, 247);  // Catppuccin mauve
        let heading = Color::Rgb(249, 226, 175);  // Catppuccin yellow
        let key_color = Color::Rgb(166, 227, 161); // Catppuccin green
        let text_color = Color::Rgb(205, 214, 244); // Catppuccin text
        let dim = Color::Rgb(88, 91, 112);         // Catppuccin overlay0

        // Fill background with semi-transparent feel
        let bg_style = Style::default().bg(bg);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        // Center panel
        let panel_w = 56u16.min(area.width.saturating_sub(4));
        let sections = self.sections();
        let total_lines: u16 = sections
            .iter()
            .map(|s| s.entries.len() as u16 + 2)
            .sum::<u16>()
            + 8; // header + footer + spacing
        let panel_h = total_lines.min(area.height.saturating_sub(4));
        let px = area.x + (area.width.saturating_sub(panel_w)) / 2;
        let py = area.y + (area.height.saturating_sub(panel_h)) / 3;

        // Draw panel background
        let panel_bg = Color::Rgb(30, 30, 46);
        let panel_style = Style::default().bg(panel_bg);
        for y in py..py + panel_h {
            for x in px..px + panel_w {
                if x < area.x + area.width && y < area.y + area.height {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(' ');
                        cell.set_style(panel_style);
                    }
                }
            }
        }

        // Draw rounded border
        let border_style = Style::default().fg(border_color).bg(panel_bg);
        // Top
        if py < area.y + area.height {
            buf.set_string(px, py, "\u{256d}", border_style);
            for x in (px + 1)..(px + panel_w - 1) {
                buf.set_string(x, py, "\u{2500}", border_style);
            }
            buf.set_string(px + panel_w - 1, py, "\u{256e}", border_style);
        }
        // Bottom
        let bot = py + panel_h - 1;
        if bot < area.y + area.height {
            buf.set_string(px, bot, "\u{2570}", border_style);
            for x in (px + 1)..(px + panel_w - 1) {
                buf.set_string(x, bot, "\u{2500}", border_style);
            }
            buf.set_string(px + panel_w - 1, bot, "\u{256f}", border_style);
        }
        // Sides
        for y in (py + 1)..bot {
            if y < area.y + area.height {
                buf.set_string(px, y, "\u{2502}", border_style);
                buf.set_string(px + panel_w - 1, y, "\u{2502}", border_style);
            }
        }

        let mut cy = py + 1;
        let inner_x = px + 2;

        // Title
        if cy < bot {
            let title_style = Style::default()
                .fg(title_color)
                .bg(panel_bg)
                .add_modifier(Modifier::BOLD);
            let title = "uterx  -  Terminal Desktop";
            let tx = px + (panel_w.saturating_sub(title.len() as u16)) / 2;
            buf.set_string(tx, cy, title, title_style);
            cy += 1;
        }

        // Subtitle
        if cy < bot {
            let sub_style = Style::default().fg(dim).bg(panel_bg);
            let sub = "Multiplexer + App Platform";
            let sx = px + (panel_w.saturating_sub(sub.len() as u16)) / 2;
            buf.set_string(sx, cy, sub, sub_style);
            cy += 1;
        }

        // Separator
        if cy < bot {
            let sep_style = Style::default().fg(dim).bg(panel_bg);
            buf.set_string(px, cy, "\u{251c}", sep_style);
            for x in (px + 1)..(px + panel_w - 1) {
                buf.set_string(x, cy, "\u{2500}", sep_style);
            }
            buf.set_string(px + panel_w - 1, cy, "\u{2524}", sep_style);
            cy += 1;
        }

        // Sections
        let heading_style = Style::default()
            .fg(heading)
            .bg(panel_bg)
            .add_modifier(Modifier::BOLD);
        let key_style = Style::default().fg(key_color).bg(panel_bg);
        let text_style = Style::default().fg(text_color).bg(panel_bg);

        for section in &sections {
            if cy >= bot {
                break;
            }

            // Section heading
            let heading_text = format!("  {}", section.title);
            buf.set_string(inner_x, cy, &heading_text, heading_style);
            cy += 1;

            for (key, desc) in &section.entries {
                if cy >= bot {
                    break;
                }

                if key.is_empty() {
                    // Description-only line (indented)
                    let indent = "                      ";
                    buf.set_string(inner_x, cy, indent, panel_style);
                    buf.set_string(inner_x + 4, cy, desc, text_style);
                } else {
                    // Key + description
                    let key_padded = format!("    {:<18}", key);
                    buf.set_string(inner_x, cy, &key_padded, key_style);
                    buf.set_string(
                        inner_x + key_padded.len() as u16,
                        cy,
                        desc,
                        text_style,
                    );
                }
                cy += 1;
            }

            cy += 1; // Space between sections
        }

        // Footer
        if cy < bot {
            let footer_style = Style::default()
                .fg(dim)
                .bg(panel_bg)
                .add_modifier(Modifier::ITALIC);
            let footer = "Press F1 or Esc to close";
            let fx = px + (panel_w.saturating_sub(footer.len() as u16)) / 2;
            buf.set_string(fx, cy, footer, footer_style);
        }
    }
}
