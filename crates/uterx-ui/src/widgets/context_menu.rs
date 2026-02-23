//! Context menu widget — right-click popup menu for file browser actions.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use std::path::PathBuf;

/// Actions available from the context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuAction {
    OpenInTerminal,
    OpenInNewTab,
    OpenInEditor,
    CopyPath,
    CopyRelativePath,
    Rename,
    Delete,
    Refresh,
    NavigateUp,
    NewFile,
    NewFolder,
}

/// A single item in the context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub label: String,
    pub shortcut: String,
    pub action: ContextMenuAction,
    pub enabled: bool,
    pub separator_after: bool,
}

impl ContextMenuItem {
    pub fn new(label: &str, shortcut: &str, action: ContextMenuAction) -> Self {
        Self {
            label: label.to_string(),
            shortcut: shortcut.to_string(),
            action,
            enabled: true,
            separator_after: false,
        }
    }

    pub fn separator_after(mut self) -> Self {
        self.separator_after = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Context menu state — tracks visibility, position, and selection.
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub visible: bool,
    pub x: u16,
    pub y: u16,
    pub items: Vec<ContextMenuItem>,
    pub selected: usize,
    pub target_path: Option<PathBuf>,
    pub target_is_dir: bool,
}

impl Default for ContextMenuState {
    fn default() -> Self {
        Self {
            visible: false,
            x: 0,
            y: 0,
            items: Vec::new(),
            selected: 0,
            target_path: None,
            target_is_dir: false,
        }
    }
}

impl ContextMenuState {
    /// Show the context menu at the given position for the given path.
    pub fn show(&mut self, x: u16, y: u16, path: PathBuf, is_dir: bool, is_parent: bool) {
        self.visible = true;
        self.x = x;
        self.y = y;
        self.target_path = Some(path);
        self.target_is_dir = is_dir;
        self.selected = 0;
        self.items = build_menu_items(is_dir, is_parent);
    }

    /// Hide the context menu.
    pub fn hide(&mut self) {
        self.visible = false;
        self.items.clear();
        self.target_path = None;
    }

    /// Move selection up.
    pub fn select_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        loop {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
            if self.items[self.selected].enabled {
                break;
            }
        }
    }

    /// Move selection down.
    pub fn select_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        loop {
            self.selected = (self.selected + 1) % self.items.len();
            if self.items[self.selected].enabled {
                break;
            }
        }
    }

    /// Get the currently selected action.
    pub fn selected_action(&self) -> Option<ContextMenuAction> {
        self.items.get(self.selected).filter(|i| i.enabled).map(|i| i.action)
    }

    /// Get the width needed for the menu.
    pub fn needed_width(&self) -> u16 {
        let max_label = self.items.iter().map(|i| i.label.len()).max().unwrap_or(0);
        let max_shortcut = self.items.iter().map(|i| i.shortcut.len()).max().unwrap_or(0);
        // icon(2) + space + label + spaces(2) + shortcut + padding(2)
        (max_label + max_shortcut + 8).min(40) as u16
    }

    /// Get the height needed for the menu.
    pub fn needed_height(&self) -> u16 {
        // items + separators + top/bottom border
        let separators = self.items.iter().filter(|i| i.separator_after).count();
        (self.items.len() + separators + 2) as u16
    }
}

/// Build menu items based on the target type.
fn build_menu_items(is_dir: bool, is_parent: bool) -> Vec<ContextMenuItem> {
    if is_parent {
        // Menu for ".." (parent directory)
        return vec![
            ContextMenuItem::new("Navigate Up", "Enter", ContextMenuAction::NavigateUp),
        ];
    }

    if is_dir {
        // Menu for directories
        vec![
            ContextMenuItem::new("Open in Terminal", "Enter", ContextMenuAction::OpenInTerminal)
                .separator_after(),
            ContextMenuItem::new("Open in New Tab", "Ctrl+T", ContextMenuAction::OpenInNewTab),
            ContextMenuItem::new("New File", "", ContextMenuAction::NewFile),
            ContextMenuItem::new("New Folder", "", ContextMenuAction::NewFolder)
                .separator_after(),
            ContextMenuItem::new("Copy Path", "Ctrl+C", ContextMenuAction::CopyPath),
            ContextMenuItem::new("Copy Relative Path", "", ContextMenuAction::CopyRelativePath)
                .separator_after(),
            ContextMenuItem::new("Rename", "F2", ContextMenuAction::Rename),
            ContextMenuItem::new("Delete", "Del", ContextMenuAction::Delete)
                .separator_after(),
            ContextMenuItem::new("Refresh", "F5", ContextMenuAction::Refresh),
        ]
    } else {
        // Menu for files
        vec![
            ContextMenuItem::new("Open in Editor", "Enter", ContextMenuAction::OpenInEditor)
                .separator_after(),
            ContextMenuItem::new("Open in Terminal", "", ContextMenuAction::OpenInTerminal)
                .separator_after(),
            ContextMenuItem::new("Copy Path", "Ctrl+C", ContextMenuAction::CopyPath),
            ContextMenuItem::new("Copy Relative Path", "", ContextMenuAction::CopyRelativePath)
                .separator_after(),
            ContextMenuItem::new("Rename", "F2", ContextMenuAction::Rename),
            ContextMenuItem::new("Delete", "Del", ContextMenuAction::Delete),
        ]
    }
}

/// Context menu widget for rendering.
pub struct ContextMenuWidget<'a> {
    state: &'a ContextMenuState,
}

impl<'a> ContextMenuWidget<'a> {
    pub fn new(state: &'a ContextMenuState) -> Self {
        Self { state }
    }
}

impl<'a> Widget for ContextMenuWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.state.visible || self.state.items.is_empty() {
            return;
        }

        let bg = Color::Rgb(30, 30, 46);         // Catppuccin base
        let border_color = Color::Rgb(137, 180, 250); // Catppuccin blue
        let text = Color::Rgb(205, 214, 244);    // Catppuccin text
        let subtext = Color::Rgb(166, 173, 200); // Catppuccin subtext0
        let green = Color::Rgb(166, 227, 161);   // Catppuccin green
        let dim = Color::Rgb(88, 91, 112);       // Catppuccin overlay0
        let sel_bg = Color::Rgb(69, 71, 90);     // Catppuccin surface1
        let peach = Color::Rgb(250, 179, 135);   // Catppuccin peach

        let menu_w = self.state.needed_width();
        let menu_h = self.state.needed_height();

        // Adjust position to stay within bounds
        let menu_x = if self.state.x + menu_w > area.width {
            area.width.saturating_sub(menu_w + 1)
        } else {
            self.state.x
        };
        let menu_y = if self.state.y + menu_h > area.height {
            area.height.saturating_sub(menu_h + 1)
        } else {
            self.state.y
        };

        // Draw shadow
        let shadow_style = Style::default().bg(Color::Rgb(17, 17, 27));
        for y in (menu_y + 1)..=(menu_y + menu_h) {
            for x in (menu_x + 1)..=(menu_x + menu_w) {
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
        for y in menu_y..menu_y + menu_h {
            for x in menu_x..menu_x + menu_w {
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
        if menu_y < area.y + area.height {
            buf.set_string(menu_x, menu_y, "\u{256d}", border_style);
            for x in (menu_x + 1)..(menu_x + menu_w - 1) {
                buf.set_string(x, menu_y, "\u{2500}", border_style);
            }
            buf.set_string(menu_x + menu_w - 1, menu_y, "\u{256e}", border_style);
        }
        // Bottom border
        let bot = menu_y + menu_h - 1;
        if bot < area.y + area.height {
            buf.set_string(menu_x, bot, "\u{2570}", border_style);
            for x in (menu_x + 1)..(menu_x + menu_w - 1) {
                buf.set_string(x, bot, "\u{2500}", border_style);
            }
            buf.set_string(menu_x + menu_w - 1, bot, "\u{256f}", border_style);
        }
        // Side borders
        for y in (menu_y + 1)..bot {
            if y < area.y + area.height {
                buf.set_string(menu_x, y, "\u{2502}", border_style);
                buf.set_string(menu_x + menu_w - 1, y, "\u{2502}", border_style);
            }
        }

        // Draw menu items
        let inner_w = (menu_w - 2) as usize;
        let mut row_y = menu_y + 1;

        for (i, item) in self.state.items.iter().enumerate() {
            if row_y >= bot || row_y >= area.y + area.height {
                break;
            }

            let is_selected = i == self.state.selected;
            let row_bg = if is_selected { sel_bg } else { bg };

            // Clear row
            let clear = " ".repeat(inner_w);
            buf.set_string(menu_x + 1, row_y, &clear, Style::default().bg(row_bg));

            // Icon/indicator
            let icon = get_action_icon(item.action);
            let icon_style = if is_selected {
                Style::default().fg(peach).bg(row_bg)
            } else if item.enabled {
                Style::default().fg(green).bg(row_bg)
            } else {
                Style::default().fg(dim).bg(row_bg)
            };
            buf.set_string(menu_x + 1, row_y, icon, icon_style);

            // Label
            let label_style = if is_selected {
                Style::default().fg(text).bg(row_bg).add_modifier(Modifier::BOLD)
            } else if item.enabled {
                Style::default().fg(text).bg(row_bg)
            } else {
                Style::default().fg(dim).bg(row_bg)
            };
            buf.set_string(menu_x + 3, row_y, &item.label, label_style);

            // Shortcut (right-aligned)
            if !item.shortcut.is_empty() {
                let shortcut_style = Style::default().fg(subtext).bg(row_bg);
                let sc_x = menu_x + menu_w - 2 - item.shortcut.len() as u16;
                if sc_x > menu_x + 3 + item.label.len() as u16 {
                    buf.set_string(sc_x, row_y, &item.shortcut, shortcut_style);
                }
            }

            row_y += 1;

            // Draw separator if needed
            if item.separator_after && row_y < bot {
                let sep_style = Style::default().fg(dim).bg(bg);
                buf.set_string(menu_x, row_y, "\u{251c}", sep_style);
                for x in (menu_x + 1)..(menu_x + menu_w - 1) {
                    buf.set_string(x, row_y, "\u{2500}", sep_style);
                }
                buf.set_string(menu_x + menu_w - 1, row_y, "\u{2524}", sep_style);
                row_y += 1;
            }
        }
    }
}

/// Get an icon for a context menu action.
fn get_action_icon(action: ContextMenuAction) -> &'static str {
    match action {
        ContextMenuAction::OpenInTerminal => "\u{f120} ", // terminal
        ContextMenuAction::OpenInNewTab => "\u{f24d} ",   // clone/tab
        ContextMenuAction::OpenInEditor => "\u{f15c} ",   // file-text
        ContextMenuAction::CopyPath => "\u{f0c5} ",       // copy
        ContextMenuAction::CopyRelativePath => "\u{f0c5} ", // copy
        ContextMenuAction::Rename => "\u{f02b} ",         // tag/edit
        ContextMenuAction::Delete => "\u{f1f8} ",         // trash
        ContextMenuAction::Refresh => "\u{f021} ",        // refresh
        ContextMenuAction::NavigateUp => "\u{f062} ",     // arrow up
        ContextMenuAction::NewFile => "\u{f15b} ",        // file
        ContextMenuAction::NewFolder => "\u{f07b} ",      // folder
    }
}
