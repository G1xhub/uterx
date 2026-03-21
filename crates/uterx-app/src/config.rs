//! Application configuration (loaded from TOML).

use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::env;
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
    /// AI/LLM settings.
    pub ai: AiConfig,
}

/// AI provider selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    Anthropic,
    Openai,
    Ollama,
    Custom,
    Zai,
    Kimi,
}

impl Default for AiProvider {
    fn default() -> Self {
        Self::Anthropic
    }
}

/// AI/LLM configuration.
///
/// Note: For security we avoid persisting long-lived API keys in plaintext config files.
/// The `api_key` field is kept in-memory but on save we will attempt to persist it to the OS
/// keyring and omit it from the on-disk TOML. On load we will try to retrieve the key from the
/// keyring if it's missing from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    /// AI provider: "anthropic", "openai", "ollama", "custom".
    pub provider: AiProvider,
    /// API key for cloud providers. For safety, keys are saved to the OS keyring on `save()`.
    /// The value written to disk will be empty. At runtime `AppConfig::load()` will try to
    /// read the key back from the keyring if present.
    pub api_key: String,
    /// Model name (e.g., "claude-opus-4-6", "gpt-4o", "llama3.2").
    pub model: String,
    /// Base URL for Ollama or custom OpenAI-compatible endpoints.
    pub base_url: String,
    /// CLI tool to launch for AI chat (auto-detected if empty).
    pub chat_command: String,
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

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: AiProvider::Anthropic,
            api_key: String::new(),
            model: "claude-opus-4-6".to_string(),
            base_url: String::new(),
            chat_command: String::new(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            terminal: TerminalConfig::default(),
            ui: UiConfig::default(),
            plugins: PluginConfig::default(),
            ai: AiConfig::default(),
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
            let mut cfg: Self = toml::from_str(&content)?;

            // If no API key in the config, attempt to read it from the OS keyring.
            if cfg.ai.api_key.is_empty() {
                if let Some(k) = read_ai_key_from_keyring()? {
                    cfg.ai.api_key = k;
                }
            }

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

        // For the default file we intentionally write an empty API key so users don't
        // accidentally commit or inspect a key in plaintext.
        let mut cfg_to_write = self.clone();
        cfg_to_write.ai.api_key = String::new();

        let toml_str = toml::to_string_pretty(&cfg_to_write)?;
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

    /// Save the current config to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::default_config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // If an AI API key is present in-memory, attempt to store it securely in the OS keyring
        // and ensure the on-disk TOML does not contain the plaintext key.
        if !self.ai.api_key.is_empty() {
            if let Err(e) = store_ai_key_in_keyring(&self.ai.api_key) {
                tracing::warn!("failed to save AI API key to keyring: {}", e);
            }
        }

        // Clone and clear the ai.api_key before writing to disk.
        let mut cfg_to_write = self.clone();
        cfg_to_write.ai.api_key = String::new();

        let toml_str = toml::to_string_pretty(&cfg_to_write)?;
        let header = "# uterx configuration\n\
                      # Edit this file to customize your terminal.\n\
                      # See PLAN.md for documentation.\n\n";
        std::fs::write(&config_path, format!("{}{}", header, toml_str))?;
        tracing::info!("config saved to {}", config_path.display());
        Ok(())
    }

    /// Check if this is the first launch (tutorial not yet shown).
    pub fn is_first_launch(&self) -> bool {
        !self.ui.first_launch_done
    }

    /// Mark the tutorial as shown and persist to config file.
    pub fn mark_first_launch_done(&mut self) -> anyhow::Result<()> {
        self.ui.first_launch_done = true;
        // Use `save()` so that any API key is persisted to the keyring (and not written to disk).
        self.save()?;
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

/// Helper: determine a username string for the keyring entry.
/// Prefer common env vars, fallback to 'default'.
fn keyring_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "default".to_string())
}

/// Read the AI API key from the OS keyring (if present).
fn read_ai_key_from_keyring() -> anyhow::Result<Option<String>> {
    // Service name is 'uterx-ai' and username is per-machine/user.
    let user = keyring_username();
    let entry = Entry::new("uterx-ai", &user);
    match entry.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(e) => {
            // Common case: not found — return Ok(None). Other errors are logged.
            tracing::debug!("keyring read error: {}", e);
            Ok(None)
        }
    }
}

/// Store the AI API key into the OS keyring. Overwrites any existing value.
fn store_ai_key_in_keyring(key: &str) -> anyhow::Result<()> {
    let user = keyring_username();
    let entry = Entry::new("uterx-ai", &user);
    entry.set_password(key)?;
    Ok(())
}

/// Delete the AI API key from the OS keyring if it exists.
pub fn delete_ai_key_from_keyring() -> anyhow::Result<()> {
    let user = keyring_username();
    let entry = Entry::new("uterx-ai", &user);
    match entry.delete_password() {
        Ok(_) => Ok(()),
        Err(e) => {
            // If the key doesn't exist, that's fine - we just log it
            tracing::debug!("keyring delete: {}", e);
            Ok(())
        }
    }
}
