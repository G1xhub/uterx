//! Quick Notes plugin for uterx.
//!
//! Provides floating note panes with Markdown support that persist across sessions.
//!
//! # Features
//! - Create, edit, and delete notes
//! - Markdown rendering with syntax highlighting
//! - Task lists with interactive checkboxes
//! - Persistent storage via uterx filesystem API

pub mod markdown;
pub mod note;
pub mod storage;
pub mod ui;

use note::{Note, NoteId, NoteManager};
use std::sync::{Mutex, OnceLock};
use storage::Storage;

#[link(wasm_import_module = "uterx_io")]
unsafe extern "C" {
    fn read(stream_id: i64, buf_ptr: i32, buf_len: i32) -> i32;
    fn write(stream_id: i64, data_ptr: i32, data_len: i32) -> i32;
}

const INFO_STREAM_ID: i64 = 1;
const STORAGE_BASE_PATH: &str = "quick-notes";

struct RuntimeState {
    storage: Storage,
    manager: NoteManager,
}

impl RuntimeState {
    fn initialize() -> Self {
        let mut storage = Storage::new(STORAGE_BASE_PATH);
        if let Err(err) = storage.init() {
            write_line(&format!("quick-notes: storage init failed: {err}"));
        }

        let manager = match storage.load_all() {
            Ok(loaded) => loaded,
            Err(err) => {
                write_line(&format!("quick-notes: load failed: {err}"));
                NoteManager::new()
            }
        };

        Self { storage, manager }
    }
}

fn runtime_state() -> &'static Mutex<RuntimeState> {
    static STATE: OnceLock<Mutex<RuntimeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(RuntimeState::initialize()))
}

fn write_line(message: &str) {
    let mut bytes = message.as_bytes().to_vec();
    bytes.push(b'\n');
    unsafe {
        let _ = write(
            INFO_STREAM_ID,
            bytes.as_ptr() as i32,
            bytes.len() as i32,
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    write_line("quick-notes: runtime ready");
    write_line("quick-notes: generic launcher integration active");
    write_line("quick-notes: type 'help' and press Enter");
    let _ = runtime_state();
}

#[unsafe(no_mangle)]
pub extern "C" fn quick_notes_poll() -> i32 {
    let mut input_buf = [0_u8; 4096];
    let read_len = unsafe {
        read(
            INFO_STREAM_ID,
            input_buf.as_mut_ptr() as i32,
            input_buf.len() as i32,
        )
    };

    if read_len <= 0 {
        return read_len;
    }

    let input = String::from_utf8_lossy(&input_buf[..read_len as usize])
        .trim()
        .to_string();
    if input.is_empty() {
        return 0;
    }

    let mut state = match runtime_state().lock() {
        Ok(guard) => guard,
        Err(_) => {
            write_line("quick-notes: internal state lock poisoned");
            return -1;
        }
    };

    match input.as_str() {
        "help" => {
            write_line("quick-notes commands:");
            write_line("  help  - show commands");
            write_line("  ping  - health check");
            write_line("  status - runtime status");
            write_line("  note <text> - create and persist note");
            write_line("  list - list saved notes");
            write_line("  show <id> - show note details");
            write_line("  delete <id> - delete note");
        }
        "ping" => {
            write_line("quick-notes: pong");
        }
        "status" => {
            write_line(&format!("quick-notes: poll loop active, notes={}", state.manager.count()));
        }
        "list" => {
            let notes = state.manager.list_by_updated();
            if notes.is_empty() {
                write_line("quick-notes: no notes saved");
            } else {
                write_line("quick-notes: saved notes");
                for note in notes.iter().take(20) {
                    write_line(&format!("  #{}  {}", note.id.0, note.title));
                }
            }
        }
        _ if input.starts_with("show ") => {
            let id_text = input[5..].trim();
            match id_text.parse::<u64>() {
                Ok(id_value) => {
                    let note_id = NoteId::new(id_value);
                    if let Some(note) = state.manager.get(note_id) {
                        write_line(&format!("quick-notes: note #{}", note.id.0));
                        write_line(&format!("title: {}", note.title));
                        if note.content.is_empty() {
                            write_line("content: [empty]");
                        } else {
                            write_line("content:");
                            for line in note.content.lines().take(20) {
                                write_line(&format!("  {}", line));
                            }
                        }
                    } else {
                        write_line(&format!("quick-notes: note #{} not found", id_value));
                    }
                }
                Err(_) => {
                    write_line("quick-notes: invalid id for show (use: show <id>)");
                }
            }
        }
        _ if input.starts_with("delete ") => {
            let id_text = input[7..].trim();
            match id_text.parse::<u64>() {
                Ok(id_value) => {
                    let note_id = NoteId::new(id_value);
                    if !state.manager.delete(note_id) {
                        write_line(&format!("quick-notes: note #{} not found", id_value));
                    } else {
                        match state.storage.delete_note(note_id) {
                            Ok(true) => write_line(&format!("quick-notes: deleted note #{}", id_value)),
                            Ok(false) => write_line(&format!("quick-notes: note #{} removed in memory, index entry missing", id_value)),
                            Err(err) => write_line(&format!("quick-notes: delete failed: {err}")),
                        }
                    }
                }
                Err(_) => {
                    write_line("quick-notes: invalid id for delete (use: delete <id>)");
                }
            }
        }
        _ if input.starts_with("note ") => {
            let body = input[5..].trim();
            if body.is_empty() {
                write_line("quick-notes: note text is empty");
            } else {
                let title = body
                    .chars()
                    .take(32)
                    .collect::<String>();
                let note_id = state.manager.create(title);
                if let Some(note) = state.manager.get_mut(note_id) {
                    note.update_content(body.to_string());
                    let note_to_save: Note = note.clone();
                    if let Err(err) = state.storage.save_note(&note_to_save) {
                        write_line(&format!("quick-notes: save failed: {err}"));
                    } else {
                        write_line(&format!("quick-notes: saved note #{}", note_to_save.id.0));
                    }
                } else {
                    write_line("quick-notes: failed to access created note");
                }
            }
        }
        _ => {
            write_line("quick-notes: unknown command (try: help)");
        }
    }

    1
}
