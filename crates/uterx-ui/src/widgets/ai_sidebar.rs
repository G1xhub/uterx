//! AI sidebar widget — configure AI providers and start chat sessions.
//!
//! Displayed as a right-side panel (Catppuccin Mocha themed).
//! Supports Anthropic, OpenAI, Ollama, and custom OpenAI-compatible endpoints.
//! API keys are stored in ~/.uterx/config.toml (use the keyring plugin for prod).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

// ── Provider ────────────────────────────────────────────────────────────────

/// AI provider variants shown as tabs in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProvider {
    Anthropic = 0,
    OpenAi = 1,
    Ollama = 2,
    Custom = 3,
    ZAi = 4,
    Kimi = 5,
}

impl AiProvider {
    pub const ALL: &'static [AiProvider] = &[
        AiProvider::Anthropic,
        AiProvider::OpenAi,
        AiProvider::Ollama,
        AiProvider::Custom,
        AiProvider::ZAi,
        AiProvider::Kimi,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAi => "OpenAI",
            Self::Ollama => "Ollama",
            Self::Custom => "Custom",
            Self::ZAi => "z.Ai",
            Self::Kimi => "Kimi",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-opus-4-6",
            Self::OpenAi => "gpt-4o",
            Self::Ollama => "llama3.2",
            Self::Custom => "",
            Self::ZAi => "zai-default",
            Self::Kimi => "kimi-k2",
        }
    }

    pub fn env_var(self) -> &'static str {
        match self {
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::OpenAi => "OPENAI_API_KEY",
            Self::Ollama => "OPENAI_API_KEY",
            Self::Custom => "OPENAI_API_KEY",
            Self::ZAi => "ZAI_API_KEY",
            Self::Kimi => "KIMI_API_KEY",
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Anthropic,
            1 => Self::OpenAi,
            2 => Self::Ollama,
            3 => Self::Custom,
            4 => Self::ZAi,
            5 => Self::Kimi,
            _ => Self::Anthropic,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether this provider needs a Base URL field.
    pub fn needs_base_url(self) -> bool {
        matches!(self, Self::Ollama | Self::Custom | Self::ZAi | Self::Kimi)
    }

    /// Whether this provider typically needs an API key.
    pub fn needs_api_key(self) -> bool {
        !matches!(self, Self::Ollama)
    }
}

// ── Focused field ────────────────────────────────────────────────────────────

/// Which interactive element is currently focused in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiSidebarField {
    /// One of the provider tabs (index 0-3).
    Provider(usize),
    ApiKey,
    Model,
    BaseUrl,
    /// Save current config to disk (this will also attempt keyring storage if available)
    SaveButton,
    /// Toggle whether to persist API key to OS keyring (checkbox)
    SaveToKeyring,
    /// Test the configured provider + key by attempting a lightweight connection
    TestConnection,
    /// Start an interactive AI chat (launches the configured CLI)
    StartChatButton,
}

// ── State ────────────────────────────────────────────────────────────────────

/// Mutable state for the AI configuration sidebar.
#[derive(Debug, Clone)]
pub struct AiSidebarState {
    pub visible: bool,
    /// Sidebar width in terminal columns.
    pub width: u16,
    /// Currently selected provider.
    pub provider: AiProvider,
    /// API key (displayed masked).
    pub api_key: String,
    /// Model name.
    pub model: String,
    /// Base URL for Ollama / Custom.
    pub base_url: String,
    /// Which field currently has focus.
    pub focused_field: AiSidebarField,
    /// True when there are unsaved edits.
    pub dirty: bool,
    /// One-shot status message shown after save / error.
    pub status_msg: Option<String>,
    /// Transient test result for the Test Connection action.
    pub test_status: Option<String>,
    /// Whether the UI checkbox to save key to OS keyring is checked.
    pub save_to_keyring: bool,
    /// Set to true when the user confirms "Start Chat" — event loop acts on it.
    pub start_chat_requested: bool,
}

impl Default for AiSidebarState {
    fn default() -> Self {
        Self::new()
    }
}

impl AiSidebarState {
    pub fn new() -> Self {
        Self {
            visible: false,
            width: 38,
            provider: AiProvider::Anthropic,
            api_key: String::new(),
            model: "claude-opus-4-6".to_string(),
            base_url: String::new(),
            focused_field: AiSidebarField::ApiKey,
            dirty: false,
            status_msg: None,
            test_status: None,
            save_to_keyring: true, // sensible default: encourage secure storage
            start_chat_requested: false,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Populate fields from saved config values.
    pub fn load(&mut self, provider: AiProvider, api_key: &str, model: &str, base_url: &str) {
        self.provider = provider;
        self.api_key = api_key.to_string();
        self.model = model.to_string();
        self.base_url = base_url.to_string();
        self.dirty = false;
    }

    /// Mutable reference to the text of the currently focused field, if it is a text input.
    pub fn focused_text_mut(&mut self) -> Option<&mut String> {
        match &self.focused_field {
            AiSidebarField::ApiKey => Some(&mut self.api_key),
            AiSidebarField::Model => Some(&mut self.model),
            AiSidebarField::BaseUrl => Some(&mut self.base_url),
            _ => None,
        }
    }

    /// Move focus forward (Tab / Down).
    pub fn focus_next(&mut self) {
        self.focused_field = match &self.focused_field {
            AiSidebarField::Provider(_) => AiSidebarField::ApiKey,
            AiSidebarField::ApiKey => AiSidebarField::Model,
            AiSidebarField::Model => {
                if self.provider.needs_base_url() {
                    AiSidebarField::BaseUrl
                } else {
                    AiSidebarField::SaveButton
                }
            }
            AiSidebarField::BaseUrl => AiSidebarField::SaveButton,
            AiSidebarField::SaveButton => AiSidebarField::SaveToKeyring,
            AiSidebarField::SaveToKeyring => AiSidebarField::TestConnection,
            AiSidebarField::TestConnection => AiSidebarField::StartChatButton,
            AiSidebarField::StartChatButton => AiSidebarField::ApiKey,
        };
    }

    /// Move focus backward (Shift+Tab / Up).
    pub fn focus_prev(&mut self) {
        let pi = self.provider.index();
        self.focused_field = match &self.focused_field {
            AiSidebarField::Provider(_) => AiSidebarField::StartChatButton,
            AiSidebarField::ApiKey => AiSidebarField::Provider(pi),
            AiSidebarField::Model => AiSidebarField::ApiKey,
            AiSidebarField::BaseUrl => AiSidebarField::Model,
            AiSidebarField::SaveButton => {
                if self.provider.needs_base_url() {
                    AiSidebarField::BaseUrl
                } else {
                    AiSidebarField::Model
                }
            }
            AiSidebarField::SaveToKeyring => AiSidebarField::SaveButton,
            AiSidebarField::TestConnection => AiSidebarField::SaveToKeyring,
            AiSidebarField::StartChatButton => AiSidebarField::TestConnection,
        };
    }

    /// Cycle provider selection left / right using arrow keys when a provider tab is focused.
    pub fn provider_left(&mut self) {
        let i = self.provider.index();
        let new = if i == 0 {
            AiProvider::ALL.len() - 1
        } else {
            i - 1
        };
        self.provider = AiProvider::from_index(new);
        self.focused_field = AiSidebarField::Provider(new);
        self.dirty = true;
    }

    pub fn provider_right(&mut self) {
        let new = (self.provider.index() + 1) % AiProvider::ALL.len();
        self.provider = AiProvider::from_index(new);
        self.focused_field = AiSidebarField::Provider(new);
        self.dirty = true;
    }

    /// Push a character into the focused text field.
    pub fn push_char(&mut self, c: char) {
        if let Some(f) = self.focused_text_mut() {
            f.push(c);
            self.dirty = true;
        }
    }

    /// Remove the last character from the focused text field.
    pub fn pop_char(&mut self) {
        if let Some(f) = self.focused_text_mut() {
            f.pop();
            self.dirty = true;
        }
    }

    /// Build the shell command to launch an AI chat session.
    /// Returns a shell command string (e.g. `export ANTHROPIC_API_KEY='...'; opencode`).
    pub fn build_chat_command(&self) -> String {
        let model = if self.model.is_empty() {
            self.provider.default_model().to_string()
        } else {
            self.model.clone()
        };

        // Escape single quotes in the key to prevent shell injection
        let safe_key = self.api_key.replace('\'', "'\\''");

        let mut env_exports = match self.provider {
            AiProvider::Anthropic => {
                format!(
                    "export ANTHROPIC_API_KEY='{}'; export ANTHROPIC_MODEL='{}'",
                    safe_key, model
                )
            }
            AiProvider::OpenAi => {
                format!(
                    "export OPENAI_API_KEY='{}'; export OPENAI_MODEL='{}'",
                    safe_key, model
                )
            }
            AiProvider::Ollama => {
                let base = if self.base_url.is_empty() {
                    "http://localhost:11434".to_string()
                } else {
                    self.base_url.clone()
                };
                format!(
                    "export OLLAMA_HOST='{}'; export OPENAI_BASE_URL='{}/v1'; export OPENAI_MODEL='{}'",
                    base, base, model
                )
            }
            AiProvider::Custom => {
                format!(
                    "export OPENAI_API_KEY='{}'; export OPENAI_BASE_URL='{}'; export OPENAI_MODEL='{}'",
                    safe_key, self.base_url, model
                )
            }
            AiProvider::ZAi => {
                let base = if self.base_url.is_empty() {
                    "https://api.z.ai".to_string()
                } else {
                    self.base_url.clone()
                };
                format!(
                    "export ZAI_API_KEY='{}'; export OPENAI_BASE_URL='{}/v1'; export OPENAI_MODEL='{}'",
                    safe_key, base, model
                )
            }
            AiProvider::Kimi => {
                let base = if self.base_url.is_empty() {
                    "https://api.moonshot.cn".to_string()
                } else {
                    self.base_url.clone()
                };
                format!(
                    "export KIMI_API_KEY='{}'; export OPENAI_BASE_URL='{}/v1'; export OPENAI_MODEL='{}'",
                    safe_key, base, model
                )
            }
        };

        // Try to detect available AI CLI tools and launch the best one.
        // Priority: opencode > aichat > llm > sgpt > ollama (for Ollama provider)
        let launcher = match self.provider {
            AiProvider::Ollama => {
                format!(
                    "command -v opencode >/dev/null 2>&1 && opencode \
                     || command -v aichat >/dev/null 2>&1 && aichat \
                     || command -v ollama >/dev/null 2>&1 && ollama run '{}' \
                     || echo 'Install opencode, aichat, or ollama for AI chat'",
                    model
                )
            }
            _ => "command -v opencode >/dev/null 2>&1 && opencode \
                 || command -v aichat >/dev/null 2>&1 && aichat \
                 || command -v llm >/dev/null 2>&1 && llm chat \
                 || command -v sgpt >/dev/null 2>&1 && sgpt --chat \
                 || echo 'Install opencode or aichat for AI chat (https://opencode.ai)'"
                .to_string(),
        };

        format!("{}; {}", env_exports, launcher)
    }
}

// ── Widget ────────────────────────────────────────────────────────────────────

/// Ratatui widget that renders the AI sidebar.
pub struct AiSidebarWidget<'a> {
    state: &'a AiSidebarState,
    focused: bool,
}

impl<'a> AiSidebarWidget<'a> {
    pub fn new(state: &'a AiSidebarState, focused: bool) -> Self {
        Self { state, focused }
    }
}

impl<'a> Widget for AiSidebarWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ── Catppuccin Mocha palette ──────────────────────────────────────
        let bg = Color::Rgb(30, 30, 46); // base
        let surface = Color::Rgb(49, 50, 68); // surface0
        let surface1 = Color::Rgb(69, 71, 90); // surface1
        let border = Color::Rgb(203, 166, 247); // mauve  (distinct from blue used by file browser)
        let text_col = Color::Rgb(205, 214, 244); // text
        let subtext = Color::Rgb(166, 173, 200); // subtext0
        let green = Color::Rgb(166, 227, 161); // green
        let yellow = Color::Rgb(249, 226, 175); // yellow
        let peach = Color::Rgb(250, 179, 135); // peach
        let dim = Color::Rgb(88, 91, 112); // overlay0
        let sky = Color::Rgb(137, 220, 235); // sky

        // ── Fill background ───────────────────────────────────────────────
        let bg_style = Style::default().bg(bg);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        // ── Left border line ──────────────────────────────────────────────
        let border_style = Style::default().fg(border).bg(bg);
        for y in area.y..area.y + area.height {
            buf.set_string(area.x, y, "│", border_style);
        }

        let inner_x = area.x + 2;
        let inner_w = area.width.saturating_sub(3) as usize;
        let sep_str: String = "─".repeat(area.width.saturating_sub(1) as usize);
        let mut y = area.y;

        // ── Title ─────────────────────────────────────────────────────────
        let title_style = Style::default()
            .fg(border)
            .bg(bg)
            .add_modifier(Modifier::BOLD);
        let title = " AI Configuration";
        buf.set_string(inner_x, y, title, title_style);
        y += 1;

        // ── Separator ─────────────────────────────────────────────────────
        buf.set_string(area.x + 1, y, &sep_str, Style::default().fg(dim).bg(bg));
        y += 1;

        // ── Provider tabs ─────────────────────────────────────────────────
        let label_style = Style::default().fg(subtext).bg(bg);
        buf.set_string(inner_x, y, "Provider:", label_style);
        y += 1;

        let mut px = inner_x;
        for prov in AiProvider::ALL {
            let is_selected = *prov == self.state.provider;
            let is_focused = self.focused
                && matches!(&self.state.focused_field, AiSidebarField::Provider(j) if *j == prov.index());

            let (fg, tab_bg) = if is_selected {
                (text_col, surface1)
            } else {
                (dim, bg)
            };
            let mut style = Style::default().fg(fg).bg(tab_bg);
            if is_focused {
                style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            } else if is_selected {
                style = style.add_modifier(Modifier::BOLD);
            }

            let tab = format!("[{}]", prov.label());
            let tab_w = tab.len() as u16;

            // Wrap to next line if it doesn't fit
            if px + tab_w > area.x + area.width - 1 {
                px = inner_x;
                y += 1;
            }
            buf.set_string(px, y, &tab, style);
            px += tab_w + 1;
        }
        y += 2;

        // ── Separator ─────────────────────────────────────────────────────
        buf.set_string(area.x + 1, y, &sep_str, Style::default().fg(dim).bg(bg));
        y += 1;

        // ── API Key field ─────────────────────────────────────────────────
        let env_label = format!("API Key ({}):", self.state.provider.env_var());
        buf.set_string(inner_x, y, &env_label, label_style);
        y += 1;

        {
            let is_foc = self.focused && self.state.focused_field == AiSidebarField::ApiKey;
            let (ind, fg, field_bg) = if is_foc {
                ("▸ ", green, surface)
            } else {
                ("  ", dim, bg)
            };
            let masked = if self.state.api_key.is_empty() {
                if !self.state.provider.needs_api_key() {
                    "(not required for Ollama)".to_string()
                } else {
                    "(not set)".to_string()
                }
            } else if is_foc {
                // Show last 4 chars unmasked so user can verify they typed correctly
                let n = self.state.api_key.len();
                let visible = n.min(4);
                format!(
                    "{}{}",
                    "●".repeat(n.saturating_sub(visible)),
                    &self.state.api_key[n - visible..]
                )
            } else {
                let n = self.state.api_key.len().min(22);
                "●".repeat(n)
            };
            let content = format!("{}{}", ind, masked);
            let padded = format!("{:<width$}", content, width = inner_w);
            let s = Style::default().fg(fg).bg(field_bg);
            buf.set_string(inner_x, y, &padded[..padded.len().min(inner_w)], s);
        }
        y += 2;

        // ── Model field ───────────────────────────────────────────────────
        buf.set_string(inner_x, y, "Model:", label_style);
        y += 1;

        {
            let is_foc = self.focused && self.state.focused_field == AiSidebarField::Model;
            let (ind, fg, field_bg) = if is_foc {
                ("▸ ", green, surface)
            } else {
                ("  ", subtext, bg)
            };
            let model_text = if self.state.model.is_empty() {
                self.state.provider.default_model()
            } else {
                &self.state.model
            };
            let content = format!("{}{}", ind, model_text);
            let padded = format!("{:<width$}", content, width = inner_w);
            let s = Style::default().fg(fg).bg(field_bg);
            buf.set_string(inner_x, y, &padded[..padded.len().min(inner_w)], s);
        }
        y += 2;

        // ── Base URL field (Ollama / Custom only) ─────────────────────────
        if self.state.provider.needs_base_url() {
            buf.set_string(inner_x, y, "Base URL:", label_style);
            y += 1;

            let is_foc = self.focused && self.state.focused_field == AiSidebarField::BaseUrl;
            let (ind, fg, field_bg) = if is_foc {
                ("▸ ", green, surface)
            } else {
                ("  ", subtext, bg)
            };
            let url_text = if self.state.base_url.is_empty() {
                match self.state.provider {
                    AiProvider::Ollama => "http://localhost:11434",
                    AiProvider::ZAi => "https://api.z.ai",
                    AiProvider::Kimi => "https://api.moonshot.cn",
                    _ => "(not set)",
                }
            } else {
                &self.state.base_url
            };
            let content = format!("{}{}", ind, url_text);
            let padded = format!("{:<width$}", content, width = inner_w);
            let s = Style::default().fg(fg).bg(field_bg);
            buf.set_string(inner_x, y, &padded[..padded.len().min(inner_w)], s);
            y += 2;
        }

        // ── Separator ─────────────────────────────────────────────────────
        buf.set_string(area.x + 1, y, &sep_str, Style::default().fg(dim).bg(bg));
        y += 1;

        // ── Save button ───────────────────────────────────────────────────
        {
            let is_foc = self.focused && self.state.focused_field == AiSidebarField::SaveButton;
            let btn_style = if is_foc {
                Style::default()
                    .fg(Color::Rgb(30, 30, 46))
                    .bg(green)
                    .add_modifier(Modifier::BOLD)
            } else if self.state.dirty {
                Style::default().fg(yellow).bg(bg)
            } else {
                Style::default().fg(green).bg(bg)
            };
            let label = if self.state.dirty {
                "[ Save Config * ]"
            } else {
                "[ Save Config   ]"
            };
            buf.set_string(inner_x, y, label, btn_style);
        }
        y += 1;

        // ── Save-to-keyring checkbox ───────────────────────────────────────
        {
            let is_foc = self.focused && self.state.focused_field == AiSidebarField::SaveToKeyring;
            let checked = self.state.save_to_keyring;
            let cb_style = if is_foc {
                Style::default()
                    .fg(Color::Rgb(30, 30, 46))
                    .bg(surface)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(subtext).bg(bg)
            };
            let label = if checked {
                "[x] Save key to OS keyring "
            } else {
                "[ ] Save key to OS keyring "
            };
            let padded = format!("{:<width$}", label, width = inner_w);
            buf.set_string(inner_x, y, &padded[..padded.len().min(inner_w)], cb_style);
        }
        y += 1;

        // ── Test Connection button ────────────────────────────────────────
        {
            let is_foc = self.focused && self.state.focused_field == AiSidebarField::TestConnection;
            let btn_style = if is_foc {
                Style::default()
                    .fg(Color::Rgb(30, 30, 46))
                    .bg(Color::Rgb(137, 220, 235)) // sky as highlight
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(137, 220, 235)).bg(bg)
            };
            buf.set_string(inner_x, y, "[ Test Connection ]", btn_style);
        }
        y += 1;

        // ── Start Chat button ─────────────────────────────────────────────
        {
            let is_foc =
                self.focused && self.state.focused_field == AiSidebarField::StartChatButton;
            let btn_style = if is_foc {
                Style::default()
                    .fg(Color::Rgb(30, 30, 46))
                    .bg(peach)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(peach).bg(bg)
            };
            buf.set_string(inner_x, y, "[ Start AI Chat ]", btn_style);
        }
        y += 2;

        // ── Status message ────────────────────────────────────────────────
        if let Some(ref msg) = self.state.status_msg {
            let msg_style = Style::default().fg(sky).bg(bg);
            let truncated = if msg.len() > inner_w {
                &msg[..inner_w]
            } else {
                msg
            };
            buf.set_string(inner_x, y, truncated, msg_style);
            y += 1;
        }

        // ── Hint line (bottom) ────────────────────────────────────────────
        let hint_y = area.y + area.height.saturating_sub(2);
        if hint_y > y {
            let hint = "Tab/↑↓:nav  ←→:provider  Esc:close";
            let hint_style = Style::default().fg(dim).bg(bg);
            let truncated = if hint.len() > inner_w {
                &hint[..inner_w]
            } else {
                hint
            };
            buf.set_string(inner_x, hint_y, truncated, hint_style);
        }

        // Suppress unused variable warning
        let _ = sky;
    }
}
