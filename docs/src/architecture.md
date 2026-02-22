# Architecture

This chapter provides a deep dive into uterx's architecture, covering the design principles, crate organization, and key components.

## Design Principles

### Modularity

uterx is organized as a Cargo workspace with multiple crates, each with a clear responsibility:

- **Separation of Concerns**: Each crate handles a specific domain (rendering, UI, plugins, etc.)
- **Clear APIs**: Well-defined interfaces between crates
- **Testability**: Each crate can be tested independently

### Performance

- **GPU Rendering**: wgpu-based rendering for smooth 60fps performance
- **Dedicated IO Thread**: Separate thread for PTY reading to minimize jitter
- **Damage Tracking**: Only redraw changed cells for efficient rendering
- **Async Operations**: Tokio runtime for non-blocking I/O

### Extensibility

- **WASM Plugin System**: Plugins run in a sandboxed environment
- **Host API**: Well-defined APIs for plugins to interact with uterx
- **Permission Model**: Security-first approach with explicit permission grants

## Crate Overview

```
uterx/
├── crates/
│   ├── uterx-core/        # Terminal emulator
│   ├── uterx-render/      # GPU rendering
│   ├── uterx-mux/         # Multiplexer
│   ├── uterx-ui/          # TUI layer
│   ├── uterx-plugin/      # Plugin engine
│   ├── uterx-platform/    # OS abstraction
│   └── uterx-app/         # Main binary
```

### uterx-core

**Responsibility**: Terminal emulation, VTE parsing, cell grid management

**Key Types**:
- [`Cell`](../crates/uterx-core/src/cell.rs) - Represents a single cell with content and attributes
- [`Grid`](../crates/uterx-core/src/grid.rs) - Manages the terminal cell grid with cursor
- [`Parser`](../crates/uterx-core/src/parser.rs) - VTE/ANSI sequence parser
- [`Scrollback`](../crates/uterx-core/src/scrollback.rs) - Ring buffer for scrollback history

**Features**:
- Full SGR (Select Graphic Rendition) parsing
- CSI sequences for cursor movement and screen manipulation
- ESC sequences for special functions
- OSC sequences for window title
- Alternate screen buffer support
- Scroll regions
- Wide character and ligature support

### uterx-render

**Responsibility**: GPU-accelerated rendering via wgpu

**Key Types**:
- [`Renderer`](../crates/uterx-render/src/renderer.rs) - Main renderer with wgpu integration
- [`GlyphAtlas`](../crates/uterx-render/src/atlas.rs) - Texture atlas for glyph caching
- [`RenderThread`](../crates/uterx-render/src/render_thread.rs) - Dedicated render thread

**Features**:
- wgpu-based rendering (OpenGL/Linux, DirectX/Windows)
- Glyph atlas with fontdue rasterization
- Damage tracking for efficient updates
- Ligature rendering
- Cursor rendering (block, bar, underline)
- Selection highlighting
- Smooth scrolling

### uterx-mux

**Responsibility**: Pane, tab, and session management

**Key Types**:
- [`Pane`](../crates/uterx-mux/src/pane.rs) - Represents a terminal pane with PTY and parser
- [`Tab`](../crates/uterx-mux/src/tab.rs) - Manages panes within a tab
- [`Session`](../crates/uterx-mux/src/session.rs) - Manages tabs and session persistence
- [`Layout`](../crates/uterx-mux/src/layout.rs) - Layout engine for pane positioning

**Features**:
- Horizontal and vertical splits
- Floating panes
- Broadcast mode
- Pane reordering and resizing
- Session save/restore
- Layout variants: Single, HorizontalSplit, VerticalSplit, Tiled, Floating

### uterx-ui

**Responsibility**: TUI layer via ratatui and crossterm

**Key Types**:
- [`TerminalView`](../crates/uterx-ui/src/terminal_view.rs) - Renders terminal grid as ratatui widget
- [`InputHandler`](../crates/uterx-ui/src/input.rs) - Handles keybindings and input dispatch
- [`TabBar`](../crates/uterx-ui/src/widgets/tab_bar.rs) - Tab bar widget
- [`StatusBar`](../crates/uterx-ui/src/widgets/status_bar.rs) - Status bar widget
- [`CommandPalette`](../crates/uterx-ui/src/widgets/command_palette.rs) - Command palette overlay
- [`HelpOverlay`](../crates/uterx-ui/src/widgets/help_overlay.rs) - Help overlay
- [`FileBrowser`](../crates/uterx-ui/src/widgets/file_browser.rs) - File browser sidebar
- [`EditorWidget`](../crates/uterx-ui/src/widgets/editor.rs) - Floating editor pane

**Features**:
- Catppuccin-themed UI
- Overlay system for help and command palette
- File browser with tree view and search
- Floating editor with syntax highlighting
- Mouse event handling

### uterx-plugin

**Responsibility**: WASM plugin engine with wasmtime

**Key Types**:
- [`PluginManager`](../crates/uterx-plugin/src/manager.rs) - Manages plugin lifecycle
- [`PluginInstance`](../crates/uterx-plugin/src/runtime.rs) - Represents a running plugin
- [`PluginManifest`](../crates/uterx-plugin/src/manifest.rs) - Plugin metadata and configuration
- [`HostApi`](../crates/uterx-plugin/src/host_api.rs) - Host functions exposed to plugins

**Features**:
- WASM module loading via wasmtime
- Host APIs: ui, io, net, fs, platform
- Permission-based security model
- Plugin installation from URLs or local paths
- Auto-update mechanism
- Plugin registry integration

### uterx-platform

**Responsibility**: OS abstraction for cross-platform support

**Key Types**:
- [`PtyProcess`](../crates/uterx-platform/src/pty.rs) - PTY process management
- [`ShellType`](../crates/uterx-platform/src/shell.rs) - Shell detection and configuration
- [`BluetoothManager`](../crates/uterx-platform/src/bluetooth.rs) - Bluetooth operations

**Features**:
- PTY spawning and I/O via portable-pty
- Shell detection (Bash, Zsh, Fish, PowerShell, Cmd)
- Bluetooth abstraction (btleplug on both platforms)
- Non-blocking PTY I/O

### uterx-app

**Responsibility**: Main binary, event loop, configuration, CLI

**Key Types**:
- [`AppConfig`](../crates/uterx-app/src/config.rs) - Configuration management
- [`EventLoop`](../crates/uterx-app/src/event_loop.rs) - Main event loop

**Features**:
- CLI with clap (plugin subcommands)
- TOML configuration loading
- Async event loop with Tokio
- PTY spawn and management
- Keybinding dispatch
- Session auto-restore

## Data Flow

### Terminal Input Flow

```
User Input → InputHandler → Keybinding Dispatch → Action Execution
                                              ↓
                                         Pane/PTY → Parser → Grid → Render
```

### PTY Output Flow

```
PTY Output → Async Reader → Parser → Grid → Damage Tracking → Render Thread
```

### Plugin Communication Flow

```
Plugin → Host API Call → Permission Check → Runtime → uterx Internal → Response
```

## Event Loop

The main event loop uses Tokio's `select!` macro to handle multiple event sources:

1. **PTY Data Channel**: Receives output from PTY processes
2. **Crossterm Event Stream**: Receives keyboard and mouse events
3. **Render Thread**: Receives render completion signals

```rust
tokio::select! {
    Some(pty_event) = pty_rx.recv() => {
        // Handle PTY output
    }
    Some(crossterm_event) = event_stream.next() => {
        // Handle keyboard/mouse events
    }
    // ... other event sources
}
```

## Memory Management

### Grid and Scrollback

- The `Grid` stores the current terminal state in a 2D array of `Cell` instances
- When scrolling occurs, evicted rows are pushed into the `Scrollback` ring buffer
- The scrollback buffer has a configurable maximum capacity

### Glyph Atlas

- Glyphs are rasterized using fontdue and cached in a texture atlas
- The atlas uses a shelf-packing algorithm for efficient space utilization
- Frequently used glyphs remain in the cache, reducing rendering overhead

### Plugin Memory

- Plugins run in a WASM sandbox with isolated memory
- Host APIs copy data between plugin memory and host memory
- Permission checks prevent unauthorized memory access

## Threading Model

### Main Thread

- Event loop coordination
- UI rendering via ratatui
- Input handling

### Render Thread

- Dedicated thread for GPU rendering
- Receives grid snapshots via channel
- Processes frame geometry and issues draw calls

### PTY Reader Tasks

- Each pane has its own Tokio task for reading PTY output
- Tasks send data to the main loop via mpsc channels
- Non-blocking I/O ensures responsiveness

## Security Model

### Plugin Sandbox

- Plugins run as WASM modules with no direct system access
- All system operations go through host APIs
- Each host API checks permissions before execution

### Permission System

- Plugins declare required permissions in `plugin.toml`
- User grants/denies permissions on first use
- "Always grant" option remembers decisions

### Filesystem Sandbox

- Plugins have a dedicated sandbox directory at `~/.uterx/plugins/<name>/`
- Path traversal attacks are prevented
- Absolute paths are rejected

## Performance Optimizations

### Damage Tracking

- Only changed cells/rows are marked as dirty
- Geometry is rebuilt only for dirty regions
- Reduces GPU bandwidth and CPU overhead

### Ligature Rendering

- Ligature runs are detected and grouped
- Single instance for entire ligature sequence
- Reduces draw calls

### Smooth Scrolling

- Scroll offset is interpolated over multiple frames
- Sub-pixel positioning for smooth animation
- Configurable smoothing factor

## Testing Strategy

### Unit Tests

- Each crate has comprehensive unit tests
- Tests cover core functionality and edge cases
- Run with `cargo test -p <crate>`

### Integration Tests

- Cross-crate integration tests verify component interaction
- Session save/restore tests
- Plugin installation and runtime tests

### Benchmarks

- Performance benchmarks using criterion
- Measure rendering throughput, PTY I/O, plugin overhead
- Run with `cargo bench`

## Future Enhancements

- **WebAssembly Compilation**: Compile uterx itself to WASM for web deployment
- **Remote Sessions**: Support for connecting to remote uterx instances
- **Collaborative Features**: Multi-user pane sharing
- **Advanced Plugins**: More sophisticated plugin capabilities

## Related Documentation

- [Getting Started](./getting-started.md) - Installation and basic usage
- [Configuration](./configuration.md) - Configuration options
- [Plugins](./plugins.md) - Plugin development guide
- [API Reference](./api-reference.md) - Detailed API documentation
