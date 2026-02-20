//! Plugin manifest (plugin.toml) schema and loading.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Capabilities a plugin can request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Draw to a pane / create overlays.
    Ui,
    /// Read/write IO streams.
    Io,
    /// Make network requests.
    Network,
    /// Access the filesystem (sandboxed).
    Filesystem,
    /// Access platform APIs (bluetooth, system info).
    Platform,
}

/// The plugin manifest (deserialized from plugin.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    /// Path to the WASM module (relative to plugin directory).
    pub wasm_module: String,
    /// Requested permissions.
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Entry point function name (default: "_start").
    #[serde(default = "default_entry")]
    pub entry_point: String,
}

fn default_entry() -> String {
    "_start".to_string()
}

impl PluginManifest {
    /// Load a manifest from a plugin.toml file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let manifest: Self = toml::from_str(&content)?;
        Ok(manifest)
    }
}
