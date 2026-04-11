//! Modal text/code editor widget — vim-inspired (Normal/Insert modes).
//!
//! Features:
//! - Normal / Insert mode (like Vim)
//! - Syntax highlighting via syntect (built-in Textmate grammars)
//! - Line numbers, cursor tracking, scroll
//! - Undo stack (single-level granularity)
//! - Ctrl+S save, Ctrl+W / Esc (Normal) close with unsaved-changes guard

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use std::path::PathBuf;
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

// ── Catppuccin Mocha palette ─────────────────────────────────────────────────
const C_BASE: Color = Color::Rgb(30, 30, 46);
const C_MANTLE: Color = Color::Rgb(24, 24, 37);
const C_SURFACE0: Color = Color::Rgb(49, 50, 68);
const C_SURFACE1: Color = Color::Rgb(69, 71, 90);
const C_OVERLAY0: Color = Color::Rgb(108, 112, 134);
const C_TEXT: Color = Color::Rgb(205, 214, 244);
const C_SUBTEXT0: Color = Color::Rgb(166, 173, 200);
const C_BLUE: Color = Color::Rgb(137, 180, 250);
const C_PINK: Color = Color::Rgb(245, 194, 231);
const C_GREEN: Color = Color::Rgb(166, 227, 161);
const C_RED: Color = Color::Rgb(243, 139, 168);
const C_YELLOW: Color = Color::Rgb(249, 226, 175);
const C_PEACH: Color = Color::Rgb(250, 179, 135);

/// Editor mode — Normal (navigation) or Insert (typing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
}

/// A single undo entry — stores the full line content before a change.
#[derive(Debug, Clone)]
struct UndoEntry {
    row: usize,
    old_line: String,
    cursor_col: usize,
}

/// Pre-highlighted span: (fg_color, text_segment).
type HighlightedLine = Vec<(Color, String)>;

/// State for the embedded editor.
pub struct EditorState {
    pub path: PathBuf,
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_offset: usize,
    pub mode: EditorMode,
    pub modified: bool,
    /// Double-Ctrl+W close guard — first Ctrl+W in Normal sets this to true.
    pub close_requested: bool,
    /// If true, the editor should be closed by the caller.
    pub should_close: bool,
    undo_stack: Vec<UndoEntry>,
    syntax_cache: Vec<HighlightedLine>,
    /// SyntaxSet/Theme are reused across renders
    ss: SyntaxSet,
    theme: Theme,
    pub extension: String,
}

impl std::fmt::Debug for EditorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorState")
            .field("path", &self.path)
            .field("cursor_row", &self.cursor_row)
            .field("cursor_col", &self.cursor_col)
            .field("mode", &self.mode)
            .field("modified", &self.modified)
            .finish()
    }
}

impl EditorState {
    /// Create a new editor for the given file.
    /// `content` is the full file text. Pass empty string for new files.
    pub fn new(path: impl Into<PathBuf>, content: String) -> Self {
        let path = path.into();
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            // Preserve lines, but strip trailing \r
            let mut v: Vec<String> = content
                .lines()
                .map(|l| l.trim_end_matches('\r').to_string())
                .collect();
            if v.is_empty() {
                v.push(String::new());
            }
            v
        };

        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        // Use a dark theme compatible with Catppuccin
        let theme = ts
            .themes
            .get("base16-ocean.dark")
            .or_else(|| ts.themes.get("base16-eighties.dark"))
            .or_else(|| ts.themes.values().next())
            .unwrap()
            .clone();

        let mut state = Self {
            path,
            lines,
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            mode: EditorMode::Normal,
            modified: false,
            close_requested: false,
            should_close: false,
            undo_stack: Vec::new(),
            syntax_cache: Vec::new(),
            ss,
            theme,
            extension,
        };
        state.rebuild_highlight_cache();
        state
    }

    fn rebuild_highlight_cache(&mut self) {
        self.syntax_cache =
            highlight_all_lines(&self.lines, &self.extension, &self.ss, &self.theme);
    }

    fn invalidate_line_cache(&mut self, row: usize) {
        let highlighted = highlight_line(&self.lines[row], &self.extension, &self.ss, &self.theme);
        if row < self.syntax_cache.len() {
            self.syntax_cache[row] = highlighted;
        } else {
            // Rebuild fully if sizes diverged
            self.rebuild_highlight_cache();
        }
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.push_undo();
        let row = self.cursor_row;
        let col = self.cursor_col.min(self.lines[row].len());
        self.lines[row].insert(col, c);
        self.cursor_col = col + 1;
        self.modified = true;
        self.invalidate_line_cache(row);
        self.close_requested = false;
    }

    /// Insert a newline at cursor — splits the current line.
    pub fn insert_newline(&mut self) {
        self.push_undo_range(self.cursor_row, self.cursor_row);
        let row = self.cursor_row;
        let col = self.cursor_col.min(self.lines[row].len());
        let tail = self.lines[row].split_off(col);
        self.lines.insert(row + 1, tail);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.modified = true;
        self.rebuild_highlight_cache();
        self.close_requested = false;
    }

    /// Delete the character before the cursor (Backspace).
    pub fn delete_char_before(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        self.push_undo();
        if col > 0 {
            let actual_col = col.min(self.lines[row].len());
            self.lines[row].remove(actual_col - 1);
            self.cursor_col = actual_col - 1;
            self.modified = true;
            self.invalidate_line_cache(row);
        } else if row > 0 {
            // Join with previous line
            self.push_undo_range(row - 1, row);
            let cur_line = self.lines.remove(row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&cur_line);
            self.modified = true;
            self.rebuild_highlight_cache();
        }
        self.close_requested = false;
    }

    /// Delete character at cursor (Normal mode `x`).
    pub fn delete_char_at(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col.min(self.lines[row].len().saturating_sub(1));
        if self.lines[row].is_empty() {
            return;
        }
        self.push_undo();
        self.lines[row].remove(col);
        self.cursor_col = col.min(self.lines[row].len().saturating_sub(1));
        self.modified = true;
        self.invalidate_line_cache(row);
        self.close_requested = false;
    }

    /// Delete current line (`dd`).
    pub fn delete_line(&mut self) {
        self.push_undo_range(self.cursor_row, self.cursor_row);
        if self.lines.len() > 1 {
            self.lines.remove(self.cursor_row);
            if self.cursor_row >= self.lines.len() {
                self.cursor_row = self.lines.len() - 1;
            }
        } else {
            self.lines[0].clear();
        }
        self.cursor_col = 0;
        self.modified = true;
        self.rebuild_highlight_cache();
    }

    /// Undo the last change.
    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            if entry.row < self.lines.len() {
                self.lines[entry.row] = entry.old_line;
                self.cursor_row = entry.row;
                self.cursor_col = entry.cursor_col;
                self.modified = true;
                self.rebuild_highlight_cache();
            }
        }
    }

    /// Move cursor: h/j/k/l.
    pub fn move_cursor(&mut self, dr: i32, dc: i32) {
        let new_row = (self.cursor_row as i32 + dr).clamp(0, self.lines.len() as i32 - 1) as usize;
        self.cursor_row = new_row;
        let line_len = self.lines[self.cursor_row].len();
        let new_col = (self.cursor_col as i32 + dc).clamp(0, line_len as i32) as usize;
        self.cursor_col = new_col;
        self.adjust_scroll_to_cursor(0);
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_to_line_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_row].len();
    }

    pub fn move_to_first_line(&mut self) {
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.adjust_scroll_to_cursor(0);
    }

    pub fn move_to_last_line(&mut self) {
        self.cursor_row = self.lines.len().saturating_sub(1);
        self.cursor_col = 0;
        self.adjust_scroll_to_cursor(0);
    }

    /// Adjust scroll so cursor stays within the visible window.
    pub fn adjust_scroll_to_cursor(&mut self, visible_height: usize) {
        let vh = if visible_height < 3 {
            20
        } else {
            visible_height
        };
        if self.cursor_row < self.scroll_offset {
            self.scroll_offset = self.cursor_row;
        }
        if self.cursor_row >= self.scroll_offset + vh {
            self.scroll_offset = self.cursor_row.saturating_sub(vh - 1);
        }
    }

    /// Save the file to disk.
    pub fn save(&mut self) -> std::io::Result<()> {
        let content = self.lines.join("\n");
        std::fs::write(&self.path, content)?;
        self.modified = false;
        self.close_requested = false;
        Ok(())
    }

    fn push_undo(&mut self) {
        let row = self.cursor_row;
        let old_line = self.lines[row].clone();
        let col = self.cursor_col;
        self.undo_stack.push(UndoEntry {
            row,
            old_line,
            cursor_col: col,
        });
        // Limit undo history
        if self.undo_stack.len() > 500 {
            self.undo_stack.remove(0);
        }
    }

    fn push_undo_range(&mut self, _start: usize, end: usize) {
        // Just push the end row for simplicity
        let row = end.min(self.lines.len().saturating_sub(1));
        let old_line = self.lines[row].clone();
        let col = self.cursor_col;
        self.undo_stack.push(UndoEntry {
            row,
            old_line,
            cursor_col: col,
        });
        if self.undo_stack.len() > 500 {
            self.undo_stack.remove(0);
        }
    }
}

// ── Syntax highlighting helpers ───────────────────────────────────────────────

fn highlight_line(line: &str, extension: &str, ss: &SyntaxSet, theme: &Theme) -> HighlightedLine {
    let syntax = ss
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, theme);
    let spans = h.highlight_line(line, ss).unwrap_or_default();
    spans
        .into_iter()
        .map(|(style, text)| {
            let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            (fg, text.to_string())
        })
        .collect()
}

fn highlight_all_lines(
    lines: &[String],
    extension: &str,
    ss: &SyntaxSet,
    theme: &Theme,
) -> Vec<HighlightedLine> {
    let syntax = ss
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, theme);

    // Build a single string with newlines so syntect can do stateful parsing
    let full_text = lines.join("\n") + "\n";
    let mut result: Vec<HighlightedLine> = Vec::with_capacity(lines.len());

    for line in LinesWithEndings::from(&full_text) {
        let spans = h.highlight_line(line, ss).unwrap_or_default();
        let colored: HighlightedLine = spans
            .into_iter()
            .map(|(style, text)| {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                (fg, text.trim_end_matches(&['\n', '\r']).to_string())
            })
            .filter(|(_, t)| !t.is_empty())
            .collect();
        result.push(colored);
    }

    // Pad or trim to match lines count
    result.resize(lines.len(), vec![]);
    result
}

// ── Widget ─────────────────────────────────────────────────────────────────────

/// Ratatui widget that renders the editor.
pub struct EditorWidget<'a> {
    pub state: &'a EditorState,
}

impl<'a> EditorWidget<'a> {
    pub fn new(state: &'a EditorState) -> Self {
        Self { state }
    }
}

impl<'a> Widget for EditorWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 10 {
            return;
        }

        let state = self.state;
        // Layout: line_num (4) + gutter (1) + code (rest), bottom row = status bar
        let line_num_w: u16 = 4;
        let gutter_w: u16 = 1;
        let code_x = area.x + line_num_w + gutter_w;
        let code_w = area.width.saturating_sub(line_num_w + gutter_w);
        let content_h = area.height.saturating_sub(1); // -1 for bottom status bar

        // Fill background
        let bg_style = Style::default().bg(C_BASE);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        // Render visible lines
        for screen_row in 0..content_h {
            let file_row = state.scroll_offset + screen_row as usize;
            let y = area.y + screen_row;

            if file_row >= state.lines.len() {
                // Tilde for lines past EOF (like vim)
                let tilde_style = Style::default().fg(C_SURFACE1).bg(C_BASE);
                if let Some(cell) = buf.cell_mut((area.x, y)) {
                    cell.set_char('~');
                    cell.set_style(tilde_style);
                }
                continue;
            }

            let is_cursor_line = file_row == state.cursor_row;
            let line_bg = if is_cursor_line { C_SURFACE0 } else { C_BASE };

            // Line number
            let linenum_style = if is_cursor_line {
                Style::default().fg(C_YELLOW).bg(C_MANTLE)
            } else {
                Style::default().fg(C_OVERLAY0).bg(C_MANTLE)
            };
            let linenum_str = format!("{:>3} ", file_row + 1);
            buf.set_string(area.x, y, &linenum_str, linenum_style);

            // Gutter (cursor line indicator)
            let gutter_x = area.x + line_num_w;
            if is_cursor_line {
                let g_style = Style::default().fg(C_BLUE).bg(C_MANTLE);
                if let Some(cell) = buf.cell_mut((gutter_x, y)) {
                    cell.set_char('\u{258f}'); // ▏
                    cell.set_style(g_style);
                }
            } else {
                if let Some(cell) = buf.cell_mut((gutter_x, y)) {
                    cell.set_char(' ');
                    cell.set_style(Style::default().bg(C_MANTLE));
                }
            }

            // Highlight the cursor-line background
            if is_cursor_line {
                for cx in code_x..area.x + area.width {
                    if let Some(cell) = buf.cell_mut((cx, y)) {
                        cell.set_bg(C_SURFACE0);
                    }
                }
            }

            // Code content — draw highlighted spans
            let line = &state.lines[file_row];
            let mut draw_x = code_x;
            let max_x = area.x + area.width;

            if file_row < state.syntax_cache.len() && !state.syntax_cache[file_row].is_empty() {
                for (fg, text) in &state.syntax_cache[file_row] {
                    for ch in text.chars() {
                        if draw_x >= max_x {
                            break;
                        }
                        let style = Style::default().fg(*fg).bg(line_bg);
                        if let Some(cell) = buf.cell_mut((draw_x, y)) {
                            cell.set_char(ch);
                            cell.set_style(style);
                        }
                        draw_x += 1;
                    }
                    if draw_x >= max_x {
                        break;
                    }
                }
            } else {
                // Fallback: plain text
                for ch in line.chars() {
                    if draw_x >= max_x {
                        break;
                    }
                    let style = Style::default().fg(C_TEXT).bg(line_bg);
                    if let Some(cell) = buf.cell_mut((draw_x, y)) {
                        cell.set_char(ch);
                        cell.set_style(style);
                    }
                    draw_x += 1;
                }
            }

            // Draw cursor block / underline
            if is_cursor_line && state.cursor_col <= line.len() {
                let cx = code_x + state.cursor_col as u16;
                if cx < max_x {
                    let cursor_char = if state.mode == EditorMode::Insert {
                        line.chars().nth(state.cursor_col).unwrap_or(' ')
                    } else {
                        line.chars().nth(state.cursor_col).unwrap_or(' ')
                    };
                    let cursor_style = if state.mode == EditorMode::Normal {
                        Style::default().fg(C_BASE).bg(C_TEXT)
                    } else {
                        Style::default()
                            .fg(C_BASE)
                            .bg(C_GREEN)
                            .add_modifier(Modifier::BOLD)
                    };
                    if let Some(cell) = buf.cell_mut((cx, y)) {
                        cell.set_char(cursor_char);
                        cell.set_style(cursor_style);
                    }
                }
            }
        }

        // ── Status / mode bar (bottom row) ──────────────────────────────────
        let status_y = area.y + content_h;
        let (mode_str, mode_fg, mode_bg) = match state.mode {
            EditorMode::Normal => (" NORMAL ", C_BASE, C_BLUE),
            EditorMode::Insert => (" INSERT ", C_BASE, C_GREEN),
        };

        // Mode badge
        let mode_style = Style::default()
            .fg(mode_fg)
            .bg(mode_bg)
            .add_modifier(Modifier::BOLD);
        buf.set_string(area.x, status_y, mode_str, mode_style);
        let mut sx = area.x + mode_str.len() as u16;

        // Filename + modified indicator
        let fname = state
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "[no name]".to_string());
        let mod_indicator = if state.modified { " [+]" } else { "" };
        let file_str = format!("  {}{}", fname, mod_indicator);

        let file_style = if state.modified {
            Style::default()
                .fg(C_PEACH)
                .bg(C_SURFACE0)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_TEXT).bg(C_SURFACE0)
        };

        let right_str = format!("Ln {} Col {} ", state.cursor_row + 1, state.cursor_col + 1);
        let right_x = (area.x + area.width).saturating_sub(right_str.len() as u16);

        // Fill status bar background
        let status_mid = format!(
            "{:<width$}",
            file_str,
            width = right_x.saturating_sub(sx) as usize
        );
        buf.set_string(sx, status_y, &status_mid, file_style);
        sx += status_mid.len() as u16;

        // Close hint or unsaved warning
        let hint_str = if state.close_requested && state.modified {
            format!(
                "{:<width$}",
                "  Unsaved! Ctrl+S=save Ctrl+W=discard",
                width = right_x.saturating_sub(sx) as usize
            )
        } else {
            String::new()
        };
        if !hint_str.is_empty() {
            let warn_style = Style::default()
                .fg(C_RED)
                .bg(C_SURFACE0)
                .add_modifier(Modifier::BOLD);
            buf.set_string(sx, status_y, &hint_str, warn_style);
        }

        // Right side: line/col
        let pos_style = Style::default().fg(C_SUBTEXT0).bg(C_SURFACE1);
        buf.set_string(right_x, status_y, &right_str, pos_style);
    }
}
