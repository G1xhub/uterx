# API Reference

This chapter provides detailed API documentation for uterx's public interfaces.

## Table of Contents

- [Core API](#core-api)
- [Multiplexer API](#multiplexer-api)
- [Plugin Host API](#plugin-host-api)
- [UI API](#ui-api)

## Core API

### uterx-core

#### Cell

```rust
pub struct Cell {
    pub c: char,
    pub width: CellWidth,
    pub attrs: CellAttributes,
}

impl Cell {
    pub fn new(c: char) -> Self;
    pub fn with_attrs(c: char, attrs: CellAttributes) -> Self;
}
```

#### CellAttributes

```rust
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

impl CellAttributes {
    pub fn new() -> Self;
    pub fn reset(&mut self);
    pub fn to_style(&self) -> Style;
}
```

#### Color

```rust
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    pub fn to_rgba(&self) -> [f32; 4];
}
```

#### Grid

```rust
pub struct Grid {
    pub width: u16,
    pub height: u16,
    pub cursor: Cursor,
    // ...
}

impl Grid {
    pub fn new(width: u16, height: u16) -> Self;
    pub fn resize(&mut self, width: u16, height: u16);
    pub fn write_char(&mut self, col: u16, row: u16, c: char);
    pub fn get(&self, col: u16, row: u16) -> Option<&Cell>;
    pub fn clear(&mut self);
    pub fn newline(&mut self);
    pub fn scroll_up(&mut self, count: u16);
    pub fn set_scroll_region(&mut self, top: u16, bottom: u16);
}
```

#### Parser

```rust
pub struct Parser {
    // ...
}

impl Parser {
    pub fn new() -> Self;
    pub fn advance(&mut self, grid: &mut Grid, byte: u8) -> TerminalAction;
}
```

#### Scrollback

```rust
pub struct Scrollback {
    // ...
}

impl Scrollback {
    pub fn new(max_lines: usize) -> Self;
    pub fn push(&mut self, row: Vec<Cell>);
    pub fn get(&self, index: usize) -> Option<&[Cell]>;
    pub fn len(&self) -> usize;
}
```

## Multiplexer API

### uterx-mux

#### Pane

```rust
pub struct Pane {
    pub id: PaneId,
    pub title: String,
    pub pty: Option<PtyProcess>,
    pub parser: Parser,
    pub grid: Grid,
    pub scrollback: Scrollback,
    pub rect: Rect,
    pub is_focused: bool,
    pub is_floating: bool,
    pub float_pos: (u16, u16),
    pub float_size: (u16, u16),
}

impl Pane {
    pub fn new(id: PaneId, shell: &str, cols: u16, rows: u16) -> Result<Self>;
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()>;
    pub fn close(&mut self) -> Result<()>;
}
```

#### Tab

```rust
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub panes: Vec<Pane>,
    pub layout: Layout,
    pub focused_pane_id: Option<PaneId>,
    pub broadcast: bool,
}

impl Tab {
    pub fn new(id: TabId, shell: &str, cols: u16, rows: u16) -> Result<Self>;
    pub fn split_horizontal(&mut self, shell: &str, cols: u16, rows: u16) -> Result<PaneId>;
    pub fn split_vertical(&mut self, shell: &str, cols: u16, rows: u16) -> Result<PaneId>;
    pub fn remove_pane(&mut self, pane_id: PaneId) -> Result<()>;
    pub fn focus_next(&mut self);
    pub fn focus_prev(&mut self);
    pub fn toggle_broadcast(&mut self);
    pub fn toggle_float(&mut self);
}
```

#### Session

```rust
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub tabs: Vec<Tab>,
    pub active_tab_id: Option<TabId>,
}

impl Session {
    pub fn new(name: String) -> Self;
    pub fn create_tab(&mut self, shell: &str, cols: u16, rows: u16) -> Result<TabId>;
    pub fn close_tab(&mut self, tab_id: TabId) -> Result<()>;
    pub fn next_tab(&mut self);
    pub fn prev_tab(&mut self);
    pub fn save(&self, path: &Path) -> Result<()>;
    pub fn load(path: &Path, shell: &str, cols: u16, rows: u16) -> Result<Self>;
}
```

#### Layout

```rust
pub enum Layout {
    Single,
    HorizontalSplit { split_ratio: f32 },
    VerticalSplit { split_ratio: f32 },
    Tiled { rows: u16, cols: u16 },
    Floating,
}

impl Layout {
    pub fn compute_rects(&self, area: Rect, panes: &[Pane]) -> Vec<Rect>;
}
```

## Plugin Host API

### uterx_ui

#### draw_text

```rust
extern "C" fn uterx_ui_draw_text(
    text_ptr: u32,
    text_len: u32,
    x: u32,
    y: u32,
) -> i32;
```

Draws text at the specified position in the plugin's pane.

**Parameters**:
- `text_ptr`: Pointer to UTF-8 encoded text in plugin memory
- `text_len`: Length of text in bytes
- `x`: X position (column)
- `y`: Y position (row)

**Returns**: 0 on success, -1 on error

**Permission**: `ui`

#### get_size

```rust
extern "C" fn uterx_ui_get_size(
    pane_id: u64,
) -> (u16, u16);
```

Gets the size of a pane.

**Parameters**:
- `pane_id`: ID of the pane

**Returns**: Tuple of (width, height)

**Permission**: `ui`

#### create_overlay

```rust
extern "C" fn uterx_ui_create_overlay(
    title_ptr: u32,
    title_len: u32,
) -> u64;
```

Creates a new overlay window.

**Parameters**:
- `title_ptr`: Pointer to overlay title in plugin memory
- `title_len`: Length of title in bytes

**Returns**: Overlay ID

**Permission**: `ui`

### uterx_io

#### read

```rust
extern "C" fn uterx_io_read(
    ptr: u32,
    len: u32,
) -> u32;
```

Reads input from the host.

**Parameters**:
- `ptr`: Pointer to buffer in plugin memory
- `len`: Maximum bytes to read

**Returns**: Number of bytes read

**Permission**: `io`

#### write

```rust
extern "C" fn uterx_io_write(
    ptr: u32,
    len: u32,
) -> i32;
```

Writes output to the host.

**Parameters**:
- `ptr`: Pointer to data in plugin memory
- `len`: Number of bytes to write

**Returns**: 0 on success, -1 on error

**Permission**: `io`

### uterx_net

#### http_get

```rust
extern "C" fn uterx_net_http_get(
    url_ptr: u32,
    url_len: u32,
    resp_ptr: u32,
    resp_len: u32,
) -> i32;
```

Performs an HTTP GET request.

**Parameters**:
- `url_ptr`: Pointer to URL in plugin memory
- `url_len`: Length of URL in bytes
- `resp_ptr`: Pointer to response buffer in plugin memory
- `resp_len`: Maximum response size

**Returns**: HTTP status code, or -1 on error

**Permission**: `net`

#### http_post

```rust
extern "C" fn uterx_net_http_post(
    url_ptr: u32,
    url_len: u32,
    headers_ptr: u32,
    headers_len: u32,
    body_ptr: u32,
    body_len: u32,
    resp_ptr: u32,
    resp_len: u32,
) -> i32;
```

Performs an HTTP POST request.

**Parameters**:
- `url_ptr`: Pointer to URL in plugin memory
- `url_len`: Length of URL in bytes
- `headers_ptr`: Pointer to headers JSON in plugin memory
- `headers_len`: Length of headers in bytes
- `body_ptr`: Pointer to request body in plugin memory
- `body_len`: Length of body in bytes
- `resp_ptr`: Pointer to response buffer in plugin memory
- `resp_len`: Maximum response size

**Returns**: HTTP status code, or -1 on error

**Permission**: `net`

#### tcp_connect

```rust
extern "C" fn uterx_net_tcp_connect(
    host_ptr: u32,
    host_len: u32,
    port: u16,
) -> u32;
```

Establishes a TCP connection.

**Parameters**:
- `host_ptr`: Pointer to hostname in plugin memory
- `host_len`: Length of hostname in bytes
- `port`: Port number

**Returns**: Socket ID, or 0 on error

**Permission**: `net`

### uterx_fs

#### read_file

```rust
extern "C" fn uterx_fs_read_file(
    path_ptr: u32,
    path_len: u32,
    resp_ptr: u32,
    resp_len: u32,
) -> i32;
```

Reads a file from the plugin's sandbox.

**Parameters**:
- `path_ptr`: Pointer to path in plugin memory
- `path_len`: Length of path in bytes
- `resp_ptr`: Pointer to response buffer in plugin memory
- `resp_len`: Maximum response size

**Returns**: Number of bytes read, or -1 on error

**Permission**: `fs`

#### write_file

```rust
extern "C" fn uterx_fs_write_file(
    path_ptr: u32,
    path_len: u32,
    data_ptr: u32,
    data_len: u32,
) -> i32;
```

Writes data to a file in the plugin's sandbox.

**Parameters**:
- `path_ptr`: Pointer to path in plugin memory
- `path_len`: Length of path in bytes
- `data_ptr`: Pointer to data in plugin memory
- `data_len`: Number of bytes to write

**Returns**: 0 on success, -1 on error

**Permission**: `fs`

### uterx_platform

#### bt_scan

```rust
extern "C" fn uterx_platform_bt_scan() -> u32;
```

Scans for Bluetooth devices.

**Returns**: Number of devices found

**Permission**: `platform`

#### keyring_get

```rust
extern "C" fn uterx_platform_keyring_get(
    key_ptr: u32,
    key_len: u32,
    value_ptr: u32,
    value_len: u32,
) -> i32;
```

Gets a value from the system keyring.

**Parameters**:
- `key_ptr`: Pointer to key in plugin memory
- `key_len`: Length of key in bytes
- `value_ptr`: Pointer to value buffer in plugin memory
- `value_len`: Maximum value size

**Returns**: Number of bytes read, or -1 on error

**Permission**: `platform`

## UI API

### TerminalView

```rust
pub struct TerminalView<'a> {
    pub grid: &'a Grid,
    pub scrollback: &'a Scrollback,
    pub scroll_offset: u16,
}

impl<'a> Widget for TerminalView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer);
}
```

### InputHandler

```rust
pub struct InputHandler {
    keybindings: HashMap<KeyEvent, Action>,
}

impl InputHandler {
    pub fn new() -> Self;
    pub fn handle_key(&self, key: KeyEvent) -> Option<Action>;
    pub fn register_keybinding(&mut self, key: KeyEvent, action: Action);
}
```

### Action

```rust
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
    ToggleFileBrowser,
    ToggleFloat,
    OpenUterxAI,
    // ...
}
```

## Related Documentation

- [Architecture](./architecture.md) - Overall architecture
- [Plugins](./plugins.md) - Plugin development guide
- [Phase 4](./phase4-plugin-system.md) - Plugin system details
