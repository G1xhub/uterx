//! WASM plugin runtime using wasmtime.

use crate::manifest::{Permission, PluginManifest};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use wasmtime::*;

/// A running plugin instance.
pub struct PluginInstance {
    store: Store<PluginState>,
    instance: Instance,
}

#[derive(Debug, Clone)]
pub struct UiDrawCall {
    pub pane_id: u64,
    pub x: u32,
    pub y: u32,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct UiOverlay {
    pub id: u64,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct IoWriteCall {
    pub stream_id: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct NetHttpCall {
    pub method: String,
    pub url: String,
    pub status: i32,
    pub request_bytes: usize,
    pub bytes_written: usize,
}

#[derive(Debug, Clone)]
pub struct PlatformBtDevice {
    pub address: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionPromptResponse {
    Deny,
    GrantOnce,
    GrantAlways,
}

#[derive(Debug, Clone)]
pub struct PermissionPromptEvent {
    pub permission: Permission,
    pub api: String,
}

/// Per-plugin state accessible from host functions.
struct PluginState {
    manifest: PluginManifest,
    granted_permissions: Vec<Permission>,
    fs_root: PathBuf,
    pane_sizes: HashMap<u64, (u32, u32)>,
    ui_draw_calls: Vec<UiDrawCall>,
    overlays: Vec<UiOverlay>,
    next_overlay_id: u64,
    stream_inputs: HashMap<u64, Vec<u8>>,
    stream_outputs: HashMap<u64, Vec<u8>>,
    io_write_calls: Vec<IoWriteCall>,
    tcp_sockets: HashMap<u64, TcpStream>,
    next_socket_id: u64,
    net_http_calls: Vec<NetHttpCall>,
    platform_bt_devices: Vec<PlatformBtDevice>,
    platform_keyring: HashMap<String, String>,
    denied_permissions: HashSet<Permission>,
    queued_prompt_responses: HashMap<Permission, PermissionPromptResponse>,
    permission_prompt_events: Vec<PermissionPromptEvent>,
}

impl PluginInstance {
    /// Load and instantiate a WASM plugin.
    pub fn load(
        wasm_path: &Path,
        manifest: PluginManifest,
        granted_permissions: Vec<Permission>,
    ) -> anyhow::Result<Self> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_path)?;

        let state = PluginState {
            manifest,
            granted_permissions,
            fs_root: default_plugin_fs_root(wasm_path),
            pane_sizes: HashMap::new(),
            ui_draw_calls: Vec::new(),
            overlays: Vec::new(),
            next_overlay_id: 1,
            stream_inputs: HashMap::new(),
            stream_outputs: HashMap::new(),
            io_write_calls: Vec::new(),
            tcp_sockets: HashMap::new(),
            next_socket_id: 1,
            net_http_calls: Vec::new(),
            platform_bt_devices: Vec::new(),
            platform_keyring: HashMap::new(),
            denied_permissions: HashSet::new(),
            queued_prompt_responses: HashMap::new(),
            permission_prompt_events: Vec::new(),
        };
        let mut store = Store::new(&engine, state);
        let mut linker = Linker::new(&engine);

        // Register host API functions
        // TODO: register uterx::ui, uterx::io, uterx::net, etc.
        register_host_api(&mut linker)?;

        let instance = linker.instantiate(&mut store, &module)?;

        Ok(Self { store, instance })
    }

    /// Call the plugin's entry point.
    pub fn start(&mut self) -> anyhow::Result<()> {
        let entry = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "_start")?;
        entry.call(&mut self.store, ())?;
        Ok(())
    }

    pub fn call_i32_func(&mut self, name: &str) -> anyhow::Result<i32> {
        let func = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, name)?;
        let out = func.call(&mut self.store, ())?;
        Ok(out)
    }

    pub fn set_pane_size(&mut self, pane_id: u64, width: u32, height: u32) {
        self.store
            .data_mut()
            .pane_sizes
            .insert(pane_id, (width, height));
    }

    pub fn ui_draw_calls(&self) -> &[UiDrawCall] {
        &self.store.data().ui_draw_calls
    }

    pub fn overlays(&self) -> &[UiOverlay] {
        &self.store.data().overlays
    }

    pub fn set_stream_input(&mut self, stream_id: u64, data: Vec<u8>) {
        self.store.data_mut().stream_inputs.insert(stream_id, data);
    }

    pub fn append_stream_input(&mut self, stream_id: u64, data: &[u8]) {
        self.store
            .data_mut()
            .stream_inputs
            .entry(stream_id)
            .or_default()
            .extend_from_slice(data);
    }

    pub fn push_ai_stream_chunk(&mut self, stream_id: u64, chunk: &[u8]) {
        self.append_stream_input(stream_id, chunk);
    }

    pub fn stream_output(&self, stream_id: u64) -> Option<&[u8]> {
        self.store
            .data()
            .stream_outputs
            .get(&stream_id)
            .map(Vec::as_slice)
    }

    pub fn take_stream_output(&mut self, stream_id: u64) -> Vec<u8> {
        self.store
            .data_mut()
            .stream_outputs
            .remove(&stream_id)
            .unwrap_or_default()
    }

    pub fn io_write_calls(&self) -> &[IoWriteCall] {
        &self.store.data().io_write_calls
    }

    pub fn set_fs_root(&mut self, root: PathBuf) -> anyhow::Result<()> {
        fs::create_dir_all(&root)?;
        self.store.data_mut().fs_root = root.canonicalize()?;
        Ok(())
    }

    pub fn fs_root(&self) -> &Path {
        &self.store.data().fs_root
    }

    pub fn connected_socket_count(&self) -> usize {
        self.store.data().tcp_sockets.len()
    }

    pub fn net_http_calls(&self) -> &[NetHttpCall] {
        &self.store.data().net_http_calls
    }

    pub fn set_mock_bt_devices(&mut self, devices: Vec<PlatformBtDevice>) {
        self.store.data_mut().platform_bt_devices = devices;
    }

    pub fn mock_bt_devices(&self) -> &[PlatformBtDevice] {
        &self.store.data().platform_bt_devices
    }

    pub fn set_keyring_entry(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.store
            .data_mut()
            .platform_keyring
            .insert(key.into(), value.into());
    }

    pub fn keyring_value(&self, key: &str) -> Option<&str> {
        self.store
            .data()
            .platform_keyring
            .get(key)
            .map(String::as_str)
    }

    pub fn set_permission_response(
        &mut self,
        permission: Permission,
        response: PermissionPromptResponse,
    ) {
        let state = self.store.data_mut();
        state.queued_prompt_responses.insert(permission.clone(), response);
        if !matches!(response, PermissionPromptResponse::Deny) {
            state.denied_permissions.remove(&permission);
        }
    }

    pub fn permission_prompt_events(&self) -> &[PermissionPromptEvent] {
        &self.store.data().permission_prompt_events
    }

    pub fn clear_permission_prompt_events(&mut self) {
        self.store.data_mut().permission_prompt_events.clear();
    }
}

/// Register the host API functions that plugins can call.
fn register_host_api(linker: &mut Linker<PluginState>) -> anyhow::Result<()> {
    // Example: uterx_log(ptr, len) — let plugins log messages
    linker.func_wrap(
        "uterx",
        "log",
        |mut caller: Caller<'_, PluginState>, ptr: i32, len: i32| {
            if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                let data = memory.data(&caller);
                if let Some(slice) = data.get(ptr as usize..(ptr + len) as usize) {
                    if let Ok(msg) = std::str::from_utf8(slice) {
                        tracing::info!(plugin = %caller.data().manifest.name, "{}", msg);
                    }
                }
            }
        },
    )?;

    linker.func_wrap(
        "uterx_ui",
        "draw_text",
        |mut caller: Caller<'_, PluginState>,
         pane_id: i64,
         x: i32,
         y: i32,
         text_ptr: i32,
         text_len: i32| {
            if !resolve_permission_decision(caller.data_mut(), Permission::Ui, "uterx_ui.draw_text") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_ui.draw_text (missing ui permission)"
                );
                return;
            }

            let Some(text) = read_guest_string(&mut caller, text_ptr, text_len) else {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "failed to read text for uterx_ui.draw_text"
                );
                return;
            };

            caller.data_mut().ui_draw_calls.push(UiDrawCall {
                pane_id: pane_id.max(0) as u64,
                x: x.max(0) as u32,
                y: y.max(0) as u32,
                text,
            });
        },
    )?;

    linker.func_wrap(
        "uterx_ui",
        "get_size",
        |mut caller: Caller<'_, PluginState>, pane_id: i64| -> (i32, i32) {
            if !resolve_permission_decision(caller.data_mut(), Permission::Ui, "uterx_ui.get_size") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_ui.get_size (missing ui permission)"
                );
                return (0, 0);
            }

            let key = pane_id.max(0) as u64;
            let (w, h) = caller
                .data()
                .pane_sizes
                .get(&key)
                .copied()
                .unwrap_or((80, 24));
            (w as i32, h as i32)
        },
    )?;

    linker.func_wrap(
        "uterx_ui",
        "create_overlay",
        |mut caller: Caller<'_, PluginState>, title_ptr: i32, title_len: i32| -> i64 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Ui, "uterx_ui.create_overlay") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_ui.create_overlay (missing ui permission)"
                );
                return 0;
            }

            let Some(title) = read_guest_string(&mut caller, title_ptr, title_len) else {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "failed to read title for uterx_ui.create_overlay"
                );
                return 0;
            };

            let state = caller.data_mut();
            let id = state.next_overlay_id;
            state.next_overlay_id = state.next_overlay_id.saturating_add(1);
            state.overlays.push(UiOverlay { id, title });
            id as i64
        },
    )?;

    linker.func_wrap(
        "uterx_io",
        "write",
        |mut caller: Caller<'_, PluginState>, stream_id: i64, data_ptr: i32, data_len: i32| -> i32 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Io, "uterx_io.write") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_io.write (missing io permission)"
                );
                return -1;
            }

            let Some(bytes) = read_guest_bytes(&mut caller, data_ptr, data_len) else {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "failed to read buffer for uterx_io.write"
                );
                return -1;
            };

            let key = stream_id.max(0) as u64;
            let state = caller.data_mut();
            push_stream_output_chunk(state, key, &bytes);
            state.io_write_calls.push(IoWriteCall {
                stream_id: key,
                bytes: bytes.clone(),
            });
            bytes.len() as i32
        },
    )?;

    linker.func_wrap(
        "uterx_io",
        "read",
        |mut caller: Caller<'_, PluginState>, stream_id: i64, buf_ptr: i32, buf_len: i32| -> i32 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Io, "uterx_io.read") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_io.read (missing io permission)"
                );
                return -1;
            }

            if buf_len <= 0 {
                return 0;
            }

            let key = stream_id.max(0) as u64;
            let bytes = {
                let state = caller.data_mut();
                pop_stream_input_chunk(state, key, buf_len as usize)
            };

            if bytes.is_empty() {
                return 0;
            }
            match write_guest_bytes(&mut caller, buf_ptr, &bytes) {
                Some(()) => bytes.len() as i32,
                None => {
                    tracing::warn!(
                        plugin = %caller.data().manifest.name,
                        "failed to write buffer for uterx_io.read"
                    );
                    -1
                }
            }
        },
    )?;

    linker.func_wrap(
        "uterx_net",
        "http_get",
        |mut caller: Caller<'_, PluginState>,
         url_ptr: i32,
         url_len: i32,
         response_ptr: i32,
         response_len: i32|
         -> i32 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Network, "uterx_net.http_get") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_net.http_get (missing network permission)"
                );
                return -1;
            }

            let Some(url) = read_guest_string(&mut caller, url_ptr, url_len) else {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "failed to read url for uterx_net.http_get"
                );
                return -1;
            };

            let response = match reqwest::blocking::get(&url) {
                Ok(resp) => resp,
                Err(err) => {
                    tracing::warn!(plugin = %caller.data().manifest.name, error = %err, "http_get request failed");
                    return -2;
                }
            };

            let status = i32::from(response.status().as_u16());
            let body = match response.bytes() {
                Ok(bytes) => bytes,
                Err(err) => {
                    tracing::warn!(plugin = %caller.data().manifest.name, error = %err, "http_get body read failed");
                    return -3;
                }
            };

            let cap = response_len.max(0) as usize;
            let write_len = body.len().min(cap);
            if write_len > 0 {
                if write_guest_bytes(&mut caller, response_ptr, &body[..write_len]).is_none() {
                    tracing::warn!(
                        plugin = %caller.data().manifest.name,
                        "failed to write response buffer for uterx_net.http_get"
                    );
                    return -4;
                }
            }

            caller.data_mut().net_http_calls.push(NetHttpCall {
                method: "GET".to_string(),
                url,
                status,
                request_bytes: 0,
                bytes_written: write_len,
            });
            status
        },
    )?;

    linker.func_wrap(
        "uterx_net",
        "http_post",
        |mut caller: Caller<'_, PluginState>,
         url_ptr: i32,
         url_len: i32,
         headers_ptr: i32,
         headers_len: i32,
         body_ptr: i32,
         body_len: i32,
         response_ptr: i32,
         response_len: i32|
         -> i32 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Network, "uterx_net.http_post") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_net.http_post (missing network permission)"
                );
                return -1;
            }

            let Some(url) = read_guest_string(&mut caller, url_ptr, url_len) else {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "failed to read url for uterx_net.http_post"
                );
                return -1;
            };
            let Some(headers_raw) = read_guest_string(&mut caller, headers_ptr, headers_len) else {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "failed to read headers for uterx_net.http_post"
                );
                return -1;
            };
            let Some(body) = read_guest_bytes(&mut caller, body_ptr, body_len) else {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "failed to read body for uterx_net.http_post"
                );
                return -1;
            };

            let headers = parse_http_headers(&headers_raw);
            let (status, response_body) = match http_post_request(&url, &headers, &body) {
                Ok(result) => result,
                Err(err) => {
                    tracing::warn!(plugin = %caller.data().manifest.name, error = %err, "http_post request failed");
                    return -2;
                }
            };

            let cap = response_len.max(0) as usize;
            let write_len = response_body.len().min(cap);
            if write_len > 0 {
                if write_guest_bytes(&mut caller, response_ptr, &response_body[..write_len]).is_none() {
                    tracing::warn!(
                        plugin = %caller.data().manifest.name,
                        "failed to write response buffer for uterx_net.http_post"
                    );
                    return -4;
                }
            }

            caller.data_mut().net_http_calls.push(NetHttpCall {
                method: "POST".to_string(),
                url,
                status,
                request_bytes: body.len(),
                bytes_written: write_len,
            });

            status
        },
    )?;

    linker.func_wrap(
        "uterx_net",
        "tcp_connect",
        |mut caller: Caller<'_, PluginState>, host_ptr: i32, host_len: i32, port: i32| -> i64 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Network, "uterx_net.tcp_connect") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_net.tcp_connect (missing network permission)"
                );
                return 0;
            }

            let Some(host) = read_guest_string(&mut caller, host_ptr, host_len) else {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "failed to read host for uterx_net.tcp_connect"
                );
                return 0;
            };

            if !(1..=65535).contains(&port) {
                return 0;
            }

            let socket = match tcp_connect_with_timeout(&host, port as u16, Duration::from_millis(1500)) {
                Ok(sock) => sock,
                Err(err) => {
                    tracing::warn!(plugin = %caller.data().manifest.name, error = %err, "tcp_connect failed");
                    return 0;
                }
            };

            let state = caller.data_mut();
            let id = state.next_socket_id;
            state.next_socket_id = state.next_socket_id.saturating_add(1);
            state.tcp_sockets.insert(id, socket);
            id as i64
        },
    )?;

    linker.func_wrap(
        "uterx_fs",
        "read_file",
        |mut caller: Caller<'_, PluginState>,
         path_ptr: i32,
         path_len: i32,
         buf_ptr: i32,
         buf_len: i32|
         -> i32 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Filesystem, "uterx_fs.read_file") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_fs.read_file (missing filesystem permission)"
                );
                return -1;
            }

            let Some(path_text) = read_guest_string(&mut caller, path_ptr, path_len) else {
                return -1;
            };
            if buf_len <= 0 {
                return 0;
            }

            let resolved = {
                let state = caller.data();
                resolve_sandbox_path(&state.fs_root, &path_text)
            };
            let Ok(path) = resolved else {
                return -1;
            };

            let Ok(content) = fs::read(&path) else {
                return -1;
            };
            let write_len = content.len().min(buf_len as usize);
            if write_len == 0 {
                return 0;
            }
            if write_guest_bytes(&mut caller, buf_ptr, &content[..write_len]).is_none() {
                return -1;
            }
            write_len as i32
        },
    )?;

    linker.func_wrap(
        "uterx_fs",
        "write_file",
        |mut caller: Caller<'_, PluginState>,
         path_ptr: i32,
         path_len: i32,
         data_ptr: i32,
         data_len: i32|
         -> i32 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Filesystem, "uterx_fs.write_file") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_fs.write_file (missing filesystem permission)"
                );
                return -1;
            }

            let Some(path_text) = read_guest_string(&mut caller, path_ptr, path_len) else {
                return -1;
            };
            let Some(bytes) = read_guest_bytes(&mut caller, data_ptr, data_len) else {
                return -1;
            };

            let resolved = {
                let state = caller.data();
                resolve_sandbox_path(&state.fs_root, &path_text)
            };
            let Ok(path) = resolved else {
                return -1;
            };

            if let Some(parent) = path.parent() {
                if fs::create_dir_all(parent).is_err() {
                    return -1;
                }
            }
            if fs::write(&path, bytes).is_err() {
                return -1;
            }
            0
        },
    )?;

    linker.func_wrap(
        "uterx_platform",
        "bt_scan",
        |mut caller: Caller<'_, PluginState>| -> i32 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Platform, "uterx_platform.bt_scan") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_platform.bt_scan (missing platform permission)"
                );
                return -1;
            }

            caller.data().platform_bt_devices.len() as i32
        },
    )?;

    linker.func_wrap(
        "uterx_platform",
        "keyring_get",
        |mut caller: Caller<'_, PluginState>, key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32| -> i32 {
            if !resolve_permission_decision(caller.data_mut(), Permission::Platform, "uterx_platform.keyring_get") {
                tracing::warn!(
                    plugin = %caller.data().manifest.name,
                    "blocked uterx_platform.keyring_get (missing platform permission)"
                );
                return -1;
            }

            let Some(key) = read_guest_string(&mut caller, key_ptr, key_len) else {
                return -1;
            };

            let Some(value) = caller.data().platform_keyring.get(&key).cloned() else {
                return 0;
            };

            let bytes = value.as_bytes();
            let cap = val_len.max(0) as usize;
            let write_len = bytes.len().min(cap);
            if write_len == 0 {
                return 0;
            }
            if write_guest_bytes(&mut caller, val_ptr, &bytes[..write_len]).is_none() {
                return -1;
            }
            write_len as i32
        },
    )?;

    // TODO: register remaining host APIs (permission prompts, auto-update, registry).

    Ok(())
}

fn resolve_permission_decision(state: &mut PluginState, permission: Permission, api: &str) -> bool {
    if state.granted_permissions.contains(&permission) {
        return true;
    }

    if state.denied_permissions.contains(&permission) {
        return false;
    }

    state.permission_prompt_events.push(PermissionPromptEvent {
        permission: permission.clone(),
        api: api.to_string(),
    });

    let response = state
        .queued_prompt_responses
        .remove(&permission)
        .unwrap_or(PermissionPromptResponse::Deny);

    match response {
        PermissionPromptResponse::Deny => {
            state.denied_permissions.insert(permission);
            false
        }
        PermissionPromptResponse::GrantOnce => true,
        PermissionPromptResponse::GrantAlways => {
            state.granted_permissions.push(permission.clone());
            state.denied_permissions.remove(&permission);
            true
        }
    }
}

fn read_guest_string(
    caller: &mut Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
) -> Option<String> {
    if ptr < 0 || len < 0 {
        return None;
    }
    let start = ptr as usize;
    let end = start.checked_add(len as usize)?;
    let memory = caller.get_export("memory")?.into_memory()?;
    let data = memory.data(caller);
    let bytes = data.get(start..end)?;
    std::str::from_utf8(bytes).ok().map(ToOwned::to_owned)
}

fn read_guest_bytes(
    caller: &mut Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
) -> Option<Vec<u8>> {
    if ptr < 0 || len < 0 {
        return None;
    }
    let start = ptr as usize;
    let end = start.checked_add(len as usize)?;
    let memory = caller.get_export("memory")?.into_memory()?;
    let data = memory.data(caller);
    let bytes = data.get(start..end)?;
    Some(bytes.to_vec())
}

fn write_guest_bytes(
    caller: &mut Caller<'_, PluginState>,
    ptr: i32,
    bytes: &[u8],
) -> Option<()> {
    if ptr < 0 {
        return None;
    }
    let start = ptr as usize;
    let end = start.checked_add(bytes.len())?;
    let memory = caller.get_export("memory")?.into_memory()?;
    let data = memory.data_mut(caller);
    let dst = data.get_mut(start..end)?;
    dst.copy_from_slice(bytes);
    Some(())
}

fn push_stream_output_chunk(state: &mut PluginState, stream_id: u64, bytes: &[u8]) {
    state
        .stream_outputs
        .entry(stream_id)
        .or_default()
        .extend_from_slice(bytes);
}

fn pop_stream_input_chunk(state: &mut PluginState, stream_id: u64, max_len: usize) -> Vec<u8> {
    let Some(buf) = state.stream_inputs.get_mut(&stream_id) else {
        return Vec::new();
    };
    if buf.is_empty() || max_len == 0 {
        return Vec::new();
    }
    let take = buf.len().min(max_len);
    let out = buf[..take].to_vec();
    buf.drain(..take);
    out
}

fn tcp_connect_with_timeout(host: &str, port: u16, timeout: Duration) -> std::io::Result<TcpStream> {
    let addrs = (host, port).to_socket_addrs()?;
    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => return Ok(stream),
            Err(err) => last_err = Some(err),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("no socket addresses resolved")))
}

fn parse_http_headers(raw: &str) -> Vec<(String, String)> {
    raw.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            let (name, value) = trimmed.split_once(':')?;
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

fn http_post_request(
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> anyhow::Result<(i32, Vec<u8>)> {
    let client = reqwest::blocking::Client::new();
    let mut request = client.post(url).body(body.to_vec());
    for (key, value) in headers {
        request = request.header(key, value);
    }

    let response = request.send()?;
    let status = i32::from(response.status().as_u16());
    let bytes = response.bytes()?.to_vec();
    Ok((status, bytes))
}

fn default_plugin_fs_root(wasm_path: &Path) -> PathBuf {
    let base = wasm_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("plugin_fs");
    if fs::create_dir_all(&base).is_err() {
        return std::env::temp_dir().join("uterx-plugin-fs");
    }
    base.canonicalize().unwrap_or(base)
}

fn resolve_sandbox_path(fs_root: &Path, requested: &str) -> anyhow::Result<PathBuf> {
    let requested_path = Path::new(requested);
    if requested_path.is_absolute() {
        anyhow::bail!("absolute paths are not allowed")
    }

    for comp in requested_path.components() {
        if matches!(comp, Component::ParentDir | Component::Prefix(_)) {
            anyhow::bail!("path traversal is not allowed")
        }
    }

    let mut out = fs_root.to_path_buf();
    for comp in requested_path.components() {
        match comp {
            Component::Normal(seg) => out.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("invalid sandbox path component")
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn make_state(granted_permissions: Vec<Permission>) -> PluginState {
        PluginState {
            manifest: PluginManifest {
                name: "p".to_string(),
                version: "0.1.0".to_string(),
                description: None,
                author: None,
                wasm_module: "p.wasm".to_string(),
                permissions: vec![],
                entry_point: "_start".to_string(),
            },
            granted_permissions,
            fs_root: std::env::temp_dir(),
            pane_sizes: HashMap::new(),
            ui_draw_calls: Vec::new(),
            overlays: Vec::new(),
            next_overlay_id: 1,
            stream_inputs: HashMap::new(),
            stream_outputs: HashMap::new(),
            io_write_calls: Vec::new(),
            tcp_sockets: HashMap::new(),
            next_socket_id: 1,
            net_http_calls: Vec::new(),
            platform_bt_devices: Vec::new(),
            platform_keyring: HashMap::new(),
            denied_permissions: HashSet::new(),
            queued_prompt_responses: HashMap::new(),
            permission_prompt_events: Vec::new(),
        }
    }

    #[test]
    fn parse_http_headers_ignores_invalid_lines() {
        let raw = "Authorization: Bearer token\nX-Test: value\ninvalid-line\nEmpty:\n";
        let parsed = parse_http_headers(raw);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "Authorization");
        assert_eq!(parsed[1].0, "X-Test");
    }

    #[test]
    fn http_post_request_sends_body_and_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener local addr");

        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept");
            let mut request_buf = [0_u8; 4096];
            let read = std::io::Read::read(&mut socket, &mut request_buf).expect("read request");
            let request = String::from_utf8_lossy(&request_buf[..read]);
            assert!(request.starts_with("POST /chat HTTP/1.1"));
            assert!(request.contains("x-test: value") || request.contains("X-Test: value"));
            assert!(request.contains("hello"));

            let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
            let _ = std::io::Write::write_all(&mut socket, response);
        });

        let url = format!("http://{}/chat", addr);
        let headers = vec![("X-Test".to_string(), "value".to_string())];
        let (status, body) = http_post_request(&url, &headers, b"hello").expect("post request");

        assert_eq!(status, 200);
        assert_eq!(body, b"ok");
        server.join().expect("join server thread");
    }

    #[test]
    fn permission_check_works() {
        let state = make_state(vec![Permission::Ui]);

        assert!(state.granted_permissions.contains(&Permission::Ui));
        assert!(!state.granted_permissions.contains(&Permission::Network));
    }

    #[test]
    fn stream_input_pop_consumes_in_order() {
        let mut state = make_state(vec![Permission::Io]);
        state.stream_inputs = HashMap::from([(7, b"abcdef".to_vec())]);

        let first = pop_stream_input_chunk(&mut state, 7, 2);
        let second = pop_stream_input_chunk(&mut state, 7, 10);
        let third = pop_stream_input_chunk(&mut state, 7, 1);

        assert_eq!(first, b"ab");
        assert_eq!(second, b"cdef");
        assert!(third.is_empty());
    }

    #[test]
    fn stream_output_push_appends() {
        let mut state = make_state(vec![Permission::Io]);

        push_stream_output_chunk(&mut state, 9, b"foo");
        push_stream_output_chunk(&mut state, 9, b"bar");

        assert_eq!(state.stream_outputs.get(&9).cloned().unwrap_or_default(), b"foobar");
    }

    #[test]
    fn tcp_connect_with_timeout_connects_to_local_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener local addr");

        let accept_thread = thread::spawn(move || {
            let _ = listener.accept();
        });

        let stream = tcp_connect_with_timeout("127.0.0.1", addr.port(), Duration::from_secs(1))
            .expect("connect to local listener");
        drop(stream);
        accept_thread.join().expect("join accept thread");
    }

    #[test]
    fn sandbox_path_rejects_parent_traversal() {
        let root = std::env::temp_dir().join("uterx-sandbox-test");
        let result = resolve_sandbox_path(&root, "../secret.txt");
        assert!(result.is_err());
    }

    #[test]
    fn sandbox_path_stays_within_root() {
        let root = std::env::temp_dir().join("uterx-sandbox-test");
        let resolved = resolve_sandbox_path(&root, "folder/data.txt").expect("resolve path");
        assert!(resolved.starts_with(&root));
        assert!(resolved.ends_with(Path::new("folder").join("data.txt")));
    }

    #[test]
    fn platform_mock_state_holds_bt_and_keyring_values() {
        let mut state = make_state(vec![Permission::Platform]);
        state.platform_bt_devices = vec![PlatformBtDevice {
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            name: Some("Mock Device".to_string()),
        }];
        state.platform_keyring = HashMap::from([("token".to_string(), "secret".to_string())]);

        assert_eq!(state.platform_bt_devices.len(), 1);
        assert_eq!(state.platform_keyring.get("token"), Some(&"secret".to_string()));

        state.platform_bt_devices.clear();
        assert!(state.platform_bt_devices.is_empty());
    }

    #[test]
    fn permission_prompt_grant_once_allows_single_call() {
        let mut state = make_state(vec![]);
        state
            .queued_prompt_responses
            .insert(Permission::Io, PermissionPromptResponse::GrantOnce);

        assert!(resolve_permission_decision(&mut state, Permission::Io, "uterx_io.read"));
        assert!(!resolve_permission_decision(&mut state, Permission::Io, "uterx_io.read"));
        assert_eq!(state.permission_prompt_events.len(), 2);
    }

    #[test]
    fn permission_prompt_grant_always_persists() {
        let mut state = make_state(vec![]);
        state
            .queued_prompt_responses
            .insert(Permission::Network, PermissionPromptResponse::GrantAlways);

        assert!(resolve_permission_decision(
            &mut state,
            Permission::Network,
            "uterx_net.http_get"
        ));
        assert!(resolve_permission_decision(
            &mut state,
            Permission::Network,
            "uterx_net.tcp_connect"
        ));
        assert_eq!(state.permission_prompt_events.len(), 1);
    }
}
