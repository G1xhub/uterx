# Introduction

**uterx** is a "Terminal Desktop" — a highly modular, extensible terminal emulator and multiplexer that runs natively on Windows and Linux. It combines the performance of Ghostty (GPU-accelerated rendering, dedicated IO threads) with the multiplexing capabilities of Zellij (panes, tabs, sessions, WASM plugins).

## What is uterx?

uterx is more than just a terminal emulator — it's a complete terminal workspace designed for power users who need:

- **High Performance**: GPU-accelerated rendering at 60fps, dedicated IO threads for low jitter
- **Desktop-like UX**: Drag-and-drop pane management, floating windows, persistent sessions
- **Extensibility**: WASM-based plugin system with sandboxed execution
- **Cross-Platform**: Native support for Windows and Linux

## Key Features

### Terminal Emulation
- Standards-compliant VTE/ANSI parsing
- Unicode 17 support with ligature rendering
- Scrollback buffer with search
- Alternate screen buffer for full-screen applications

### Multiplexer
- Multiple panes with horizontal/vertical splits
- Tab management with session persistence
- Broadcast mode for synchronized input
- Drag-and-drop pane reordering and resizing

### GPU Rendering
- wgpu-based rendering (OpenGL on Linux, DirectX on Windows)
- Glyph atlas with fontdue rasterization
- Damage tracking for efficient updates
- Smooth scrolling and selection highlighting

### Plugin System
- WASM plugins via wasmtime runtime
- Host APIs for UI, IO, networking, filesystem, and platform access
- Permission-based security model
- Plugin repository with automatic updates

### Integrated Tools
- Built-in text/code editor with syntax highlighting
- File browser with tree view
- Command palette with fuzzy search
- Multi-provider AI assistant (UterxAI)

## Architecture

uterx is organized as a Cargo workspace with multiple crates:

```
uterx/
├── crates/
│   ├── uterx-core/        # Terminal emulator: VTE parsing, cell grid, ANSI sequences
│   ├── uterx-render/      # GPU rendering via wgpu
│   ├── uterx-mux/         # Multiplexer: panes, tabs, sessions, layouts
│   ├── uterx-ui/          # TUI layer via ratatui + crossterm
│   ├── uterx-plugin/      # WASM plugin engine via wasmtime
│   ├── uterx-platform/    # OS abstraction (pty, bluetooth, networking)
│   └── uterx-app/         # Main binary, event loop, config, CLI
├── plugins/               # Example WASM plugins
└── docs/                  # This documentation
```

## Tech Stack

| Component | Technology |
|-----------|------------|
| Language | Rust 1.92+ |
| Async Runtime | Tokio |
| Rendering | wgpu (OpenGL/Linux, DirectX/Windows) |
| TUI | Ratatui + Crossterm |
| Plugins | Wasmtime |
| Build | Cargo + cargo-xtask |

## Platforms

- **Windows**: Native WinAPI for Bluetooth and networking
- **Linux**: GTK for UI, BlueZ for Bluetooth

## License

[Specify your license here]

## Next Steps

- [Getting Started](./getting-started.md) - Installation and first steps
- [Architecture](./architecture.md) - Deep dive into the architecture
- [Configuration](./configuration.md) - Customizing uterx
- [Plugins](./plugins.md) - Creating and using plugins
