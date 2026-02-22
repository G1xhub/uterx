# Plugins

This chapter provides a comprehensive guide to developing plugins for uterx.

## Overview

uterx plugins are WASM modules that extend the application's functionality. Plugins run in a sandboxed environment using wasmtime, providing security while allowing powerful extensions.

## Plugin Architecture

### Plugin Structure

A typical uterx plugin has the following structure:

```
my-plugin/
├── Cargo.toml
├── plugin.toml          # Plugin manifest
├── src/
│   └── lib.rs          # Plugin code
└── README.md
```

### Plugin Manifest

The `plugin.toml` file defines plugin metadata and permissions:

```toml
[plugin]
name = "my-plugin"
version = "0.1.0"
description = "My awesome plugin"
author = "Your Name <you@example.com>"
homepage = "https://github.com/yourname/my-plugin"
repository = "https://github.com/yourname/my-plugin"

[permissions]
# Required permissions
ui = true        # Draw to UI
io = true        # Read/write IO
net = true       # Network access
fs = true        # Filesystem access
platform = true  # Platform APIs (Bluetooth, keyring)

[entry]
# WASM entry point
wasm = "my_plugin.wasm"
start = "_start"

# Optional exports
exports = ["my_function", "another_function"]
```

## Getting Started

### 1. Create a New Plugin

```bash
# Create plugin directory
mkdir my-plugin
cd my-plugin

# Initialize Cargo project
cargo init --lib

# Add wasmtime dependency
cargo add wasmtime --target wasm32-wasi
```

### 2. Configure Cargo.toml

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
# Add your dependencies here
```

### 3. Write Plugin Code

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

// Panic handler
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// Entry point
#[no_mangle]
pub extern "C" fn _start() -> i32 {
    // Plugin initialization
    0
}

// Export function
#[no_mangle]
pub extern "C" fn my_function() -> i32 {
    // Your plugin logic here
    0
}
```

### 4. Build for WASM

```bash
# Add wasm32-wasi target
rustup target add wasm32-wasi

# Build
cargo build --target wasm32-wasi --release

# The WASM file will be at target/wasm32-wasi/release/my_plugin.wasm
```

### 5. Create Plugin Manifest

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

### 6. Install Plugin

```bash
# Install from local directory
uterx plugin add ./my-plugin

# Or from URL
uterx plugin add https://github.com/yourname/my-plugin/archive/main.tar.gz
```

## Host API

The host API provides functions that plugins can call to interact with uterx.

### uterx_ui

#### draw_text

Draw text at a position in the plugin's pane.

```rust
extern "C" fn uterx_ui_draw_text(
    text_ptr: u32,
    text_len: u32,
    x: u32,
    y: u32,
) -> i32;
```

**Example**:

```rust
#[no_mangle]
pub extern "C" fn draw_hello() -> i32 {
    let text = b"Hello, uterx!";
    unsafe {
        uterx_ui_draw_text(
            text.as_ptr() as u32,
            text.len() as u32,
            0,  // x
            0,  // y
        );
    }
    0
}
```

#### get_size

Get the size of a pane.

```rust
extern "C" fn uterx_ui_get_size(pane_id: u64) -> (u16, u16);
```

**Example**:

```rust
#[no_mangle]
pub extern "C" fn get_pane_size() -> i32 {
    unsafe {
        let (width, height) = uterx_ui_get_size(0);
        // Use width and height
    }
    0
}
```

#### create_overlay

Create a new overlay window.

```rust
extern "C" fn uterx_ui_create_overlay(
    title_ptr: u32,
    title_len: u32,
) -> u64;
```

### uterx_io

#### read

Read input from the host.

```rust
extern "C" fn uterx_io_read(ptr: u32, len: u32) -> u32;
```

**Example**:

```rust
#[no_mangle]
pub extern "C" fn read_input() -> i32 {
    let mut buffer = [0u8; 1024];
    unsafe {
        let bytes_read = uterx_io_read(buffer.as_mut_ptr() as u32, buffer.len() as u32);
        // Process input
    }
    0
}
```

#### write

Write output to the host.

```rust
extern "C" fn uterx_io_write(ptr: u32, len: u32) -> i32;
```

**Example**:

```rust
#[no_mangle]
pub extern "C" fn write_output() -> i32 {
    let message = b"Hello from plugin!";
    unsafe {
        uterx_io_write(message.as_ptr() as u32, message.len() as u32);
    }
    0
}
```

### uterx_net

#### http_get

Perform an HTTP GET request.

```rust
extern "C" fn uterx_net_http_get(
    url_ptr: u32,
    url_len: u32,
    resp_ptr: u32,
    resp_len: u32,
) -> i32;
```

**Example**:

```rust
#[no_mangle]
pub extern "C" fn fetch_url() -> i32 {
    let url = b"https://example.com/api";
    let mut buffer = [0u8; 4096];
    unsafe {
        let status = uterx_net_http_get(
            url.as_ptr() as u32,
            url.len() as u32,
            buffer.as_mut_ptr() as u32,
            buffer.len() as u32,
        );
        // Process response
    }
    0
}
```

#### http_post

Perform an HTTP POST request.

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

#### tcp_connect

Establish a TCP connection.

```rust
extern "C" fn uterx_net_tcp_connect(
    host_ptr: u32,
    host_len: u32,
    port: u16,
) -> u32;
```

### uterx_fs

#### read_file

Read a file from the plugin's sandbox.

```rust
extern "C" fn uterx_fs_read_file(
    path_ptr: u32,
    path_len: u32,
    resp_ptr: u32,
    resp_len: u32,
) -> i32;
```

**Example**:

```rust
#[no_mangle]
pub extern "C" fn read_config() -> i32 {
    let path = b"config.json";
    let mut buffer = [0u8; 4096];
    unsafe {
        let bytes_read = uterx_fs_read_file(
            path.as_ptr() as u32,
            path.len() as u32,
            buffer.as_mut_ptr() as u32,
            buffer.len() as u32,
        );
        // Process file content
    }
    0
}
```

#### write_file

Write data to a file in the plugin's sandbox.

```rust
extern "C" fn uterx_fs_write_file(
    path_ptr: u32,
    path_len: u32,
    data_ptr: u32,
    data_len: u32,
) -> i32;
```

### uterx_platform

#### bt_scan

Scan for Bluetooth devices.

```rust
extern "C" fn uterx_platform_bt_scan() -> u32;
```

#### keyring_get

Get a value from the system keyring.

```rust
extern "C" fn uterx_platform_keyring_get(
    key_ptr: u32,
    key_len: u32,
    value_ptr: u32,
    value_len: u32,
) -> i32;
```

## Permissions

Plugins must declare required permissions in their manifest. The host checks permissions before allowing API access.

### Permission Types

| Permission | Description |
|------------|-------------|
| `ui` | Draw to UI, create overlays |
| `io` | Read/write IO streams |
| `net` | Network access (HTTP, TCP) |
| `fs` | Filesystem access (sandboxed) |
| `platform` | Platform APIs (Bluetooth, keyring) |

### Permission Checks

The host prompts the user on first use of each permission:

- **Deny**: Permission denied for this session
- **Grant Once**: Permission granted for this session only
- **Grant Always**: Permission granted and remembered

## Plugin Storage

Plugins have a dedicated sandbox directory at:

```
~/.uterx/plugins/<plugin-name>/
```

This is the only directory the plugin can access via the filesystem API.

## Example Plugin

Here's a complete example plugin that responds to user input:

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

// Panic handler
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// External host API functions
extern "C" {
    fn uterx_ui_draw_text(text_ptr: u32, text_len: u32, x: u32, y: u32) -> i32;
    fn uterx_io_read(ptr: u32, len: u32) -> u32;
    fn uterx_io_write(ptr: u32, len: u32) -> i32;
}

// Entry point
#[no_mangle]
pub extern "C" fn _start() -> i32 {
    let welcome = b"Echo Plugin - Type something and I'll echo it back!";
    unsafe {
        uterx_ui_draw_text(
            welcome.as_ptr() as u32,
            welcome.len() as u32,
            0,
            0,
        );
    }
    0
}

// Main plugin function
#[no_mangle]
pub extern "C" fn echo_loop() -> i32 {
    let mut buffer = [0u8; 1024];

    loop {
        unsafe {
            // Read input
            let bytes_read = uterx_io_read(
                buffer.as_mut_ptr() as u32,
                buffer.len() as u32,
            );

            if bytes_read == 0 {
                break;
            }

            // Echo back
            let prefix = b"Echo: ";
            uterx_ui_draw_text(
                prefix.as_ptr() as u32,
                prefix.len() as u32,
                0,
                1,
            );

            uterx_ui_draw_text(
                buffer.as_ptr() as u32,
                bytes_read,
                5,
                1,
            );
        }
    }

    0
}
```

## Testing Plugins

### Local Testing

1. Build the plugin:
   ```bash
   cargo build --target wasm32-wasi --release
   ```

2. Install locally:
   ```bash
   uterx plugin add ./my-plugin
   ```

3. Run uterx and test the plugin

### Unit Testing

Add tests to your plugin:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {
        assert_eq!(2 + 2, 4);
    }
}
```

Run tests:
```bash
cargo test
```

## Publishing Plugins

### Plugin Registry

Publish your plugin to the uterx plugin registry:

1. Create a GitHub repository with your plugin
2. Add a `plugin.toml` manifest
3. Submit your plugin to the registry

### Manual Installation

Users can install your plugin from:

- Local directory: `uterx plugin add ./my-plugin`
- URL: `uterx plugin add https://github.com/yourname/my-plugin/archive/main.tar.gz`
- Registry name: `uterx plugin add my-plugin`

## Best Practices

### Security

- Only request permissions you actually need
- Validate all input from the host
- Don't expose sensitive data in error messages

### Performance

- Minimize API calls
- Cache results when appropriate
- Use efficient data structures

### User Experience

- Provide clear error messages
- Handle edge cases gracefully
- Document your plugin's features

## Troubleshooting

### Common Issues

**Plugin not loading**
- Check that the WASM file exists
- Verify the manifest is valid
- Check permissions are declared

**Permission denied**
- The user denied the permission
- Check that the permission is declared in the manifest

**Memory access error**
- Check pointer arithmetic
- Ensure buffers are large enough
- Validate array bounds

## Related Documentation

- [API Reference](./api-reference.md) - Host API documentation
- [Phase 4](./phase4-plugin-system.md) - Plugin system details
- [Phase 5](./phase5-example-plugins.md) - Example plugins
