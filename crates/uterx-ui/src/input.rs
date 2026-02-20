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
        // Ctrl+N = New pane
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
