//! Host API definitions exposed to WASM plugins.
//!
//! These are the function signatures that plugins can import
//! from the `uterx` module.

/// API categories exposed to plugins.
pub mod categories {
    /// UI API — draw to pane, create overlays, get terminal size.
    pub const UI: &str = "uterx_ui";
    /// IO API — read/write streams, interact with PTY.
    pub const IO: &str = "uterx_io";
    /// Network API — HTTP requests, socket operations.
    pub const NET: &str = "uterx_net";
    /// Filesystem API — sandboxed read/write.
    pub const FS: &str = "uterx_fs";
    /// Platform API — bluetooth, system info, keyring.
    pub const PLATFORM: &str = "uterx_platform";
}

// The actual host function implementations will be registered
// in runtime.rs via the wasmtime Linker. This module documents
// the API contract.

// --- UI API ---
// uterx_ui::draw_text(pane_id: u64, x: u32, y: u32, text_ptr: i32, text_len: i32)
// uterx_ui::get_size(pane_id: u64) -> (width: u32, height: u32)
// uterx_ui::create_overlay(title_ptr: i32, title_len: i32) -> overlay_id: u64

// --- IO API ---
// uterx_io::write(stream_id: u64, data_ptr: i32, data_len: i32) -> bytes_written: i32
// uterx_io::read(stream_id: u64, buf_ptr: i32, buf_len: i32) -> bytes_read: i32

// --- Network API ---
// uterx_net::http_get(url_ptr: i32, url_len: i32, response_ptr: i32, response_len: i32) -> status: i32
// uterx_net::tcp_connect(host_ptr: i32, host_len: i32, port: u16) -> socket_id: u64

// --- Filesystem API ---
// uterx_fs::read_file(path_ptr: i32, path_len: i32, buf_ptr: i32, buf_len: i32) -> bytes_read: i32
// uterx_fs::write_file(path_ptr: i32, path_len: i32, data_ptr: i32, data_len: i32) -> result: i32

// --- Platform API ---
// uterx_platform::bt_scan() -> device_count: i32
// uterx_platform::bt_connect(device_id: u64) -> result: i32
// uterx_platform::keyring_get(key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32) -> result: i32
