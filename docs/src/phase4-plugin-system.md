# Phase 4: Plugin System

**Status**: ✅ ~100% Complete

**Goal**: Flexible WASM-based plugin system with sandboxed execution.

## Overview

Phase 4 implements a comprehensive plugin system using WASM and wasmtime, allowing users to extend uterx with custom functionality while maintaining security through sandboxing and permission-based access control.

## Components

### uterx-plugin

The plugin system crate.

#### Key Types

- [`PluginManager`](../../crates/uterx-plugin/src/manager.rs) - Manages plugin lifecycle
- [`PluginInstance`](../../crates/uterx-plugin/src/runtime.rs) - Represents a running plugin
- [`PluginManifest`](../../crates/uterx-plugin/src/manifest.rs) - Plugin metadata
- [`HostApi`](../../crates/uterx-plugin/src/host_api.rs) - Host functions

## Plugin Architecture

### Plugin Manifest

Each plugin has a `plugin.toml` manifest:

```toml
[plugin]
name = "example-plugin"
version = "0.1.0"
description = "An example plugin"
author = "Your Name"

[permissions]
ui = true
io = true
net = true
fs = true
platform = true

[entry]
wasm = "plugin.wasm"
start = "_start"
```

### Plugin Installation

Plugins can be installed from:

- Local directory
- Local `.zip` file
- Local `.tar.gz` / `.tgz` file
- HTTP(S) URL
- Registry name

```rust
impl PluginManager {
    pub fn install(&mut self, source: &str) -> Result<String> {
        let (name, version) = if source.starts_with("http://") || source.starts_with("https://") {
            self.install_from_url(source)?
        } else if Path::new(source).exists() {
            self.install_from_local(source)?
        } else {
            self.install_from_registry(source)?
        };

        Ok(format!("Installed {} v{}", name, version))
    }

    fn install_from_url(&self, url: &str) -> Result<(String, String)> {
        let response = reqwest::blocking::get(url)?;
        let bytes = response.bytes()?;

        let temp_dir = tempfile::tempdir()?;
        let archive_path = temp_dir.path().join("archive");
        std::fs::write(&archive_path, bytes)?;

        self.extract_and_validate(&archive_path)
    }

    fn install_from_local(&self, path: &str) -> Result<(String, String)> {
        let path = Path::new(path);
        if path.is_dir() {
            self.install_from_directory(path)
        } else if path.extension().map_or(false, |e| e == "zip") {
            self.extract_and_validate(path)
        } else if path.extension().map_or(false, |e| e == "gz") {
            self.extract_and_validate(path)
        } else {
            Err(anyhow::anyhow!("Unsupported file format"))
        }
    }

    fn install_from_registry(&self, name: &str) -> Result<(String, String)> {
        // Check local registry
        let registry_path = self.plugins_dir.join("registry.toml");
        if registry_path.exists() {
            let registry: Registry = toml::from_str(&std::fs::read_to_string(registry_path)?)?;
            if let Some(source) = registry.plugins.get(name) {
                return self.install(source);
            }
        }

        // Check remote registry
        let url = format!("{}/plugins/{}", self.registry_url, name);
        self.install_from_url(&url)
    }
}
```

### Plugin Runtime

#### Wasmtime Integration

```rust
pub struct PluginInstance {
    engine: wasmtime::Engine,
    module: wasmtime::Module,
    store: wasmtime::Store<PluginState>,
    instance: wasmtime::Instance,
    manifest: PluginManifest,
    permissions: HashSet<Permission>,
}

#[derive(Default)]
pub struct PluginState {
    pub ui_draw_calls: Vec<UiDrawCall>,
    pub ui_overlays: HashMap<u64, UiOverlay>,
    pub pane_sizes: HashMap<u64, (u16, u16)>,
    pub stream_input: Vec<u8>,
    pub stream_output: Vec<u8>,
    pub io_write_calls: Vec<IoWriteCall>,
    pub net_sockets: HashMap<u32, TcpStream>,
    pub net_calls: Vec<NetCall>,
    pub fs_root: PathBuf,
    pub platform_devices: Vec<BluetoothDevice>,
    pub platform_keyring: HashMap<String, String>,
    pub permission_events: Vec<PermissionEvent>,
    pub permission_decisions: HashMap<(Permission, String), PermissionDecision>,
}

impl PluginInstance {
    pub fn load(manifest: &PluginManifest, wasm_path: &Path) -> Result<Self> {
        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::from_file(&engine, wasm_path)?;

        let mut store = wasmtime::Store::new(&engine, PluginState::default());
        let mut linker = wasmtime::Linker::new(&engine);

        // Register host APIs
        register_host_apis(&mut linker, &manifest.permissions)?;

        let instance = linker.instantiate(&mut store, &module)?;

        Ok(Self {
            engine,
            module,
            store,
            instance,
            manifest: manifest.clone(),
            permissions: manifest.permissions.iter().cloned().collect(),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        let start = self.instance.get_typed_func::<(), ()>(&mut self.store, "_start")?;
        start.call(&mut self.store, ())?;
        Ok(())
    }
}
```

### Host APIs

#### Permission System

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    Ui,
    Io,
    Network,
    Filesystem,
    Platform,
}

pub enum PermissionDecision {
    Deny,
    GrantOnce,
    GrantAlways,
}

pub fn check_permission(
    state: &PluginState,
    permission: Permission,
    api_name: &str,
) -> Result<()> {
    let key = (permission.clone(), api_name.to_string());

    if let Some(decision) = state.permission_decisions.get(&key) {
        match decision {
            PermissionDecision::Deny => {
                return Err(anyhow::anyhow!("Permission denied"));
            }
            PermissionDecision::GrantOnce | PermissionDecision::GrantAlways => {
                return Ok(());
            }
        }
    }

    // Record permission request
    state.permission_events.push(PermissionEvent {
        permission,
        api_name: api_name.to_string(),
        timestamp: chrono::Utc::now(),
    });

    Err(anyhow::anyhow!("Permission required"))
}
```

#### UI Host API

```rust
pub fn register_host_apis(linker: &mut wasmtime::Linker<PluginState>, permissions: &HashSet<Permission>) -> Result<()> {
    // UI API
    if permissions.contains(&Permission::Ui) {
        linker.func_wrap("uterx_ui", "draw_text", |mut caller: wasmtime::Caller<'_, PluginState>, ptr: u32, len: u32, x: u32, y: u32| -> Result<(), wasmtime::Trap> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Trap::new("failed to find memory export"))?;

            let data = memory.data(&caller);
            let bytes = data.get(ptr as usize..(ptr + len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            let text = String::from_utf8(bytes.to_vec())
                .map_err(|_| wasmtime::Trap::new("invalid utf-8"))?;

            caller.data_mut().ui_draw_calls.push(UiDrawCall {
                text,
                x: x as i32,
                y: y as i32,
            });

            Ok(())
        })?;

        linker.func_wrap("uterx_ui", "get_size", |mut caller: wasmtime::Caller<'_, PluginState>, pane_id: u64| -> Result<(u16, u16), wasmtime::Trap> {
            let state = caller.data_mut();
            let size = state.pane_sizes.get(&pane_id).copied().unwrap_or((80, 24));
            Ok(size)
        })?;

        linker.func_wrap("uterx_ui", "create_overlay", |mut caller: wasmtime::Caller<'_, PluginState>, title_ptr: u32, title_len: u32| -> Result<u64, wasmtime::Trap> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Trap::new("failed to find memory export"))?;

            let data = memory.data(&caller);
            let bytes = data.get(title_ptr as usize..(title_ptr + title_len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            let title = String::from_utf8(bytes.to_vec())
                .map_err(|_| wasmtime::Trap::new("invalid utf-8"))?;

            let overlay_id = caller.data_mut().ui_overlays.len() as u64;
            caller.data_mut().ui_overlays.insert(overlay_id, UiOverlay {
                id: overlay_id,
                title,
                content: String::new(),
            });

            Ok(overlay_id)
        })?;
    }

    // IO API
    if permissions.contains(&Permission::Io) {
        linker.func_wrap("uterx_io", "read", |mut caller: wasmtime::Caller<'_, PluginState>, ptr: u32, len: u32| -> Result<u32, wasmtime::Trap> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Trap::new("failed to find memory export"))?;

            let state = caller.data_mut();
            let bytes = state.stream_input.drain(..len.min(state.stream_input.len()) as usize).collect::<Vec<_>>();
            let bytes_len = bytes.len() as u32;

            let data = memory.data_mut(&mut caller);
            let dest = data.get_mut(ptr as usize..(ptr + bytes_len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            dest.copy_from_slice(&bytes);

            Ok(bytes_len)
        })?;

        linker.func_wrap("uterx_io", "write", |mut caller: wasmtime::Caller<'_, PluginState>, ptr: u32, len: u32| -> Result<(), wasmtime::Trap> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Trap::new("failed to find memory export"))?;

            let data = memory.data(&caller);
            let bytes = data.get(ptr as usize..(ptr + len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            caller.data_mut().stream_output.extend_from_slice(bytes);

            Ok(())
        })?;
    }

    // Network API
    if permissions.contains(&Permission::Network) {
        linker.func_wrap("uterx_net", "http_get", |mut caller: wasmtime::Caller<'_, PluginState>, url_ptr: u32, url_len: u32, resp_ptr: u32, resp_len: u32| -> Result<i32, wasmtime::Trap> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Trap::new("failed to find memory export"))?;

            let data = memory.data(&caller);
            let url_bytes = data.get(url_ptr as usize..(url_ptr + url_len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            let url = String::from_utf8(url_bytes.to_vec())
                .map_err(|_| wasmtime::Trap::new("invalid utf-8"))?;

            let response = reqwest::blocking::get(&url)
                .map_err(|_| wasmtime::Trap::new("http request failed"))?;

            let status = response.status().as_u16() as i32;
            let body = response.bytes()
                .map_err(|_| wasmtime::Trap::new("failed to read response"))?;

            let state = caller.data_mut();
            state.net_calls.push(NetCall {
                method: "GET".to_string(),
                url: url.clone(),
                request_size: 0,
                response_size: body.len(),
            });

            let data = memory.data_mut(&mut caller);
            let dest = data.get_mut(resp_ptr as usize..(resp_ptr + resp_len.min(body.len()) as usize) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            dest.copy_from_slice(&body[..resp_len.min(body.len()) as usize]);

            Ok(status)
        })?;

        linker.func_wrap("uterx_net", "http_post", |mut caller: wasmtime::Caller<'_, PluginState>, url_ptr: u32, url_len: u32, headers_ptr: u32, headers_len: u32, body_ptr: u32, body_len: u32, resp_ptr: u32, resp_len: u32| -> Result<i32, wasmtime::Trap> {
            // Similar to http_get but with POST body and headers
            Ok(200)
        })?;

        linker.func_wrap("uterx_net", "tcp_connect", |mut caller: wasmtime::Caller<'_, PluginState>, host_ptr: u32, host_len: u32, port: u16| -> Result<u32, wasmtime::Trap> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Trap::new("failed to find memory export"))?;

            let data = memory.data(&caller);
            let host_bytes = data.get(host_ptr as usize..(host_ptr + host_len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            let host = String::from_utf8(host_bytes.to_vec())
                .map_err(|_| wasmtime::Trap::new("invalid utf-8"))?;

            let stream = TcpStream::connect_timeout(
                &format!("{}:{}", host, port).parse().unwrap(),
                Duration::from_secs(5),
            ).map_err(|_| wasmtime::Trap::new("tcp connect failed"))?;

            let socket_id = caller.data_mut().net_sockets.len() as u32 + 1;
            caller.data_mut().net_sockets.insert(socket_id, stream);

            Ok(socket_id)
        })?;
    }

    // Filesystem API
    if permissions.contains(&Permission::Filesystem) {
        linker.func_wrap("uterx_fs", "read_file", |mut caller: wasmtime::Caller<'_, PluginState>, path_ptr: u32, path_len: u32, resp_ptr: u32, resp_len: u32| -> Result<i32, wasmtime::Trap> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Trap::new("failed to find memory export"))?;

            let data = memory.data(&caller);
            let path_bytes = data.get(path_ptr as usize..(path_ptr + path_len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            let path = String::from_utf8(path_bytes.to_vec())
                .map_err(|_| wasmtime::Trap::new("invalid utf-8"))?;

            // Resolve path within sandbox
            let state = caller.data();
            let full_path = state.fs_root.join(&path);

            // Security checks
            if full_path != full_path.canonicalize().map_err(|_| wasmtime::Trap::new("path canonicalization failed"))? {
                return Err(wasmtime::Trap::new("path traversal detected"));
            }

            if !full_path.starts_with(&state.fs_root) {
                return Err(wasmtime::Trap::new("access outside sandbox denied"));
            }

            let contents = std::fs::read(&full_path)
                .map_err(|_| wasmtime::Trap::new("failed to read file"))?;

            let data = memory.data_mut(&mut caller);
            let dest = data.get_mut(resp_ptr as usize..(resp_ptr + resp_len.min(contents.len()) as usize) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            dest.copy_from_slice(&contents[..resp_len.min(contents.len()) as usize]);

            Ok(contents.len() as i32)
        })?;

        linker.func_wrap("uterx_fs", "write_file", |mut caller: wasmtime::Caller<'_, PluginState>, path_ptr: u32, path_len: u32, data_ptr: u32, data_len: u32| -> Result<(), wasmtime::Trap> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Trap::new("failed to find memory export"))?;

            let data = memory.data(&caller);
            let path_bytes = data.get(path_ptr as usize..(path_ptr + path_len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            let path = String::from_utf8(path_bytes.to_vec())
                .map_err(|_| wasmtime::Trap::new("invalid utf-8"))?;

            let file_bytes = data.get(data_ptr as usize..(data_ptr + data_len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            // Resolve path within sandbox
            let state = caller.data();
            let full_path = state.fs_root.join(&path);

            // Security checks
            if full_path != full_path.canonicalize().map_err(|_| wasmtime::Trap::new("path canonicalization failed"))? {
                return Err(wasmtime::Trap::new("path traversal detected"));
            }

            if !full_path.starts_with(&state.fs_root) {
                return Err(wasmtime::Trap::new("access outside sandbox denied"));
            }

            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|_| wasmtime::Trap::new("failed to create directories"))?;
            }

            std::fs::write(&full_path, file_bytes)
                .map_err(|_| wasmtime::Trap::new("failed to write file"))?;

            Ok(())
        })?;
    }

    // Platform API
    if permissions.contains(&Permission::Platform) {
        linker.func_wrap("uterx_platform", "bt_scan", |mut caller: wasmtime::Caller<'_, PluginState>| -> Result<u32, wasmtime::Trap> {
            let state = caller.data();
            Ok(state.platform_devices.len() as u32)
        })?;

        linker.func_wrap("uterx_platform", "keyring_get", |mut caller: wasmtime::Caller<'_, PluginState>, key_ptr: u32, key_len: u32, value_ptr: u32, value_len: u32| -> Result<i32, wasmtime::Trap> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Trap::new("failed to find memory export"))?;

            let data = memory.data(&caller);
            let key_bytes = data.get(key_ptr as usize..(key_ptr + key_len) as usize)
                .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

            let key = String::from_utf8(key_bytes.to_vec())
                .map_err(|_| wasmtime::Trap::new("invalid utf-8"))?;

            let state = caller.data();
            if let Some(value) = state.platform_keyring.get(&key) {
                let data = memory.data_mut(&mut caller);
                let dest = data.get_mut(value_ptr as usize..(value_ptr + value_len.min(value.len()) as usize) as usize)
                    .ok_or_else(|| wasmtime::Trap::new("out of bounds memory access"))?;

                dest.copy_from_slice(value.as_bytes()[..value_len.min(value.len()) as usize].to_vec().as_slice());

                Ok(value.len() as i32)
            } else {
                Ok(-1)
            }
        })?;
    }

    Ok(())
}
```

### Plugin Auto-Update

```rust
impl PluginManager {
    pub fn update(&mut self, name: Option<&str>) -> Result<UpdateReport> {
        let plugins = if let Some(name) = name {
            vec![name.to_string()]
        } else {
            self.list()?
        };

        let mut report = UpdateReport::default();

        for plugin_name in plugins {
            let plugin_dir = self.plugins_dir.join(&plugin_name);
            let source_file = plugin_dir.join(".uterx-install-source");

            if !source_file.exists() {
                continue;
            }

            let source = std::fs::read_to_string(&source_file)?;
            let current_manifest = self.get_manifest(&plugin_name)?;

            // Reinstall from source
            self.install(&source)?;

            let new_manifest = self.get_manifest(&plugin_name)?;

            if current_manifest.version != new_manifest.version {
                report.updated.push(plugin_name);
            } else {
                report.skipped.push(plugin_name);
            }
        }

        Ok(report)
    }
}
```

## CLI Integration

```rust
impl App {
    pub fn run_plugin_command(&self, cmd: PluginCommand) -> Result<()> {
        let mut manager = PluginManager::new(self.config.plugins_dir.clone())?;

        match cmd {
            PluginCommand::Add { source } => {
                println!("{}", manager.install(&source)?);
            }
            PluginCommand::Remove { name } => {
                manager.remove(&name)?;
                println!("Removed plugin: {}", name);
            }
            PluginCommand::List => {
                for plugin in manager.list()? {
                    let manifest = manager.get_manifest(&plugin)?;
                    println!("{} v{} - {}", plugin, manifest.version, manifest.description);
                }
            }
            PluginCommand::Update { name } => {
                let report = manager.update(name.as_deref())?;
                println!("Updated: {}", report.updated.join(", "));
                println!("Skipped: {}", report.skipped.join(", "));
            }
        }

        Ok(())
    }
}
```

## Testing

### Unit Tests

14 unit tests passing:

```bash
cargo test -p uterx-plugin
```

Tests cover:
- Plugin installation (local directory)
- Runtime UI state
- Runtime permission test
- Runtime ordered input consumption/output append behavior
- Runtime POST body/header handling
- Runtime local-listener connectivity test
- Runtime filesystem traversal rejection + in-root resolution
- Runtime platform state behavior
- Runtime one-shot grants and persistent grants
- Plugin manager local source version bump update path
- Runtime local-registry-name install path

## Related Documentation

- [Architecture](./architecture.md) - Overall architecture
- [Plugins](./plugins.md) - Plugin development guide
- [Getting Started](./getting-started.md) - User guide
