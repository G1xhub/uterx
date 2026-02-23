//! Command palette overlay — a searchable list of all available actions.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

/// Category for a command entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Navigation,
    Plugins,
    Misc,
}

impl CommandCategory {
    pub fn label(&self) -> &'static str {
        match self {
            CommandCategory::Navigation => "Navigation",
            CommandCategory::Plugins => "Plugins",
            CommandCategory::Misc => "Misc",
        }
    }
}

/// A single command entry in the palette.
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub label: String,
    pub shortcut: String,
    pub description: String,
    pub category: CommandCategory,
}

impl CommandEntry {
    pub fn new(label: &str, shortcut: &str, description: &str, category: CommandCategory) -> Self {
        Self {
            label: label.to_string(),
            shortcut: shortcut.to_string(),
            description: description.to_string(),
            category,
        }
    }
}

/// Returns the default list of commands organized by category.
pub fn default_commands() -> Vec<CommandEntry> {
    vec![
        // Navigation commands
        CommandEntry::new("New Tab", "Ctrl+T", "Open a new terminal tab", CommandCategory::Navigation),
        CommandEntry::new("Close Tab", "Ctrl+W", "Close the active tab", CommandCategory::Navigation),
        CommandEntry::new("Next Tab", "Ctrl+Tab", "Switch to the next tab", CommandCategory::Navigation),
        CommandEntry::new("Previous Tab", "Ctrl+Shift+Tab", "Switch to the previous tab", CommandCategory::Navigation),
        CommandEntry::new("Split Vertical", "Alt+V", "Split pane left/right", CommandCategory::Navigation),
        CommandEntry::new("Split Horizontal", "Alt+H", "Split pane top/bottom", CommandCategory::Navigation),
        CommandEntry::new("Focus Next Pane", "Alt+Right", "Move focus to next pane", CommandCategory::Navigation),
        CommandEntry::new("Focus Previous Pane", "Alt+Left", "Move focus to previous pane", CommandCategory::Navigation),
        CommandEntry::new("Toggle Broadcast", "Alt+B", "Type in all panes simultaneously", CommandCategory::Navigation),
        CommandEntry::new("Toggle Floating", "Alt+F", "Float/unfloat focused pane", CommandCategory::Navigation),
        CommandEntry::new("File Browser", "Ctrl+E", "Toggle file explorer sidebar", CommandCategory::Navigation),
        CommandEntry::new("Search Files", "/", "Search files in explorer (open sidebar first)", CommandCategory::Navigation),
        // Plugin commands
        CommandEntry::new("Plugin Launcher", "Ctrl+Shift+P", "Open plugin quick-list overlay", CommandCategory::Plugins),
        CommandEntry::new("Open Last Plugin", "Ctrl+Shift+L", "Reopen/focus the most recent plugin", CommandCategory::Plugins),
        CommandEntry::new("UterxAI", "Ctrl+Shift+A", "Open/focus UterxAI assistant pane", CommandCategory::Plugins),
        CommandEntry::new("Quick Notes", "Ctrl+N", "Open quick notes plugin", CommandCategory::Plugins),
        // Misc commands
        CommandEntry::new("Help", "F1", "Show help & keybindings", CommandCategory::Misc),
        CommandEntry::new("Command Palette", "Ctrl+P", "Open this command palette", CommandCategory::Misc),
        CommandEntry::new("Quit", "Ctrl+Q", "Exit uterx", CommandCategory::Misc),
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
        let bg = Color::Rgb(30, 30, 46);         // Catppuccin base
        let surface = Color::Rgb(49, 50, 68);    // Catppuccin surface0
        let border_color = Color::Rgb(137, 180, 250); // Catppuccin blue
        let text = Color::Rgb(205, 214, 244);    // Catppuccin text
        let subtext = Color::Rgb(166, 173, 200); // Catppuccin subtext0
        let green = Color::Rgb(166, 227, 161);   // Catppuccin green
        let sel_bg = Color::Rgb(69, 71, 90);     // Catppuccin surface1
        let lavender = Color::Rgb(180, 190, 254); // Catppuccin lavender
        let peach = Color::Rgb(250, 179, 135);   // Catppuccin peach
        let teal = Color::Rgb(148, 226, 213);    // Catppuccin teal

        // Organize commands by category
        let nav_cmds: Vec<_> = self.commands.iter().filter(|c| c.category == CommandCategory::Navigation).collect();
        let plugin_cmds: Vec<_> = self.commands.iter().filter(|c| c.category == CommandCategory::Plugins).collect();
        let misc_cmds: Vec<_> = self.commands.iter().filter(|c| c.category == CommandCategory::Misc).collect();

        let max_rows = nav_cmds.len().max(plugin_cmds.len()).max(misc_cmds.len());
        
        // Calculate palette dimensions - wider for 3 columns
        let col_width = 28u16;
        let palette_w = (col_width * 3 + 8).min(area.width.saturating_sub(4)); // 8 for padding/separators
        let palette_h = (max_rows as u16 + 5).min(area.height.saturating_sub(4)); // +5 for title, search, separator, header
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
            let padded = format!(
                "{:<width$}",
                prompt,
                width = (palette_w - 2) as usize
            );
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

        // Column headers
        let header_y = py + 3;
        let col_w = (palette_w - 2) / 3;
        
        if header_y < area.y + area.height {
            // Clear header row
            let clear = " ".repeat((palette_w - 2) as usize);
            buf.set_string(px + 1, header_y, &clear, Style::default().bg(surface));
            
            // Column headers with icons
            let nav_header = " Navigation ";
            let plugin_header = " Plugins ";
            let misc_header = " Misc ";
            
            let nav_style = Style::default().fg(lavender).bg(surface).add_modifier(Modifier::BOLD);
            let plugin_style = Style::default().fg(peach).bg(surface).add_modifier(Modifier::BOLD);
            let misc_style = Style::default().fg(teal).bg(surface).add_modifier(Modifier::BOLD);
            
            buf.set_string(px + 2, header_y, nav_header, nav_style);
            buf.set_string(px + 1 + col_w, header_y, plugin_header, plugin_style);
            buf.set_string(px + 1 + col_w * 2, header_y, misc_header, misc_style);
        }

        // Separator under headers
        let header_sep_y = py + 4;
        if header_sep_y < area.y + area.height {
            buf.set_string(px, header_sep_y, "\u{251c}", border_style);
            for x in (px + 1)..(px + palette_w - 1) {
                buf.set_string(x, header_sep_y, "\u{2500}", border_style);
            }
            buf.set_string(px + palette_w - 1, header_sep_y, "\u{2524}", border_style);
        }

        // Command list - three columns
        let list_start_y = py + 5;
        let inner_w = col_w as usize;
        
        // Find the global index of the selected command
        let selected_cmd = self.commands.get(self.selected);
        
        for row in 0..max_rows {
            let cy = list_start_y + row as u16;
            if cy >= bot || cy >= area.y + area.height {
                break;
            }

            // Clear row
            let clear = " ".repeat((palette_w - 2) as usize);
            buf.set_string(px + 1, cy, &clear, Style::default().bg(bg));

            // Render Navigation column
            if let Some(cmd) = nav_cmds.get(row) {
                let is_selected = selected_cmd.map_or(false, |s| std::ptr::eq(*cmd, s));
                render_command_entry(buf, px + 1, cy, cmd, is_selected, inner_w, bg, sel_bg, text, subtext, green, border_color);
            }
            
            // Render Plugins column
            if let Some(cmd) = plugin_cmds.get(row) {
                let is_selected = selected_cmd.map_or(false, |s| std::ptr::eq(*cmd, s));
                render_command_entry(buf, px + 1 + col_w, cy, cmd, is_selected, inner_w, bg, sel_bg, text, subtext, green, border_color);
            }
            
            // Render Misc column
            if let Some(cmd) = misc_cmds.get(row) {
                let is_selected = selected_cmd.map_or(false, |s| std::ptr::eq(*cmd, s));
                render_command_entry(buf, px + 1 + col_w * 2, cy, cmd, is_selected, inner_w, bg, sel_bg, text, subtext, green, border_color);
            }
        }
    }
}

/// Helper function to render a single command entry
fn render_command_entry(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    cmd: &CommandEntry,
    is_selected: bool,
    width: usize,
    bg: Color,
    sel_bg: Color,
    text: Color,
    subtext: Color,
    green: Color,
    border_color: Color,
) {
    let row_bg = if is_selected { sel_bg } else { bg };
    let label_style = Style::default()
        .fg(if is_selected { text } else { subtext })
        .bg(row_bg)
        .add_modifier(if is_selected { Modifier::BOLD } else { Modifier::empty() });
    let shortcut_style = Style::default().fg(green).bg(row_bg);

    // Clear the cell area
    let clear = " ".repeat(width);
    buf.set_string(x, y, &clear, Style::default().bg(row_bg));

    // Indicator
    if is_selected {
        let ind_style = Style::default().fg(border_color).bg(row_bg);
        buf.set_string(x, y, "\u{25b8}", ind_style);
    }

    // Label (truncate if needed)
    let max_label_len = width.saturating_sub(cmd.shortcut.len() + 2);
    let label: String = cmd.label.chars().take(max_label_len).collect();
    let label_x = if is_selected { x + 1 } else { x };
    buf.set_string(label_x, y, &label, label_style);

    // Shortcut (right-aligned within column)
    let sc_len = cmd.shortcut.len() as u16;
    let sc_x = x + width as u16 - sc_len - 1;
    if sc_x > label_x + label.len() as u16 {
        buf.set_string(sc_x, y, &cmd.shortcut, shortcut_style);
    }
}
