# Phase 2: Multiplexer

**Status**: ✅ ~85% Complete

**Goal**: Multi-pane, multi-tab terminal workspace with session persistence.

## Overview

Phase 2 implements the multiplexer functionality, allowing users to manage multiple panes and tabs within a single terminal session. This includes pane splitting, tab management, session persistence, and mouse-based interactions.

## Components

### uterx-mux

The multiplexer crate managing panes, tabs, and sessions.

#### Key Types

- [`Pane`](../../crates/uterx-mux/src/pane.rs) - Represents a terminal pane
- [`Tab`](../../crates/uterx-mux/src/tab.rs) - Manages panes within a tab
- [`Session`](../../crates/uterx-mux/src/session.rs) - Manages tabs and session persistence
- [`Layout`](../../crates/uterx-mux/src/layout.rs) - Layout engine for pane positioning

## Completed Features

### Pane Management

#### Pane Structure

Each pane contains its own PTY, parser, and grid:

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
```

#### Pane Lifecycle

```rust
impl Pane {
    pub fn new(id: PaneId, shell: &str, cols: u16, rows: u16) -> Result<Self> {
        let pty = PtyProcess::spawn(shell, cols, rows)?;
        let parser = Parser::new();
        let grid = Grid::new(cols, rows);
        let scrollback = Scrollback::new(10000);

        Ok(Self {
            id,
            title: "Terminal".to_string(),
            pty: Some(pty),
            parser,
            grid,
            scrollback,
            rect: Rect::default(),
            is_focused: false,
            is_floating: false,
            float_pos: (0, 0),
            float_size: (cols, rows),
        })
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        if let Some(pty) = &mut self.pty {
            pty.resize(cols, rows)?;
        }
        self.grid.resize(cols, rows);
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        if let Some(mut pty) = self.pty.take() {
            pty.writer().write_all(b"exit\n")?;
        }
        Ok(())
    }
}
```

### Tab Management

#### Tab Structure

```rust
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub panes: Vec<Pane>,
    pub layout: Layout,
    pub focused_pane_id: Option<PaneId>,
    pub broadcast: bool,
}
```

#### Tab Operations

```rust
impl Tab {
    pub fn new(id: TabId, shell: &str, cols: u16, rows: u16) -> Result<Self> {
        let pane = Pane::new(PaneId::new(0), shell, cols, rows)?;
        let layout = Layout::Single;

        Ok(Self {
            id,
            title: "Tab 1".to_string(),
            panes: vec![pane],
            layout,
            focused_pane_id: Some(PaneId::new(0)),
            broadcast: false,
        })
    }

    pub fn split_horizontal(&mut self, shell: &str, cols: u16, rows: u16) -> Result<PaneId> {
        let new_id = PaneId::new(self.panes.len() as u64);
        let pane = Pane::new(new_id, shell, cols, rows)?;
        self.panes.push(pane);

        // Update layout
        self.layout = Layout::HorizontalSplit { split_ratio: 0.5 };

        Ok(new_id)
    }

    pub fn split_vertical(&mut self, shell: &str, cols: u16, rows: u16) -> Result<PaneId> {
        let new_id = PaneId::new(self.panes.len() as u64);
        let pane = Pane::new(new_id, shell, cols, rows)?;
        self.panes.push(pane);

        // Update layout
        self.layout = Layout::VerticalSplit { split_ratio: 0.5 };

        Ok(new_id)
    }

    pub fn remove_pane(&mut self, pane_id: PaneId) -> Result<()> {
        self.panes.retain(|p| p.id != pane_id);
        if self.panes.is_empty() {
            return Err(anyhow::anyhow!("Cannot remove last pane"));
        }
        self.layout = Layout::Single;
        Ok(())
    }

    pub fn focus_next(&mut self) {
        if let Some(current) = self.focused_pane_id {
            let idx = self.panes.iter().position(|p| p.id == current).unwrap_or(0);
            let next_idx = (idx + 1) % self.panes.len();
            self.focused_pane_id = Some(self.panes[next_idx].id);
        }
    }

    pub fn focus_prev(&mut self) {
        if let Some(current) = self.focused_pane_id {
            let idx = self.panes.iter().position(|p| p.id == current).unwrap_or(0);
            let prev_idx = if idx == 0 { self.panes.len() - 1 } else { idx - 1 };
            self.focused_pane_id = Some(self.panes[prev_idx].id);
        }
    }

    pub fn toggle_broadcast(&mut self) {
        self.broadcast = !self.broadcast;
    }

    pub fn toggle_float(&mut self) {
        if let Some(pane_id) = self.focused_pane_id {
            if let Some(pane) = self.panes.iter_mut().find(|p| p.id == pane_id) {
                pane.is_floating = !pane.is_floating;
                if pane.is_floating {
                    pane.float_size = (pane.rect.width * 3 / 5, pane.rect.height * 3 / 5);
                    pane.float_pos = (
                        pane.rect.x + pane.rect.width / 5,
                        pane.rect.y + pane.rect.height / 5,
                    );
                }
            }
        }
    }
}
```

### Layout Engine

#### Layout Variants

```rust
pub enum Layout {
    Single,
    HorizontalSplit { split_ratio: f32 },
    VerticalSplit { split_ratio: f32 },
    Tiled { rows: u16, cols: u16 },
    Floating,
}

impl Layout {
    pub fn compute_rects(&self, area: Rect, panes: &[Pane]) -> Vec<Rect> {
        match self {
            Layout::Single => vec![area],
            Layout::HorizontalSplit { split_ratio } => {
                let split = (area.width as f32 * split_ratio) as u16;
                vec![
                    Rect::new(area.x, area.y, split, area.height),
                    Rect::new(area.x + split, area.y, area.width - split, area.height),
                ]
            }
            Layout::VerticalSplit { split_ratio } => {
                let split = (area.height as f32 * split_ratio) as u16;
                vec![
                    Rect::new(area.x, area.y, area.width, split),
                    Rect::new(area.x, area.y + split, area.width, area.height - split),
                ]
            }
            Layout::Tiled { rows, cols } => {
                let pane_width = area.width / cols;
                let pane_height = area.height / rows;
                panes.iter()
                    .enumerate()
                    .map(|(i, _)| {
                        let row = i as u16 / cols;
                        let col = i as u16 % cols;
                        Rect::new(
                            area.x + col * pane_width,
                            area.y + row * pane_height,
                            pane_width,
                            pane_height,
                        )
                    })
                    .collect()
            }
            Layout::Floating => panes.iter()
                .filter(|p| p.is_floating)
                .map(|p| Rect::new(p.float_pos.0, p.float_pos.1, p.float_size.0, p.float_size.1))
                .collect(),
        }
    }
}
```

### Session Management

#### Session Structure

```rust
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub tabs: Vec<Tab>,
    pub active_tab_id: Option<TabId>,
    next_tab_id: u64,
}

impl Session {
    pub fn new(name: String) -> Self {
        Self {
            id: SessionId::new(),
            name,
            tabs: Vec::new(),
            active_tab_id: None,
            next_tab_id: 0,
        }
    }

    pub fn create_tab(&mut self, shell: &str, cols: u16, rows: u16) -> Result<TabId> {
        let id = TabId::new(self.next_tab_id);
        self.next_tab_id += 1;
        let tab = Tab::new(id, shell, cols, rows)?;
        self.tabs.push(tab);
        self.active_tab_id = Some(id);
        Ok(id)
    }

    pub fn close_tab(&mut self, tab_id: TabId) -> Result<()> {
        if self.tabs.len() <= 1 {
            return Err(anyhow::anyhow!("Cannot close last tab"));
        }
        self.tabs.retain(|t| t.id != tab_id);
        if self.active_tab_id == Some(tab_id) {
            self.active_tab_id = self.tabs.first().map(|t| t.id);
        }
        Ok(())
    }

    pub fn next_tab(&mut self) {
        if let Some(current) = self.active_tab_id {
            let idx = self.tabs.iter().position(|t| t.id == current).unwrap_or(0);
            let next_idx = (idx + 1) % self.tabs.len();
            self.active_tab_id = Some(self.tabs[next_idx].id);
        }
    }

    pub fn prev_tab(&mut self) {
        if let Some(current) = self.active_tab_id {
            let idx = self.tabs.iter().position(|t| t.id == current).unwrap_or(0);
            let prev_idx = if idx == 0 { self.tabs.len() - 1 } else { idx - 1 };
            self.active_tab_id = Some(self.tabs[prev_idx].id);
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let config = SessionConfig::from_session(self);
        let toml = toml::to_string_pretty(&config)?;
        std::fs::write(path, toml)?;
        Ok(())
    }

    pub fn load(path: &Path, shell: &str, cols: u16, rows: u16) -> Result<Self> {
        let toml = std::fs::read_to_string(path)?;
        let config: SessionConfig = toml::from_str(&toml)?;
        config.to_session(shell, cols, rows)
    }
}
```

### Mouse Support

#### Pane Resizing

```rust
impl Tab {
    pub fn adjust_split_boundary(&mut self, delta: i16, area: Rect) -> Result<()> {
        match &mut self.layout {
            Layout::HorizontalSplit { split_ratio } => {
                let current = (*split_ratio * area.width as f32) as i16;
                let new = (current + delta).clamp(area.width as i16 / 10, area.width as i16 * 9 / 10);
                *split_ratio = new as f32 / area.width as f32;
            }
            Layout::VerticalSplit { split_ratio } => {
                let current = (*split_ratio * area.height as f32) as i16;
                let new = (current + delta).clamp(area.height as i16 / 10, area.height as i16 * 9 / 10);
                *split_ratio = new as f32 / area.height as f32;
            }
            _ => {}
        }
        Ok(())
    }
}
```

#### Pane Reordering

```rust
impl Tab {
    pub fn reorder_tiled_panes(&mut self, dragged: PaneId, target: PaneId) -> Result<()> {
        let dragged_idx = self.panes.iter().position(|p| p.id == dragged)
            .ok_or_else(|| anyhow::anyhow!("Dragged pane not found"))?;
        let target_idx = self.panes.iter().position(|p| p.id == target)
            .ok_or_else(|| anyhow::anyhow!("Target pane not found"))?;

        let pane = self.panes.remove(dragged_idx);
        self.panes.insert(target_idx, pane);
        Ok(())
    }
}
```

### Broadcast Mode

When broadcast mode is active, input is sent to all panes:

```rust
impl Tab {
    pub fn broadcast_input(&self, bytes: &[u8]) {
        if self.broadcast {
            for pane in &self.panes {
                if let Some(pty) = &pane.pty {
                    let _ = pty.writer().write_all(bytes);
                }
            }
        }
    }
}
```

## Keybindings

| Keybinding | Action |
|------------|--------|
| `Alt+H` | Split pane horizontally |
| `Alt+V` | Split pane vertically |
| `Alt+Left` | Focus previous pane |
| `Alt+Right` | Focus next pane |
| `Alt+B` | Toggle broadcast mode |
| `Alt+F` | Toggle floating pane |
| `Ctrl+T` | Create new tab |
| `Ctrl+W` | Close pane |
| `Ctrl+Tab` | Switch to next tab |
| `Ctrl+Shift+Tab` | Switch to previous tab |

## Testing

### Unit Tests

12 unit tests passing:

```bash
cargo test -p uterx-mux
```

Tests cover:
- Tab add/focus
- Tab cycling
- Tab remove
- Pane split
- Broadcast mode
- Layout relayout
- Split boundary resize and clamping
- Tiled pane reordering
- Session roundtrip
- Session save/load

## Related Documentation

- [Architecture](./architecture.md) - Overall architecture
- [Getting Started](./getting-started.md) - User guide
- [Phase 1](./phase1-core-terminal.md) - Core terminal
