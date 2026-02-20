//! Plugin manager: install, list, remove, update plugins.

use crate::manifest::PluginManifest;
use std::collections::HashMap;
use std::path::PathBuf;

/// Manages installed plugins.
pub struct PluginManager {
    plugins_dir: PathBuf,
    installed: HashMap<String, PluginManifest>,
}

impl PluginManager {
    /// Create a new plugin manager rooted at the given directory.
    /// Typically `~/.uterx/plugins/`.
    pub fn new(plugins_dir: PathBuf) -> Self {
        Self {
            plugins_dir,
            installed: HashMap::new(),
        }
    }

    /// Scan the plugins directory and load all manifests.
    pub fn scan(&mut self) -> anyhow::Result<()> {
        self.installed.clear();
        if !self.plugins_dir.exists() {
            std::fs::create_dir_all(&self.plugins_dir)?;
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.plugins_dir)? {
            let entry = entry?;
            let manifest_path = entry.path().join("plugin.toml");
            if manifest_path.exists() {
                match PluginManifest::load(&manifest_path) {
                    Ok(m) => {
                        self.installed.insert(m.name.clone(), m);
                    }
                    Err(e) => {
                        tracing::warn!("failed to load plugin manifest {:?}: {}", manifest_path, e);
                    }
                }
            }
        }
        Ok(())
    }

    /// List all installed plugins.
    pub fn list(&self) -> Vec<&PluginManifest> {
        self.installed.values().collect()
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<&PluginManifest> {
        self.installed.get(name)
    }

    /// Install a plugin from a local path or URL (stub).
    pub fn install(&mut self, _source: &str) -> anyhow::Result<()> {
        // TODO: download from URL or copy from path, extract, load manifest
        anyhow::bail!("plugin installation not yet implemented")
    }

    /// Remove an installed plugin.
    pub fn remove(&mut self, name: &str) -> anyhow::Result<()> {
        let plugin_dir = self.plugins_dir.join(name);
        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir)?;
        }
        self.installed.remove(name);
        Ok(())
    }

    /// Get the WASM module path for a plugin.
    pub fn wasm_path(&self, name: &str) -> Option<PathBuf> {
        let manifest = self.installed.get(name)?;
        Some(self.plugins_dir.join(name).join(&manifest.wasm_module))
    }
}
