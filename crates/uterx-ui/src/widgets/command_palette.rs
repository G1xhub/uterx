//! Command palette overlay — a searchable list of all available actions.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

/// A single command entry in the palette.
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub label: String,
    pub shortcut: String,
    pub description: String,
}

impl CommandEntry {
    pub fn new(label: &str, shortcut: &str, description: &str) -> Self {
        Self {
            label: label.to_string(),
            shortcut: shortcut.to_string(),
            description: description.to_string(),
        }
    }
}

/// Returns the default list of commands.
pub fn default_commands() -> Vec<CommandEntry> {
    vec![
        // ── Terminal ──────────────────────────────────────────────────────
        CommandEntry::new("New Tab", "Ctrl+T", "Open a new terminal tab"),
        CommandEntry::new("Close Tab", "Ctrl+W", "Close the active tab"),
        CommandEntry::new("Next Tab", "Ctrl+Tab", "Switch to the next tab"),
        CommandEntry::new(
            "Previous Tab",
            "Ctrl+Shift+Tab",
            "Switch to the previous tab",
        ),
        CommandEntry::new("Split Vertical", "Alt+V", "Split pane left/right"),
        CommandEntry::new("Split Horizontal", "Alt+H", "Split pane top/bottom"),
        CommandEntry::new("Focus Next Pane", "Alt+Right", "Move focus to next pane"),
        CommandEntry::new(
            "Focus Previous Pane",
            "Alt+Left",
            "Move focus to previous pane",
        ),
        CommandEntry::new(
            "Toggle Broadcast",
            "Alt+B",
            "Type in all panes simultaneously",
        ),
        CommandEntry::new("Toggle Floating", "Alt+F", "Float/unfloat focused pane"),
        CommandEntry::new("Toggle Maximize", "Alt+M", "Maximize/restore focused pane"),
        // ── Sidebars ─────────────────────────────────────────────────────
        CommandEntry::new("File Browser", "Ctrl+E", "Toggle file explorer sidebar"),
        CommandEntry::new(
            "Search Files",
            "/",
            "Search files in explorer (open sidebar first)",
        ),
        CommandEntry::new("AI Sidebar", "Alt+A", "Configure AI providers and API keys"),
        // ── AI ────────────────────────────────────────────────────────────
        CommandEntry::new(
            "New AI Chat",
            "Alt+I",
            "Launch AI chat in a new terminal pane",
        ),
        // ── App ───────────────────────────────────────────────────────────
        CommandEntry::new("Help", "F1", "Show help & keybindings"),
        CommandEntry::new("Command Palette", "Ctrl+P", "Open this command palette"),
        CommandEntry::new("Quit", "Ctrl+Q", "Exit uterx"),
    ]
}

/// The command palette overlay widget.
pub struct CommandPalette<'a> {
    commands: &'a [CommandEntry],
    selected: usize,
    filter: &'a str,
}

impl<'a> CommandPalette<'a> {
    pub fn new(commands: &'a [CommandEntry], selected: usize, filter: &'a str) -> Self {
        Self {
            commands,
            selected,
            filter,
        }
    }
}

impl<'a> Widget for CommandPalette<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = Color::Rgb(30, 30, 46); // Catppuccin base
        let surface = Color::Rgb(49, 50, 68); // Catppuccin surface0
        let border_color = Color::Rgb(137, 180, 250); // Catppuccin blue
        let text = Color::Rgb(205, 214, 244); // Catppuccin text
        let subtext = Color::Rgb(166, 173, 200); // Catppuccin subtext0
        let green = Color::Rgb(166, 227, 161); // Catppuccin green
        let dim = Color::Rgb(88, 91, 112); // Catppuccin overlay0
        let sel_bg = Color::Rgb(69, 71, 90); // Catppuccin surface1

        // Center the palette
        let palette_w = 60u16.min(area.width.saturating_sub(4));
        let palette_h = (self.commands.len() as u16 + 4).min(area.height.saturating_sub(4));
        let px = area.x + (area.width.saturating_sub(palette_w)) / 2;
        let py = area.y + (area.height.saturating_sub(palette_h)) / 3;

        // Draw shadow
        let shadow_style = Style::default().bg(Color::Rgb(17, 17, 27));
        for y in (py + 1)..=(py + palette_h) {
            for x in (px + 1)..=(px + palette_w) {
                if x < area.x + area.width && y < area.y + area.height {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(' ');
                        cell.set_style(shadow_style);
                    }
                }
            }
        }

        // Draw background
        let bg_style = Style::default().bg(bg);
        for y in py..py + palette_h {
            for x in px..px + palette_w {
                if x < area.x + area.width && y < area.y + area.height {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(' ');
                        cell.set_style(bg_style);
                    }
                }
            }
        }

        // Draw border
        let border_style = Style::default().fg(border_color).bg(bg);
        // Top border
        if py < area.y + area.height {
            buf.set_string(px, py, "\u{256d}", border_style);
            for x in (px + 1)..(px + palette_w - 1) {
                buf.set_string(x, py, "\u{2500}", border_style);
            }
            buf.set_string(px + palette_w - 1, py, "\u{256e}", border_style);
        }
        // Bottom border
        let bot = py + palette_h - 1;
        if bot < area.y + area.height {
            buf.set_string(px, bot, "\u{2570}", border_style);
            for x in (px + 1)..(px + palette_w - 1) {
                buf.set_string(x, bot, "\u{2500}", border_style);
            }
            buf.set_string(px + palette_w - 1, bot, "\u{256f}", border_style);
        }
        // Side borders
        for y in (py + 1)..bot {
            if y < area.y + area.height {
                buf.set_string(px, y, "\u{2502}", border_style);
                buf.set_string(px + palette_w - 1, y, "\u{2502}", border_style);
            }
        }

        // Title
        let title = " Command Palette ";
        let title_style = Style::default()
            .fg(border_color)
            .bg(bg)
            .add_modifier(Modifier::BOLD);
        let title_x = px + (palette_w.saturating_sub(title.len() as u16)) / 2;
        if py < area.y + area.height {
            buf.set_string(title_x, py, title, title_style);
        }

        // Search input line
        let input_y = py + 1;
        if input_y < area.y + area.height {
            let input_style = Style::default().fg(text).bg(surface);
            let prompt = format!(" > {}", self.filter);
            let padded = format!("{:<width$}", prompt, width = (palette_w - 2) as usize);
            buf.set_string(px + 1, input_y, &padded, input_style);
        }

        // Separator
        let sep_y = py + 2;
        if sep_y < area.y + area.height {
            buf.set_string(px, sep_y, "\u{251c}", border_style);
            for x in (px + 1)..(px + palette_w - 1) {
                buf.set_string(x, sep_y, "\u{2500}", border_style);
            }
            buf.set_string(px + palette_w - 1, sep_y, "\u{2524}", border_style);
        }

        // Command list
        let list_start_y = py + 3;
        let inner_w = (palette_w - 2) as usize;
        for (i, cmd) in self.commands.iter().enumerate() {
            let cy = list_start_y + i as u16;
            if cy >= bot || cy >= area.y + area.height {
                break;
            }

            let is_selected = i == self.selected;
            let row_bg = if is_selected { sel_bg } else { bg };
            let label_style = Style::default()
                .fg(if is_selected { text } else { subtext })
                .bg(row_bg)
                .add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
            let shortcut_style = Style::default().fg(green).bg(row_bg);

            // Clear row
            let clear = " ".repeat(inner_w);
            buf.set_string(px + 1, cy, &clear, Style::default().bg(row_bg));

            // Indicator
            if is_selected {
                let ind_style = Style::default().fg(border_color).bg(row_bg);
                buf.set_string(px + 1, cy, " \u{25b8} ", ind_style);
            } else {
                buf.set_string(px + 1, cy, "   ", Style::default().bg(row_bg));
            }

            // Label
            buf.set_string(px + 4, cy, &cmd.label, label_style);

            // Shortcut (right-aligned)
            let sc_len = cmd.shortcut.len() as u16;
            let sc_x = px + palette_w - 2 - sc_len;
            if sc_x > px + 4 + cmd.label.len() as u16 {
                buf.set_string(sc_x, cy, &cmd.shortcut, shortcut_style);
            }
        }

        // ── Description for selected command ──────────────────────────────
        if let Some(selected_cmd) = self.commands.get(self.selected) {
            let desc_y = bot.saturating_sub(2);
            if desc_y > list_start_y && desc_y < area.y + area.height {
                // Separator before description
                if desc_y > list_start_y + 1 {
                    let sep_y = desc_y.saturating_sub(1);
                    buf.set_string(px, sep_y, "\u{251c}", border_style);
                    for x in (px + 1)..(px + palette_w - 1) {
                        buf.set_string(x, sep_y, "\u{2500}", border_style);
                    }
                    buf.set_string(px + palette_w - 1, sep_y, "\u{2524}", border_style);
                }

                // Description text
                let desc_style = Style::default().fg(dim).bg(bg);
                let desc_text = format!(" {} ", selected_cmd.description);
                let max_desc_w = (palette_w - 2) as usize;
                let truncated = if desc_text.len() > max_desc_w {
                    &desc_text[..max_desc_w]
                } else {
                    &desc_text
                };
                let padded = format!("{:<width$}", truncated, width = max_desc_w);
                buf.set_string(px + 1, desc_y, &padded, desc_style);
            }
        }
    }
}
