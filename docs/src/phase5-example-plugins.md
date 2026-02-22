# Phase 5: Example Plugins

**Status**: 🔄 In Progress

**Goal**: Demonstrate plugin capabilities with practical plugins.

## Overview

Phase 5 implements example plugins to showcase the plugin system's capabilities. These plugins cover various domains including AI assistance, Bluetooth messaging, mesh networking, blockchain integration, and more.

## Plugins

### UterxAI (✅ Complete)

Multi-provider AI assistant pane with streaming support.

#### Features

- Support for multiple AI providers: Claude, Gemini, OpenAI, xAI, Z.ai, Moonshot, Minimax
- Streaming responses
- Provider-agnostic configuration
- Dedicated pane with Ctrl+Shift+A shortcut

#### Configuration

```toml
[plugin]
name = "uterxai"
version = "0.1.0"
description = "Multi-provider AI assistant"
author = "uterx Contributors"

[permissions]
io = true
net = true

[entry]
wasm = "uterxai.wasm"
start = "_start"
```

#### Provider Configuration

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub api_key_env: Option<String>,
    pub keyring_key: Option<String>,
    pub endpoint: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UterxAIConfig {
    pub default_provider: String,
    pub providers: HashMap<String, ProviderConfig>,
}
```

#### Export Functions

```rust
#[no_mangle]
pub extern "C" fn uterxai_poll() -> i32 {
    // Read prompt from uterx_io::read
    // Generate response
    // Write chunks via uterx_io::write
    0
}
```

#### z.ai Integration

```rust
pub fn call_zai(prompt: &str, config: &ProviderConfig) -> Result<String> {
    let api_key = get_api_key(config)?;
    let endpoint = config.endpoint.as_deref().unwrap_or("https://api.z.ai/v1/chat/completions");

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": config.model.as_deref().unwrap_or("zai-1"),
            "messages": [{"role": "user", "content": prompt}],
        }))
        .send()?;

    let json: serde_json::Value = response.json()?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No content in response"))?;

    Ok(content.to_string())
}
```

### Bluetooth Messaging (⏳ Planned)

Enable messaging and file transfer over Bluetooth.

#### Features

- Device scanning
- Connection management
- Encrypted messaging
- File transfer

#### Host APIs Used

- `uterx_platform::bt_scan` - Scan for Bluetooth devices
- `uterx_io::read` / `uterx_io::write` - Send/receive messages
- `uterx_fs::read_file` / `uterx_fs::write_file` - File transfer

### Mesh & WLAN Messaging (⏳ Planned)

Peer-to-peer messaging over mesh networks or WLAN.

#### Features

- Mesh creation/joining
- Offline messaging
- Encryption
- NAT traversal

#### Host APIs Used

- `uterx_net::tcp_connect` - P2P connections
- `uterx_net::http_get` / `uterx_net::http_post` - Discovery
- `uterx_io::read` / `uterx_io::write` - Message exchange

### Midnight Blockchain Integration (⏳ Planned)

Integrate with the Midnight blockchain (Cardano sidechain with ZK-SNARKs).

#### Features

- Wallet management
- Private transactions with ZK-SNARKs
- Dust handling
- NIGHT token support
- Secure key storage

#### Host APIs Used

- `uterx_platform::keyring_get` / `uterx_platform::keyring_set` - Key storage
- `uterx_net::http_post` - Blockchain API calls
- `uterx_fs::read_file` / `uterx_fs::write_file` - Wallet storage

### Text/Code Editor (✅ Built-in)

Built-in floating editor pane with syntax highlighting.

#### Features

- `syntect`-based syntax highlighting (base16-ocean.dark theme)
- Modal vim-style editing (Normal/Insert modes)
- Undo stack
- Ctrl+S save, Ctrl+W close (2-press guard)
- Drag title bar to reposition
- Opens from file browser or search results

#### Keybindings

| Mode | Keybinding | Action |
|------|------------|--------|
| Normal | `i` | Enter Insert mode |
| Normal | `Esc` | Exit Insert mode |
| Normal | `h/j/k/l` | Move cursor |
| Normal | `w` | Next word |
| Normal | `b` | Previous word |
| Normal | `0` | Start of line |
| Normal | `$` | End of line |
| Normal | `dd` | Delete line |
| Normal | `yy` | Yank line |
| Normal | `p` | Paste |
| Normal | `u` | Undo |
| Normal | `Ctrl+S` | Save |
| Normal | `Ctrl+W` | Close (2x) |
| Insert | `Esc` | Return to Normal mode |

### File Sharing (⏳ Planned)

Secure P2P file sharing.

#### Features

- Upload/download via links
- Encryption
- Drag-and-drop in panes

#### Host APIs Used

- `uterx_net::tcp_connect` - P2P connections
- `uterx_fs::read_file` / `uterx_fs::write_file` - File operations

### Network Tools (⏳ Planned)

Diagnostic suite with visual graphs.

#### Features

- Ping
- Traceroute
- Port scan
- Visual graphs in panes

#### Host APIs Used

- `uterx_net::tcp_connect` - Port scanning
- `uterx_ui::draw_text` - Visual output

### SSH Tools (⏳ Planned)

Advanced SSH session management.

#### Features

- Multi-sessions
- Key management
- Tunneling in separate panes

#### Host APIs Used

- `uterx_net::tcp_connect` - SSH connections
- `uterx_io::read` / `uterx_io::write` - SSH protocol
- `uterx_fs::read_file` / `uterx_fs::write_file` - Key storage

### Converter (⏳ Planned)

Universal converter for units, currencies, formats.

#### Features

- Interactive pane
- Extensible conversion rules

#### Host APIs Used

- `uterx_ui::draw_text` - Display results
- `uterx_ui::get_size` - Layout awareness

## Plugin Development

### Creating a New Plugin

1. **Create plugin directory**:

```bash
mkdir plugins/my-plugin
cd plugins/my-plugin
```

2. **Create plugin.toml**:

```toml
[plugin]
name = "my-plugin"
version = "0.1.0"
description = "My awesome plugin"
author = "Your Name"

[permissions]
ui = true
io = true

[entry]
wasm = "my_plugin.wasm"
start = "_start"
```

3. **Write Rust code**:

```rust
#![no_std]
#![no_main]

use uterx_plugin::*;

#[no_mangle]
pub extern "C" fn _start() -> i32 {
    // Plugin initialization
    0
}

#[no_mangle]
pub extern "C" fn my_export() -> i32 {
    // Export function that can be called from host
    0
}
```

4. **Build for WASM**:

```bash
cargo build --target wasm32-wasi --release
```

5. **Test plugin**:

```bash
uterx plugin add ./plugins/my-plugin
```

### Plugin Storage

Plugins are stored in `~/.uterx/plugins/<name>/`:

```
~/.uterx/plugins/
├── my-plugin/
│   ├── plugin.toml
│   ├── my_plugin.wasm
│   └── data/
└── utherxai/
    ├── plugin.toml
    ├── utherxai.wasm
    └── .uterx-install-source
```

## Testing

### UterxAI Tests

```bash
cargo test -p uterxai
```

Tests cover:
- Multi-provider config parsing
- Response content parsing

## Related Documentation

- [Phase 4: Plugin System](./phase4-plugin-system.md) - Plugin system details
- [Plugins](./plugins.md) - Plugin development guide
- [API Reference](./api-reference.md) - Host API documentation
