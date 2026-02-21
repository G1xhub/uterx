# uterx — Super-Terminal App

## Original Prompt (Reference)

"You are an expert Rust developer tasked with building a cross-platform terminal application called 'uterx'. This app is designed to be a 'Terminal Desktop', super functional Terminal, UX with Desktop feeling – a highly modular, extensible terminal emulator and multiplexer that runs natively on Windows and Linux. Draw inspiration from Ghostty (a fast, GPU-accelerated terminal emulator with features like dedicated IO threads for low jitter, standards-compliant ANSI parsing, Unicode support up to version 17, and libghostty for embedding terminal functionality) and Zellij (a terminal workspace with advanced pane management, mouse-based resizing, WASM plugin system, and recent updates like signal handling refactoring and removal of async_std for streamlined async operations). The app should combine Ghostty's performance (e.g., GPU rendering with around 60fps under load, ligature support, and 4x faster text rendering than competitors) with Zellij's multiplexing (e.g., sessions, broadcast modes, and tab-bar improvements for small widths).

**Core Requirements:**
- **Platforms:** Full native support for Windows (using WinAPI for APIs like Bluetooth and networking) and Linux (using GTK for UI and BlueZ for Bluetooth). Use cross-platform abstractions to ensure compatibility. No macOS support needed.
- **Performance and Rendering:** Implement GPU acceleration using wgpu (OpenGL on Linux, DirectX on Windows) for smooth rendering, including ligatures and Unicode 17. Include a dedicated IO thread to minimize jitter under heavy load. Aim for standards-compliance with xterm audits and broad terminal sequence support.
- **Terminal Desktop Features:** Allow users to create, move, resize, and manage multiple panes (windows) freely via drag-and-drop or keyboard shortcuts. Panes can be used separately (independent shells) or simultaneously (e.g., synchronized input). Support multi-window, tabbing, splits, and persistent sessions that restore on restart. Add mouse interaction for resizing and fullscreen modes.
- **UI and Configuration:** Use Ratatui for TUI elements like tabs and splits. Configuration via YAML or TOML files. Include search across panes and crash reporting.
- **Integrated Tools:** Base shell integration for Bash/Zsh on Linux and PowerShell on Windows.

**Plugin System:**
- Build a flexible, downloadable plugin system similar to Zellij's WASM-based one. Plugins are WASM modules (compiled from Rust, JS, etc.) that run in a sandboxed environment using wasmtime. Users can add plugins via a CLI command like 'uterx plugin add <name or URL>' from a central repository (e.g., GitHub). Plugins extend the app by loading into panes or overlays, accessing APIs for UI, IO, and networking with user permissions.
- Plugin Management: Built-in manager for listing, updating, and removing plugins. Automatic updates optional.
- Develop example plugins as specified below, ensuring they are secure (e.g., sandboxed access) and cross-platform.

**Example Plugins to Implement:**
1. **Bluetooth Messaging:** Enable messaging and file transfer over Bluetooth. Use bluer crate on Linux and btleplug for async BLE on both platforms. Features: Device scanning, connection, encrypted messaging, and transfers. Integrate as a pane showing device lists.
2. **Mesh & WLAN Messaging:** Peer-to-peer messaging over mesh networks or WLAN. Use libp2p for P2P discovery and iroh for chats. Features: Mesh creation/joining, offline messaging, encryption. Display in a chat pane with NAT traversal.
3. **Midnight Blockchain Integration:** Integrate with the Midnight blockchain (a Cardano sidechain focused on privacy with ZK-SNARKs for selective disclosure, currently in Kūkolu phase as of February 2026, with mainnet launch planned for late March 2026). Use cardano-sdk-rs and halo2 for ZK proofs. Features: Wallet management, private transactions with ZK-SNARKs, Dust handling (small amounts via DUST token marketplace, post-launch), and NIGHT token support. Secure key storage with keyring crate. Use testnet APIs until mainnet (federated nodes like Google Cloud, Blockdaemon, AlphaTON Capital, and Shielded Technologies are being added for launch). Display transactions in a secure pane.
4. **Text/Code Editor:** Built-in editor with syntax highlighting. Use helix crate and tree-sitter for parsing. Features: Modal editing, auto-complete, multi-file support. Integrate as a pane.
5. **File Sharing:** Secure P2P file sharing. Use iroh and libp2p. Features: Upload/download via links, encryption. Support drag-and-drop in panes.
6. **Network Tools:** Diagnostic suite. Use pnet for low-level tools like ping/traceroute/port-scan. Features: Visual graphs in panes.
7. **SSH Tools:** Advanced SSH management. Use russh or makiko for async clients. Features: Multi-sessions, key management, tunneling in separate panes.
8. **Converter:** Universal converter for units, currencies, formats. Use uom and unit-conversions crates. Features: Interactive pane, extensible.

**Tech Stack:**
- **Language:** Rust (version 1.92 or later).
- **Async Runtime:** Tokio for non-blocking IO and events.
- **Build:** Cargo with cargo-xtask for dev tasks.
- **Emulation:** vte or custom based on libghostty-vt for parsing.
- **Rendering/UI:** wgpu for GPU, Ratatui for TUI, crossterm for TTY.
- **Plugins:** wasmtime for WASM.
- **Security:** ring for crypto, sandboxing for plugins.
- **Other Crates:** reqwest for downloads, custom platform layers for OS-specific APIs."

---

## Architecture

```
uterx/
├── crates/
│   ├── uterx-core/        # Terminal emulator: VTE parsing, cell grid, ANSI sequences
│   ├── uterx-render/      # GPU rendering via wgpu (OpenGL/Linux, DX12/Windows)
│   ├── uterx-mux/         # Multiplexer: panes, tabs, sessions, layouts
│   ├── uterx-ui/          # TUI layer via ratatui + crossterm
│   ├── uterx-plugin/      # WASM plugin engine via wasmtime
│   ├── uterx-platform/    # OS abstraction (pty, bluetooth, networking)
│   └── uterx-app/         # Main binary, event loop, config, CLI
├── plugins/
│   ├── bluetooth/          # Bluetooth messaging plugin (WASM)
│   ├── mesh-chat/          # Mesh & WLAN messaging plugin
│   ├── midnight/           # Midnight blockchain integration
│   ├── editor/             # Text/code editor plugin
│   ├── file-sharing/       # P2P file sharing plugin
│   ├── network-tools/      # Ping, traceroute, port-scan
│   ├── ssh-tools/          # SSH session management
│   └── converter/          # Unit/currency converter
├── xtask/                  # Build automation (cargo-xtask)
├── docs/                   # mdBook documentation
├── Cargo.toml              # Workspace root
├── PLAN.md                 # This file — project memory
└── README.md               # Overview
```

### Crate Responsibilities

- **uterx-core**: VTE/ANSI parser, cell grid, Unicode 17, ligatures, scrollback, dedicated IO thread.
- **uterx-render**: wgpu GPU renderer, glyph atlas (fontdue), 60fps target, damage tracking.
- **uterx-mux**: Pane/tab/session manager, broadcast mode, drag-and-drop, session persistence.
- **uterx-ui**: Ratatui TUI chrome, crossterm backend, keybindings, search overlay, mouse.
- **uterx-plugin**: Wasmtime WASM runtime, host API (ui/io/net/fs/platform), permissions, CLI.
- **uterx-platform**: PTY (portable-pty), Bluetooth (btleplug/bluer), keyring, shell detection.
- **uterx-app**: Main binary, Tokio runtime, config (TOML), CLI (clap), event loop, crash reporting.

### Plugin System

1. Plugins are WASM modules (wasm32-wasi target).
2. Manifest: `plugin.toml` with name, version, permissions, entry point.
3. Host APIs: `uterx::ui`, `uterx::io`, `uterx::net`, `uterx::fs`, `uterx::platform`.
4. Storage: `~/.uterx/plugins/<name>/`.
5. CLI: `uterx plugin add/remove/list/update`.
6. Sandboxed: capabilities require user approval.

---

## Development Phases

### Phase 1 — Core Terminal (DONE — ~95% complete)

#### 1.1 Workspace & Scaffolding
- [x] Workspace `Cargo.toml` with all internal crates
- [x] Directory structure for all 7 crates + xtask
- [x] Workspace-wide dependency management
- [x] `cargo-xtask` setup (build-plugins, dist, docs)

#### 1.2 uterx-core: VTE Parser & Cell Grid (DONE)
- [x] VTE parser wrapping `vte` crate with `Performer` impl
- [x] `TerminalAction` enum (Print, Execute, CSI, ESC, OSC)
- [x] `Cell` with content, width, `CellAttributes` (fg, bg, bold, italic, underline, strikethrough, inverse, hidden)
- [x] `Grid` with cursor, `write_char`, `newline`, `scroll_up`, `resize`, `clear`
- [x] `Scrollback` ring buffer (push, line access, max capacity)
- [x] CSI basics: CUU/CUD/CUF/CUB (cursor movement), CUP/HVP (cursor position), ED (erase display — mode 2/3)
- [x] **SGR (Select Graphic Rendition)** — full parsing (bold, fg/bg indexed 0-255, RGB, italic, underline, inverse, strikethrough, hidden, reset)
- [x] ED mode 0 (erase below), mode 1 (erase above)
- [x] EL (Erase in Line) — CSI K modes 0/1/2
- [x] ESC sequence handling (DECSC/DECRC save/restore cursor, RI reverse index, IND, NEL, RIS full reset)
- [x] OSC handling (window title — OSC 0/2)
- [ ] DCS sequences (DECRQSS, Sixel, etc.)
- [x] **Scrollback integration** — `Grid.scroll_up_region()` pushes evicted rows into `Scrollback`
- [ ] Wide character / Unicode 17 support (wcwidth, grapheme clusters)
- [ ] Ligature detection (multi-cell character runs)
- [x] Scroll regions (DECSTBM — CSI r)
- [x] Alternate screen buffer (DECSET 1049)
- [x] Insert/Delete lines (CSI L / CSI M)
- [ ] Tab stops (HTS, TBC)
- [x] DEC private modes: cursor visibility (25), autowrap (7), bracketed paste (2004), application cursor (1), blinking cursor (12)
- [x] Additional CSI: CNL, CPL, CHA, VPA, DCH, ICH, ECH, SU, SD, DSR (cursor position report)

#### 1.3 uterx-platform: PTY & Shell (DONE)
- [x] `PtyProcess` via `portable-pty` — spawn, resize, reader/writer
- [x] `default_shell()` — Windows (`COMSPEC`/`powershell.exe`) & Linux (`$SHELL`)
- [x] `ShellType` detection (Bash, Zsh, Fish, PowerShell, Cmd)
- [x] `BluetoothManager` trait + placeholder `BtleplugManager`
- [x] **Non-blocking PTY IO** — `PtyReader` wrapper with `spawn_blocking`-based async read, mpsc channel
- [ ] PTY child process lifecycle (wait, kill, status)

#### 1.4 uterx-ui: TUI Layer (DONE)
- [x] `TerminalView` ratatui widget — renders `Grid` cells with style conversion
- [x] `TabBar` widget — Catppuccin-themed with branding, indexed tabs, broadcast indicator, [+] and [?] buttons
- [x] `StatusBar` widget — session badge, pane title, pane count, broadcast tag, keybinding hints
- [x] `CommandPalette` overlay — searchable command list (Ctrl+P), keyboard nav, fuzzy filter
- [x] `HelpOverlay` widget — full keybinding reference with sections (F1 toggle, also shown on first launch)
- [x] `InputHandler` with keybinding engine (Ctrl+Q, Ctrl+N, Ctrl+W, Ctrl+T, Ctrl+F, Ctrl+Tab, F1, Ctrl+P)
- [x] **Tab bar + status bar wired into event loop**
- [x] **`InputHandler` wired into event loop** — keybinding dispatch for all actions
- [ ] Search overlay widget
- [x] **Mouse event handling** — mouse capture enabled, click-to-focus pane, scroll stubs
- [x] **Overlay system** — Help and CommandPalette overlays render on top of terminal, intercept input
- [x] **File browser sidebar** (Ctrl+E) — tree-view with expand/collapse, file icons, cursor nav, scroll, Catppuccin theme
- [x] **Focus system** — Terminal ↔ FileBrowser ↔ Editor(u64) focus, sidebar layout with horizontal split
- [x] **File browser search** — `/` enters search mode; local (fuzzy match in current entries) and recursive (walk subtree, max 200 results); Tab toggles local/recursive; Up/Down navigate results; Enter opens or jumps; yellow match highlighting in tree
- [x] **Floating editor pane** — `syntect`-based syntax highlighting (base16-ocean.dark), modal editing (Normal/Insert vim-style), undo stack, Ctrl+S save, Ctrl+W close guard; drag title bar to move; opens files from file browser; pink/blue border accent; renders on top of tiled + floating panes

#### 1.5 uterx-app: Main Binary (DONE)
- [x] CLI with `clap` — plugin subcommands (add, remove, list, update)
- [x] `AppConfig` TOML loading (terminal, ui, plugins sections)
- [x] Event loop: raw mode → PTY spawn → read/parse/render loop → cleanup
- [x] Basic key-to-bytes conversion (Enter, arrows, Ctrl+letter, special keys, F1-F12)
- [x] Terminal resize handling
- [x] **Async event loop** — Tokio `select!` on PTY mpsc channel + crossterm `EventStream`
- [x] **TabBar + StatusBar + TerminalView** rendering with ratatui `Layout`
- [x] **InputHandler** wired for keybinding dispatch
- [x] **Integrate `uterx-mux`** (Session/Tab/Pane lifecycle) — Sprint 2
- [x] **Config file creation on first launch** — writes default `~/.uterx/config.toml` with comments
- [ ] Crash reporting

### Phase 2 — Multiplexer (CURRENT — ~85% done)
- [x] `PaneId`, `Pane` struct with Grid, rect, focus
- [x] `TabId`, `Tab` struct with panes, layout, broadcast, focus management
- [x] `Session` with ID generator, save/restore placeholders
- [x] `Layout` engine — Single, HorizontalSplit, VerticalSplit, Tiled, Floating
- [x] `Layout::compute_rects()` — distributes panes in available area
- [x] **Wire PTY into Pane** — each `Pane` owns `PtyProcess` + `Parser` + async reader task
- [x] **Pane split operations** — `split_horizontal()` / `split_vertical()` on `Tab`
- [x] **Pane close, focus cycling** — `remove_pane()`, `focus_next()`, `focus_prev()`, visual border indicator
- [x] **Tab create/close/switch** — `Session::create_tab()`, `close_tab()`, `next_tab()`, `prev_tab()`
- [x] **Broadcast mode** — `toggle_broadcast()`, input routed to all panes when active
- [x] **Event loop rewritten** — uses Session/Tab/Pane architecture, all actions wired
- [x] **Keybindings** — Alt+H split-h, Alt+V split-v, Alt+Left/Right focus, Alt+B broadcast, Ctrl+T tab, Ctrl+W close
- [x] **Session persistence** — save to TOML on exit (`~/.uterx/sessions/`), restore with PTY re-spawning
- [x] **Mouse support** — click-to-focus pane, mouse capture on/off, scroll stubs
- [x] **File browser → pane** — Right-arrow on folder opens new tab cd'd into that directory, folder name as pane title
- [x] **Sidebar-aware layout** — Pane area computed with sidebar offset, resize events account for sidebar width
- [x] Session auto-restore on launch (CLI flag) ← Sprint 4
- [x] Mouse-based pane border drag resizing ← Sprint 4
- [x] Drag-and-drop pane reordering (tiled panes)
- **Tests:** 12 unit tests (tab: add/focus, cycling, remove, split, broadcast, relayout, split-boundary resize+clamp, tiled reorder; session: roundtrip, save/load)

### Phase 3 — GPU Rendering (~20% scaffolding done)
- [x] `GlyphAtlas` with `fontdue` rasterization + cache structure
- [x] `Renderer` placeholder struct with config
- [x] wgpu adapter + device + queue initialization (headless, no surface)
- [x] Surface/window integration for presentation path (platform-aware config + present path)
- [x] Glyph atlas texture packing (shelf algorithm + cache insertion)
- [x] Vertex/instance data generation from grid cells (frame geometry builder)
- [x] Render pipeline skeleton (WGSL shader + bind group + draw-call encoder)
- [x] Damage tracking (dirty cells/rows; only changed-cell geometry rebuild)
- [x] Dedicated render thread with channel-based updates
- [x] Ligature rendering
- [x] Cursor rendering (block, bar, underline styles)
- [x] Selection highlighting
- [x] Smooth scrolling

### Phase 4 — Plugin System (~30% scaffolding done)
- [x] `PluginManifest` schema + TOML loading
- [x] `PluginManager` — scan, list, get, remove, wasm_path
- [x] `PluginInstance` — wasmtime load + start (`_start` entry point)
- [x] Host API categories documented (ui, io, net, fs, platform)
- [x] `uterx::log` host function implemented
- [x] CLI subcommands (add, remove, list, update) in main binary
- [ ] Plugin installation (download from URL, extract, validate)
- [ ] Host API: `uterx_ui::draw_text`, `get_size`, `create_overlay`
- [ ] Host API: `uterx_io::read`, `write`
- [ ] Host API: `uterx_net::http_get`, `tcp_connect`
- [ ] Host API: `uterx_fs::read_file`, `write_file` (sandboxed)
- [ ] Host API: `uterx_platform::bt_scan`, `keyring_get`
- [ ] Permission checking (grant/deny on first use)
- [ ] Plugin auto-update mechanism
- [ ] Plugin repository / registry integration

### Phase 5 — Example Plugins
- [ ] Bluetooth Messaging
- [ ] Mesh & WLAN Messaging
- [ ] Midnight Blockchain Integration
- [x] Text/Code Editor — built-in floating editor pane (syntect, modal vim, undo); advanced plugin version (helix + tree-sitter) deferred
- [ ] File Sharing
- [ ] Network Tools
- [ ] SSH Tools
- [ ] Converter

### Phase 6 — Polish & Release
- [ ] Cross-platform CI (GitHub Actions)
- [ ] Benchmarks with criterion
- [ ] mdBook documentation
- [ ] Crash reporting
- [ ] Open-source release

---

## Next Steps — Prioritized Implementation Order

The following is the recommended order for completing Phase 1 and transitioning
to Phase 2. Each step builds on the previous one.

### Sprint 1: Working Single-Pane Terminal (Phase 1 completion)

**Goal:** A usable terminal emulator in a single pane that correctly handles
common shell interactions (ls, vim, htop, etc).

1. **SGR parsing** (`parser.rs`) — Parse CSI `m` sequences to set bold, fg/bg
   color (indexed 0-255 + RGB), italic, underline, inverse, reset on the grid's
   `CellAttributes`. This is critical for colored shell output.

2. **Async PTY IO** (`event_loop.rs`) — Move PTY reading into a `tokio::spawn`
   task that sends data via `tokio::sync::mpsc` channel. The main loop
   `select!`s on PTY data + crossterm events. Eliminates the blocking read.

3. **Scrollback wiring** — When `Grid::scroll_up()` is called, push the evicted
   top row into the `Scrollback` buffer. Add scroll-back viewing (Shift+PgUp/PgDn).

4. **ED/EL completion** — Implement ED mode 0 (erase below), mode 1 (erase
   above), and EL (CSI K) modes 0/1/2. Many programs rely on these.

5. **Alternate screen buffer** — DECSET 1049 (enter) / DECRST 1049 (exit). 
   Required for programs like vim, less, htop.

6. **Scroll regions** — DECSTBM (CSI r) for setting scroll margins. Needed for
   vim and other full-screen apps.

7. **Insert/Delete lines** — CSI L / CSI M.

8. **UI integration** — Render `TabBar` + `StatusBar` + `TerminalView` in the
   event loop. Wire `InputHandler` for keybinding dispatch.

### Sprint 2: Multiplexer Basics (Phase 2 core)

**Goal:** Multiple panes and tabs with keyboard navigation.

1. **Pane ↔ PTY binding** — Give each `Pane` its own `PtyProcess` + `Parser`.
   Each PTY reader gets its own Tokio task.

2. **Session management** — `Session` holds `Vec<Tab>`, each `Tab` holds
   `Vec<Pane>`. Wire into the App struct.

3. **Split operations** — Implement `SplitHorizontal` / `SplitVertical` from the
   active pane. Recalculate layouts, spawn new PTY.

4. **Tab create/close/switch** — Ctrl+T new tab, Ctrl+W close pane, Ctrl+Tab
   switch tabs.

5. **Focus management** — Arrow-key or Alt+arrow pane navigation. Visual focus
   indicator (highlighted border).

6. **Broadcast mode** — Toggle with keybinding; duplicates input to all panes
   in the active tab.

### Sprint 3: Session Persistence & Mouse (Phase 2 polish)

1. **Session save/restore** — Serialize session state to
   `~/.uterx/sessions/<name>.toml`. Restore on launch.

2. **Mouse pane resizing** — Detect border clicks, drag to resize. Update layout
   ratios dynamically.

3. **Pane drag reordering** — Swap pane positions via mouse drag.

### Sprint 4: GPU Rendering (Phase 3)

1. **wgpu init** — Device, adapter, surface, swap chain. Platform backend
   selection.

2. **Atlas texture** — Upload rasterized glyphs to GPU texture. Shelf packing.

3. **Render pipeline** — Vertex shader (cell position) + fragment shader (glyph
   sampling, color). Instanced rendering.

4. **Damage tracking** — Dirty row/cell tracking, partial re-upload.

5. **Render thread** — Separate thread, receives grid snapshots via channel.

### Sprint 5: Plugin System (Phase 4)

1. **Host API implementation** — ui, io, net, fs, platform functions in
   wasmtime linker.

2. **Plugin installation** — Download `.tar.gz` from URL, extract, validate
   manifest, copy to `~/.uterx/plugins/`.

3. **Permission prompts** — On first API call, prompt user for
   grant/deny/always.

4. **Plugin pane rendering** — Plugins can create their own panes and draw into
   them.

### Sprint 6-8: Example Plugins (Phase 5)

Build plugins in priority order:
1. Converter (simplest, good test of plugin API)
2. Network Tools (pnet-based, visual graphs)
3. SSH Tools (russh async sessions)
4. ~~Text/Code Editor~~ — **DONE** as built-in (syntect + modal vim); advanced plugin (helix + tree-sitter) may follow
5. File Sharing (iroh + libp2p)
6. Bluetooth Messaging (btleplug)
7. Mesh & WLAN Messaging (libp2p + iroh)
8. Midnight Blockchain (cardano-sdk-rs + halo2 ZK)

---

## Status Tracker

| Date       | Phase   | Milestone                          | Notes                                      |
|------------|---------|------------------------------------|---------------------------------------------|
| 2026-02-20 | Phase 1 | Project scaffolding created        | All crate skeletons done                    |
| 2026-02-21 | Phase 1 | Code audit & build fix             | Fixed workspace members, tracing-subscriber env-filter, unused imports. Project compiles cleanly. |
| 2026-02-21 | Phase 1 | **Sprint 1 complete**              | Full SGR (256+RGB), async PTY IO, scrollback integration, ED/EL, alt screen, scroll regions, insert/delete lines, ESC/OSC handling, DEC private modes, UI integration (TabBar+StatusBar+InputHandler). 13 tests pass. |
| 2026-02-21 | Phase 2 | Sprint 2 started                   | Multiplexer basics: Pane↔PTY binding, Session/Tab/Pane lifecycle, splits, focus, broadcast. |
| 2026-02-21 | Phase 2 | **Sprint 2 complete**              | Pane owns PTY+Parser+async reader. Tab: split-h/v, focus cycling, remove, broadcast, relayout. Session: create/close/switch tabs, split delegation. Event loop rewritten for mux. 6 new mux tests + 13 core tests = 19 total passing. |
| 2026-02-21 | Phase 2 | **Sprint 3 complete**              | Session persistence (save TOML on exit, load+restore with PTY respawn). Config auto-creation on first launch. Mouse capture + click-to-focus pane. 21 total tests (13 core + 8 mux). |
| 2026-02-22 | Phase 2 | **UI Overhaul complete**            | Catppuccin theme. Redesigned TabBar (branding, indexed tabs, [+]/[?] buttons, broadcast indicator). Redesigned StatusBar (session badge, pane count, keybinding hints). New HelpOverlay (F1, replaces old tutorial — sections: General, Tabs, Panes, Broadcast, Config). New CommandPalette (Ctrl+P, fuzzy filter, keyboard nav, executes actions). Overlay system (Help/CommandPalette render on top, intercept input). Pane borders themed (blue=focused, dim=unfocused). First-launch shows help overlay automatically. 21 tests passing. |
| 2026-02-22 | Phase 2 | **File Browser & Desktop Features** | File browser sidebar (Ctrl+E toggle). Tree-view with expand/collapse, file icons (Nerd Font), cursor navigation, scroll, Catppuccin theme. Focus system (Terminal ↔ FileBrowser). Sidebar layout (horizontal split). Folder→pane: Right arrow on folder opens new tab cd'd into directory. Mouse click in sidebar focuses file browser. Command palette + help overlay updated with File Browser entries. Event loop fully rewritten with Focus enum, sidebar-aware pane area, file browser input handling. 21 tests passing. |
| 2026-02-23 | Phase 2 | **Mouse Tab Navigation & Floating Panes** | Mouse-driven tab bar: click tab label to switch, [+] to create tab, [?] to open help. Floating panes (Alt+F): toggle focused pane to free-floating, centered at 60% of terminal area. Drag title bar to move floating pane. Floating panes render on top of tiled panes with drop shadow (Catppuccin crust) and pink accent border (focused) / blue (unfocused). `DragState` tracks drag offset for smooth movement. `Pane::is_floating`, `Tab::toggle_float()`, `Tab::move_floating_pane()`, `Tab::tiled_panes()`, `Tab::floating_panes()` added. Command palette + help overlay updated. 21 tests passing. |
| 2026-02-21 | Phase 2 | **File Browser Search + Floating Editor** | File browser search (`/`): local fuzzy filter in current entries + recursive tree walk (Tab toggles, max 200 results). Yellow match highlighting, Up/Down result navigation, Enter to open/jump. Floating editor pane: `syntect` syntax highlighting (base16-ocean.dark), modal vim editing (Normal/Insert), undo stack, Ctrl+S save, Ctrl+W close guard (2-press). Opens text/code files from browser or search results; drag title bar to reposition. New types: `Focus::Editor(u64)`, `EditorPane`, `EditorDragState`, `EditorState`, `EditorWidget`. Help overlay expanded (File Browser, Editor Normal, Editor Insert). Command palette: "Search Files" entry. `syntect = "5"` added to uterx-ui. 21 tests passing. |
| 2026-02-21 | Phase 2 | **Resize + Desktop Click UX + Auto-Restore** | Tiled pane border drag resizing implemented for `VerticalSplit`/`HorizontalSplit` with min pane constraints (10x4), ratio clamping, and relayout-on-drag. Added `Tab::adjust_split_boundary(...)` and split boundary hit-testing in event loop. File Browser mouse behavior now desktop-like: single-click selects, double-click opens file or expands/collapses directory. Tab click hit-testing aligned with rendered tab labels so tab switching is reliable. Added launch restore flow with CLI toggle `--no-restore`: app restores from `~/.uterx/sessions/main.toml` when available, otherwise falls back to fresh session. Mux tests increased to 11 passing; workspace `cargo check` passes. |
| 2026-02-21 | Phase 2 | **Pane Reordering complete** | Drag-and-drop reordering for tiled panes implemented: drag from tiled pane title bar and drop onto another tiled pane to reorder pane vector and relayout. Added `Tab::reorder_tiled_panes(dragged, target)` with new mux test coverage. Existing interactions remain prioritized (editor drag, split resize, floating drag). `cargo check` passes; mux tests increased to 12 passing. |
| 2026-02-21 | Phase 3 | **wgpu Init Kickoff (headless)** | `uterx-render::Renderer` now supports real GPU bootstrap via `try_init_wgpu()` with `wgpu::Instance`, adapter selection, and `Device`/`Queue` creation. Added `GpuState` storage and adapter diagnostics (`adapter_info`). Rendering path remains placeholder but now distinguishes initialized vs non-initialized GPU state. Exported `RendererConfig` and `GpuState` from render crate. Workspace `cargo build` and `cargo check` succeed. |
| 2026-02-21 | Phase 3 | **Grid → Frame Geometry** | Added per-cell frame geometry generation in `uterx-render`: `CellInstance` + `FrameGeometry` with position, size, resolved fg/bg colors, codepoint, and style flags per grid cell. `Renderer::build_frame_geometry()` now produces instance-ready data from `Grid`; `render()` stores last frame snapshot for upcoming GPU buffer upload. Added render-crate tests for geometry count/content and inverse-color swapping. `cargo test -p uterx-render` (2 tests) and workspace `cargo check` pass. |
| 2026-02-21 | Phase 3 | **Damage Tracking (dirty cells/rows)** | Added `DamageReport` and grid snapshot diffing in renderer: first frame/full resize triggers full redraw, subsequent frames mark only changed cells/rows. `render()` now computes damage and builds geometry only for dirty cells when possible (`build_damage_geometry`). Added tests for no-op frames, single-cell diffs, and dirty-instance generation. `cargo test -p uterx-render` now 4 tests passing; workspace `cargo check` passes. |
| 2026-02-21 | Phase 3 | **Render Pipeline Skeleton** | Added WGSL shader module, globals uniform buffer, bind-group layout, bind group, and render pipeline creation in `Renderer::try_init_pipeline(format)`. Added viewport uniform update helper and `encode_render_pass()` that emits a real draw call (`draw(0..3, 0..1)`) into a caller-provided `TextureView`. This keeps rendering headless/surface-agnostic while establishing shaders + bind groups + draw path for later surface integration. `cargo test -p uterx-render` now 5 tests passing; workspace `cargo check` passes. |
| 2026-02-21 | Phase 3 | **Glyph Atlas Shelf Packing** | Implemented shelf-based texture packing in `GlyphAtlas`: atlas dimensions/pixel storage, shelf tracking, best-fit shelf placement, bitmap upload into atlas alpha buffer, and cache-backed `get_or_insert(key)` API. Added atlas sizing/pixel accessors for upcoming GPU texture upload stage. Added atlas tests for cache behavior and bitmap write validation. `cargo test -p uterx-render` now 7 tests passing; workspace `cargo check` passes. |
| 2026-02-21 | Phase 3 | **Dedicated Render Thread** | Added `render_thread` module with channel-based command loop (`UpdateGrid`, `Flush`, `Shutdown`) running on a dedicated thread. The thread owns a renderer instance, processes incoming grid snapshots, and tracks frame statistics (`frames_processed`, instance count, changed cells). Exposed `RenderThreadHandle` API (`start`, `send_grid`, `flush`, `stats`, `shutdown`) and re-exported from render crate. Added unit test for processing multiple updates. `cargo test -p uterx-render` now 8 tests passing. |
| 2026-02-21 | Phase 3 | **Cursor Rendering Styles** | Added explicit cursor rendering primitives in renderer with `CursorStyle` (`Block`, `Bar`, `Underline`) and `CursorInstance` geometry included in `FrameGeometry`. `RendererConfig` now carries cursor style; geometry builder emits cursor quad with style-specific dimensions/placement. Damage tracking now considers cursor movement by marking old/new cursor cells dirty so cursor-only movement can trigger targeted updates. Exported cursor types from render crate API. `cargo test -p uterx-render` now 9 tests passing; workspace `cargo check` passes. |
| 2026-02-21 | Phase 3 | **Surface/Window Presentation Path** | Added surface integration APIs in renderer: unsafe window-handle based `try_init_surface_from_window`, capability-driven `configure_surface`, and `render_to_surface` with robust frame acquisition handling (`Lost/Outdated` reconfigure, `Timeout` skip, `OutOfMemory` error). Added `SurfaceState` and format/present/alpha selection helpers with tests. Render crate now supports full configure+present path while remaining app-window-framework agnostic. `cargo test -p uterx-render` now 10 tests passing; `cargo check -p uterx-render` passes cleanly. |
| 2026-02-21 | Phase 3 | **Selection Highlighting** | Added selection support in renderer via `SelectionRange` and `SelectionInstance`, including overlay geometry generation in both full-frame and damage paths. Renderer now tracks current/previous selection state and triggers full redraw when selection changes to keep highlight transitions correct. Added selection tests (overlay instance generation and selection-change damage behavior). Exported selection types from render crate API. `cargo test -p uterx-render` now 12 tests passing; workspace `cargo check` passes (with pre-existing warnings in other crates). |
| 2026-02-21 | Phase 3 | **Ligature Rendering** | Added ligature-aware geometry building in renderer using pattern-based run detection (e.g. `->`, `=>`, `==`, `!=`) with row-level grouping into single `CellInstance` spans. `CellInstance` now carries source `text`, and ligature runs are flagged for downstream glyph/atlas handling. Damage geometry now rebuilds full changed rows when ligatures are present to avoid partial-run artifacts. Added ligature tests for full-frame grouping and damage-path behavior. `cargo test -p uterx-render` now 14 tests passing; workspace `cargo check` passes (with pre-existing warnings in other crates). |
| 2026-02-21 | Phase 3 | **Smooth Scrolling** | Added smooth scrolling state to renderer (`scroll_offset_px`, `scroll_target_px`) with interpolation step (`tick_smooth_scroll`) controlled by `RendererConfig::scroll_smoothing_factor`. Geometry generation now applies scroll offset to cell, ligature, selection, and cursor Y positions via a shared row-offset helper. Damage tracking marks full redraw when scroll offset changes to keep animation frames visually consistent. Added tests for offset application and scroll-change damage behavior. `cargo test -p uterx-render` now 16 tests passing; workspace `cargo check` passes (with pre-existing warnings in other crates). |
| 2026-02-21 | Phase 7 | **Feature Brainstorming** | 10 innovative new features designed: Smart Workspace Manager, Pane Snippets & Templates, Real-time Process Monitor, Intelligent Search Across Panes, Pane History & Replay, Collaborative Editing, Keyboard Macro System, Integrated Git Graph, Task Runner & Dashboard, Smart Notifications & Alerts. Top 3 recommendations prioritized for performance and innovation. |

---

## Phase 7 — Innovative Features (Planned)

### Overview

Based on the "Terminal Desktop" concept and uterx's modular architecture, the following innovative features are proposed to extend functionality while maintaining high performance across low-end to high-end devices.

### Feature List

#### 1. Smart Workspace Manager
- **Description**: Named workspace presets for different development contexts
- **Use Cases**: "Dev" workspace (terminal, editor, file browser), "Admin" workspace (SSH panes, network monitor)
- **Integration**: Extends Session system with named presets, stored in config
- **Performance**: Minimal overhead - only configuration stored
- **Implementation Priority**: High (top 3)
- **Key Components**: `WorkspaceManager`, `WorkspaceConfig`, workspace save/restore commands

#### 2. Pane Snippets & Templates
- **Description**: Reusable pane configurations for common workflows
- **Use Cases**: "Docker Dev" = 3 panes (logs, shell, db), "Frontend" = (npm run dev, git, editor)
- **Integration**: Command palette → Template selection → Auto-layout
- **Performance**: One-time configuration, instant availability
- **Implementation Priority**: Medium
- **Key Components**: `PaneTemplate`, template registry, template application logic

#### 3. Real-time Process Monitor
- **Description**: Visual system resource monitoring (CPU, RAM, Disk, Network)
- **Use Cases**: Like htop but in uterx style, floating pane with Catppuccin theme
- **Integration**: New widget in `uterx-ui`, uses OS abstractions
- **Performance**: Updates every 1-2 seconds, async
- **Implementation Priority**: High (top 3)
- **Key Components**: `ProcessMonitorWidget`, OS metrics collectors, async update loop

#### 4. Intelligent Search Across Panes
- **Description**: Global search across all open terminals and files
- **Use Cases**: `Ctrl+Shift+F` → search text → results in all panes
- **Integration**: Extends parser results, fuzzy matching
- **Performance**: Incremental search, background threads
- **Implementation Priority**: High
- **Key Components**: `GlobalSearchEngine`, `SearchOverlay`, pane content indexing

#### 5. Pane History & Replay
- **Description**: Time travel through pane contents
- **Use Cases**: Clickable timestamps restore pane state, like iTerm2 Shell Integration
- **Integration**: Extends Scrollback with metadata and snapshots
- **Performance**: Stores only changes, not full snapshots
- **Implementation Priority**: Medium
- **Key Components**: `HistoryManager`, `HistorySnapshot`, timestamp metadata

#### 6. Collaborative Editing
- **Description**: Multi-user pane sharing (remote or local)
- **Use Cases**: Pair programming via WebRTC or local multi-terminal
- **Integration**: Extends Session system, new event types
- **Performance**: Optimized delta sync, only changed areas transmitted
- **Implementation Priority**: Low (complex)
- **Key Components**: `CollaborationManager`, delta sync protocol, WebRTC integration

#### 7. Keyboard Macro System
- **Description**: Record and replay keyboard sequences
- **Use Cases**: `Ctrl+R` to record → enter sequence → `Ctrl+P` to replay
- **Integration**: Extends InputHandler, stores macros in config
- **Performance**: Minimal overhead, pure event replay
- **Implementation Priority**: Medium
- **Key Components**: `MacroRecorder`, `MacroPlayer`, macro storage

#### 8. Integrated Git Graph
- **Description**: Visual Git history and branch display
- **Use Cases**: Floating pane shows graph like GitKraken/Sourcetree, clickable commits
- **Integration**: New widget, calls git commands, parses output
- **Performance**: Lazy loading, only visible commits rendered
- **Implementation Priority**: High
- **Key Components**: `GitGraphWidget`, git command parser, graph layout engine

#### 9. Task Runner & Dashboard
- **Description**: Define and execute build/test/deploy tasks
- **Use Cases**: Dashboard shows running task status, logs in separate panes
- **Integration**: Task config in TOML, async task runner
- **Performance**: Tasks run in separate threads, UI remains responsive
- **Implementation Priority**: Medium
- **Key Components**: `TaskRunner`, `TaskDashboard`, task configuration parser

#### 10. Smart Notifications & Alerts
- **Description**: Visual notifications for important events
- **Use Cases**: Toast overlay for completed long-running commands, errors
- **Integration**: Extends Event system, animation framework
- **Performance**: Non-blocking, async rendering
- **Implementation Priority**: Low
- **Key Components**: `NotificationManager`, `ToastOverlay`, notification queue

### Top 3 Recommendations (Priority Order)

Based on performance impact, implementation complexity, and user value:

1. **Smart Workspace Manager** — Maximum productivity, low implementation effort
2. **Real-time Process Monitor** — Great performance showcase, perfect "Terminal Desktop" fit
3. **Integrated Git Graph** — High developer value, visually impressive

### Implementation Phases

#### Phase 7.1: Quick Wins (Low complexity, high value)
- Smart Workspace Manager
- Keyboard Macro System
- Smart Notifications & Alerts

#### Phase 7.2: Core Features (Medium complexity)
- Real-time Process Monitor
- Pane Snippets & Templates
- Task Runner & Dashboard
- Integrated Git Graph

#### Phase 7.3: Advanced Features (High complexity)
- Intelligent Search Across Panes
- Pane History & Replay
- Collaborative Editing

### Technical Considerations

- **Performance**: All features designed for low-end devices with async operations and efficient data structures
- **Modularity**: Each feature can be developed independently as separate widgets/extensions
- **Consistency**: All features follow uterx's Catppuccin theme and UX patterns
- **Extensibility**: Features use existing abstractions (Session, Pane, Widget system)
