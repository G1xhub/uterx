//! Shell detection and default shell resolution.

/// Detect the user's default shell.
pub fn default_shell() -> String {
    #[cfg(windows)]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string())
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

/// Known shell types for integration features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Other(String),
}

impl ShellType {
    /// Detect shell type from the shell path.
    pub fn detect(shell_path: &str) -> Self {
        let lower = shell_path.to_lowercase();
        if lower.contains("bash") {
            Self::Bash
        } else if lower.contains("zsh") {
            Self::Zsh
        } else if lower.contains("fish") {
            Self::Fish
        } else if lower.contains("pwsh") || lower.contains("powershell") {
            Self::PowerShell
        } else if lower.contains("cmd") {
            Self::Cmd
        } else {
            Self::Other(shell_path.to_string())
        }
    }
}
