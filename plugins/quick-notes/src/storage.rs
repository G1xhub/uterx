//! File-based persistence for Quick Notes.
//!
//! Uses the uterx_fs host API to read and write notes to the plugin's sandboxed filesystem.

use crate::note::{Note, NoteId, NoteManager};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Storage error types.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("failed to serialize note: {0}")]
    Serialize(#[from] serde_json::Error),
    
    #[error("failed to read note file: {0}")]
    Read(String),
    
    #[error("failed to write note file: {0}")]
    Write(String),
    
    #[error("failed to parse note JSON: {0}")]
    Parse(String),
    
    #[error("host API error: {0}")]
    HostApi(String),
}

/// Note storage index for tracking all notes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteIndex {
    /// Map of note ID to filename.
    pub notes: BTreeMap<u64, String>,
    /// Next available ID.
    pub next_id: u64,
}

impl Default for NoteIndex {
    fn default() -> Self {
        Self {
            notes: BTreeMap::new(),
            next_id: 1,
        }
    }
}

impl NoteIndex {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a note to the index.
    pub fn add(&mut self, id: NoteId) -> String {
        let filename = format!("note-{}.json", id.0);
        self.notes.insert(id.0, filename.clone());
        if id.0 >= self.next_id {
            self.next_id = id.0 + 1;
        }
        filename
    }

    /// Remove a note from the index.
    pub fn remove(&mut self, id: NoteId) -> Option<String> {
        self.notes.remove(&id.0)
    }

    /// Get the filename for a note.
    pub fn get_filename(&self, id: NoteId) -> Option<&str> {
        self.notes.get(&id.0).map(String::as_str)
    }
}

/// Storage manager for notes.
pub struct Storage {
    /// Base path for note storage.
    base_path: String,
    /// Note index.
    index: NoteIndex,
}

impl Storage {
    /// Create a new storage manager.
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
            index: NoteIndex::new(),
        }
    }

    /// Get the path for a note file.
    fn note_path(&self, filename: &str) -> String {
        format!("{}/{}", self.base_path, filename)
    }

    /// Get the path for the index file.
    fn index_path(&self) -> String {
        format!("{}/index.json", self.base_path)
    }

    /// Save a note to storage.
    pub fn save_note(&mut self, note: &Note) -> Result<(), StorageError> {
        let filename = if let Some(existing) = self.index.get_filename(note.id) {
            existing.to_string()
        } else {
            self.index.add(note.id)
        };

        let path = self.note_path(&filename);
        let json = serde_json::to_string_pretty(note)?;
        let bytes = json.as_bytes();

        // Call host API to write file
        let result = unsafe {
            uterx_fs_write_file(
                path.as_ptr() as i32,
                path.len() as i32,
                bytes.as_ptr() as i32,
                bytes.len() as i32,
            )
        };

        if result < 0 {
            return Err(StorageError::Write(format!("host API returned {}", result)));
        }

        // Save index after successful note save
        self.save_index()?;

        Ok(())
    }

    /// Load a note from storage.
    pub fn load_note(&self, id: NoteId) -> Result<Option<Note>, StorageError> {
        let filename = match self.index.get_filename(id) {
            Some(f) => f,
            None => return Ok(None),
        };

        let path = self.note_path(filename);
        let mut buffer = vec![0u8; 65536]; // 64KB buffer

        let bytes_read = unsafe {
            uterx_fs_read_file(
                path.as_ptr() as i32,
                path.len() as i32,
                buffer.as_mut_ptr() as i32,
                buffer.len() as i32,
            )
        };

        if bytes_read < 0 {
            return Err(StorageError::Read(format!(
                "host API returned {}",
                bytes_read
            )));
        }

        if bytes_read == 0 {
            return Ok(None);
        }

        buffer.truncate(bytes_read as usize);

        let note: Note = serde_json::from_slice(&buffer)
            .map_err(|e| StorageError::Parse(e.to_string()))?;

        Ok(Some(note))
    }

    /// Delete a note from storage.
    pub fn delete_note(&mut self, id: NoteId) -> Result<bool, StorageError> {
        let _filename = match self.index.remove(id) {
            Some(f) => f,
            None => return Ok(false),
        };

        // Note: uterx_fs may not have a delete function yet
        // For now, we just remove from index and save
        // The actual file deletion would require a host API extension

        self.save_index()?;

        Ok(true)
    }

    /// Load all notes into a manager.
    pub fn load_all(&self) -> Result<NoteManager, StorageError> {
        let mut manager = NoteManager::new();

        for (&id, _filename) in &self.index.notes {
            if let Some(note) = self.load_note(NoteId::new(id))? {
                manager.import(note);
            }
        }

        Ok(manager)
    }

    /// Save the note index.
    pub fn save_index(&self) -> Result<(), StorageError> {
        let path = self.index_path();
        let json = serde_json::to_string_pretty(&self.index)?;
        let bytes = json.as_bytes();

        let result = unsafe {
            uterx_fs_write_file(
                path.as_ptr() as i32,
                path.len() as i32,
                bytes.as_ptr() as i32,
                bytes.len() as i32,
            )
        };

        if result < 0 {
            return Err(StorageError::Write(format!("host API returned {}", result)));
        }

        Ok(())
    }

    /// Load the note index.
    pub fn load_index(&mut self) -> Result<(), StorageError> {
        let path = self.index_path();
        let mut buffer = vec![0u8; 16384]; // 16KB buffer for index

        let bytes_read = unsafe {
            uterx_fs_read_file(
                path.as_ptr() as i32,
                path.len() as i32,
                buffer.as_mut_ptr() as i32,
                buffer.len() as i32,
            )
        };

        if bytes_read <= 0 {
            // No index file yet, start fresh
            self.index = NoteIndex::new();
            return Ok(());
        }

        buffer.truncate(bytes_read as usize);

        self.index = serde_json::from_slice(&buffer)
            .map_err(|e| StorageError::Parse(e.to_string()))?;

        Ok(())
    }

    /// Initialize storage (create base directory if needed).
    pub fn init(&mut self) -> Result<(), StorageError> {
        // Load existing index
        self.load_index()?;

        // Note: Directory creation would be handled by uterx_fs
        // when we write the first file

        Ok(())
    }

    /// Get the current index.
    pub fn index(&self) -> &NoteIndex {
        &self.index
    }
}

// External host API functions (provided by uterx runtime)
// These are only available when running as a WASM plugin with host-apis feature.
#[cfg(feature = "host-apis")]
unsafe extern "C" {
    /// Read a file from the sandboxed filesystem.
    /// Returns the number of bytes read, or negative on error.
    fn uterx_fs_read_file(
        path_ptr: i32,
        path_len: i32,
        buf_ptr: i32,
        buf_len: i32,
    ) -> i32;

    /// Write a file to the sandboxed filesystem.
    /// Returns 0 on success, negative on error.
    fn uterx_fs_write_file(
        path_ptr: i32,
        path_len: i32,
        data_ptr: i32,
        data_len: i32,
    ) -> i32;
}

// Mock implementations for testing (when host-apis feature is disabled)
#[cfg(not(feature = "host-apis"))]
unsafe fn uterx_fs_read_file(_path_ptr: i32, _path_len: i32, _buf_ptr: i32, _buf_len: i32) -> i32 {
    -1 // Mock: file not found
}

#[cfg(not(feature = "host-apis"))]
unsafe fn uterx_fs_write_file(_path_ptr: i32, _path_len: i32, _data_ptr: i32, _data_len: i32) -> i32 {
    0 // Mock: success
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_index() {
        let mut index = NoteIndex::new();
        
        let filename = index.add(NoteId::new(1));
        assert_eq!(filename, "note-1.json");
        assert_eq!(index.next_id, 2);
        
        let filename = index.add(NoteId::new(5));
        assert_eq!(filename, "note-5.json");
        assert_eq!(index.next_id, 6);
        
        let removed = index.remove(NoteId::new(1));
        assert_eq!(removed, Some("note-1.json".to_string()));
        assert!(index.get_filename(NoteId::new(1)).is_none());
    }

    #[test]
    fn test_note_index_serialization() {
        let mut index = NoteIndex::new();
        index.add(NoteId::new(1));
        index.add(NoteId::new(2));
        
        let json = serde_json::to_string_pretty(&index).unwrap();
        assert!(json.contains("\"next_id\": 3"));
        
        let parsed: NoteIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.notes.len(), 2);
        assert_eq!(parsed.next_id, 3);
    }

    #[test]
    fn test_storage_paths() {
        let storage = Storage::new("notes");
        
        assert_eq!(storage.index_path(), "notes/index.json");
        assert_eq!(storage.note_path("note-1.json"), "notes/note-1.json");
    }
}
