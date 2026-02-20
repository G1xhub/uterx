//! WASM plugin runtime using wasmtime.

use crate::manifest::{Permission, PluginManifest};
use std::path::Path;
use wasmtime::*;

/// A running plugin instance.
pub struct PluginInstance {
    store: Store<PluginState>,
    instance: Instance,
}

/// Per-plugin state accessible from host functions.
struct PluginState {
    manifest: PluginManifest,
    granted_permissions: Vec<Permission>,
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

    // TODO: register uterx::ui::draw, uterx::io::read, uterx::net::request, etc.

    Ok(())
}
