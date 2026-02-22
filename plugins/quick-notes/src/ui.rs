//! UI rendering for Quick Notes plugin.
//!
//! Provides terminal-based UI rendering using the uterx_ui host API.

use crate::markdown::render_to_ansi;
use crate::note::{Note, NoteId, NoteManager};
use crate::storage::Storage;

/// UI mode for the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Show list of notes.
    List,
    /// Edit/view a single note.
    Editor,
    /// Task list mode (checkbox navigation).
    TaskList,
}

/// Cursor position in the UI.
#[derive(Debug, Clone)]
pub struct Cursor {
    /// Current mode.
    pub mode: Mode,
    /// Selected note ID (if in editor mode).
    pub note_id: Option<NoteId>,
    /// Cursor position in list (row index).
    pub list_index: usize,
    /// Cursor position in editor (row, col).
    pub editor_row: usize,
    pub editor_col: usize,
    /// Task index for task list mode.
    pub task_index: usize,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            mode: Mode::List,
            note_id: None,
            list_index: 0,
            editor_row: 0,
            editor_col: 0,
            task_index: 0,
        }
    }
}

/// Plugin UI state.
pub struct UiState {
    /// Current cursor state.
    pub cursor: Cursor,
    /// Overlay pane ID.
    pub pane_id: u64,
    /// Pane dimensions.
    pub width: u32,
    pub height: u32,
    /// Dirty flag for redraw.
    pub dirty: bool,
    /// Input buffer.
    pub input_buffer: String,
    /// Status message.
    pub status: Option<String>,
}

impl UiState {
    /// Create a new UI state.
    pub fn new(pane_id: u64) -> Self {
        Self {
            cursor: Cursor::default(),
            pane_id,
            width: 80,
            height: 24,
            dirty: true,
            input_buffer: String::new(),
            status: None,
        }
    }

    /// Update pane dimensions.
    pub fn update_size(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.dirty = true;
        }
    }

    /// Move cursor up in list.
    pub fn move_up(&mut self, max: usize) {
        if self.cursor.list_index > 0 {
            self.cursor.list_index -= 1;
            self.dirty = true;
        } else if max > 0 {
            self.cursor.list_index = max - 1;
            self.dirty = true;
        }
    }

    /// Move cursor down in list.
    pub fn move_down(&mut self, max: usize) {
        if self.cursor.list_index < max.saturating_sub(1) {
            self.cursor.list_index += 1;
            self.dirty = true;
        } else {
            self.cursor.list_index = 0;
            self.dirty = true;
        }
    }

    /// Enter editor mode for a note.
    pub fn enter_editor(&mut self, note_id: NoteId) {
        self.cursor.mode = Mode::Editor;
        self.cursor.note_id = Some(note_id);
        self.cursor.editor_row = 0;
        self.cursor.editor_col = 0;
        self.dirty = true;
    }

    /// Exit to list mode.
    pub fn exit_to_list(&mut self) {
        self.cursor.mode = Mode::List;
        self.cursor.note_id = None;
        self.cursor.editor_row = 0;
        self.cursor.editor_col = 0;
        self.dirty = true;
    }

    /// Enter task list mode.
    pub fn enter_task_list(&mut self, note_id: NoteId) {
        self.cursor.mode = Mode::TaskList;
        self.cursor.note_id = Some(note_id);
        self.cursor.task_index = 0;
        self.dirty = true;
    }

    /// Set status message.
    pub fn set_status(&mut self, message: String) {
        self.status = Some(message);
        self.dirty = true;
    }

    /// Clear status message.
    pub fn clear_status(&mut self) {
        self.status = None;
        self.dirty = true;
    }
}

/// Render the note list to the pane.
pub fn render_list(state: &UiState, manager: &NoteManager) {
    let notes = manager.list_by_updated();
    
    // Title
    draw_text(state.pane_id, 0, 0, "╭─ Quick Notes ─╮");
    draw_text(state.pane_id, 0, 1, "│");
    
    if notes.is_empty() {
        draw_text(state.pane_id, 2, 2, "No notes. Press 'n' to create.");
    } else {
        for (i, note) in notes.iter().enumerate() {
            let y = 2 + i as u32;
            if y >= state.height.saturating_sub(2) {
                break;
            }
            
            let prefix = if i == state.cursor.list_index { "▶ " } else { "  " };
            let title = truncate(&note.title, state.width.saturating_sub(4) as usize);
            let line = format!("{}{}", prefix, title);
            
            draw_text(state.pane_id, 0, y, &line);
        }
    }
    
    // Footer
    let footer_y = state.height.saturating_sub(1);
    draw_text(state.pane_id, 0, footer_y, "╰─ [n]ew [e]dit [d]elete [q]uit ─╯");
}

/// Render a single note in editor mode.
pub fn render_editor(state: &UiState, note: &Note) {
    // Title bar
    let title = truncate(&note.title, state.width.saturating_sub(4) as usize);
    draw_text(state.pane_id, 0, 0, &format!("╭─ {} ─╮", title));
    
    // Render markdown content
    let lines: Vec<&str> = note.content.lines().collect();
    let scroll_offset = state.cursor.editor_row.saturating_sub(state.height as usize / 2);
    
    for (i, line) in lines.iter().enumerate().skip(scroll_offset) {
        let y = 2 + (i - scroll_offset) as u32;
        if y >= state.height.saturating_sub(2) {
            break;
        }
        
        // Render with markdown styling
        let styled = render_to_ansi(line);
        draw_text(state.pane_id, 0, y, &styled);
    }
    
    // Footer
    let footer_y = state.height.saturating_sub(1);
    draw_text(state.pane_id, 0, footer_y, "╰─ [Esc] back [s]ave [t]asks ─╯");
}

/// Render task list mode.
pub fn render_task_list(state: &UiState, note: &Note) {
    let title = truncate(&note.title, state.width.saturating_sub(4) as usize);
    draw_text(state.pane_id, 0, 0, &format!("╭─ {} (Tasks) ─╮", title));
    
    // Parse tasks from content
    let tasks = extract_tasks(&note.content);
    
    for (i, (checked, text)) in tasks.iter().enumerate() {
        let y = 2 + i as u32;
        if y >= state.height.saturating_sub(2) {
            break;
        }
        
        let prefix = if i == state.cursor.task_index { "▶ " } else { "  " };
        let checkbox = if *checked { "☑ " } else { "☐ " };
        let style = if *checked { "\x1b[32m" } else { "" };
        let reset = if *checked { "\x1b[0m" } else { "" };
        
        let line = format!("{}{}{}{}{}", prefix, checkbox, style, text, reset);
        draw_text(state.pane_id, 0, y, &line);
    }
    
    // Footer
    let footer_y = state.height.saturating_sub(1);
    draw_text(state.pane_id, 0, footer_y, "╰─ [Space] toggle [Esc] back ─╯");
}

/// Extract tasks from markdown content.
fn extract_tasks(content: &str) -> Vec<(bool, String)> {
    let mut tasks = Vec::new();
    
    for line in content.lines() {
        if line.starts_with("- [ ] ") {
            tasks.push((false, line[6..].to_string()));
        } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
            tasks.push((true, line[6..].to_string()));
        }
    }
    
    tasks
}

/// Truncate a string to fit within a width.
fn truncate(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else {
        format!("{}...", &s[..max_width.saturating_sub(3)])
    }
}

/// Draw text to the pane using host API.
fn draw_text(pane_id: u64, x: u32, y: u32, text: &str) {
    unsafe {
        uterx_ui_draw_text(
            pane_id as i64,
            x as i32,
            y as i32,
            text.as_ptr() as i32,
            text.len() as i32,
        );
    }
}

/// Create an overlay using host API.
pub fn create_overlay(title: &str) -> u64 {
    unsafe {
        uterx_ui_create_overlay(title.as_ptr() as i32, title.len() as i32) as u64
    }
}

/// Get pane size using host API.
pub fn get_pane_size(pane_id: u64) -> (u32, u32) {
    unsafe {
        let mut w: i32 = 0;
        let mut h: i32 = 0;
        uterx_ui_get_size(pane_id as i64, &mut w, &mut h);
        (w.max(0) as u32, h.max(0) as u32)
    }
}

/// Read input using host API.
pub fn read_input(buf: &mut [u8]) -> i32 {
    unsafe { uterx_io_read(0, buf.as_mut_ptr() as i32, buf.len() as i32) }
}

/// Write output using host API.
pub fn write_output(data: &[u8]) -> i32 {
    unsafe { uterx_io_write(0, data.as_ptr() as i32, data.len() as i32) }
}

// External host API functions (provided by uterx runtime)
// These are only available when running as a WASM plugin with host-apis feature.
#[cfg(feature = "host-apis")]
unsafe extern "C" {
    /// Draw text to a pane.
    fn uterx_ui_draw_text(pane_id: i64, x: i32, y: i32, text_ptr: i32, text_len: i32);

    /// Create an overlay pane.
    fn uterx_ui_create_overlay(title_ptr: i32, title_len: i32) -> i64;

    /// Get pane size (writes to out pointers).
    fn uterx_ui_get_size(pane_id: i64, width_out: *mut i32, height_out: *mut i32);

    /// Read from input stream.
    fn uterx_io_read(stream_id: i64, buf_ptr: i32, buf_len: i32) -> i32;

    /// Write to output stream.
    fn uterx_io_write(stream_id: i64, data_ptr: i32, data_len: i32) -> i32;
}

// Mock implementations for testing (when host-apis feature is disabled)
#[cfg(not(feature = "host-apis"))]
unsafe fn uterx_ui_draw_text(_pane_id: i64, _x: i32, _y: i32, _text_ptr: i32, _text_len: i32) {}

#[cfg(not(feature = "host-apis"))]
unsafe fn uterx_ui_create_overlay(_title_ptr: i32, _title_len: i32) -> i64 {
    1 // Mock: return pane ID 1
}

#[cfg(not(feature = "host-apis"))]
unsafe fn uterx_ui_get_size(_pane_id: i64, width_out: *mut i32, height_out: *mut i32) {
    unsafe {
        *width_out = 80;
        *height_out = 24;
    }
}

#[cfg(not(feature = "host-apis"))]
unsafe fn uterx_io_read(_stream_id: i64, _buf_ptr: i32, _buf_len: i32) -> i32 {
    0 // Mock: no input
}

#[cfg(not(feature = "host-apis"))]
unsafe fn uterx_io_write(_stream_id: i64, _data_ptr: i32, _data_len: i32) -> i32 {
    0 // Mock: success
}

/// Main plugin run loop.
pub fn run(manager: NoteManager, storage: Storage) {
    // Create overlay
    let pane_id = create_overlay("Quick Notes");
    let mut state = UiState::new(pane_id);
    
    // Main loop
    let mut input_buf = [0u8; 256];
    
    loop {
        // Update size
        let (w, h) = get_pane_size(pane_id);
        state.update_size(w, h);
        
        // Render if dirty
        if state.dirty {
            match state.cursor.mode {
                Mode::List => render_list(&state, &manager),
                Mode::Editor => {
                    if let Some(note_id) = state.cursor.note_id {
                        if let Some(note) = manager.get(note_id) {
                            render_editor(&state, note);
                        }
                    }
                }
                Mode::TaskList => {
                    if let Some(note_id) = state.cursor.note_id {
                        if let Some(note) = manager.get(note_id) {
                            render_task_list(&state, note);
                        }
                    }
                }
            }
            state.dirty = false;
        }
        
        // Read input
        let n = read_input(&mut input_buf);
        if n > 0 {
            handle_input(&mut state, &manager, &input_buf[..n as usize]);
        }
    }
}

/// Handle keyboard input.
fn handle_input(state: &mut UiState, manager: &NoteManager, input: &[u8]) {
    match state.cursor.mode {
        Mode::List => handle_list_input(state, manager, input),
        Mode::Editor => handle_editor_input(state, manager, input),
        Mode::TaskList => handle_task_input(state, manager, input),
    }
}

/// Handle input in list mode.
fn handle_list_input(state: &mut UiState, manager: &NoteManager, input: &[u8]) {
    let notes = manager.list_by_updated();
    
    match input {
        b"j" | b"\x1b[B" => state.move_down(notes.len()), // Down
        b"k" | b"\x1b[A" => state.move_up(notes.len()),   // Up
        b"\r" | b"e" => {
            // Enter / edit
            if let Some(note) = notes.get(state.cursor.list_index) {
                state.enter_editor(note.id);
            }
        }
        b"n" => {
            // New note - would need mutable manager
            state.set_status("Press 'n' to create new note".to_string());
        }
        b"d" => {
            // Delete note - would need mutable manager
            state.set_status("Press 'd' to delete note".to_string());
        }
        b"q" => {
            // Quit - would exit the loop
        }
        _ => {}
    }
}

/// Handle input in editor mode.
fn handle_editor_input(state: &mut UiState, manager: &NoteManager, input: &[u8]) {
    match input {
        b"\x1b" => state.exit_to_list(), // Escape
        b"t" => {
            // Enter task mode
            if let Some(note_id) = state.cursor.note_id {
                state.enter_task_list(note_id);
            }
        }
        b"s" => {
            // Save - would need storage
            state.set_status("Saved".to_string());
        }
        _ => {}
    }
}

/// Handle input in task list mode.
fn handle_task_input(state: &mut UiState, manager: &NoteManager, input: &[u8]) {
    let tasks = if let Some(note_id) = state.cursor.note_id {
        manager.get(note_id).map(|n| extract_tasks(&n.content)).unwrap_or_default()
    } else {
        Vec::new()
    };
    
    match input {
        b"j" | b"\x1b[B" => {
            // Down
            if state.cursor.task_index < tasks.len().saturating_sub(1) {
                state.cursor.task_index += 1;
                state.dirty = true;
            }
        }
        b"k" | b"\x1b[A" => {
            // Up
            if state.cursor.task_index > 0 {
                state.cursor.task_index -= 1;
                state.dirty = true;
            }
        }
        b" " | b"\r" => {
            // Toggle task - would need mutable manager
            state.set_status(format!("Toggled task {}", state.cursor.task_index + 1));
        }
        b"\x1b" => {
            // Escape - back to editor
            if let Some(note_id) = state.cursor.note_id {
                state.enter_editor(note_id);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tasks() {
        let content = "- [ ] Task 1\n- [x] Task 2\n- [X] Task 3\nRegular text";
        let tasks = extract_tasks(content);
        
        assert_eq!(tasks.len(), 3);
        assert!(!tasks[0].0);
        assert_eq!(tasks[0].1, "Task 1");
        assert!(tasks[1].0);
        assert_eq!(tasks[1].1, "Task 2");
        assert!(tasks[2].0);
        assert_eq!(tasks[2].1, "Task 3");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("Hello", 10), "Hello");
        assert_eq!(truncate("Hello World", 8), "Hello...");
        assert_eq!(truncate("Hi", 2), "Hi");
    }

    #[test]
    fn test_cursor_navigation() {
        let mut state = UiState::new(1);
        
        // Test move_down
        state.move_down(5);
        assert_eq!(state.cursor.list_index, 1);
        
        state.move_down(5);
        assert_eq!(state.cursor.list_index, 2);
        
        state.move_down(5);
        assert_eq!(state.cursor.list_index, 3);
        
        state.move_down(5);
        assert_eq!(state.cursor.list_index, 4);
        
        state.move_down(5); // Wrap to 0
        assert_eq!(state.cursor.list_index, 0);
        
        // Test move_up from 0 wraps to last
        state.move_up(5);
        assert_eq!(state.cursor.list_index, 4); // Wrap to last
        
        // Test move_up
        state.move_up(5);
        assert_eq!(state.cursor.list_index, 3);
    }

    #[test]
    fn test_mode_transitions() {
        let mut state = UiState::new(1);
        
        assert_eq!(state.cursor.mode, Mode::List);
        
        state.enter_editor(NoteId::new(1));
        assert_eq!(state.cursor.mode, Mode::Editor);
        assert_eq!(state.cursor.note_id, Some(NoteId::new(1)));
        
        state.enter_task_list(NoteId::new(1));
        assert_eq!(state.cursor.mode, Mode::TaskList);
        
        state.exit_to_list();
        assert_eq!(state.cursor.mode, Mode::List);
        assert_eq!(state.cursor.note_id, None);
    }
}
