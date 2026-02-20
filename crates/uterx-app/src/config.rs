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
            Ok(Self::default())
        }
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
}

/// Base directory for uterx data: ~/.uterx/
fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".uterx")
}
