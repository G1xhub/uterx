//! Application configuration (loaded from TOML).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Terminal settings.
    pub terminal: TerminalConfig,
    /// UI settings.
    pub ui: UiConfig,
    /// Plugin settings.
    pub plugins: PluginConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    /// Shell to launch (auto-detected if empty).
    pub shell: Option<String>,
    /// Default number of columns.
    pub cols: u16,
    /// Default number of rows.
    pub rows: u16,
    /// Scrollback buffer size (lines).
    pub scrollback: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Font size in pixels.
    pub font_size: f32,
    /// Show tab bar.
    pub show_tab_bar: bool,
    /// Show status bar.
    pub show_status_bar: bool,
    /// Whether the first-launch tutorial has been shown.
    pub first_launch_done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginConfig {
    /// Enable plugin system.
    pub enabled: bool,
    /// Auto-update plugins on launch.
    pub auto_update: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            terminal: TerminalConfig::default(),
            ui: UiConfig::default(),
            plugins: PluginConfig::default(),
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: None,
            cols: 80,
            rows: 24,
            scrollback: 10_000,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            show_tab_bar: true,
            show_status_bar: true,
            first_launch_done: false,
        }
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_update: false,
        }
    }
}

impl AppConfig {
    /// Load config from a file path. Falls back to defaults if the file doesn't exist.
    /// Creates a default config file on first launch.
    pub fn load(path: Option<&str>) -> anyhow::Result<Self> {
        let config_path = match path {
            Some(p) => PathBuf::from(p),
            None => Self::default_config_path(),
        };

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let cfg: Self = toml::from_str(&content)?;
            Ok(cfg)
        } else {
            // First launch — create default config
            let cfg = Self::default();
            cfg.save_default(&config_path)?;
            Ok(cfg)
        }
    }

    /// Save the default config to disk so users have a template to edit.
    fn save_default(&self, path: &PathBuf) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        let header = "# uterx configuration\n\
                      # Edit this file to customize your terminal.\n\
                      # See PLAN.md for documentation.\n\n";
        std::fs::write(path, format!("{}{}", header, toml_str))?;
        tracing::info!("created default config at {}", path.display());
        Ok(())
    }

    /// Default config file location: ~/.uterx/config.toml
    pub fn default_config_path() -> PathBuf {
        dirs_path().join("config.toml")
    }

    /// Plugins directory: ~/.uterx/plugins/
    pub fn plugins_dir(&self) -> PathBuf {
        dirs_path().join("plugins")
    }

    /// Resolve the shell to use.
    pub fn shell(&self) -> String {
        self.terminal
            .shell
            .clone()
            .unwrap_or_else(|| uterx_platform::shell::default_shell())
    }

    /// Check if this is the first launch (tutorial not yet shown).
    pub fn is_first_launch(&self) -> bool {
        !self.ui.first_launch_done
    }

    /// Mark the tutorial as shown and persist to config file.
    pub fn mark_first_launch_done(&mut self) -> anyhow::Result<()> {
        self.ui.first_launch_done = true;
        let config_path = Self::default_config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        let header = "# uterx configuration\n\
                      # Edit this file to customize your terminal.\n\
                      # See PLAN.md for documentation.\n\n";
        std::fs::write(&config_path, format!("{}{}", header, toml_str))?;
        Ok(())
    }
}

/// Base directory for uterx data: ~/.uterx/
fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".uterx")
}
