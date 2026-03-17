//! Input handling and keyboard shortcut engine.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

/// An action that can be triggered by a keyboard shortcut.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    Quit,
    NewPane,
    ClosePane,
    NextPane,
    PrevPane,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    SplitHorizontal,
    SplitVertical,
    ToggleBroadcast,
    Search,
    Fullscreen,
    /// Show help / tutorial overlay.
    ShowHelp,
    /// Open command palette overlay.
    CommandPalette,
    /// Toggle file browser sidebar.
    ToggleFileBrowser,
    /// Toggle AI configuration sidebar.
    ToggleAiSidebar,
    /// Launch a new AI chat pane (without opening the sidebar first).
    NewAiChat,
    /// Toggle focused pane between tiled and floating.
    ToggleFloat,
    /// Open a folder in a new pane (cd into it).
    OpenFolder(String),
    /// Pass raw input to the active pane's PTY.
    RawInput(Vec<u8>),
}

/// A keybinding maps a key combo to an action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

/// The keyboard shortcut engine.
pub struct InputHandler {
    bindings: HashMap<KeyBinding, Action>,
}

impl InputHandler {
    /// Create with default keybindings.
    pub fn new() -> Self {
        let mut bindings = HashMap::new();

        // Ctrl+Q = Quit
        bindings.insert(
            KeyBinding { code: KeyCode::Char('q'), modifiers: KeyModifiers::CONTROL },
            Action::Quit,
        );
        // Ctrl+N = New pane (vertical split)
        bindings.insert(
            KeyBinding { code: KeyCode::Char('n'), modifiers: KeyModifiers::CONTROL },
            Action::NewPane,
        );
        // Ctrl+W = Close pane
        bindings.insert(
            KeyBinding { code: KeyCode::Char('w'), modifiers: KeyModifiers::CONTROL },
            Action::ClosePane,
        );
        // Ctrl+Tab = Next tab
        bindings.insert(
            KeyBinding { code: KeyCode::Tab, modifiers: KeyModifiers::CONTROL },
            Action::NextTab,
        );
        // Ctrl+T = New tab
        bindings.insert(
            KeyBinding { code: KeyCode::Char('t'), modifiers: KeyModifiers::CONTROL },
            Action::NewTab,
        );
        // Ctrl+F = Search
        bindings.insert(
            KeyBinding { code: KeyCode::Char('f'), modifiers: KeyModifiers::CONTROL },
            Action::Search,
        );
        // Alt+H = Split horizontal (top/bottom)
        bindings.insert(
            KeyBinding { code: KeyCode::Char('h'), modifiers: KeyModifiers::ALT },
            Action::SplitHorizontal,
        );
        // Alt+V = Split vertical (left/right)
        bindings.insert(
            KeyBinding { code: KeyCode::Char('v'), modifiers: KeyModifiers::ALT },
            Action::SplitVertical,
        );
        // Alt+Right = Focus next pane
        bindings.insert(
            KeyBinding { code: KeyCode::Right, modifiers: KeyModifiers::ALT },
            Action::NextPane,
        );
        // Alt+Left = Focus previous pane
        bindings.insert(
            KeyBinding { code: KeyCode::Left, modifiers: KeyModifiers::ALT },
            Action::PrevPane,
        );
        // Alt+B = Toggle broadcast mode
        bindings.insert(
            KeyBinding { code: KeyCode::Char('b'), modifiers: KeyModifiers::ALT },
            Action::ToggleBroadcast,
        );
        // Ctrl+Shift+Tab = Previous tab
        bindings.insert(
            KeyBinding { code: KeyCode::BackTab, modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT },
            Action::PrevTab,
        );
        // F1 = Show help overlay
        bindings.insert(
            KeyBinding { code: KeyCode::F(1), modifiers: KeyModifiers::NONE },
            Action::ShowHelp,
        );
        // Ctrl+P = Command palette
        bindings.insert(
            KeyBinding { code: KeyCode::Char('p'), modifiers: KeyModifiers::CONTROL },
            Action::CommandPalette,
        );
        // Ctrl+E = Toggle file browser
        bindings.insert(
            KeyBinding { code: KeyCode::Char('e'), modifiers: KeyModifiers::CONTROL },
            Action::ToggleFileBrowser,
        );
        // Alt+F = Toggle floating pane
        bindings.insert(
            KeyBinding { code: KeyCode::Char('f'), modifiers: KeyModifiers::ALT },
            Action::ToggleFloat,
        );
        // Alt+A = Toggle AI sidebar
        bindings.insert(
            KeyBinding { code: KeyCode::Char('a'), modifiers: KeyModifiers::ALT },
            Action::ToggleAiSidebar,
        );
        // Alt+I = New AI chat pane (quick-launch without sidebar)
        bindings.insert(
            KeyBinding { code: KeyCode::Char('i'), modifiers: KeyModifiers::ALT },
            Action::NewAiChat,
        );

        Self { bindings }
    }

    /// Look up an action for a key event.
    pub fn resolve(&self, event: &KeyEvent) -> Option<&Action> {
        let binding = KeyBinding {
            code: event.code,
            modifiers: event.modifiers,
        };
        self.bindings.get(&binding)
    }

    /// Register or override a keybinding.
    pub fn bind(&mut self, binding: KeyBinding, action: Action) {
        self.bindings.insert(binding, action);
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}
