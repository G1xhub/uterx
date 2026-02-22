//! Note data structures and management.
//!
//! Provides the core data model for notes with Markdown support.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Unique identifier for a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NoteId(pub u64);

impl NoteId {
    /// Create a new note ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Note format / content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteFormat {
    /// Plain text without formatting.
    Plain,
    /// Markdown with full formatting support.
    Markdown,
}

impl Default for NoteFormat {
    fn default() -> Self {
        Self::Markdown
    }
}

/// A single note with content and metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// Unique identifier.
    pub id: NoteId,
    /// Note title (displayed in list).
    pub title: String,
    /// Note content (Markdown or plain text).
    pub content: String,
    /// Content format.
    #[serde(default)]
    pub format: NoteFormat,
    /// Tags for categorization.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Last update timestamp (Unix epoch seconds).
    pub updated_at: u64,
}

impl Note {
    /// Create a new note with the given title.
    pub fn new(id: NoteId, title: String) -> Self {
        let now = current_timestamp();
        Self {
            id,
            title,
            content: String::new(),
            format: NoteFormat::Markdown,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the note content and timestamp.
    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.updated_at = current_timestamp();
    }

    /// Toggle a task checkbox in the note content.
    /// Returns true if a task was toggled.
    pub fn toggle_task(&mut self, task_index: usize) -> bool {
        let toggled = toggle_task_in_markdown(&mut self.content, task_index);
        if toggled {
            self.updated_at = current_timestamp();
        }
        toggled
    }
}

/// Toggle a task checkbox in markdown content.
/// Returns true if a task was found and toggled.
pub fn toggle_task_in_markdown(content: &mut String, task_index: usize) -> bool {
    let mut current_task = 0;
    let mut modified = false;
    
    // Process line by line
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::with_capacity(lines.len());
    
    for line in lines {
        let mut new_line = line.to_string();
        
        if line.starts_with("- [ ] ") {
            if current_task == task_index {
                new_line = format!("- [x] {}", &line[6..]);
                modified = true;
            }
            current_task += 1;
        } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
            if current_task == task_index {
                new_line = format!("- [ ] {}", &line[6..]);
                modified = true;
            }
            current_task += 1;
        }
        
        new_lines.push(new_line);
    }
    
    if modified {
        *content = new_lines.join("\n");
    }
    modified
}

/// Manager for all notes.
#[derive(Debug, Clone, Default)]
pub struct NoteManager {
    /// All notes indexed by ID.
    notes: BTreeMap<NoteId, Note>,
    /// Next available ID.
    next_id: u64,
}

impl NoteManager {
    /// Create a new empty note manager.
    pub fn new() -> Self {
        Self {
            notes: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Create a new note with an auto-generated ID.
    pub fn create(&mut self, title: String) -> NoteId {
        let id = NoteId::new(self.next_id);
        self.next_id += 1;
        let note = Note::new(id, title);
        self.notes.insert(id, note);
        id
    }

    /// Get a note by ID.
    pub fn get(&self, id: NoteId) -> Option<&Note> {
        self.notes.get(&id)
    }

    /// Get a mutable note by ID.
    pub fn get_mut(&mut self, id: NoteId) -> Option<&mut Note> {
        self.notes.get_mut(&id)
    }

    /// List all notes sorted by update time (newest first).
    pub fn list_by_updated(&self) -> Vec<&Note> {
        let mut notes: Vec<&Note> = self.notes.values().collect();
        notes.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        notes
    }

    /// List all notes sorted by title.
    pub fn list_by_title(&self) -> Vec<&Note> {
        let mut notes: Vec<&Note> = self.notes.values().collect();
        notes.sort_by(|a, b| a.title.cmp(&b.title));
        notes
    }

    /// Update a note's content.
    pub fn update(&mut self, id: NoteId, content: String) -> bool {
        if let Some(note) = self.notes.get_mut(&id) {
            note.update_content(content);
            true
        } else {
            false
        }
    }

    /// Toggle a task in a note.
    pub fn toggle_task(&mut self, id: NoteId, task_index: usize) -> bool {
        if let Some(note) = self.notes.get_mut(&id) {
            note.toggle_task(task_index)
        } else {
            false
        }
    }

    /// Delete a note by ID.
    pub fn delete(&mut self, id: NoteId) -> bool {
        self.notes.remove(&id).is_some()
    }

    /// Rename a note.
    pub fn rename(&mut self, id: NoteId, new_title: String) -> bool {
        if let Some(note) = self.notes.get_mut(&id) {
            note.title = new_title;
            note.updated_at = current_timestamp();
            true
        } else {
            false
        }
    }

    /// Add a tag to a note.
    pub fn add_tag(&mut self, id: NoteId, tag: String) -> bool {
        if let Some(note) = self.notes.get_mut(&id) {
            if !note.tags.contains(&tag) {
                note.tags.push(tag);
                note.updated_at = current_timestamp();
            }
            true
        } else {
            false
        }
    }

    /// Remove a tag from a note.
    pub fn remove_tag(&mut self, id: NoteId, tag: &str) -> bool {
        if let Some(note) = self.notes.get_mut(&id) {
            let len_before = note.tags.len();
            note.tags.retain(|t| t != tag);
            if note.tags.len() != len_before {
                note.updated_at = current_timestamp();
            }
            true
        } else {
            false
        }
    }

    /// Import a note (used when loading from storage).
    pub fn import(&mut self, note: Note) {
        if note.id.0 >= self.next_id {
            self.next_id = note.id.0 + 1;
        }
        self.notes.insert(note.id, note);
    }

    /// Export all notes (used when saving to storage).
    pub fn export_all(&self) -> Vec<&Note> {
        self.notes.values().collect()
    }

    /// Get the count of notes.
    pub fn count(&self) -> usize {
        self.notes.len()
    }
}

/// Get the current timestamp (Unix epoch seconds).
/// In a WASM plugin, this would use a host API.
/// For now, returns 0 as a placeholder.
fn current_timestamp() -> u64 {
    // TODO: Use host API for timestamp
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_creation() {
        let mut manager = NoteManager::new();
        let id = manager.create("Test Note".to_string());
        
        let note = manager.get(id).expect("note should exist");
        assert_eq!(note.title, "Test Note");
        assert_eq!(note.content, "");
        assert_eq!(note.format, NoteFormat::Markdown);
    }

    #[test]
    fn test_note_update() {
        let mut manager = NoteManager::new();
        let id = manager.create("Test".to_string());
        
        manager.update(id, "Hello, world!".to_string());
        
        let note = manager.get(id).expect("note should exist");
        assert_eq!(note.content, "Hello, world!");
    }

    #[test]
    fn test_toggle_task() {
        let mut content = "- [ ] Task 1\n- [x] Task 2\n- [ ] Task 3".to_string();
        
        // Toggle first task
        let toggled = toggle_task_in_markdown(&mut content, 0);
        assert!(toggled);
        assert!(content.contains("- [x] Task 1"));
        
        // Toggle second task (was checked, now unchecked)
        let toggled = toggle_task_in_markdown(&mut content, 1);
        assert!(toggled);
        assert!(content.contains("- [ ] Task 2"));
    }

    #[test]
    fn test_list_sorted() {
        let mut manager = NoteManager::new();
        let id1 = manager.create("B Note".to_string());
        let id2 = manager.create("A Note".to_string());
        
        let by_title = manager.list_by_title();
        assert_eq!(by_title[0].title, "A Note");
        assert_eq!(by_title[1].title, "B Note");
    }

    #[test]
    fn test_tags() {
        let mut manager = NoteManager::new();
        let id = manager.create("Tagged Note".to_string());
        
        manager.add_tag(id, "work".to_string());
        manager.add_tag(id, "important".to_string());
        
        let note = manager.get(id).expect("note should exist");
        assert_eq!(note.tags.len(), 2);
        assert!(note.tags.contains(&"work".to_string()));
        
        manager.remove_tag(id, "work");
        let note = manager.get(id).expect("note should exist");
        assert_eq!(note.tags.len(), 1);
    }
}
