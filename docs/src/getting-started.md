# Getting Started

This guide will help you install and configure uterx for the first time.

## Installation

### Prerequisites

- Rust 1.92 or later
- Windows 10/11 or Linux (Ubuntu 20.04+, Fedora 35+, etc.)
- For Linux: GTK development libraries

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/uterx.git
cd uterx

# Build the project
cargo build --release

# The binary will be available at target/release/uterx
```

### Installing via Cargo (when published)

```bash
cargo install uterx
```

## First Launch

When you run uterx for the first time, it will:

1. Create a default configuration file at `~/.uterx/config.toml`
2. Display the help overlay with keybindings
3. Create a new session with a single terminal pane

```bash
uterx
```

## Basic Usage

### Keybindings

| Keybinding | Action |
|------------|--------|
| `Ctrl+T` | Create new tab |
| `Ctrl+W` | Close current pane (2x press for editor) |
| `Ctrl+Tab` | Switch to next tab |
| `Ctrl+Shift+Tab` | Switch to previous tab |
| `Alt+H` | Split pane horizontally |
| `Alt+V` | Split pane vertically |
| `Alt+Left/Right` | Focus previous/next pane |
| `Alt+B` | Toggle broadcast mode |
| `Ctrl+P` | Open command palette |
| `F1` | Toggle help overlay |
| `Ctrl+E` | Toggle file browser sidebar |
| `Alt+F` | Toggle floating pane |
| `Ctrl+Shift+A` | Open UterxAI pane |

### Panes and Tabs

- **Create a new tab**: Press `Ctrl+T`
- **Split a pane**: Press `Alt+H` for horizontal or `Alt+V` for vertical split
- **Navigate panes**: Use `Alt+Left/Right` to focus different panes
- **Close a pane**: Press `Ctrl+W` (press twice to close editor)
- **Resize panes**: Drag the border between panes with your mouse
- **Reorder panes**: Drag a pane title bar and drop onto another pane

### Broadcast Mode

Broadcast mode sends your input to all panes in the current tab simultaneously:

1. Press `Alt+B` to toggle broadcast mode
2. Type your command - it appears in all panes
3. Press `Alt+B` again to disable broadcast mode

### File Browser

The file browser sidebar provides desktop-like file navigation:

1. Press `Ctrl+E` to toggle the file browser
2. Use arrow keys to navigate
3. Press `Enter` to open a file or expand/collapse a directory
4. Press `/` to search files (local or recursive)
5. Press `Right Arrow` on a directory to open it in a new tab

### Command Palette

The command palette provides quick access to all commands:

1. Press `Ctrl+P` to open the command palette
2. Type to filter commands
3. Use arrow keys to navigate
4. Press `Enter` to execute the selected command

### Floating Editor

The built-in editor provides syntax highlighting and vim-style editing:

1. Open a file from the file browser or search results
2. The editor opens as a floating pane
3. Press `i` to enter Insert mode
4. Press `Esc` to return to Normal mode
5. Press `Ctrl+S` to save
6. Drag the title bar to move the floating pane

### UterxAI

The multi-provider AI assistant pane:

1. Press `Ctrl+Shift+A` to open the UterxAI pane
2. Type your prompt and press `Enter`
3. The AI response streams into the pane

## Session Persistence

uterx automatically saves your session when you exit. On the next launch:

```bash
uterx  # Restores previous session
uterx --no-restore  # Start fresh without restoring
```

Sessions are stored in `~/.uterx/sessions/`.

## Configuration

The default configuration is created at `~/.uterx/config.toml`. See [Configuration](./configuration.md) for details.

## Troubleshooting

### Common Issues

**Terminal not rendering correctly**
- Ensure you're using a modern terminal emulator with true color support
- Check your terminal's font settings - Nerd Fonts are recommended for icons

**PTY spawn errors**
- Verify your shell is correctly configured
- On Windows, ensure PowerShell is available

**Plugin installation fails**
- Check network connectivity
- Verify the plugin URL is correct
- Ensure you have write permissions for `~/.uterx/plugins/`

## Next Steps

- [Configuration](./configuration.md) - Customize uterx to your needs
- [Plugins](./plugins.md) - Extend uterx with plugins
- [Architecture](./architecture.md) - Understand how uterx works
