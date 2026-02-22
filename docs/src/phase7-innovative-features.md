# Phase 7: Innovative Features

**Status**: ⏳ Planned

**Goal**: Extend uterx with innovative "Terminal Desktop" features.

## Overview

Phase 7 introduces innovative features that enhance the "Terminal Desktop" concept, making uterx more powerful and user-friendly. These features are designed to improve productivity, provide better system integration, and offer new ways to interact with terminal-based workflows.

## Top 3 Features (Priority Order)

### 1. Smart Workspace Manager

**Priority**: High

**Description**: Named workspace presets for different development contexts.

**Use Cases**:
- "Dev" workspace: terminal, editor, file browser
- "Admin" workspace: SSH panes, network monitor
- "Data" workspace: database panes, log viewer

**Implementation**:

```rust
pub struct WorkspaceManager {
    workspaces: HashMap<String, WorkspaceConfig>,
    active_workspace: Option<String>,
}

pub struct WorkspaceConfig {
    pub name: String,
    pub tabs: Vec<TabConfig>,
    pub layout: LayoutConfig,
    pub plugins: Vec<String>,
}

impl WorkspaceManager {
    pub fn create_workspace(&mut self, name: String, config: WorkspaceConfig) {
        self.workspaces.insert(name.clone(), config);
    }

    pub fn load_workspace(&mut self, name: &str) -> Result<()> {
        let config = self.workspaces.get(name)
            .ok_or_else(|| anyhow::anyhow!("Workspace not found"))?;

        // Create tabs and panes from config
        // Load required plugins
        // Apply layout

        self.active_workspace = Some(name.to_string());
        Ok(())
    }

    pub fn save_workspace(&self, name: &str) -> Result<()> {
        // Save current session state as workspace
        Ok(())
    }
}
```

**Keybindings**:
- `Ctrl+Shift+W` - Save current workspace
- `Ctrl+Shift+L` - Load workspace
- `Ctrl+Shift+N` - New workspace

**Storage**: `~/.uterx/workspaces/<name>.toml`

### 2. Real-time Process Monitor

**Priority**: High

**Description**: Visual system resource monitoring (CPU, RAM, Disk, Network).

**Use Cases**:
- Like htop but in uterx style
- Floating pane with Catppuccin theme
- Updates every 1-2 seconds

**Implementation**:

```rust
pub struct ProcessMonitorWidget {
    processes: Vec<ProcessInfo>,
    system: System,
    update_interval: Duration,
}

pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub state: ProcessState,
}

impl ProcessMonitorWidget {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            system: System::new_all(),
            update_interval: Duration::from_secs(1),
        }
    }

    pub fn update(&mut self) {
        self.system.refresh_all();
        self.processes = self.system.processes()
            .values()
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string(),
                cpu_usage: p.cpu_usage(),
                memory_usage: p.memory(),
                state: p.state(),
            })
            .collect();
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        // Render process list with CPU/memory bars
        // Use Catppuccin colors
    }
}
```

**Keybindings**:
- `Ctrl+Shift+M` - Toggle process monitor
- `k/j` - Navigate processes
- `K` - Kill selected process
- `P` - Sort by CPU
- `M` - Sort by memory

**Dependencies**: `sysinfo` crate

### 3. Integrated Git Graph

**Priority**: High

**Description**: Visual Git history and branch display.

**Use Cases**:
- Floating pane shows graph like GitKraken/Sourcetree
- Clickable commits
- Branch visualization

**Implementation**:

```rust
pub struct GitGraphWidget {
    commits: Vec<CommitInfo>,
    branches: Vec<BranchInfo>,
    selected_commit: Option<String>,
}

pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    pub parents: Vec<String>,
    pub branches: Vec<String>,
}

pub struct BranchInfo {
    pub name: String,
    pub commit_hash: String,
    pub is_head: bool,
}

impl GitGraphWidget {
    pub fn new(repo_path: &Path) -> Result<Self> {
        let repo = git2::Repository::open(repo_path)?;
        let mut walk = repo.revwalk()?;
        walk.push_head()?;

        let commits = walk
            .map(|oid| {
                let oid = oid?;
                let commit = repo.find_commit(oid)?;
                Ok(CommitInfo {
                    hash: oid.to_string(),
                    message: commit.message().unwrap_or("").to_string(),
                    author: commit.author().name().unwrap_or("").to_string(),
                    timestamp: DateTime::from_timestamp(commit.time().seconds(), 0).unwrap_or_default(),
                    parents: commit.parent_ids().map(|id| id.to_string()).collect(),
                    branches: Vec::new(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            commits,
            branches: Vec::new(),
            selected_commit: None,
        })
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        // Render commit graph with branch lines
        // Use Catppuccin colors for different branches
    }
}
```

**Keybindings**:
- `Ctrl+Shift+G` - Toggle git graph
- `j/k` - Navigate commits
- `Enter` - Show commit details
- `b` - Checkout branch
- `c` - Create branch
- `d` - Delete branch

**Dependencies**: `git2` crate

## Additional Features

### 4. Pane Snippets & Templates

**Priority**: Medium

**Description**: Reusable pane configurations for common workflows.

**Use Cases**:
- "Docker Dev" = 3 panes (logs, shell, db)
- "Frontend" = (npm run dev, git, editor)

**Implementation**:

```rust
pub struct PaneTemplate {
    pub name: String,
    pub description: String,
    pub panes: Vec<PaneTemplateConfig>,
}

pub struct PaneTemplateConfig {
    pub title: String,
    pub command: String,
    pub layout: LayoutPosition,
}

impl PaneTemplate {
    pub fn apply(&self, session: &mut Session, shell: &str, cols: u16, rows: u16) -> Result<()> {
        for pane_config in &self.panes {
            // Create pane with specified command
            // Apply layout
        }
        Ok(())
    }
}
```

**Keybindings**:
- `Ctrl+Shift+T` - Open template palette

### 5. Intelligent Search Across Panes

**Priority**: High

**Description**: Global search across all open terminals and files.

**Use Cases**:
- `Ctrl+Shift+F` → search text → results in all panes

**Implementation**:

```rust
pub struct GlobalSearchEngine {
    panes: HashMap<PaneId, PaneSearchIndex>,
}

pub struct PaneSearchIndex {
    pub lines: Vec<String>,
    pub line_numbers: Vec<usize>,
}

impl GlobalSearchEngine {
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();
        for (pane_id, index) in &self.panes {
            for (line_idx, line) in index.lines.iter().enumerate() {
                if line.contains(query) {
                    results.push(SearchResult {
                        pane_id: *pane_id,
                        line_number: index.line_numbers[line_idx],
                        line: line.clone(),
                        match_positions: find_match_positions(line, query),
                    });
                }
            }
        }
        results
    }
}
```

**Keybindings**:
- `Ctrl+Shift+F` - Open global search
- `Tab` - Cycle through results
- `Enter` - Jump to result

### 6. Pane History & Replay

**Priority**: Medium

**Description**: Time travel through pane contents.

**Use Cases**:
- Clickable timestamps restore pane state
- Like iTerm2 Shell Integration

**Implementation**:

```rust
pub struct HistoryManager {
    snapshots: VecDeque<HistorySnapshot>,
    max_snapshots: usize,
}

pub struct HistorySnapshot {
    pub timestamp: DateTime<Utc>,
    pub grid: Grid,
    pub scrollback: Scrollback,
}

impl HistoryManager {
    pub fn capture(&mut self, grid: &Grid, scrollback: &Scrollback) {
        let snapshot = HistorySnapshot {
            timestamp: Utc::now(),
            grid: grid.clone(),
            scrollback: scrollback.clone(),
        };
        self.snapshots.push_back(snapshot);
        if self.snapshots.len() > self.max_snapshots {
            self.snapshots.pop_front();
        }
    }

    pub fn restore(&self, index: usize) -> Option<(&Grid, &Scrollback)> {
        self.snapshots.get(index).map(|s| (&s.grid, &s.scrollback))
    }
}
```

**Keybindings**:
- `Ctrl+Shift+H` - Toggle history view
- `j/k` - Navigate history
- `Enter` - Restore snapshot

### 7. Collaborative Editing

**Priority**: Low (complex)

**Description**: Multi-user pane sharing (remote or local).

**Use Cases**:
- Pair programming via WebRTC or local multi-terminal

**Implementation**:

```rust
pub struct CollaborationManager {
    pub sessions: HashMap<SessionId, CollaborationSession>,
}

pub struct CollaborationSession {
    pub participants: Vec<Participant>,
    pub shared_panes: Vec<PaneId>,
    pub delta_queue: VecDeque<Delta>,
}

pub struct Delta {
    pub pane_id: PaneId,
    pub changes: Vec<CellChange>,
}
```

**Keybindings**:
- `Ctrl+Shift+C` - Start collaboration
- `Ctrl+Shift+I` - Invite participant

### 8. Keyboard Macro System

**Priority**: Medium

**Description**: Record and replay keyboard sequences.

**Use Cases**:
- `Ctrl+R` to record → enter sequence → `Ctrl+P` to replay

**Implementation**:

```rust
pub struct MacroRecorder {
    pub recording: bool,
    pub current_macro: Vec<KeyEvent>,
    pub macros: HashMap<String, Vec<KeyEvent>>,
}

impl MacroRecorder {
    pub fn start_recording(&mut self) {
        self.recording = true;
        self.current_macro.clear();
    }

    pub fn stop_recording(&mut self) -> Result<String> {
        self.recording = false;
        let name = format!("macro_{}", self.macros.len() + 1);
        self.macros.insert(name.clone(), self.current_macro.clone());
        Ok(name)
    }

    pub fn record_key(&mut self, key: KeyEvent) {
        if self.recording {
            self.current_macro.push(key);
        }
    }

    pub fn replay(&self, name: &str) -> Result<Vec<KeyEvent>> {
        self.macros.get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Macro not found"))
    }
}
```

**Keybindings**:
- `Ctrl+R` - Start/stop recording
- `Ctrl+P` - Play last macro
- `Ctrl+Shift+P` - Play named macro

### 9. Task Runner & Dashboard

**Priority**: Medium

**Description**: Define and execute build/test/deploy tasks.

**Use Cases**:
- Dashboard shows running task status
- Logs in separate panes

**Implementation**:

```rust
pub struct TaskRunner {
    pub tasks: HashMap<String, TaskConfig>,
    pub running_tasks: HashMap<TaskId, RunningTask>,
}

pub struct TaskConfig {
    pub name: String,
    pub command: String,
    pub working_dir: PathBuf,
    pub env: HashMap<String, String>,
}

pub struct RunningTask {
    pub id: TaskId,
    pub config: TaskConfig,
    pub process: Child,
    pub output_pane_id: PaneId,
}
```

**Keybindings**:
- `Ctrl+Shift+K` - Open task dashboard
- `Ctrl+Shift+R` - Run task

### 10. Smart Notifications & Alerts

**Priority**: Low

**Description**: Visual notifications for important events.

**Use Cases**:
- Toast overlay for completed long-running commands
- Error notifications

**Implementation**:

```rust
pub struct NotificationManager {
    pub notifications: VecDeque<Notification>,
}

pub struct Notification {
    pub id: u64,
    pub title: String,
    pub message: String,
    pub level: NotificationLevel,
    pub timestamp: DateTime<Utc>,
}

pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

impl NotificationManager {
    pub fn show(&mut self, title: String, message: String, level: NotificationLevel) {
        let notification = Notification {
            id: self.notifications.len() as u64,
            title,
            message,
            level,
            timestamp: Utc::now(),
        };
        self.notifications.push_back(notification);
    }

    pub fn dismiss(&mut self, id: u64) {
        self.notifications.retain(|n| n.id != id);
    }
}
```

**Keybindings**:
- `Ctrl+Shift+N` - Toggle notifications
- `Esc` - Dismiss notification

## Implementation Phases

### Phase 7.1: Quick Wins (Low complexity, high value)

- Smart Workspace Manager
- Keyboard Macro System
- Smart Notifications & Alerts

### Phase 7.2: Core Features (Medium complexity)

- Real-time Process Monitor
- Pane Snippets & Templates
- Task Runner & Dashboard
- Integrated Git Graph

### Phase 7.3: Advanced Features (High complexity)

- Intelligent Search Across Panes
- Pane History & Replay
- Collaborative Editing

## Technical Considerations

### Performance

All features designed for low-end devices with:
- Async operations
- Efficient data structures
- Lazy loading where applicable

### Modularity

Each feature can be developed independently as:
- Separate widgets
- Extensions
- Optional plugins

### Consistency

All features follow uterx's:
- Catppuccin theme
- UX patterns
- Keybinding conventions

### Extensibility

Features use existing abstractions:
- Session management
- Pane system
- Widget system
- Plugin APIs

## Related Documentation

- [Architecture](./architecture.md) - Overall architecture
- [Getting Started](./getting-started.md) - User guide
- [Phase 2](./phase2-multiplexer.md) - Multiplexer details
