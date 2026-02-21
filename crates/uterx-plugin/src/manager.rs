//! Plugin manager: install, list, remove, update plugins.

use crate::manifest::PluginManifest;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Manages installed plugins.
pub struct PluginManager {
    plugins_dir: PathBuf,
    installed: HashMap<String, PluginManifest>,
}

const INSTALL_SOURCE_FILE: &str = ".uterx-install-source";
const REGISTRY_INDEX_FILE: &str = "registry.toml";
const DEFAULT_REGISTRY_INDEX_URL: &str =
    "https://raw.githubusercontent.com/uterx/plugins-registry/main/index.toml";

pub struct PluginUpdateReport {
    pub updated: Vec<String>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PluginRegistryIndex {
    #[serde(default)]
    plugins: HashMap<String, String>,
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

    /// Install a plugin from a local path or URL.
    ///
    /// Supported sources:
    /// - local plugin directory containing `plugin.toml`
    /// - local `.zip` / `.tar.gz` / `.tgz` archive
    /// - `http(s)` URL pointing to one of the above archive types
    /// - registry plugin name resolved via local/remote registry index
    pub fn install(&mut self, source: &str) -> anyhow::Result<PluginManifest> {
        std::fs::create_dir_all(&self.plugins_dir)?;

        let resolved_source = self.resolve_install_source(source)?;

        let staging = tempfile::tempdir()?;
        let extracted_root = staging.path().join("src");
        std::fs::create_dir_all(&extracted_root)?;

        if is_url(&resolved_source) {
            let downloaded_path = download_to_temp_file(&resolved_source, staging.path())?;
            extract_source_path(&downloaded_path, &extracted_root)?;
        } else {
            extract_source_path(Path::new(&resolved_source), &extracted_root)?;
        }

        let manifest_path = find_manifest_path(&extracted_root)
            .ok_or_else(|| anyhow::anyhow!("plugin.toml not found in source"))?;
        let plugin_root = manifest_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid plugin source layout"))?;
        let manifest = PluginManifest::load(&manifest_path)?;

        validate_manifest(&manifest, plugin_root)?;

        let target_dir = self.plugins_dir.join(&manifest.name);
        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir)?;
        }
        std::fs::create_dir_all(&target_dir)?;
        copy_dir_recursive(plugin_root, &target_dir)?;
        let normalized_source = normalize_install_source(&resolved_source);
        std::fs::write(target_dir.join(INSTALL_SOURCE_FILE), normalized_source.as_bytes())?;

        self.installed
            .insert(manifest.name.clone(), manifest.clone());
        Ok(manifest)
    }

    /// Update one plugin or all installed plugins.
    pub fn update(&mut self, name: Option<&str>) -> anyhow::Result<PluginUpdateReport> {
        let mut report = PluginUpdateReport {
            updated: Vec::new(),
            skipped: Vec::new(),
        };

        match name {
            Some(plugin_name) => {
                self.update_one(plugin_name, &mut report)?;
            }
            None => {
                let names: Vec<String> = self.installed.keys().cloned().collect();
                for plugin_name in names {
                    self.update_one(&plugin_name, &mut report)?;
                }
            }
        }

        self.scan()?;
        Ok(report)
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

    fn update_one(&mut self, name: &str, report: &mut PluginUpdateReport) -> anyhow::Result<()> {
        if self.installed.get(name).is_none() {
            report
                .skipped
                .push(format!("{} (not installed)", name));
            return Ok(());
        }

        let Some(source) = self.read_install_source(name)? else {
            report
                .skipped
                .push(format!("{} (missing install source metadata)", name));
            return Ok(());
        };

        let updated = self.install(&source)?;
        report
            .updated
            .push(format!("{} v{}", updated.name, updated.version));
        Ok(())
    }

    fn read_install_source(&self, name: &str) -> anyhow::Result<Option<String>> {
        let source_path = self.plugins_dir.join(name).join(INSTALL_SOURCE_FILE);
        if !source_path.exists() {
            return Ok(None);
        }
        let source = std::fs::read_to_string(&source_path)?;
        let source = source.trim().to_string();
        if source.is_empty() {
            return Ok(None);
        }
        Ok(Some(source))
    }

    fn resolve_install_source(&self, source: &str) -> anyhow::Result<String> {
        if is_url(source) || Path::new(source).exists() {
            return Ok(source.to_string());
        }

        if let Some(mapped) = self.lookup_registry_source(source)? {
            return Ok(mapped);
        }

        anyhow::bail!(
            "plugin source '{}' not found as path/url and no registry entry exists",
            source
        )
    }

    fn lookup_registry_source(&self, plugin_name: &str) -> anyhow::Result<Option<String>> {
        if let Some(local) = self.lookup_local_registry_source(plugin_name)? {
            return Ok(Some(local));
        }
        self.lookup_remote_registry_source(plugin_name)
    }

    fn lookup_local_registry_source(&self, plugin_name: &str) -> anyhow::Result<Option<String>> {
        let registry_path = self.plugins_dir.join(REGISTRY_INDEX_FILE);
        if !registry_path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(registry_path)?;
        let index: PluginRegistryIndex = toml::from_str(&content)?;
        Ok(index.plugins.get(plugin_name).cloned())
    }

    fn lookup_remote_registry_source(&self, plugin_name: &str) -> anyhow::Result<Option<String>> {
        let response = reqwest::blocking::get(DEFAULT_REGISTRY_INDEX_URL);
        let Ok(response) = response else {
            return Ok(None);
        };
        if !response.status().is_success() {
            return Ok(None);
        }
        let body = response
            .text()
            .map_err(|e| anyhow::anyhow!("failed reading registry index: {}", e))?;
        let index: PluginRegistryIndex = toml::from_str(&body)?;
        Ok(index.plugins.get(plugin_name).cloned())
    }
}

fn is_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

fn normalize_install_source(source: &str) -> String {
    if is_url(source) {
        return source.to_string();
    }

    let source_path = Path::new(source);
    match source_path.canonicalize() {
        Ok(abs) => abs.to_string_lossy().to_string(),
        Err(_) => source.to_string(),
    }
}

fn download_to_temp_file(url: &str, temp_root: &Path) -> anyhow::Result<PathBuf> {
    let response = reqwest::blocking::get(url)
        .map_err(|e| anyhow::anyhow!("failed to download plugin: {}", e))?;
    if !response.status().is_success() {
        anyhow::bail!("failed to download plugin: HTTP {}", response.status());
    }

    let bytes = response
        .bytes()
        .map_err(|e| anyhow::anyhow!("failed reading download body: {}", e))?;

    let mut filename = url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("plugin-download");
    if let Some((head, _)) = filename.split_once('?') {
        filename = head;
    }
    if filename.is_empty() {
        filename = "plugin-download";
    }

    let out_path = temp_root.join(filename);
    let mut file = std::fs::File::create(&out_path)?;
    file.write_all(&bytes)?;
    Ok(out_path)
}

fn extract_source_path(source_path: &Path, dest_root: &Path) -> anyhow::Result<()> {
    if source_path.is_dir() {
        copy_dir_recursive(source_path, dest_root)?;
        return Ok(());
    }
    if !source_path.exists() {
        anyhow::bail!("plugin source does not exist: {}", source_path.display());
    }

    let path_str = source_path.to_string_lossy().to_lowercase();
    if path_str.ends_with(".zip") {
        extract_zip(source_path, dest_root)?;
        return Ok(());
    }
    if path_str.ends_with(".tar.gz") || path_str.ends_with(".tgz") {
        extract_tar_gz(source_path, dest_root)?;
        return Ok(());
    }

    anyhow::bail!(
        "unsupported plugin source format: {} (expected directory, .zip, .tar.gz, or .tgz)",
        source_path.display()
    )
}

fn extract_zip(zip_path: &Path, dest_root: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| anyhow::anyhow!("invalid zip archive: {}", e))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| anyhow::anyhow!("failed reading zip entry: {}", e))?;
        let Some(rel_path) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let out_path = dest_root.join(rel_path);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

fn extract_tar_gz(tar_gz_path: &Path, dest_root: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::open(tar_gz_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest_root)?;
    Ok(())
}

fn find_manifest_path(root: &Path) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }

    let direct = root.join("plugin.toml");
    if direct.exists() {
        return Some(direct);
    }

    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_manifest_path(&path) {
                return Some(found);
            }
        }
    }
    None
}

fn validate_manifest(manifest: &PluginManifest, plugin_root: &Path) -> anyhow::Result<()> {
    if manifest.name.trim().is_empty() {
        anyhow::bail!("plugin manifest contains empty name");
    }
    if manifest.version.trim().is_empty() {
        anyhow::bail!("plugin manifest contains empty version");
    }

    let wasm_rel = Path::new(&manifest.wasm_module);
    if wasm_rel.is_absolute() {
        anyhow::bail!("`wasm_module` must be a relative path");
    }

    let wasm_full = plugin_root.join(wasm_rel);
    let plugin_root_canon = plugin_root.canonicalize()?;
    let wasm_canon = wasm_full
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("WASM module not found: {}", wasm_full.display()))?;
    if !wasm_canon.starts_with(&plugin_root_canon) {
        anyhow::bail!("`wasm_module` must resolve inside plugin directory");
    }
    if wasm_canon.extension().and_then(|e| e.to_str()) != Some("wasm") {
        anyhow::bail!("`wasm_module` must point to a .wasm file");
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            if let Some(parent) = dst_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use std::thread;
    use std::time::Duration;

    fn write_plugin_source(source_dir: &Path, version: &str) {
        std::fs::create_dir_all(source_dir).expect("create source dir");
        let wasm_path = source_dir.join("plugin.wasm");
        std::fs::write(&wasm_path, [0x00, 0x61, 0x73, 0x6d]).expect("write wasm");

        let manifest = format!(
            r#"
name = "example"
version = "{}"
wasm_module = "plugin.wasm"
permissions = ["ui"]
"#,
            version
        );
        std::fs::write(source_dir.join("plugin.toml"), manifest).expect("write manifest");
    }

    #[test]
    fn installs_plugin_from_local_directory() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let plugins_dir = temp.path().join("plugins");
        let source_dir = temp.path().join("source").join("example");
        write_plugin_source(&source_dir, "0.1.0");

        let mut manager = PluginManager::new(plugins_dir.clone());
        let installed = manager
            .install(source_dir.to_string_lossy().as_ref())
            .expect("install plugin");

        assert_eq!(installed.name, "example");
        assert!(plugins_dir.join("example").join("plugin.toml").exists());
        assert!(plugins_dir.join("example").join("plugin.wasm").exists());
        assert!(plugins_dir
            .join("example")
            .join(INSTALL_SOURCE_FILE)
            .exists());
        assert!(manager.get("example").is_some());
    }

    #[test]
    fn updates_plugin_from_saved_source() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let plugins_dir = temp.path().join("plugins");
        let source_dir = temp.path().join("source").join("example");
        write_plugin_source(&source_dir, "0.1.0");

        let mut manager = PluginManager::new(plugins_dir.clone());
        manager
            .install(source_dir.to_string_lossy().as_ref())
            .expect("install plugin");

        thread::sleep(Duration::from_millis(5));
        write_plugin_source(&source_dir, "0.2.0");

        let report = manager.update(Some("example")).expect("update plugin");
        assert_eq!(report.skipped.len(), 0);
        assert_eq!(report.updated.len(), 1);

        let plugin = manager.get("example").expect("plugin still installed");
        assert_eq!(plugin.version, "0.2.0");
    }

    #[test]
    fn installs_plugin_from_local_registry_name() {
        #[derive(Serialize)]
        struct IndexDoc {
            plugins: HashMap<String, String>,
        }

        let temp = tempfile::tempdir().expect("create temp dir");
        let plugins_dir = temp.path().join("plugins");
        let source_dir = temp.path().join("source").join("example");
        write_plugin_source(&source_dir, "0.3.0");
        std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");

        let mut plugins = HashMap::new();
        plugins.insert("example".to_string(), source_dir.to_string_lossy().to_string());
        let registry = toml::to_string(&IndexDoc { plugins }).expect("serialize registry");
        std::fs::write(plugins_dir.join(REGISTRY_INDEX_FILE), registry).expect("write registry");

        let mut manager = PluginManager::new(plugins_dir.clone());
        let installed = manager.install("example").expect("install from registry name");
        assert_eq!(installed.name, "example");
        assert_eq!(installed.version, "0.3.0");
        assert!(manager.get("example").is_some());
    }
}
