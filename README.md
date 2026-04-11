# Super-Terminal

A modular, GPU-accelerated terminal emulator and multiplexer for **Windows** and **Linux**.

Super-Terminal pairs GPU-accelerated terminal emulation with a flexible pane and session manager, creating a **"Terminal Desktop"** where panes can be freely moved, resized, grouped, or run independently — similar to a graphical desktop environment. An open WebAssembly plugin ecosystem extends it far beyond classic shell work.

---

## Features

### Terminal Emulation
- Full ANSI/ECMA-48 sequence support, ligatures, true color, and Unicode
- GPU-accelerated rendering (OpenGL on Linux, DirectX on Windows) via `wgpu`
- Dedicated I/O thread to prevent jitter under heavy load

### Pane Management
- Drag-and-drop, resize, stack, or float panes
- Run panes independently or synchronize them via **broadcast mode**
- Persistent sessions with restore-on-restart and multi-client collaboration

### UI & Configuration
- Tab bar, split screens, mouse-driven resizing
- Configuration via YAML or TOML

### Built-in Tools
- Shell integration: Bash, Zsh, PowerShell
- Cross-pane search
- Crash reporting

---

## Plugin System

Plugins are distributed as **WebAssembly modules** and loaded at runtime — no recompilation required. They run inside a WASM sandbox with no direct OS access unless explicitly permitted.

### Managing Plugins

```bash
# Install a plugin
super-terminal plugin add <url-or-name>

# List, update, or remove
super-terminal plugin list
super-terminal plugin update <name>
super-terminal plugin remove <name>
```

### Writing Plugins

Plugins can be written in any language that compiles to WASM (Rust, JS, …). A hook-based API is provided (`on-load`, `on-command`, etc.). Plugins load into dedicated panes or overlays and can access app APIs for I/O, networking, and UI.

### Available Plugins

| Plugin | Description |
|--------|-------------|
| **Bluetooth Messaging** | Scan/connect devices, send/receive messages, file transfer via OS Bluetooth APIs (BlueZ / Windows Bluetooth API) |
| **Mesh & WLAN Messaging** | Peer-to-peer encrypted chat over mesh/WLAN networks with offline support |
| **Midnight Blockchain** | Wallet management and private transactions on the Midnight sidechain (Cardano, ZK-SNARKs, NIGHT token) |
| **Text/Code Editor** | Syntax highlighting, auto-complete, multi-file editing — powered by Tree-sitter |
| **File Sharing** | Encrypted P2P file sharing (IPFS integration), drag-and-drop |
| **Network Tools** | Ping, traceroute, port scanning, WiFi analysis with visual graphs |
| **SSH Tools** | Multi-session SSH, key management, tunneling in dedicated panes |
| **Converter** | Currencies, units, file formats, crypto conversions — extensible via sub-plugins |

---

## Architecture

```
┌─────────────────────────────────────────────┐
│                  Platform Layer              │
│           (WinAPI / GTK / TTY)              │
├──────────────┬──────────────┬───────────────┤
│    Core      │  Multiplexer │  Plugin-Engine│
│  Emulator    │  Panes &     │  WASM Runtime │
│  Parsing &   │  Sessions    │  (wasmtime)   │
│  Rendering   │  Event Loop  │  API Bridge   │
├──────────────┴──────────────┴───────────────┤
│              wgpu  ·  crossterm              │
└─────────────────────────────────────────────┘
```

- **Core** — Terminal emulation engine: parsing, rendering, I/O
- **Multiplexer** — Pane & session management with a central event loop
- **Plugin-Engine** — WASM runtime (`wasmtime`) with sandboxed API access
- **Platform Layer** — Abstraction over Windows (WinAPI) and Linux (GTK/TTY)

The central event loop processes inputs, renders panes in parallel, and delegates work to plugins. All plugins are isolated; no direct OS access without user confirmation.

---

## Build & Install

> **Stack:** Rust · crossterm · wgpu · wasmtime

```bash
# Clone
git clone https://github.com/<org>/super-terminal.git
cd super-terminal

# Build
cargo build --release

# Run
cargo run --release
```

Pre-built binaries for Windows and Linux are available on the [Releases](https://github.com/<org>/super-terminal/releases) page.

---

## Roadmap

- [ ] Base emulator with pane splitting
- [ ] Linux support (GTK/TTY)
- [ ] Windows support (WinAPI)
- [ ] WASM plugin system + example plugins
- [ ] Bluetooth & WLAN plugins via OS APIs
- [ ] Midnight blockchain plugin via Cardano SDK
- [ ] CI pipeline with cross-platform builds (GitHub Actions)
- [ ] Public release — open source, Windows & Linux binaries

---

## Contributing

Contributions are welcome — especially new plugins! See the plugin development docs to get started. Please open an issue or PR on GitHub.

## License

Apache 2.0
