//! File browser widget — directory tree sidebar for desktop-like file navigation.
//!
//! Displays a tree view of the filesystem. Folders can be expanded/collapsed.
//! Pressing Enter on a folder opens a new terminal pane cd'd into that directory.
//! Pressing Enter on a file opens it in the user's $EDITOR (if configured).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use std::path::{Path, PathBuf};

/// A single entry in the file browser tree.
#[derive(Debug, Clone)]
pub struct FsEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
    pub is_symlink: bool,
}

/// File browser state — tracks cwd, entries, cursor, scroll.
#[derive(Debug, Clone)]
pub struct FileBrowserState {
    pub root: PathBuf,
    pub entries: Vec<FsEntry>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub visible: bool,
    pub width: u16,
}

impl FileBrowserState {
    /// Create a new file browser rooted at the given path.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let mut state = Self {
            root: root.clone(),
            entries: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            visible: false,
            width: 30,
        };
        state.refresh_root();
        state
    }

    /// Refresh the root directory listing.
    pub fn refresh_root(&mut self) {
        self.entries.clear();
        // Add parent entry (..)
        if let Some(parent) = self.root.parent() {
            self.entries.push(FsEntry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
                depth: 0,
                expanded: false,
                is_symlink: false,
            });
        }
        self.load_children(&self.root.clone(), 0);
    }

    /// Load children of a directory at the given depth.
    fn load_children(&mut self, dir: &Path, depth: usize) {
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files/folders starting with .
                // (but show them — desktop should show everything)
                let is_symlink = entry
                    .file_type()
                    .map(|ft| ft.is_symlink())
                    .unwrap_or(false);
                let is_dir = path.is_dir();

                let fs_entry = FsEntry {
                    name,
                    path,
                    is_dir,
                    depth: depth + 1,
                    expanded: false,
                    is_symlink,
                };

                if is_dir {
                    dirs.push(fs_entry);
                } else {
                    files.push(fs_entry);
                }
            }
        }

        // Sort: directories first (alphabetical), then files (alphabetical)
        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        // Find insertion point (after parent entries at this depth)
        let insert_at = self.entries.len();
        for entry in dirs {
            self.entries.insert(insert_at + self.entries.len() - insert_at, entry);
        }
        for entry in files {
            self.entries.push(entry);
        }
    }

    /// Navigate to a new root directory.
    pub fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.root = path;
            self.cursor = 0;
            self.scroll_offset = 0;
            self.refresh_root();
        }
    }

    /// Toggle expansion of a directory entry.
    pub fn toggle_expand(&mut self) {
        if self.cursor >= self.entries.len() {
            return;
        }

        let entry = &self.entries[self.cursor];
        if !entry.is_dir {
            return;
        }

        // Special case: ".." navigates up
        if entry.name == ".." {
            let parent = entry.path.clone();
            self.navigate_to(parent);
            return;
        }

        let was_expanded = entry.expanded;
        let entry_path = entry.path.clone();
        let entry_depth = entry.depth;

        if was_expanded {
            // Collapse: remove all children
            self.entries[self.cursor].expanded = false;
            let remove_start = self.cursor + 1;
            let mut remove_end = remove_start;
            while remove_end < self.entries.len()
                && self.entries[remove_end].depth > entry_depth
            {
                remove_end += 1;
            }
            self.entries.drain(remove_start..remove_end);
        } else {
            // Expand: load children
            self.entries[self.cursor].expanded = true;
            let insert_pos = self.cursor + 1;
            let mut children = Vec::new();
            let mut child_dirs = Vec::new();
            let mut child_files = Vec::new();

            if let Ok(entries) = std::fs::read_dir(&entry_path) {
                for e in entries.flatten() {
                    let path = e.path();
                    let name = e.file_name().to_string_lossy().to_string();
                    let is_symlink = e
                        .file_type()
                        .map(|ft| ft.is_symlink())
                        .unwrap_or(false);
                    let is_dir = path.is_dir();

                    let fs_entry = FsEntry {
                        name,
                        path,
                        is_dir,
                        depth: entry_depth + 1,
                        expanded: false,
                        is_symlink,
                    };

                    if is_dir {
                        child_dirs.push(fs_entry);
                    } else {
                        child_files.push(fs_entry);
                    }
                }
            }

            child_dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            child_files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            children.extend(child_dirs);
            children.extend(child_files);

            // Insert children right after the parent
            for (i, child) in children.into_iter().enumerate() {
                self.entries.insert(insert_pos + i, child);
            }
        }
    }

    /// Move cursor up.
    pub fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            if self.cursor < self.scroll_offset {
                self.scroll_offset = self.cursor;
            }
        }
    }

    /// Move cursor down.
    pub fn cursor_down(&mut self) {
        if self.cursor + 1 < self.entries.len() {
            self.cursor += 1;
        }
    }

    /// Get the currently selected entry.
    pub fn selected_entry(&self) -> Option<&FsEntry> {
        self.entries.get(self.cursor)
    }

    /// Get the selected path for opening in a pane.
    pub fn selected_path(&self) -> Option<PathBuf> {
        self.entries.get(self.cursor).map(|e| e.path.clone())
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Adjust scroll offset for visible height.
    pub fn adjust_scroll(&mut self, visible_height: usize) {
        if self.cursor >= self.scroll_offset + visible_height {
            self.scroll_offset = self.cursor.saturating_sub(visible_height - 1);
        }
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        }
    }
}

/// File icon based on entry type and extension.
fn file_icon(entry: &FsEntry) -> &'static str {
    if entry.name == ".." {
        return "\u{f062} "; // ↑ up arrow
    }
    if entry.is_dir {
        if entry.expanded {
            return "\u{f07c} "; // open folder
        } else {
            return "\u{f07b} "; // closed folder
        }
    }
    if entry.is_symlink {
        return "\u{f0c1} "; // link
    }

    // File type icons based on extension
    let ext = entry
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext.to_lowercase().as_str() {
        "rs" => "\u{e7a8} ",    // Rust
        "py" => "\u{e73c} ",    // Python
        "js" | "jsx" => "\u{e74e} ", // JavaScript
        "ts" | "tsx" => "\u{e628} ", // TypeScript
        "html" | "htm" => "\u{e736} ", // HTML
        "css" | "scss" => "\u{e749} ", // CSS
        "json" => "\u{e60b} ",  // JSON
        "toml" | "yaml" | "yml" => "\u{e615} ", // Config
        "md" => "\u{e73e} ",    // Markdown
        "txt" => "\u{f0f6} ",   // Text
        "sh" | "bash" | "zsh" => "\u{f489} ", // Shell
        "exe" | "msi" => "\u{f013} ", // Executable
        "zip" | "tar" | "gz" | "7z" | "rar" => "\u{f1c6} ", // Archive
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "bmp" => "\u{f1c5} ", // Image
        "mp4" | "avi" | "mkv" | "mov" => "\u{f1c8} ", // Video
        "mp3" | "wav" | "flac" | "ogg" => "\u{f1c7} ", // Audio
        "pdf" => "\u{f1c1} ",   // PDF
        "doc" | "docx" => "\u{f1c2} ", // Word
        "lock" => "\u{f023} ",  // Lock
        "git" | "gitignore" => "\u{f1d3} ", // Git
        _ => "\u{f15b} ",       // Generic file
    }
}

/// Render the file browser as a ratatui widget.
pub struct FileBrowserWidget<'a> {
    state: &'a FileBrowserState,
    focused: bool,
}

impl<'a> FileBrowserWidget<'a> {
    pub fn new(state: &'a FileBrowserState, focused: bool) -> Self {
        Self { state, focused }
    }
}

impl<'a> Widget for FileBrowserWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = Color::Rgb(24, 24, 37);          // Catppuccin mantle
        let surface = Color::Rgb(30, 30, 46);     // Catppuccin base
        let border_color = if self.focused {
            Color::Rgb(137, 180, 250)              // Catppuccin blue
        } else {
            Color::Rgb(69, 71, 90)                 // Catppuccin surface1
        };
        let text_color = Color::Rgb(205, 214, 244); // Catppuccin text
        let dir_color = Color::Rgb(137, 180, 250); // Catppuccin blue
        let file_color = Color::Rgb(166, 173, 200); // Catppuccin subtext0
        let link_color = Color::Rgb(203, 166, 247); // Catppuccin mauve
        let cursor_bg = Color::Rgb(69, 71, 90);   // Catppuccin surface1
        let dim = Color::Rgb(88, 91, 112);        // Catppuccin overlay0
        let green = Color::Rgb(166, 227, 161);    // Catppuccin green

        // Fill background
        let bg_style = Style::default().bg(bg);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        // Draw left border
        let border_style = Style::default().fg(border_color).bg(bg);
        // Right edge separator
        let right_x = area.x + area.width.saturating_sub(1);
        for y in area.y..area.y + area.height {
            if let Some(cell) = buf.cell_mut((right_x, y)) {
                cell.set_char('\u{2502}'); // │
                cell.set_style(border_style);
            }
        }

        let inner_w = area.width.saturating_sub(1) as usize; // exclude right border
        let content_area_h = area.height.saturating_sub(2) as usize; // header + path

        // Header
        let header_style = Style::default()
            .fg(text_color)
            .bg(surface)
            .add_modifier(Modifier::BOLD);
        let header = " \u{f07b} Explorer";
        let header_padded = format!("{:<width$}", header, width = inner_w);
        buf.set_string(area.x, area.y, &header_padded, header_style);

        // Current path
        let path_str = self.state.root.to_string_lossy();
        let path_display = if path_str.len() > inner_w.saturating_sub(2) {
            format!(
                " ..{}",
                &path_str[path_str.len().saturating_sub(inner_w.saturating_sub(4))..]
            )
        } else {
            format!(" {}", path_str)
        };
        let path_style = Style::default().fg(dim).bg(bg);
        let path_padded = format!("{:<width$}", path_display, width = inner_w);
        buf.set_string(area.x, area.y + 1, &path_padded, path_style);

        // File entries
        let start_y = area.y + 2;
        let visible_entries = &self.state.entries
            [self.state.scroll_offset..self.state.entries.len().min(self.state.scroll_offset + content_area_h)];

        for (i, entry) in visible_entries.iter().enumerate() {
            let abs_idx = self.state.scroll_offset + i;
            let y = start_y + i as u16;
            if y >= area.y + area.height {
                break;
            }

            let is_cursor = abs_idx == self.state.cursor;

            let row_bg = if is_cursor { cursor_bg } else { bg };
            let row_style = Style::default().bg(row_bg);

            // Clear row
            let clear = " ".repeat(inner_w);
            buf.set_string(area.x, y, &clear, row_style);

            // Indent
            let indent = "  ".repeat(entry.depth);
            let indent_len = indent.len();

            // Tree connector
            let connector = if entry.name == ".." {
                ""
            } else if entry.is_dir && entry.expanded {
                "\u{25be} " // ▾ (expanded)
            } else if entry.is_dir {
                "\u{25b8} " // ▸ (collapsed)
            } else {
                "  "
            };

            // Name style
            let name_style = if entry.name == ".." {
                Style::default().fg(dim).bg(row_bg)
            } else if entry.is_dir {
                Style::default()
                    .fg(dir_color)
                    .bg(row_bg)
                    .add_modifier(Modifier::BOLD)
            } else if entry.is_symlink {
                Style::default()
                    .fg(link_color)
                    .bg(row_bg)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(file_color).bg(row_bg)
            };

            // Cursor indicator
            let cursor_indicator = if is_cursor && self.focused { "\u{25b8}" } else { " " };
            let cursor_style = Style::default().fg(green).bg(row_bg);
            buf.set_string(area.x, y, cursor_indicator, cursor_style);

            // Build the line: indent + connector + name
            let x_start = area.x + 1;
            let connector_style = Style::default().fg(dim).bg(row_bg);

            // Indent
            buf.set_string(x_start, y, &indent, row_style);
            let mut cx = x_start + indent_len as u16;

            // Connector arrow
            buf.set_string(cx, y, connector, connector_style);
            cx += connector.chars().count() as u16;

            // File icon
            let icon = file_icon(entry);
            let icon_style = name_style;
            buf.set_string(cx, y, icon, icon_style);
            cx += icon.chars().count() as u16;

            // File/folder name (truncate if needed)
            let max_name_len = (area.x + area.width)
                .saturating_sub(cx + 1) as usize; // -1 for the right border
            let display_name = if entry.name.len() > max_name_len {
                format!("{}...", &entry.name[..max_name_len.saturating_sub(3)])
            } else {
                entry.name.clone()
            };
            buf.set_string(cx, y, &display_name, name_style);
        }

        // Scrollbar hint if needed
        if self.state.entries.len() > content_area_h && content_area_h > 0 {
            let scroll_pct = self.state.scroll_offset as f64
                / (self.state.entries.len().saturating_sub(content_area_h)) as f64;
            let sb_y = start_y
                + (scroll_pct * (content_area_h.saturating_sub(1)) as f64) as u16;
            if sb_y < area.y + area.height {
                let sb_style = Style::default().fg(dim).bg(bg);
                buf.set_string(right_x, sb_y, "\u{2588}", sb_style); // █ scrollbar thumb
            }
        }
    }
}
