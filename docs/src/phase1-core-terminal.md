# Phase 1: Core Terminal

**Status**: ✅ ~95% Complete

**Goal**: A fully functional terminal emulator with standards-compliant VTE parsing.

## Overview

Phase 1 implements the core terminal emulation functionality, providing a solid foundation for all subsequent features. This includes VTE/ANSI parsing, cell grid management, scrollback, and PTY integration.

## Components

### uterx-core

The core terminal emulation crate.

#### Key Types

- [`Cell`](../../crates/uterx-core/src/cell.rs) - Represents a single terminal cell
- [`CellAttributes`](../../crates/uterx-core/src/cell.rs) - Styling attributes for cells
- [`Grid`](../../crates/uterx-core/src/grid.rs) - Terminal cell grid with cursor
- [`Parser`](../../crates/uterx-core/src/parser.rs) - VTE/ANSI sequence parser
- [`Scrollback`](../../crates/uterx-core/src/scrollback.rs) - Scrollback history buffer

### uterx-platform

Platform abstraction for PTY and shell operations.

#### Key Types

- [`PtyProcess`](../../crates/uterx-platform/src/pty.rs) - PTY process management
- [`ShellType`](../../crates/uterx-platform/src/shell.rs) - Shell detection
- [`PtyReader`](../../crates/uterx-platform/src/pty.rs) - Non-blocking PTY reader

### uterx-ui

Terminal UI components.

#### Key Types

- [`TerminalView`](../../crates/uterx-ui/src/terminal_view.rs) - Renders grid as ratatui widget
- [`InputHandler`](../../crates/uterx-ui/src/input.rs) - Keybinding and input handling

## Completed Features

### 1.1 Workspace & Scaffolding

- ✅ Workspace `Cargo.toml` with all internal crates
- ✅ Directory structure for all 7 crates + xtask
- ✅ Workspace-wide dependency management
- ✅ `cargo-xtask` setup (build-plugins, dist, docs)

### 1.2 VTE Parser & Cell Grid

#### VTE Parser

The parser wraps the `vte` crate and implements the `Performer` trait to handle terminal actions.

```rust
pub struct Parser {
    parser: vte::Parser,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
        }
    }

    pub fn advance(&mut self, grid: &mut Grid, byte: u8) -> TerminalAction {
        let mut performer = GridPerformer::new(grid);
        self.parser.advance(&mut performer, byte);
        performer.into_action()
    }
}
```

#### Terminal Actions

The parser emits `TerminalAction` enum values:

```rust
pub enum TerminalAction {
    Print(char),
    Execute(u8),
    CSI(Vec<i64>, Vec<u8>, bool),
    ESC(Vec<u8>),
    OSC(Vec<u8>, Vec<u8>),
    DCS(Vec<u8>, Vec<u8>, Vec<u8>),
    None,
}
```

#### Cell and CellAttributes

Each cell contains content and styling:

```rust
pub struct Cell {
    pub c: char,
    pub width: CellWidth,
    pub attrs: CellAttributes,
}

pub struct CellAttributes {
    pub foreground: Color,
    pub background: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
    pub hidden: bool,
}
```

#### Grid Operations

The grid manages the terminal display:

```rust
impl Grid {
    pub fn new(width: u16, height: u16) -> Self;
    pub fn write_char(&mut self, col: u16, row: u16, c: char);
    pub fn newline(&mut self);
    pub fn scroll_up(&mut self, count: u16);
    pub fn resize(&mut self, width: u16, height: u16);
    pub fn clear(&mut self);
    pub fn set_scroll_region(&mut self, top: u16, bottom: u16);
}
```

#### CSI Sequences

Implemented CSI sequences:

| Sequence | Description |
|----------|-------------|
| CUU/CUD/CUF/CUB | Cursor movement (up/down/left/right) |
| CUP/HVP | Cursor position |
| ED | Erase display (modes 0, 1, 2, 3) |
| EL | Erase in line (modes 0, 1, 2) |
| CNL/CPL | Cursor next/previous line |
| CHA/VPA | Cursor horizontal/vertical absolute |
| DCH/ICH | Delete/insert characters |
| ECH | Erase characters |
| SU/SD | Scroll up/down |
| DSR | Device status report (cursor position) |
| SGR | Select Graphic Rendition |

#### SGR Parsing

Full SGR implementation for styling:

```rust
fn parse_sgr(params: &[i64], attrs: &mut CellAttributes) {
    for param in params {
        match param {
            0 => attrs.reset(),
            1 => attrs.bold = true,
            3 => attrs.italic = true,
            4 => attrs.underline = true,
            7 => attrs.inverse = true,
            9 => attrs.strikethrough = true,
            22 => attrs.bold = false,
            23 => attrs.italic = false,
            24 => attrs.underline = false,
            27 => attrs.inverse = false,
            29 => attrs.strikethrough = false,
            30..=37 => attrs.foreground = Color::Indexed((param - 30) as u8),
            38 => parse_extended_color(&mut iter, &mut attrs.foreground),
            39 => attrs.foreground = Color::Default,
            40..=47 => attrs.background = Color::Indexed((param - 40) as u8),
            48 => parse_extended_color(&mut iter, &mut attrs.background),
            49 => attrs.background = Color::Default,
            _ => {}
        }
    }
}
```

#### ESC Sequences

Implemented ESC sequences:

| Sequence | Description |
|----------|-------------|
| DECSC | Save cursor |
| DECRC | Restore cursor |
| RI | Reverse index |
| IND | Index |
| NEL | Next line |
| RIS | Reset to initial state |

#### OSC Sequences

Implemented OSC sequences:

| Sequence | Description |
|----------|-------------|
| OSC 0/2 | Set window title |

#### Scrollback Integration

When the grid scrolls up, evicted rows are pushed into the scrollback buffer:

```rust
impl Grid {
    pub fn scroll_up_region(&mut self, count: u16, scrollback: &mut Scrollback) {
        for _ in 0..count {
            if let Some(row) = self.remove_top_row() {
                scrollback.push(row);
            }
        }
    }
}
```

### 1.3 PTY & Shell

#### PtyProcess

Spawn and manage PTY processes:

```rust
pub struct PtyProcess {
    child: Box<dyn portable_pty::MasterPty + Send>,
    reader: PtyReader,
    writer: Box<dyn portable_pty::Child + Send>,
}

impl PtyProcess {
    pub fn spawn(shell: &str, cols: u16, rows: u16) -> Result<Self>;
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()>;
    pub fn reader(&self) -> &PtyReader;
    pub fn writer(&mut self) -> &mut dyn std::io::Write;
}
```

#### Shell Detection

Detect the default shell for the platform:

```rust
pub fn default_shell() -> String {
    if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
}

pub fn detect_shell_type(shell: &str) -> ShellType {
    match shell {
        s if s.contains("bash") => ShellType::Bash,
        s if s.contains("zsh") => ShellType::Zsh,
        s if s.contains("fish") => ShellType::Fish,
        s if s.contains("powershell") || s.contains("pwsh") => ShellType::PowerShell,
        s if s.contains("cmd") => ShellType::Cmd,
        _ => ShellType::Unknown,
    }
}
```

#### Non-blocking PTY IO

Async PTY reading using `spawn_blocking`:

```rust
pub struct PtyReader {
    reader: Box<dyn portable_pty::MasterPty + Send>,
}

impl PtyReader {
    pub fn read_async(&self) -> impl Future<Output = Result<Vec<u8>>> {
        let reader = self.reader.try_clone_reader()?;
        spawn_blocking(move || {
            let mut buffer = vec![0u8; 8192];
            let n = reader.read(&mut buffer)?;
            buffer.truncate(n);
            Ok(buffer)
        })
    }
}
```

### 1.4 TUI Layer

#### TerminalView Widget

Renders the grid as a ratatui widget:

```rust
pub struct TerminalView<'a> {
    pub grid: &'a Grid,
    pub scrollback: &'a Scrollback,
    pub scroll_offset: u16,
}

impl<'a> Widget for TerminalView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for row in 0..area.height {
            for col in 0..area.width {
                if let Some(cell) = self.grid.get(col, row) {
                    let style = cell.attrs.to_style();
                    buf.get_mut(area.x + col, area.y + row)
                        .set_char(cell.c)
                        .set_style(style);
                }
            }
        }
    }
}
```

#### InputHandler

Handles keybindings and input dispatch:

```rust
pub struct InputHandler {
    keybindings: HashMap<KeyEvent, Action>,
}

impl InputHandler {
    pub fn handle_key(&self, key: KeyEvent) -> Option<Action> {
        self.keybindings.get(&key).copied()
    }
}

pub enum Action {
    Quit,
    NewTab,
    ClosePane,
    SplitHorizontal,
    SplitVertical,
    FocusNext,
    FocusPrev,
    ToggleBroadcast,
    OpenCommandPalette,
    ToggleHelp,
    // ... more actions
}
```

## Remaining Work

### DCS Sequences

Device Control String sequences for advanced features:

- DECRQSS - Request status string
- Sixel - Graphics mode

### Unicode 17 Support

Full Unicode 17 support with:

- Grapheme cluster handling
- Wide character detection (wcwidth)
- Emoji rendering

### Ligature Detection

Detect and render ligatures for common character combinations:

```rust
fn detect_ligatures(row: &[Cell]) -> Vec<LigatureRun> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut pattern = String::new();

    for (i, cell) in row.iter().enumerate() {
        pattern.push(cell.c);
        if is_ligature_pattern(&pattern) {
            continue;
        } else {
            if pattern.len() > 1 {
                runs.push(LigatureRun { start, end: i, text: pattern.clone() });
            }
            pattern.clear();
            start = i;
        }
    }
    runs
}
```

### Tab Stops

Horizontal tab stop management:

- HTS (Horizontal Tab Set)
- TBC (Tab Clear)

### PTY Child Process Lifecycle

Full lifecycle management:

- Wait for process exit
- Kill process on close
- Get process status

## Testing

### Unit Tests

13 unit tests passing:

```bash
cargo test -p uterx-core
```

Tests cover:
- Cell attributes
- Grid operations
- Parser actions
- Scrollback buffer
- SGR parsing
- CSI sequences
- ESC sequences

## Related Documentation

- [Architecture](./architecture.md) - Overall architecture
- [Getting Started](./getting-started.md) - User guide
- [API Reference](./api-reference.md) - Detailed API docs
