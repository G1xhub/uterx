# Development Phases

This chapter describes the development phases of uterx, from the initial core terminal implementation to the final release and innovative features.

## Overview

uterx is developed in phases, each building upon the previous one:

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 1 | ✅ ~95% Complete | Core Terminal - VTE parsing, cell grid, ANSI sequences |
| Phase 2 | ✅ ~85% Complete | Multiplexer - panes, tabs, sessions, layouts |
| Phase 3 | ✅ ~100% Complete | GPU Rendering - wgpu-based rendering with damage tracking |
| Phase 4 | ✅ ~100% Complete | Plugin System - WASM runtime and host APIs |
| Phase 5 | 🔄 In Progress | Example Plugins - UterxAI, Bluetooth, Mesh, etc. |
| Phase 6 | ⏳ Planned | Polish & Release - CI, benchmarks, documentation |
| Phase 7 | ⏳ Planned | Innovative Features - Workspace manager, process monitor, etc. |

## Phase 1: Core Terminal

**Status**: ~95% Complete

**Goal**: A fully functional terminal emulator with standards-compliant VTE parsing.

**Key Components**:
- VTE/ANSI parser wrapping the `vte` crate
- Cell grid with cursor management
- Scrollback buffer
- SGR (Select Graphic Rendition) parsing
- CSI sequences for cursor movement and screen manipulation
- ESC sequences for special functions
- OSC sequences for window title
- Alternate screen buffer
- Scroll regions
- Non-blocking PTY I/O

**Completed Features**:
- ✅ Full SGR parsing (256+RGB colors, bold, italic, underline, inverse, strikethrough, hidden)
- ✅ CSI sequences: CUU/CUD/CUF/CUB, CUP/HVP, ED, EL, CNL, CPL, CHA, VPA, DCH, ICH, ECH, SU, SD, DSR
- ✅ ESC sequences: DECSC/DECRC, RI, IND, NEL, RIS
- ✅ OSC handling (window title)
- ✅ Scrollback integration
- ✅ Alternate screen buffer (DECSET 1049)
- ✅ Scroll regions (DECSTBM)
- ✅ Insert/Delete lines
- ✅ DEC private modes (cursor visibility, autowrap, bracketed paste, application cursor, blinking cursor)
- ✅ Async PTY I/O with dedicated reader tasks
- ✅ Shell detection (Bash, Zsh, Fish, PowerShell, Cmd)

**Remaining Work**:
- ⏳ DCS sequences (DECRQSS, Sixel, etc.)
- ⏳ Wide character / Unicode 17 support (wcwidth, grapheme clusters)
- ⏳ Ligature detection (multi-cell character runs)
- ⏳ Tab stops (HTS, TBC)
- ⏳ PTY child process lifecycle (wait, kill, status)

**Tests**: 13 unit tests passing

See [Phase 1: Core Terminal](./phase1-core-terminal.md) for details.

## Phase 2: Multiplexer

**Status**: ~85% Complete

**Goal**: Multi-pane, multi-tab terminal workspace with session persistence.

**Key Components**:
- Pane management with PTY binding
- Tab management with layout engine
- Session persistence
- Mouse-based pane resizing and reordering
- Broadcast mode

**Completed Features**:
- ✅ Pane ↔ PTY binding (each pane owns PTY + Parser + async reader)
- ✅ Tab create/close/switch
- ✅ Pane split operations (horizontal/vertical)
- ✅ Pane close, focus cycling
- ✅ Broadcast mode
- ✅ Layout engine (Single, HorizontalSplit, VerticalSplit, Tiled, Floating)
- ✅ Session persistence (save/restore with PTY re-spawning)
- ✅ Mouse support (click-to-focus, scroll)
- ✅ Mouse-based pane border drag resizing
- ✅ Drag-and-drop pane reordering
- ✅ Sidebar-aware layout
- ✅ Session auto-restore on launch
- ✅ File browser → pane integration

**Tests**: 12 unit tests passing

See [Phase 2: Multiplexer](./phase2-multiplexer.md) for details.

## Phase 3: GPU Rendering

**Status**: ~100% Complete

**Goal**: High-performance GPU-accelerated rendering at 60fps.

**Key Components**:
- wgpu initialization (adapter, device, queue)
- Glyph atlas with fontdue rasterization
- Render pipeline with WGSL shaders
- Damage tracking
- Dedicated render thread

**Completed Features**:
- ✅ wgpu adapter + device + queue initialization (headless)
- ✅ Surface/window integration for presentation
- ✅ Glyph atlas texture packing (shelf algorithm)
- ✅ Vertex/instance data generation from grid cells
- ✅ Render pipeline skeleton (WGSL shader + bind group + draw-call encoder)
- ✅ Damage tracking (dirty cells/rows)
- ✅ Dedicated render thread with channel-based updates
- ✅ Ligature rendering
- ✅ Cursor rendering (block, bar, underline styles)
- ✅ Selection highlighting
- ✅ Smooth scrolling

**Tests**: 16 unit tests passing

See [Phase 3: GPU Rendering](./phase3-gpu-rendering.md) for details.

## Phase 4: Plugin System

**Status**: ~100% Complete

**Goal**: Flexible WASM-based plugin system with sandboxed execution.

**Key Components**:
- Plugin manifest and installation
- Wasmtime runtime
- Host APIs (ui, io, net, fs, platform)
- Permission system

**Completed Features**:
- ✅ PluginManifest schema + TOML loading
- ✅ PluginManager (scan, list, get, remove, install, update)
- ✅ PluginInstance (wasmtime load + start)
- ✅ Host API: `uterx::log`
- ✅ Host API: `uterx_ui::draw_text`, `get_size`, `create_overlay`
- ✅ Host API: `uterx_io::read`, `write`
- ✅ Host API: `uterx_net::http_get`, `http_post`, `tcp_connect`
- ✅ Host API: `uterx_fs::read_file`, `write_file` (sandboxed)
- ✅ Host API: `uterx_platform::bt_scan`, `keyring_get`
- ✅ Permission checking (grant/deny on first use)
- ✅ Plugin auto-update mechanism
- ✅ Plugin repository / registry integration

**Tests**: 14 unit tests passing

See [Phase 4: Plugin System](./phase4-plugin-system.md) for details.

## Phase 5: Example Plugins

**Status**: 🔄 In Progress

**Goal**: Demonstrate plugin capabilities with practical plugins.

**Plugins**:
- ✅ UterxAI - Multi-provider AI assistant pane (Claude/Gemini/OpenAI/xAI/Z.ai/Moonshot/Minimax)
- ⏳ Bluetooth Messaging - Device scanning, connection, encrypted messaging
- ⏳ Mesh & WLAN Messaging - P2P discovery, offline messaging, encryption
- ⏳ Midnight Blockchain Integration - Wallet management, private transactions with ZK-SNARKs
- ✅ Text/Code Editor - Built-in floating editor pane (syntect, modal vim, undo)
- ⏳ File Sharing - Secure P2P file sharing
- ⏳ Network Tools - Ping, traceroute, port-scan with visual graphs
- ⏳ SSH Tools - Multi-sessions, key management, tunneling
- ⏳ Converter - Unit/currency converter

**Completed**:
- ✅ UterxAI plugin scaffold with multi-provider config
- ✅ UterxAI transport API (http_post)
- ✅ UterxAI app lifecycle (MVP wiring)
- ✅ UterxAI UX (palette + help + prompt input)
- ✅ UterxAI end-to-end poll loop
- ✅ UterxAI provider integration (z.ai)
- ✅ Built-in editor pane (syntect, modal vim, undo)

**Tests**: 1 test passing (UterxAI config parsing)

See [Phase 5: Example Plugins](./phase5-example-plugins.md) for details.

## Phase 6: Polish & Release

**Status**: ⏳ Planned

**Goal**: Production-ready release with comprehensive documentation and tooling.

**Tasks**:
- ⏳ Cross-platform CI (GitHub Actions)
- ⏳ Benchmarks with criterion
- ⏳ mdBook documentation (this documentation!)
- ⏳ Crash reporting
- ⏳ Open-source release

See [Phase 6: Polish & Release](./phase6-polish-release.md) for details.

## Phase 7: Innovative Features

**Status**: ⏳ Planned

**Goal**: Extend uterx with innovative "Terminal Desktop" features.

**Top 3 Features**:
1. **Smart Workspace Manager** - Named workspace presets for different contexts
2. **Real-time Process Monitor** - Visual system resource monitoring
3. **Integrated Git Graph** - Visual Git history and branch display

**Additional Features**:
- Pane Snippets & Templates
- Intelligent Search Across Panes
- Pane History & Replay
- Collaborative Editing
- Keyboard Macro System
- Task Runner & Dashboard
- Smart Notifications & Alerts

See [Phase 7: Innovative Features](./phase7-innovative-features.md) for details.

## Sprint Planning

The development follows a sprint-based approach:

### Sprint 1: Working Single-Pane Terminal (Phase 1 completion)
- SGR parsing
- Async PTY IO
- Scrollback wiring
- ED/EL completion
- Alternate screen buffer
- Scroll regions
- Insert/Delete lines
- UI integration

### Sprint 2: Multiplexer Basics (Phase 2 core)
- Pane ↔ PTY binding
- Session management
- Split operations
- Tab create/close/switch
- Focus management
- Broadcast mode

### Sprint 3: Session Persistence & Mouse (Phase 2 polish)
- Session save/restore
- Mouse pane resizing
- Pane drag reordering

### Sprint 4: GPU Rendering (Phase 3)
- wgpu init
- Atlas texture
- Render pipeline
- Damage tracking
- Render thread

### Sprint 5: Plugin System (Phase 4)
- Host API implementation
- Plugin installation
- Permission prompts
- Plugin pane rendering

### Sprint 6-8: Example Plugins (Phase 5)
- UterxAI (multi-provider AI assistant)
- Converter
- Network Tools
- SSH Tools
- Text/Code Editor (built-in)
- File Sharing
- Bluetooth Messaging
- Mesh & WLAN Messaging
- Midnight Blockchain

## Status Tracker

| Date | Phase | Milestone | Notes |
|------|-------|-----------|-------|
| 2026-02-20 | Phase 1 | Project scaffolding created | All crate skeletons done |
| 2026-02-21 | Phase 1 | Code audit & build fix | Fixed workspace members, tracing-subscriber env-filter, unused imports |
| 2026-02-21 | Phase 1 | Sprint 1 complete | Full SGR, async PTY IO, scrollback, ED/EL, alt screen, scroll regions, insert/delete lines, ESC/OSC handling, DEC private modes, UI integration |
| 2026-02-21 | Phase 2 | Sprint 2 complete | Pane↔PTY binding, Session/Tab/Pane lifecycle, splits, focus, broadcast |
| 2026-02-21 | Phase 2 | Sprint 3 complete | Session persistence, mouse capture + click-to-focus pane |
| 2026-02-22 | Phase 2 | UI Overhaul complete | Catppuccin theme, redesigned TabBar/StatusBar, HelpOverlay, CommandPalette, overlay system |
| 2026-02-22 | Phase 2 | File Browser & Desktop Features | File browser sidebar, focus system, folder→pane, sidebar layout |
| 2026-02-23 | Phase 2 | Mouse Tab Navigation & Floating Panes | Mouse-driven tab bar, floating panes (Alt+F), drag title bar |
| 2026-02-21 | Phase 2 | File Browser Search + Floating Editor | File browser search, floating editor pane with syntax highlighting |
| 2026-02-21 | Phase 2 | Resize + Desktop Click UX + Auto-Restore | Pane border drag resizing, desktop-like file browser clicks, session auto-restore |
| 2026-02-21 | Phase 2 | Pane Reordering complete | Drag-and-drop reordering for tiled panes |
| 2026-02-21 | Phase 3 | wgpu Init Kickoff | Renderer with wgpu adapter, device, queue initialization |
| 2026-02-21 | Phase 3 | Grid → Frame Geometry | CellInstance + FrameGeometry, frame geometry builder |
| 2026-02-21 | Phase 3 | Damage Tracking | DamageReport, grid snapshot diffing, dirty cell/row tracking |
| 2026-02-21 | Phase 3 | Render Pipeline Skeleton | WGSL shader, globals uniform, bind-group, render pipeline |
| 2026-02-21 | Phase 3 | Glyph Atlas Shelf Packing | Shelf-based texture packing, cache-backed API |
| 2026-02-21 | Phase 3 | Dedicated Render Thread | Channel-based command loop, RenderThreadHandle API |
| 2026-02-21 | Phase 3 | Cursor Rendering Styles | CursorStyle (Block, Bar, Underline), CursorInstance geometry |
| 2026-02-21 | Phase 3 | Surface/Window Presentation Path | try_init_surface_from_window, configure_surface, render_to_surface |
| 2026-02-21 | Phase 3 | Selection Highlighting | SelectionRange, SelectionInstance, overlay geometry |
| 2026-02-21 | Phase 3 | Ligature Rendering | Pattern-based run detection, row-level grouping |
| 2026-02-21 | Phase 3 | Smooth Scrolling | Scroll offset interpolation, scroll_smoothing_factor |
| 2026-02-21 | Phase 4 | Plugin Installation Pipeline | Install from local, zip, tar.gz, URL |
| 2026-02-22 | Phase 4 | UI Host API | draw_text, get_size, create_overlay |
| 2026-02-22 | Phase 4 | IO Host API | read, write |
| 2026-02-22 | Phase 4 | Network Host API | http_get, http_post, tcp_connect |
| 2026-02-22 | Phase 4 | Filesystem Host API | read_file, write_file (sandboxed) |
| 2026-02-22 | Phase 4 | Platform Host API | bt_scan, keyring_get |
| 2026-02-22 | Phase 4 | Permission Prompts | First-use grant/deny/always |
| 2026-02-22 | Phase 4 | Plugin Auto-Update | Update from install-source metadata |
| 2026-02-22 | Phase 4 | Plugin Repository/Registry | Local registry.toml, remote index URL |
| 2026-02-22 | Phase 5 | UterxAI Kickoff | Plugin scaffold, multi-provider config |
| 2026-02-22 | Phase 5 | UterxAI Transport API | http_post host function |
| 2026-02-22 | Phase 5 | UterxAI App Lifecycle | Plugin load/start, dedicated pane (Ctrl+Shift+A) |
| 2026-02-22 | Phase 5 | UterxAI UX | Palette entry, help overlay, prompt input |
| 2026-02-22 | Phase 5 | UterxAI End-to-End Poll Loop | uterxai_poll export, prompt→plugin→host loop |
| 2026-02-22 | Phase 5 | UterxAI Provider Integration | z.ai provider with real API calls |
| 2026-02-21 | Phase 7 | Feature Brainstorming | 10 innovative features designed |

## Next Steps

- Continue Phase 5: Complete remaining example plugins
- Begin Phase 6: Set up CI, benchmarks, and documentation
- Plan Phase 7: Implement top 3 innovative features
