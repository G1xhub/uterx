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

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    // Plugin entry point - will be implemented incrementally
    // For now, minimal entry point for WASM loading
}
