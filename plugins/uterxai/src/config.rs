use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenAi,
    Claude,
    Gemini,
    Xai,
    Zai,
    Moonshot,
    Minimax,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub api_key_env: Option<String>,
    pub api_key_keyring: Option<String>,
    #[serde(default)]
    pub default_headers: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub streaming: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UterxAiConfig {
    pub default_provider: String,
    #[serde(default)]
    pub providers: Vec<ProviderProfile>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to parse uterxai config: {0}")]
    Parse(#[from] toml::de::Error),
}

impl UterxAiConfig {
    pub fn from_toml_str(input: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(input)?)
    }

    pub fn default_provider_profile(&self) -> Option<&ProviderProfile> {
        self.providers
            .iter()
            .find(|profile| profile.name == self.default_provider)
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_provider_config() {
        let config = r#"
default_provider = "claude"

[[providers]]
name = "claude"
kind = "claude"
base_url = "https://api.anthropic.com"
model = "claude-3-7-sonnet"
api_key_env = "ANTHROPIC_API_KEY"
streaming = true

[providers.default_headers]
anthropic-version = "2023-06-01"

[[providers]]
name = "openai"
kind = "open_ai"
base_url = "https://api.openai.com"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
streaming = true
"#;

        let parsed = UterxAiConfig::from_toml_str(config).expect("config should parse");
        assert_eq!(parsed.providers.len(), 2);

        let default_profile = parsed
            .default_provider_profile()
            .expect("default provider should exist");
        assert_eq!(default_profile.kind, ProviderKind::Claude);
    }
}
