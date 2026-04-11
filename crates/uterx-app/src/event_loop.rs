//! Main event loop: terminal setup, input handling, PTY IO, rendering.
//!
//! Uses the uterx-mux Session/Tab/Pane architecture.
//! Each pane owns its own PTY, parser, and grid.
//! Tokio async with crossterm event stream.
//! Features:
//!   - Catppuccin Mocha themed UI
//!   - Tab bar with branding, numbered tabs, action buttons
//!   - Status bar with session info, pane count, keybinding hints
//!   - Help overlay (F1) with full keybinding reference
//!   - Command palette (Ctrl+P) with fuzzy search
//!   - File browser sidebar (Ctrl+E) — desktop-like directory navigation
//!   - Open folders in new panes for a "Terminal Desktop" experience

use crate::config::AppConfig;
use crossterm::{
    ExecutableCommand,
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind,
        KeyModifiers, MouseButton, MouseEventKind,
    },
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders},
};
use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use uterx_mux::{Rect as MuxRect, Session};
use uterx_mux::layout::SplitDirection;
use uterx_ui::{TerminalView, SearchMatch as UiSearchMatch};
use uterx_ui::input::{Action, InputHandler};
use uterx_ui::widgets::ai_sidebar::{
    AiProvider as UiAiProvider, AiSidebarField, AiSidebarState, AiSidebarWidget,
};
use uterx_ui::widgets::command_palette::{CommandEntry, CommandPalette, default_commands};
use uterx_ui::widgets::editor::{EditorMode, EditorState, EditorWidget};
use uterx_ui::widgets::file_browser::{FileBrowserState, FileBrowserWidget};
use uterx_ui::widgets::help_overlay::HelpOverlay;
use uterx_ui::widgets::status_bar::StatusBar;
use uterx_ui::widgets::tab_bar::{TabBar, TabInfo};

/// UI overlay state.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Overlay {
    None,
    Help,
    CommandPalette,
    Search,
    TabRename { tab_id: uterx_mux::TabId },
}

/// Where keyboard input is currently directed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Focus {
    Terminal,
    FileBrowser,
    AiSidebar,
    Editor(u64),
}

/// A floating editor pane for viewing/editing files.
struct EditorPane {
    id: u64,
    state: EditorState,
    /// Position/size in screen coordinates (includes chrome offset already).
    rect: ratatui::layout::Rect,
}

/// State for mouse-dragging a floating pane.
struct DragState {
    pane_id: uterx_mux::PaneId,
    /// Mouse offset from the pane's top-left corner.
    offset_x: u16,
    offset_y: u16,
}

/// State for mouse-dragging an editor pane.
struct EditorDragState {
    editor_id: u64,
    offset_x: u16,
}

/// State for resizing split panes.
struct SplitResizeState {
    /// Index of the first pane in the split (the one before the border)
    pane_idx: usize,
    /// Initial mouse position when drag started
    start_x: u16,
    start_y: u16,
}

/// State for mouse-drag text selection.
struct SelectionDragState {
    pane_id: uterx_mux::PaneId,
    start_row: usize,
    start_col: usize,
}

/// Search match position (row, col, length).
#[derive(Debug, Clone, Copy)]
struct SearchMatch {
    row: usize,
    col: usize,
    len: usize,
}

/// State for terminal search functionality.
struct SearchState {
    query: String,
    matches: Vec<SearchMatch>,
    current_match: usize,
}

impl SearchState {
    fn new() -> Self {
        Self {
            query: String::new(),
            matches: Vec::new(),
            current_match: 0,
        }
    }

    fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.current_match = 0;
    }

    fn find_matches(&mut self, pane: &uterx_mux::Pane) {
        self.matches.clear();
        if self.query.is_empty() {
            return;
        }

        let query_lower = self.query.to_lowercase();

        // Search in current grid
        for row in 0..pane.grid.rows() {
            for col in 0..pane.grid.cols() {
                if let Some(cell) = pane.grid.cell(row, col) {
                    let cell_text = cell.content.to_lowercase();
                    if cell_text.starts_with(&query_lower) {
                        self.matches.push(SearchMatch {
                            row,
                            col,
                            len: self.query.len(),
                        });
                    }
                }
            }
        }

        // Sort by row, then col
        self.matches.sort_by(|a, b| {
            a.row.cmp(&b.row).then_with(|| a.col.cmp(&b.col))
        });

        self.current_match = 0;
    }

    fn next_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        self.current_match = (self.current_match + 1) % self.matches.len();
        Some(&self.matches[self.current_match])
    }

    fn prev_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        if self.current_match == 0 {
            self.current_match = self.matches.len() - 1;
        } else {
            self.current_match -= 1;
        }
        Some(&self.matches[self.current_match])
    }
}

/// Transient notification shown in the status bar.
struct Notification {
    msg: String,
    created: Instant,
    duration: Duration,
}

impl Notification {
    fn new(msg: String, duration_ms: u64) -> Self {
        Self {
            msg,
            created: Instant::now(),
            duration: Duration::from_millis(duration_ms),
        }
    }

    fn expired(&self) -> bool {
        Instant::now().duration_since(self.created) >= self.duration
    }
}

/// Run the main terminal event loop.
pub async fn run(mut cfg: AppConfig) -> anyhow::Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Show help on first launch
    let mut overlay = if cfg.is_first_launch() {
        cfg.mark_first_launch_done()?;
        Overlay::Help
    } else {
        Overlay::None
    };

    let size = terminal.size()?;
    let cols = size.width;
    let rows = size.height.saturating_sub(2); // Reserve for tab bar + status bar

    let shell = cfg.shell();
    let show_tab_bar = cfg.ui.show_tab_bar;
    let show_status_bar = cfg.ui.show_status_bar;

    // Create session with an initial tab + pane
    let mut session = Session::new("main");
    let pane_area = MuxRect {
        x: 0,
        y: 0,
        width: cols,
        height: rows,
    };
    session.create_tab(&shell, pane_area)?;

    // Set the first pane as focused
    if let Some(tab) = session.active_tab_mut() {
        if let Some(pane) = tab.focused_pane_mut() {
            pane.focused = true;
        }
    }

    let input_handler = InputHandler::new();

    // Command palette state
    let commands = default_commands();
    let mut palette_selected: usize = 0;
    let mut palette_filter = String::new();
    let mut filtered_commands: Vec<CommandEntry> = commands.clone();
    let mut recent_commands: VecDeque<String> = VecDeque::new(); // Track recently used commands

    // File browser state
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let mut file_browser = FileBrowserState::new(&home);
    file_browser.width = 32;
    file_browser.load_preview();

    // AI sidebar state — pre-populate from config
    let mut ai_sidebar = AiSidebarState::new();
    {
        let ui_provider = match cfg.ai.provider {
            crate::config::AiProvider::Anthropic => UiAiProvider::Anthropic,
            crate::config::AiProvider::Openai => UiAiProvider::OpenAi,
            crate::config::AiProvider::Ollama => UiAiProvider::Ollama,
            crate::config::AiProvider::Custom => UiAiProvider::Custom,
            crate::config::AiProvider::Zai => UiAiProvider::ZAi,
            crate::config::AiProvider::Kimi => UiAiProvider::Kimi,
        };
        ai_sidebar.load(
            ui_provider,
            &cfg.ai.api_key,
            &cfg.ai.model,
            &cfg.ai.base_url,
        );
    }

    // Focus state
    let mut focus = Focus::Terminal;

    // Floating pane drag state
    let mut dragging: Option<DragState> = None;

    // Editor panes
    let mut editor_panes: Vec<EditorPane> = Vec::new();
    let mut next_editor_id: u64 = 1;
    let mut editor_dragging: Option<EditorDragState> = None;

    // Split resize state for dragging pane borders
    let mut split_resizing: Option<SplitResizeState> = None;

    // Text selection drag state
    let mut selection_dragging: Option<SelectionDragState> = None;

    // Search state for "Find in Terminal"
    let mut search_state: SearchState = SearchState::new();

    // Tab rename buffer
    let mut tab_rename_buffer: String = String::new();

    // Notifications (transient messages displayed in status bar)
    let mut notifications: VecDeque<Notification> = VecDeque::new();

    // Helper function to push a notification into the queue.
    // We use an inner function (not a closure) to avoid borrow checker issues.
    fn push_notification(notifications: &mut VecDeque<Notification>, msg: String, dur_ms: u64) {
        notifications.push_front(Notification::new(msg, dur_ms));
        // keep a small history (e.g., 6)
        while notifications.len() > 6 {
            notifications.pop_back();
        }
    }

    // Channel for async test connection results
    let (test_tx, mut test_rx) = mpsc::unbounded_channel::<String>();

    // crossterm event stream
    let mut event_stream = EventStream::new();

    tracing::info!(
        "event loop started ({}x{}, shell={}, session={})",
        cols,
        rows,
        shell,
        session.name
    );

    loop {
        // Process PTY output for all panes
        session.process_all_pty_output();

        // Prune expired notifications before rendering so status area stays up-to-date.
        while notifications.front().map_or(false, |n| n.expired()) {
            notifications.pop_front();
        }

        // Render
        let fb_visible = file_browser.visible;
        let fb_width = file_browser.width;
        let fb_focused = focus == Focus::FileBrowser;
        let ai_visible = ai_sidebar.visible;
        let ai_width = ai_sidebar.width;
        let ai_focused = focus == Focus::AiSidebar;

        terminal.draw(|frame| {
            let area = frame.area();

            // Vertical layout: tab bar + main area + status bar
            let mut v_constraints = Vec::new();
            if show_tab_bar {
                v_constraints.push(Constraint::Length(1));
            }
            v_constraints.push(Constraint::Min(1));
            if show_status_bar {
                v_constraints.push(Constraint::Length(1));
            }

            let v_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(v_constraints)
                .split(area);

            let mut v_idx = 0;

            // ── Tab bar ──
            if show_tab_bar {
                let broadcast = session.active_tab().map(|t| t.broadcast).unwrap_or(false);
                let tabs: Vec<TabInfo> = session
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let name = if let Some(pane) = t.focused_pane() {
                            if pane.grid.title.is_empty() {
                                t.name.clone()
                            } else {
                                pane.grid.title.clone()
                            }
                        } else {
                            t.name.clone()
                        };
                        TabInfo {
                            name,
                            active: session.active_tab == Some(t.id),
                            index: i,
                        }
                    })
                    .collect();
                let tab_bar = TabBar::new(&tabs).broadcast(broadcast);
                frame.render_widget(tab_bar, v_chunks[v_idx]);
                v_idx += 1;
            }

            // ── Main area (sidebars + terminal) ──
            let main_area = v_chunks[v_idx];
            v_idx += 1;

            // Horizontal split: [file browser] | terminal | [AI sidebar]
            let (fb_area, term_area, ai_area) = match (fb_visible, ai_visible) {
                (false, false) => (None, main_area, None),
                (true, false) => {
                    let h = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Length(fb_width), Constraint::Min(1)])
                        .split(main_area);
                    (Some(h[0]), h[1], None)
                }
                (false, true) => {
                    let h = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Min(1), Constraint::Length(ai_width)])
                        .split(main_area);
                    (None, h[0], Some(h[1]))
                }
                (true, true) => {
                    let h = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Length(fb_width),
                            Constraint::Min(1),
                            Constraint::Length(ai_width),
                        ])
                        .split(main_area);
                    (Some(h[0]), h[1], Some(h[2]))
                }
            };

            // ── File browser sidebar (left) ──
            if let Some(sb_area) = fb_area {
                let fb_widget = FileBrowserWidget::new(&file_browser, fb_focused);
                frame.render_widget(fb_widget, sb_area);
            }

            // ── AI sidebar (right) ──
            if let Some(ai_sb_area) = ai_area {
                let ai_widget = AiSidebarWidget::new(&ai_sidebar, ai_focused);
                frame.render_widget(ai_widget, ai_sb_area);
            }

            // ── Terminal panes ──
            if let Some(tab) = session.active_tab() {
                // Check if a pane is maximized
                if let Some(maximized_pane) = tab.maximized_pane() {
                    // Render only the maximized pane, filling the entire area
                    let maximized_id = maximized_pane.id;
                    if let Some(pane) = tab.panes.iter().find(|p| p.id == maximized_id) {
                        // Show border and title for maximized pane
                        let border_style = Style::default()
                            .fg(Color::Rgb(166, 227, 161)) // Green accent for maximized
                            .add_modifier(Modifier::BOLD);
                        let title_style = Style::default()
                            .fg(Color::Rgb(205, 214, 244))
                            .add_modifier(Modifier::BOLD);
                        
                        let pane_title = if pane.grid.title.is_empty() {
                            "[MAXIMIZED] shell".to_string()
                        } else {
                            format!("[MAXIMIZED] {}", pane.grid.title)
                        };
                        
                        let block = Block::default()
                            .borders(Borders::ALL)
                            .border_style(border_style)
                            .title(Span::styled(format!(" {} ", pane_title), title_style));
                        let inner = block.inner(term_area);
                        frame.render_widget(block, term_area);
                        
                        // Prepare search matches for this pane
                        let search_matches: Vec<UiSearchMatch> = if overlay == Overlay::Search {
                            search_state.matches.iter().map(|m| UiSearchMatch {
                                row: m.row,
                                col: m.col,
                                len: m.len,
                            }).collect()
                        } else {
                            Vec::new()
                        };
                        
                        let view = TerminalView::new(&pane.grid)
                            .show_cursor(pane.focused)
                            .scrollback_offset(pane.scrollback_offset)
                            .selection(pane.selection.as_ref())
                            .search_matches(&search_matches, search_state.current_match);
                        frame.render_widget(view, inner);
                    }
                } else {
                    // Count only tiled panes for layout
                    let tiled_count = tab.panes.iter().filter(|p| !p.is_floating).count();

                    if tiled_count == 1 && tab.floating_panes().count() == 0 {
                        // Single tiled pane — fill the area
                        if let Some(pane) = tab.tiled_panes().next() {
                            // Prepare search matches for this pane
                            let search_matches: Vec<UiSearchMatch> = if overlay == Overlay::Search {
                                search_state.matches.iter().map(|m| UiSearchMatch {
                                    row: m.row,
                                    col: m.col,
                                    len: m.len,
                                }).collect()
                            } else {
                                Vec::new()
                            };
                            
                            let view = TerminalView::new(&pane.grid)
                                .scrollback_offset(pane.scrollback_offset)
                                .selection(pane.selection.as_ref())
                                .search_matches(&search_matches, search_state.current_match);
                            frame.render_widget(view, term_area);
                        }
                    } else {
                    // Multiple tiled panes — compute layout rects with styled borders
                    let mux_area = MuxRect {
                        x: term_area.x,
                        y: term_area.y,
                        width: term_area.width,
                        height: term_area.height,
                    };
                    let rects = tab.layout.compute_rects(mux_area, tiled_count);

                    for (pane, mux_rect) in tab.tiled_panes().zip(rects.iter()) {
                        let pane_area = ratatui::layout::Rect {
                            x: mux_rect.x,
                            y: mux_rect.y,
                            width: mux_rect.width,
                            height: mux_rect.height,
                        };

                        let (border_style, title_style) = if pane.focused {
                            (
                                Style::default()
                                    .fg(Color::Rgb(137, 180, 250))
                                    .add_modifier(Modifier::BOLD),
                                Style::default()
                                    .fg(Color::Rgb(205, 214, 244))
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            (
                                Style::default().fg(Color::Rgb(69, 71, 90)),
                                Style::default().fg(Color::Rgb(108, 112, 134)),
                            )
                        };

                        let pane_title = if pane.grid.title.is_empty() {
                            "shell".to_string()
                        } else {
                            pane.grid.title.clone()
                        };

                        let block = Block::default()
                            .borders(Borders::ALL)
                            .border_style(border_style)
                            .title(Span::styled(format!(" {} ", pane_title), title_style));
                        let inner = block.inner(pane_area);
                        frame.render_widget(block, pane_area);

                        // Prepare search matches for this pane
                        let search_matches: Vec<UiSearchMatch> = if overlay == Overlay::Search {
                            search_state.matches.iter().map(|m| UiSearchMatch {
                                row: m.row,
                                col: m.col,
                                len: m.len,
                            }).collect()
                        } else {
                            Vec::new()
                        };

                        let view = TerminalView::new(&pane.grid)
                            .show_cursor(pane.focused)
                            .scrollback_offset(pane.scrollback_offset)
                            .selection(pane.selection.as_ref())
                            .search_matches(&search_matches, search_state.current_match);
                        frame.render_widget(view, inner);
                    }
                }

                // ── Floating panes (rendered on TOP of tiled panes) ──
                // Collect floating pane info first to avoid borrow issues
                let floating_info: Vec<_> = tab
                    .floating_panes()
                    .map(|pane| {
                        let chrome_y = if show_tab_bar { 1u16 } else { 0 };
                        let sidebar_x = if fb_visible { fb_width } else { 0 };
                        let float_area = ratatui::layout::Rect {
                            x: pane.rect.x.saturating_add(sidebar_x),
                            y: pane.rect.y.saturating_add(chrome_y),
                            width: pane
                                .rect
                                .width
                                .min(area.width.saturating_sub(pane.rect.x + sidebar_x)),
                            height: pane
                                .rect
                                .height
                                .min(area.height.saturating_sub(pane.rect.y + chrome_y)),
                        };
                        (float_area, pane.focused, pane.grid.title.clone(), pane.scrollback_offset)
                    })
                    .collect();

                // Draw shadows
                {
                    let buf = frame.buffer_mut();
                    let shadow_style = Style::default().bg(Color::Rgb(17, 17, 27));
                    for (float_area, _, _, _) in &floating_info {
                        if float_area.width < 4 || float_area.height < 3 {
                            continue;
                        }
                        // Right shadow
                        for sy in (float_area.y + 1)..=(float_area.y + float_area.height) {
                            if sy < area.y + area.height {
                                let sx = float_area.x + float_area.width;
                                if sx < area.x + area.width {
                                    if let Some(cell) = buf.cell_mut((sx, sy)) {
                                        cell.set_char(' ');
                                        cell.set_style(shadow_style);
                                    }
                                }
                            }
                        }
                        // Bottom shadow
                        if float_area.y + float_area.height < area.y + area.height {
                            let sy = float_area.y + float_area.height;
                            for sx in (float_area.x + 1)..=(float_area.x + float_area.width) {
                                if sx < area.x + area.width {
                                    if let Some(cell) = buf.cell_mut((sx, sy)) {
                                        cell.set_char(' ');
                                        cell.set_style(shadow_style);
                                    }
                                }
                            }
                        }
                    }
                }

                // Render floating pane widgets
                let mut float_pane_idx = 0;
                for pane in tab.floating_panes() {
                    if float_pane_idx >= floating_info.len() {
                        break;
                    }
                    let (float_area, focused, _, scrollback_offset) = &floating_info[float_pane_idx];
                    float_pane_idx += 1;

                    if float_area.width < 4 || float_area.height < 3 {
                        continue;
                    }

                    let (border_style, title_style) = if *focused {
                        (
                            Style::default()
                                .fg(Color::Rgb(245, 194, 231)) // Catppuccin pink (floating accent)
                                .add_modifier(Modifier::BOLD),
                            Style::default()
                                .fg(Color::Rgb(205, 214, 244))
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        (
                            Style::default().fg(Color::Rgb(137, 180, 250)),
                            Style::default().fg(Color::Rgb(108, 112, 134)),
                        )
                    };

                    let pane_title = if pane.grid.title.is_empty() {
                        "\u{f0a8} float".to_string() // floating icon
                    } else {
                        format!("\u{f0a8} {}", pane.grid.title)
                    };

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_style(border_style)
                        .title(Span::styled(format!(" {} ", pane_title), title_style));
                    let inner = block.inner(*float_area);
                    frame.render_widget(block, *float_area);

                    // Prepare search matches for this pane
                    let search_matches: Vec<UiSearchMatch> = if overlay == Overlay::Search {
                        search_state.matches.iter().map(|m| UiSearchMatch {
                            row: m.row,
                            col: m.col,
                            len: m.len,
                        }).collect()
                    } else {
                        Vec::new()
                    };

                    let view = TerminalView::new(&pane.grid)
                        .show_cursor(pane.focused)
                        .scrollback_offset(*scrollback_offset)
                        .selection(pane.selection.as_ref())
                        .search_matches(&search_matches, search_state.current_match);
                    frame.render_widget(view, inner);
                }
                }
            }

            // ── Editor panes (on top of terminal panes, below overlays) ──
            for ep in &editor_panes {
                if ep.rect.width < 10 || ep.rect.height < 4 {
                    continue;
                }
                let is_focused = focus == Focus::Editor(ep.id);
                let border_color = if is_focused {
                    Color::Rgb(245, 194, 231) // pink
                } else {
                    Color::Rgb(137, 180, 250) // blue
                };
                let title_style = Style::default()
                    .fg(Color::Rgb(205, 214, 244))
                    .add_modifier(Modifier::BOLD);
                let border_style = Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD);
                let fname = ep
                    .state
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "[no name]".to_string());
                let mod_flag = if ep.state.modified { " [+]" } else { "" };
                let block = ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_style(border_style)
                    .title(ratatui::text::Span::styled(
                        format!(" \u{f0f6} {}{} ", fname, mod_flag),
                        title_style,
                    ));
                let inner = block.inner(ep.rect);
                frame.render_widget(block, ep.rect);
                let editor_widget = EditorWidget::new(&ep.state);
                frame.render_widget(editor_widget, inner);
            }

            // ── Status bar ──
            if show_status_bar && v_idx < v_chunks.len() {
                let (pane_title, broadcast, pane_count, focused_idx) = session
                    .active_tab()
                    .map(|t| {
                        let title = t
                            .focused_pane()
                            .map(|p| {
                                if p.grid.title.is_empty() {
                                    "shell"
                                } else {
                                    p.grid.title.as_str()
                                }
                            })
                            .unwrap_or("shell");
                        let fi = t.panes.iter().position(|p| p.focused).unwrap_or(0);
                        (title, t.broadcast, t.panes.len(), fi)
                    })
                    .unwrap_or(("shell", false, 0, 0));
                let current_notification = notifications.front().map(|n| n.msg.as_str());
                let status = StatusBar {
                    session_name: &session.name,
                    pane_title,
                    pane_count,
                    tab_count: session.tabs.len(),
                    broadcast,
                    focused_index: focused_idx,
                    notification: current_notification,
                };
                frame.render_widget(status, v_chunks[v_idx]);

                // (Notification rendering is handled by StatusBar via the
                // `notification` field passed above; no extra manual drawing here.)
            }

            // ── Overlays ──
            match &overlay {
                Overlay::Help => {
                    let help = HelpOverlay::new();
                    frame.render_widget(help, area);
                }
                Overlay::CommandPalette => {
                    let palette =
                        CommandPalette::new(&filtered_commands, palette_selected, &palette_filter);
                    frame.render_widget(palette, area);
                }
                Overlay::Search => {
                    // Render search bar at bottom
                    let search_height = 3u16;
                    let search_area = ratatui::layout::Rect {
                        x: 0,
                        y: area.height.saturating_sub(search_height),
                        width: area.width,
                        height: search_height,
                    };
                    
                    // Clear area
                    for y in search_area.y..search_area.y + search_area.height {
                        for x in search_area.x..search_area.x + search_area.width {
                            if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
                                cell.set_char(' ');
                                cell.set_style(Style::default().bg(Color::Rgb(49, 50, 68)));
                            }
                        }
                    }
                    
                    // Draw search bar
                    let match_text = if search_state.matches.is_empty() {
                        "No matches".to_string()
                    } else {
                        format!("{}/{}", search_state.current_match + 1, search_state.matches.len())
                    };
                    
                    let search_text = format!("Search: {} {}", search_state.query, match_text);
                    
                    // Draw border
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Rgb(137, 180, 250)))
                        .title(Span::styled(" Find ", Style::default().fg(Color::Rgb(205, 214, 244))));
                    
                    frame.render_widget(block, search_area);
                    
                    // Draw text
                    let inner = search_area.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 });
                    for (i, ch) in search_text.chars().enumerate() {
                        let x = inner.x + i as u16;
                        if x < inner.x + inner.width {
                            if let Some(cell) = frame.buffer_mut().cell_mut((x, inner.y + 1)) {
                                cell.set_char(ch);
                                cell.set_style(Style::default().fg(Color::Rgb(205, 214, 244)));
                            }
                        }
                    }
                }
                Overlay::TabRename { .. } => {
                    // Render rename dialog at bottom
                    let rename_height = 3u16;
                    let rename_area = ratatui::layout::Rect {
                        x: 0,
                        y: area.height.saturating_sub(rename_height),
                        width: area.width,
                        height: rename_height,
                    };
                    
                    // Clear area
                    for y in rename_area.y..rename_area.y + rename_area.height {
                        for x in rename_area.x..rename_area.x + rename_area.width {
                            if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
                                cell.set_char(' ');
                                cell.set_style(Style::default().bg(Color::Rgb(49, 50, 68)));
                            }
                        }
                    }
                    
                    // Draw rename bar
                    let rename_text = format!("Rename tab: {}", tab_rename_buffer);
                    
                    // Draw border
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Rgb(245, 194, 231))) // Pink for rename
                        .title(Span::styled(" Rename Tab ", Style::default().fg(Color::Rgb(205, 214, 244))));
                    
                    frame.render_widget(block, rename_area);
                    
                    // Draw text
                    let inner = rename_area.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 });
                    for (i, ch) in rename_text.chars().enumerate() {
                        let x = inner.x + i as u16;
                        if x < inner.x + inner.width {
                            if let Some(cell) = frame.buffer_mut().cell_mut((x, inner.y + 1)) {
                                cell.set_char(ch);
                                cell.set_style(Style::default().fg(Color::Rgb(205, 214, 244)));
                            }
                        }
                    }
                }
                Overlay::None => {}
            }
        })?;

        // ── Input handling ──
        tokio::select! {
            Some(Ok(ev)) = event_stream.next() => {
                match ev {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        // ── Overlay input (takes priority) ──
                        match &overlay {
                            Overlay::Help => {
                                overlay = Overlay::None;
                                continue;
                            }
                            Overlay::CommandPalette => {
                                match key.code {
                                    KeyCode::Esc => {
                                        overlay = Overlay::None;
                                        palette_filter.clear();
                                        palette_selected = 0;
                                        filtered_commands = commands.clone();
                                    }
                                    KeyCode::Up => {
                                        if palette_selected > 0 {
                                            palette_selected -= 1;
                                        }
                                    }
                                    KeyCode::Down => {
                                        if palette_selected + 1 < filtered_commands.len() {
                                            palette_selected += 1;
                                        }
                                    }
                                    KeyCode::Enter => {
                                        if let Some(cmd) = filtered_commands.get(palette_selected) {
                                            // Track this command as recently used
                                            let cmd_label = cmd.label.clone();
                                            recent_commands.retain(|c| c != &cmd_label);
                                            recent_commands.push_front(cmd_label);
                                            if recent_commands.len() > 10 {
                                                recent_commands.pop_back();
                                            }

                                            let action = palette_action_for(cmd);
                                            overlay = Overlay::None;
                                            palette_filter.clear();
                                            palette_selected = 0;
                                            filtered_commands = filter_commands(&commands, "", &recent_commands);

                                            if let Some(a) = action {
                                                if execute_palette_action(
                                                    a,
                                                    &mut session,
                                                    &mut overlay,
                                                    &mut file_browser,
                                                    &mut ai_sidebar,
                                                    &mut focus,
                                                    &terminal,
                                                    &shell,
                                                    show_tab_bar,
                                                    show_status_bar,
                                                ) {
                                                    break; // Quit
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Backspace => {
                                        palette_filter.pop();
                                        palette_selected = 0;
                                        filtered_commands = filter_commands(&commands, &palette_filter, &recent_commands);
                                    }
                                    KeyCode::Char(c) => {
                                        palette_filter.push(c);
                                        palette_selected = 0;
                                        filtered_commands = filter_commands(&commands, &palette_filter, &recent_commands);
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                            Overlay::Search => {
                                match key.code {
                                    KeyCode::Esc => {
                                        overlay = Overlay::None;
                                        search_state.clear();
                                    }
                                    KeyCode::Enter => {
                                        // Jump to next match
                                        if let Some(tab) = session.active_tab() {
                                            if let Some(pane) = tab.focused_pane() {
                                                search_state.find_matches(pane);
                                                if let Some(m) = search_state.next_match() {
                                                    // Could scroll to match here
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::F(3) => {
                                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                                            search_state.prev_match();
                                        } else {
                                            search_state.next_match();
                                        }
                                    }
                                    KeyCode::Backspace => {
                                        search_state.query.pop();
                                        if let Some(tab) = session.active_tab() {
                                            if let Some(pane) = tab.focused_pane() {
                                                search_state.find_matches(pane);
                                            }
                                        }
                                    }
                                    KeyCode::Char(c) => {
                                        search_state.query.push(c);
                                        if let Some(tab) = session.active_tab() {
                                            if let Some(pane) = tab.focused_pane() {
                                                search_state.find_matches(pane);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                            Overlay::TabRename { tab_id } => {
                                match key.code {
                                    KeyCode::Esc => {
                                        overlay = Overlay::None;
                                        tab_rename_buffer.clear();
                                    }
                                    KeyCode::Enter => {
                                        // Apply the rename
                                        if let Some(tab) = session.tabs.iter_mut().find(|t| t.id == *tab_id) {
                                            if !tab_rename_buffer.is_empty() {
                                                tab.name = tab_rename_buffer.clone();
                                            }
                                        }
                                        overlay = Overlay::None;
                                        tab_rename_buffer.clear();
                                    }
                                    KeyCode::Backspace => {
                                        tab_rename_buffer.pop();
                                    }
                                    KeyCode::Char(c) => {
                                        tab_rename_buffer.push(c);
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                            Overlay::None => {}
                        }

                        // ── Global keybindings (always processed) ──
                        if let Some(action) = input_handler.resolve(&key) {
                            match action {
                                Action::Quit => break,
                                Action::ShowHelp => {
                                    overlay = Overlay::Help;
                                    continue;
                                }
                                Action::CommandPalette => {
                                    overlay = Overlay::CommandPalette;
                                    palette_filter.clear();
                                    palette_selected = 0;
                                    filtered_commands = filter_commands(&commands, "", &recent_commands);
                                    continue;
                                }
                                Action::ToggleFileBrowser => {
                                    file_browser.toggle();
                                    if file_browser.visible {
                                        focus = Focus::FileBrowser;
                                    } else {
                                        focus = Focus::Terminal;
                                    }
                                    let area = compute_pane_area(
                                        &terminal,
                                        show_tab_bar,
                                        show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                        if ai_sidebar.visible { ai_sidebar.width } else { 0 },
                                    );
                                    session.relayout_all(area);
                                    continue;
                                }
                                Action::ToggleAiSidebar => {
                                    ai_sidebar.toggle();
                                    if ai_sidebar.visible {
                                        focus = Focus::AiSidebar;
                                    } else {
                                        focus = Focus::Terminal;
                                    }
                                    let area = compute_pane_area(
                                        &terminal,
                                        show_tab_bar,
                                        show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                        if ai_sidebar.visible { ai_sidebar.width } else { 0 },
                                    );
                                    session.relayout_all(area);
                                    continue;
                                }
                                Action::NewAiChat => {
                                    let ai_w = if ai_sidebar.visible { ai_sidebar.width } else { 0 };
                                    let area = compute_pane_area(
                                        &terminal,
                                        show_tab_bar,
                                        show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                        ai_w,
                                    );
                                    spawn_ai_chat_pane(&mut session, &ai_sidebar, area);
                                    focus = Focus::Terminal;
                                    continue;
                                }
                                _ => {
                                    // Fall through to focus-specific handling below
                                }
                            }
                        }

                        // ── File browser focused input ──
                        if focus == Focus::FileBrowser && file_browser.visible {
                            // If search mode is active, intercept most keys for search
                            if file_browser.search_mode {
                                match key.code {
                                    KeyCode::Esc => {
                                        file_browser.exit_search();
                                    }
                                    KeyCode::Tab => {
                                        file_browser.search_toggle_recursive();
                                    }
                                    KeyCode::Up => {
                                        file_browser.search_prev();
                                        let h = terminal.size().map(|s| s.height.saturating_sub(4) as usize).unwrap_or(20);
                                        file_browser.adjust_scroll(h);
                                    }
                                    KeyCode::Down => {
                                        file_browser.search_next();
                                        let h = terminal.size().map(|s| s.height.saturating_sub(4) as usize).unwrap_or(20);
                                        file_browser.adjust_scroll(h);
                                    }
                                    KeyCode::Enter => {
                                        let path = file_browser.search_accept();
                                        if let Some(p) = path {
                                            if p.is_dir() {
                                                // Navigate to selected directory
                                                file_browser.navigate_to(p);
                                            } else {
                                                // Open file in editor
                                                let screen_area = compute_screen_area(&terminal, show_tab_bar, show_status_bar,
                                                    if file_browser.visible { file_browser.width } else { 0 },
                                                    if ai_sidebar.visible  { ai_sidebar.width   } else { 0 });
                                                open_file_in_editor(&p, screen_area, &mut editor_panes, &mut next_editor_id, &mut focus);
                                            }
                                        }
                                    }
                                    KeyCode::Backspace => {
                                        file_browser.search_pop();
                                    }
                                    KeyCode::Char(c) => {
                                        file_browser.search_push(c);
                                    }
                                    _ => {}
                                }
                                continue;
                            }

                            match key.code {
                                KeyCode::Up => {
                                    file_browser.cursor_up();
                                    file_browser.load_preview();
                                }
                                KeyCode::Down => {
                                    file_browser.cursor_down();
                                    let h = terminal.size().map(|s| s.height.saturating_sub(4) as usize).unwrap_or(20);
                                    file_browser.adjust_scroll(h);
                                    file_browser.load_preview();
                                }
                                KeyCode::Enter => {
                                    // Ctrl+Enter: open in split (file in new editor, folder in new pane)
                                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                                        if let Some(entry) = file_browser.selected_entry() {
                                            if entry.is_dir && entry.name != ".." {
                                                let path = entry.path.clone();
                                                let area = compute_pane_area(
                                                    &terminal,
                                                    show_tab_bar,
                                                    show_status_bar,
                                                    file_browser.width,
                                                    if ai_sidebar.visible { ai_sidebar.width } else { 0 },
                                                );
                                                open_folder_in_pane(
                                                    &mut session,
                                                    &shell,
                                                    &path,
                                                    area,
                                                );
                                                focus = Focus::Terminal;
                                                push_notification(&mut notifications, "Opened folder in new pane".to_string(), 2000);
                                            } else if !entry.is_dir {
                                                // Open file in floating editor
                                                let path = entry.path.clone();
                                                let screen_area = compute_screen_area(&terminal, show_tab_bar, show_status_bar,
                                                    if file_browser.visible { file_browser.width } else { 0 },
                                                    if ai_sidebar.visible  { ai_sidebar.width   } else { 0 });
                                                open_file_in_editor(&path, screen_area, &mut editor_panes, &mut next_editor_id, &mut focus);
                                            }
                                        }
                                    } else {
                                        // Regular Enter: navigate folders, open files in editor
                                        if let Some(entry) = file_browser.selected_entry() {
                                            if entry.is_dir {
                                                let path = entry.path.clone();
                                                let entry_name = entry.name.clone();
                                                if entry_name == ".." {
                                                    file_browser.navigate_to(path);
                                                    file_browser.load_preview();
                                                } else {
                                                    file_browser.toggle_expand();
                                                }
                                            } else {
                                                // ── Open file in editor ──
                                                let path = entry.path.clone();
                                                let screen_area = compute_screen_area(&terminal, show_tab_bar, show_status_bar,
                                                    if file_browser.visible { file_browser.width } else { 0 },
                                                    if ai_sidebar.visible  { ai_sidebar.width   } else { 0 });
                                                open_file_in_editor(&path, screen_area, &mut editor_panes, &mut next_editor_id, &mut focus);
                                            }
                                        }
                                    }
                                }
                                KeyCode::Right => {
                                    // Open folder in new pane
                                    if let Some(entry) = file_browser.selected_entry() {
                                        if entry.is_dir && entry.name != ".." {
                                            let path = entry.path.clone();
                                            let area = compute_pane_area(
                                                &terminal,
                                                show_tab_bar,
                                                show_status_bar,
                                                file_browser.width,
                                                if ai_sidebar.visible { ai_sidebar.width } else { 0 },
                                            );
                                            open_folder_in_pane(
                                                &mut session,
                                                &shell,
                                                &path,
                                                area,
                                            );
                                            focus = Focus::Terminal;
                                        }
                                    }
                                }
                                KeyCode::Left => {
                                    // Navigate to parent in tree
                                    if let Some(parent) = file_browser.root.parent() {
                                        let parent = parent.to_path_buf();
                                        file_browser.navigate_to(parent);
                                        file_browser.load_preview();
                                    }
                                }
                                KeyCode::Esc | KeyCode::Tab => {
                                    // Return focus to terminal
                                    focus = Focus::Terminal;
                                }
                                KeyCode::Char('r') => {
                                    file_browser.refresh_root();
                                    file_browser.load_preview();
                                }
                                KeyCode::Char('/') => {
                                    file_browser.enter_search();
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // ── AI sidebar focused input ──
                        if focus == Focus::AiSidebar && ai_sidebar.visible {
                            match key.code {
                                KeyCode::Esc => {
                                    // Close AI sidebar, return focus to terminal
                                    ai_sidebar.visible = false;
                                    focus = Focus::Terminal;
                                    let area = compute_pane_area(
                                        &terminal,
                                        show_tab_bar,
                                        show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                        0,
                                    );
                                    session.relayout_all(area);
                                }
                                KeyCode::Tab => ai_sidebar.focus_next(),
                                KeyCode::BackTab => ai_sidebar.focus_prev(),
                                KeyCode::Up => {
                                    if matches!(ai_sidebar.focused_field, AiSidebarField::Provider(_)) {
                                        ai_sidebar.focus_prev();
                                    } else {
                                        ai_sidebar.focus_prev();
                                    }
                                }
                                KeyCode::Down => ai_sidebar.focus_next(),
                                KeyCode::Left => {
                                    if matches!(ai_sidebar.focused_field, AiSidebarField::Provider(_)) {
                                        ai_sidebar.provider_left();
                                    }
                                }
                                KeyCode::Right => {
                                    if matches!(ai_sidebar.focused_field, AiSidebarField::Provider(_)) {
                                        ai_sidebar.provider_right();
                                    }
                                }
                                KeyCode::Enter | KeyCode::Char(' ') => {
                                    match ai_sidebar.focused_field.clone() {
                                        AiSidebarField::SaveButton => {
                                            // Sync sidebar state → config → persist
                                            cfg.ai.provider = match ai_sidebar.provider {
                                                UiAiProvider::Anthropic => crate::config::AiProvider::Anthropic,
                                                UiAiProvider::OpenAi    => crate::config::AiProvider::Openai,
                                                UiAiProvider::Ollama    => crate::config::AiProvider::Ollama,
                                                UiAiProvider::Custom    => crate::config::AiProvider::Custom,
                                                UiAiProvider::ZAi       => crate::config::AiProvider::Zai,
                                                UiAiProvider::Kimi      => crate::config::AiProvider::Kimi,
                                            };
                                            cfg.ai.api_key   = ai_sidebar.api_key.clone();
                                            cfg.ai.model     = ai_sidebar.model.clone();
                                            cfg.ai.base_url  = ai_sidebar.base_url.clone();

                                            // Handle keyring save based on checkbox
                                            if ai_sidebar.save_to_keyring && !cfg.ai.api_key.is_empty() {
                                                // Will be saved to keyring by cfg.save()
                                            } else if !ai_sidebar.save_to_keyring {
                                                // User doesn't want keyring - try to delete any existing entry
                                                let _ = crate::config::delete_ai_key_from_keyring();
                                            }

                                            match cfg.save() {
                                                Ok(_) => {
                                                    ai_sidebar.dirty = false;
                                                    ai_sidebar.status_msg = Some("Saved!".to_string());
                                                    push_notification(&mut notifications, "AI configuration saved".to_string(), 3000);
                                                }
                                                Err(e) => {
                                                    ai_sidebar.status_msg = Some(format!("Error: {}", e));
                                                    push_notification(
                                                        &mut notifications,
                                                        format!("Failed to save AI config: {}", e),
                                                        5000,
                                                    );
                                                }
                                            }
                                        }
                                        AiSidebarField::SaveToKeyring => {
                                            // Toggle the checkbox
                                            ai_sidebar.save_to_keyring = !ai_sidebar.save_to_keyring;
                                            ai_sidebar.dirty = true;
                                        }
                                        AiSidebarField::TestConnection => {
                                            // Spawn async test connection
                                            ai_sidebar.test_status = Some("Testing...".to_string());
                                            push_notification(&mut notifications, "Testing AI connection...".to_string(), 2000);

                                            let provider = ai_sidebar.provider;
                                            let api_key = ai_sidebar.api_key.clone();
                                            let base_url = ai_sidebar.base_url.clone();
                                            let tx = test_tx.clone();

                                            tokio::spawn(async move {
                                                let result = test_ai_connection(provider, &api_key, &base_url).await;
                                                let _ = tx.send(result);
                                            });
                                        }
                                        AiSidebarField::StartChatButton => {
                                            let ai_w = ai_sidebar.width;
                                            let area = compute_pane_area(
                                                &terminal,
                                                show_tab_bar,
                                                show_status_bar,
                                                if file_browser.visible { file_browser.width } else { 0 },
                                                ai_w,
                                            );
                                            spawn_ai_chat_pane(&mut session, &ai_sidebar, area);
                                            ai_sidebar.visible = false;
                                            focus = Focus::Terminal;
                                            let area2 = compute_pane_area(
                                                &terminal,
                                                show_tab_bar,
                                                show_status_bar,
                                                if file_browser.visible { file_browser.width } else { 0 },
                                                0,
                                            );
                                            session.relayout_all(area2);
                                            push_notification(&mut notifications, "AI chat started".to_string(), 3000);
                                        }
                                        AiSidebarField::Provider(i) => {
                                            ai_sidebar.provider = uterx_ui::widgets::ai_sidebar::AiProvider::from_index(i);
                                            ai_sidebar.dirty = true;
                                        }
                                        _ => {
                                            // Move to next field on Enter for text inputs
                                            ai_sidebar.focus_next();
                                        }
                                    }
                                }
                                KeyCode::Backspace => {
                                    // Only handle backspace for text fields, not buttons/checkboxes
                                    if !matches!(ai_sidebar.focused_field,
                                        AiSidebarField::SaveButton |
                                        AiSidebarField::SaveToKeyring |
                                        AiSidebarField::TestConnection |
                                        AiSidebarField::StartChatButton)
                                    {
                                        ai_sidebar.pop_char();
                                    }
                                }
                                KeyCode::Char(c) => {
                                    // Ctrl+V = paste from clipboard (check for 'v' or SYN char \x16)
                                    let is_paste = key.modifiers.contains(KeyModifiers::CONTROL) 
                                        && (c == 'v' || c == '\x16');
                                    
                                    if is_paste {
                                        tracing::debug!("Paste triggered with Ctrl+V");
                                        match paste_from_clipboard() {
                                            Ok(text) => {
                                                tracing::debug!("Paste successful, text length: {}", text.len());
                                                // Insert the pasted text into the current field
                                                for ch in text.chars() {
                                                    if !matches!(ai_sidebar.focused_field,
                                                        AiSidebarField::SaveButton |
                                                        AiSidebarField::SaveToKeyring |
                                                        AiSidebarField::TestConnection |
                                                        AiSidebarField::StartChatButton)
                                                    {
                                                        ai_sidebar.push_char(ch);
                                                    }
                                                }
                                                ai_sidebar.dirty = true;
                                            }
                                            Err(e) => {
                                                tracing::warn!("Failed to paste from clipboard: {}", e);
                                                push_notification(&mut notifications, 
                                                    format!("Paste failed: {}", e), 3000);
                                            }
                                        }
                                        continue;
                                    // Ctrl+W = close sidebar
                                    } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'w' {
                                        ai_sidebar.visible = false;
                                        focus = Focus::Terminal;
                                        let area = compute_pane_area(
                                            &terminal,
                                            show_tab_bar,
                                            show_status_bar,
                                            if file_browser.visible { file_browser.width } else { 0 },
                                            0,
                                        );
                                        session.relayout_all(area);
                                    } else if c == ' ' && matches!(ai_sidebar.focused_field, AiSidebarField::SaveToKeyring) {
                                        // Space toggles checkbox (already handled in Enter above)
                                    } else if !matches!(ai_sidebar.focused_field,
                                        AiSidebarField::SaveButton |
                                        AiSidebarField::SaveToKeyring |
                                        AiSidebarField::TestConnection |
                                        AiSidebarField::StartChatButton)
                                    {
                                        // Only push chars to text fields
                                        ai_sidebar.push_char(c);
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // ── Editor focused input ──
                        if let Focus::Editor(eid) = focus.clone() {
                            if let Some(ep) = editor_panes.iter_mut().find(|e| e.id == eid) {
                                let state = &mut ep.state;
                                let visible_h = ep.rect.height.saturating_sub(3) as usize;

                                // Handle close request
                                if key.code == KeyCode::Char('w') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                    if state.modified && !state.close_requested {
                                        state.close_requested = true;
                                    } else {
                                        state.should_close = true;
                                    }
                                    continue;
                                }
                                // Save
                                if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                    let _ = state.save();
                                    continue;
                                }

                                match state.mode {
                                    EditorMode::Normal => match key.code {
                                        KeyCode::Char('i') => { state.mode = EditorMode::Insert; state.close_requested = false; }
                                        KeyCode::Char('a') => {
                                            state.mode = EditorMode::Insert;
                                            let row = state.cursor_row;
                                            let len = state.lines[row].len();
                                            if state.cursor_col < len { state.cursor_col += 1; }
                                        }
                                        KeyCode::Char('o') => {
                                            let row = state.cursor_row;
                                            let len = state.lines[row].len();
                                            state.cursor_col = len;
                                            state.insert_newline();
                                            state.mode = EditorMode::Insert;
                                        }
                                        KeyCode::Char('h') | KeyCode::Left  => state.move_cursor(0, -1),
                                        KeyCode::Char('j') | KeyCode::Down  => { state.move_cursor(1, 0); state.adjust_scroll_to_cursor(visible_h); }
                                        KeyCode::Char('k') | KeyCode::Up    => { state.move_cursor(-1, 0); state.adjust_scroll_to_cursor(visible_h); }
                                        KeyCode::Char('l') | KeyCode::Right => state.move_cursor(0, 1),
                                        KeyCode::Char('0') => state.move_to_line_start(),
                                        KeyCode::Char('$') => state.move_to_line_end(),
                                        KeyCode::Char('x') => state.delete_char_at(),
                                        KeyCode::Char('u') => state.undo(),
                                        KeyCode::Char('G') => { state.move_to_last_line(); state.adjust_scroll_to_cursor(visible_h); }
                                        KeyCode::Char('g') => { state.move_to_first_line(); }
                                        KeyCode::Char('d') => state.delete_line(),
                                        KeyCode::Esc => { state.close_requested = false; }
                                        _ => {}
                                    },
                                    EditorMode::Insert => match key.code {
                                        KeyCode::Esc => { state.mode = EditorMode::Normal; }
                                        KeyCode::Enter => state.insert_newline(),
                                        KeyCode::Backspace => state.delete_char_before(),
                                        KeyCode::Delete => state.delete_char_at(),
                                        KeyCode::Up    => { state.move_cursor(-1, 0); state.adjust_scroll_to_cursor(visible_h); }
                                        KeyCode::Down  => { state.move_cursor(1, 0); state.adjust_scroll_to_cursor(visible_h); }
                                        KeyCode::Left  => state.move_cursor(0, -1),
                                        KeyCode::Right => state.move_cursor(0, 1),
                                        KeyCode::Home  => state.move_to_line_start(),
                                        KeyCode::End   => state.move_to_line_end(),
                                        KeyCode::Char(c) => {
                                            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                                                state.insert_char(c);
                                                state.adjust_scroll_to_cursor(visible_h);
                                            }
                                        }
                                        _ => {}
                                    },
                                }
                            }
                            // Remove closed editors
                            editor_panes.retain(|ep| !ep.state.should_close);
                            // If the focused editor was just closed, return focus to terminal
                            if !editor_panes.iter().any(|ep| ep.id == eid) {
                                focus = Focus::Terminal;
                            }
                            continue;
                        }

                        // ── Terminal focused input ──
                        if let Some(action) = input_handler.resolve(&key) {
                            match action {
                                Action::NewTab => {
                                    let area = compute_pane_area(
                                        &terminal, show_tab_bar, show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                        if ai_sidebar.visible  { ai_sidebar.width   } else { 0 },
                                    );
                                    if let Err(e) = session.create_tab(&shell, area) {
                                        tracing::error!("failed to create tab: {}", e);
                                    }
                                }
                                Action::CloseTab => {
                                    session.close_active_tab();
                                    if session.tabs.is_empty() {
                                        break;
                                    }
                                }
                                Action::NextTab => session.next_tab(),
                                Action::PrevTab => session.prev_tab(),
                                Action::NewPane | Action::SplitVertical => {
                                    let area = compute_pane_area(
                                        &terminal, show_tab_bar, show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                        if ai_sidebar.visible  { ai_sidebar.width   } else { 0 },
                                    );
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.relayout(area);
                                    }
                                    if let Err(e) = session.split_vertical(&shell) {
                                        tracing::error!("failed to split vertical: {}", e);
                                    }
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.relayout(area);
                                    }
                                }
                                Action::SplitHorizontal => {
                                    let area = compute_pane_area(
                                        &terminal, show_tab_bar, show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                        if ai_sidebar.visible  { ai_sidebar.width   } else { 0 },
                                    );
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.relayout(area);
                                    }
                                    if let Err(e) = session.split_horizontal(&shell) {
                                        tracing::error!("failed to split horizontal: {}", e);
                                    }
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.relayout(area);
                                    }
                                }
                                Action::ClosePane => {
                                    session.close_active_pane();
                                    let tab_empty = session
                                        .active_tab()
                                        .map(|t| t.is_empty())
                                        .unwrap_or(false);
                                    if tab_empty {
                                        session.close_active_tab();
                                        if session.tabs.is_empty() {
                                            break;
                                        }
                                    } else {
                                        let area = compute_pane_area(
                                            &terminal, show_tab_bar, show_status_bar,
                                            if file_browser.visible { file_browser.width } else { 0 },
                                            if ai_sidebar.visible  { ai_sidebar.width   } else { 0 },
                                        );
                                        if let Some(tab) = session.active_tab_mut() {
                                            tab.relayout(area);
                                        }
                                    }
                                }
                                Action::NextPane => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.focus_next();
                                    }
                                }
                                Action::PrevPane => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.focus_prev();
                                    }
                                }
                                Action::ToggleBroadcast => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.toggle_broadcast();
                                    }
                                }
                                Action::ToggleFloat => {
                                    let area = compute_pane_area(
                                        &terminal, show_tab_bar, show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                        if ai_sidebar.visible  { ai_sidebar.width   } else { 0 },
                                    );
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.toggle_float(area);
                                    }
                                }
                                Action::ToggleMaximize => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.toggle_maximize();
                                    }
                                }
                                Action::Search => {
                                    overlay = Overlay::Search;
                                    if let Some(tab) = session.active_tab() {
                                        if let Some(pane) = tab.focused_pane() {
                                            search_state.find_matches(pane);
                                        }
                                    }
                                }
                                Action::Fullscreen => {
                                    tracing::debug!("action {:?} not yet implemented", action);
                                }
                                Action::RenameTab => {
                                    if let Some(tab) = session.active_tab() {
                                        let tab_id = tab.id;
                                        tab_rename_buffer = tab.name.clone();
                                        overlay = Overlay::TabRename { tab_id };
                                    }
                                }
                                Action::RawInput(data) => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        let _ = tab.write_input(data);
                                    }
                                }
                                // Already handled above
                                Action::Quit | Action::ShowHelp | Action::CommandPalette
                                | Action::ToggleFileBrowser | Action::ToggleAiSidebar
                                | Action::NewAiChat | Action::OpenFolder(_) | Action::RenameTab => {}
                            }
                        } else {
                            // Forward key to active pane
                            let bytes = key_to_bytes(&key);
                            if !bytes.is_empty() {
                                if let Some(tab) = session.active_tab_mut() {
                                    let _ = tab.write_input(&bytes);
                                }
                            }
                        }
                    }
                    Event::Resize(w, h) => {
                        let left_w  = if file_browser.visible { file_browser.width } else { 0 };
                        let right_w = if ai_sidebar.visible   { ai_sidebar.width   } else { 0 };
                        let chrome_rows: u16 =
                            if show_tab_bar { 1 } else { 0 } + if show_status_bar { 1 } else { 0 };
                        let area = MuxRect {
                            x: 0,
                            y: 0,
                            width: w.saturating_sub(left_w + right_w),
                            height: h.saturating_sub(chrome_rows),
                        };
                        session.relayout_all(area);
                    }
                    Event::Mouse(mouse) => {
                        if overlay == Overlay::None {
                            let sidebar_w = if file_browser.visible { file_browser.width } else { 0 };
                            let ai_right_w = if ai_sidebar.visible { ai_sidebar.width } else { 0 };
                            let chrome_y = if show_tab_bar { 1u16 } else { 0 };

                            // ── Handle active drag of a floating pane ──
                            match mouse.kind {
                                MouseEventKind::Drag(MouseButton::Left) => {
                                    // Split resize takes priority over other drags
                                    if let Some(ref resize) = split_resizing {
                                        if let Some(tab) = session.active_tab_mut() {
                                            if let Some(direction) = tab.layout.split_direction() {
                                                let dx = mouse.column as f32 - resize.start_x as f32;
                                                let dy = mouse.row as f32 - resize.start_y as f32;
                                                
                                                // Get terminal area for calculating ratios
                                                if let Ok(size) = terminal.size() {
                                                    let term_area = MuxRect {
                                                        x: sidebar_w,
                                                        y: chrome_y,
                                                        width: size.width.saturating_sub(sidebar_w + ai_right_w),
                                                        height: size.height.saturating_sub(chrome_y + if show_status_bar { 1 } else { 0 }),
                                                    };
                                                    
                                                    let delta_ratio = match direction {
                                                        SplitDirection::Vertical => {
                                                            dx / term_area.width as f32
                                                        }
                                                        SplitDirection::Horizontal => {
                                                            dy / term_area.height as f32
                                                        }
                                                    };
                                                    
                                                    // Apply delta to current ratios, not initial
                                                    if let Some(ref ratios) = tab.layout.ratios() {
                                                        if resize.pane_idx + 1 < ratios.len() {
                                                            let current_first = ratios[resize.pane_idx];
                                                            let current_second = ratios[resize.pane_idx + 1];
                                                            let new_first = (current_first + delta_ratio).max(0.05);
                                                            let new_second = (current_second - delta_ratio).max(0.05);
                                                            
                                                            // Only update if ratios are valid
                                                            if new_first > 0.0 && new_second > 0.0 {
                                                                tab.layout.adjust_ratio(resize.pane_idx, delta_ratio);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        continue;
                                    }
                                    
                                    // Editor pane drag
                                    if let Some(ref ed) = editor_dragging {
                                        let eid = ed.editor_id;
                                        let ox = ed.offset_x;
                                        if let Some(ep) = editor_panes.iter_mut().find(|e| e.id == eid) {
                                            ep.rect.x = mouse.column.saturating_sub(ox);
                                            ep.rect.y = mouse.row;
                                        }
                                        continue;
                                    }
                                    if let Some(ref drag) = dragging {
                                        let drag_id = drag.pane_id;
                                        let drag_ox = drag.offset_x;
                                        let drag_oy = drag.offset_y;
                                        // New position in pane-local coordinates (subtract chrome/sidebar)
                                        let new_x = mouse.column
                                            .saturating_sub(sidebar_w)
                                            .saturating_sub(drag_ox);
                                        let new_y = mouse.row
                                            .saturating_sub(chrome_y)
                                            .saturating_sub(drag_oy);
                                        if let Some(tab) = session.active_tab_mut() {
                                            tab.move_floating_pane(drag_id, new_x, new_y);
                                        }
                                        continue;
                                    }
                                }
                                MouseEventKind::Up(MouseButton::Left) => {
                                    // Handle text selection end
                                    if let Some(ref sel_drag) = selection_dragging {
                                        let pid = sel_drag.pane_id;
                                        if let Some(tab) = session.active_tab() {
                                            if let Some(pane) = tab.panes.iter().find(|p| p.id == pid) {
                                                if let Some(text) = pane.selected_text() {
                                                    if !text.is_empty() {
                                                        // Copy to clipboard
                                                        if let Err(e) = copy_to_clipboard(&text) {
                                                            tracing::warn!("Failed to copy to clipboard: {}", e);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    
                                    dragging = None;
                                    editor_dragging = None;
                                    split_resizing = None;
                                    selection_dragging = None;
                                    continue;
                                }
                                _ => {}
                            }

                            // ── Tab bar click (row 0 when tab bar visible) ──
                            if show_tab_bar && mouse.row == 0 {
                                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                                    // Compute tab hit zones (same layout as TabBar widget)
                                    // Brand: " uterx " (7) + separator (1) = offset 8
                                    let brand_width: u16 = 8; // " uterx │"
                                    let right_section_width: u16 = 12;
                                    let term_width = terminal.size().map(|s| s.width).unwrap_or(80);
                                    let plus_x = term_width.saturating_sub(right_section_width);
                                    let help_x = plus_x + 5;

                                    // [+] button: columns plus_x..plus_x+5
                                    if mouse.column >= plus_x && mouse.column < plus_x + 5 {
                                        let area = compute_pane_area(&terminal, show_tab_bar, show_status_bar, sidebar_w, ai_right_w);
                                        let _ = session.create_tab(&shell, area);
                                    // [?] button: columns help_x..help_x+5
                                    } else if mouse.column >= help_x && mouse.column < help_x + 5 {
                                        overlay = Overlay::Help;
                                    } else if mouse.column >= brand_width {
                                        // Tab click — compute which tab
                                        let mut x = brand_width;
                                        let max_tab_x = term_width.saturating_sub(right_section_width);
                                        for (i, t) in session.tabs.iter().enumerate() {
                                            let label = format!(" {}:{} ", i + 1, t.name);
                                            let tab_w = label.len() as u16;
                                            if x + tab_w > max_tab_x { break; }
                                            if mouse.column >= x && mouse.column < x + tab_w {
                                                // Clicked on this tab
                                                session.active_tab = Some(t.id);
                                                break;
                                            }
                                            x += tab_w + 1; // +1 for separator
                                        }
                                    }
                                }
                                continue;
                            }

                            // ── File browser area (left) ──
                            if file_browser.visible && mouse.column < sidebar_w {
                                match mouse.kind {
                                    MouseEventKind::Down(MouseButton::Left) => {
                                        focus = Focus::FileBrowser;
                                        let header_offset = chrome_y + 2;
                                        if mouse.row >= header_offset {
                                            let clicked_idx = file_browser.scroll_offset
                                                + (mouse.row - header_offset) as usize;
                                            if clicked_idx < file_browser.entries.len() {
                                                file_browser.cursor = clicked_idx;
                                            }
                                        }
                                    }
                                    MouseEventKind::ScrollUp => {
                                        // Scroll file browser up
                                        focus = Focus::FileBrowser;
                                        let term_h = terminal.size().map(|s| s.height).unwrap_or(24);
                                        let visible_h = term_h.saturating_sub(chrome_y + 4) as usize;
                                        file_browser.scroll_offset = file_browser.scroll_offset.saturating_sub(3);
                                        file_browser.adjust_scroll(visible_h);
                                    }
                                    MouseEventKind::ScrollDown => {
                                        // Scroll file browser down
                                        focus = Focus::FileBrowser;
                                        let term_h = terminal.size().map(|s| s.height).unwrap_or(24);
                                        let visible_h = term_h.saturating_sub(chrome_y + 4) as usize;
                                        file_browser.scroll_offset = (file_browser.scroll_offset + 3)
                                            .min(file_browser.entries.len().saturating_sub(visible_h));
                                        file_browser.adjust_scroll(visible_h);
                                    }
                                    _ => {}
                                }
                                continue;
                            }

                            // ── AI sidebar area (right) ──
                            let term_width = terminal.size().map(|s| s.width).unwrap_or(80);
                            if ai_sidebar.visible && mouse.column >= term_width.saturating_sub(ai_right_w) {
                                match mouse.kind {
                                    MouseEventKind::Down(MouseButton::Left) => {
                                        focus = Focus::AiSidebar;
                                    }
                                    MouseEventKind::ScrollUp => {
                                        // Navigate up through AI sidebar fields
                                        focus = Focus::AiSidebar;
                                        ai_sidebar.focus_prev();
                                    }
                                    MouseEventKind::ScrollDown => {
                                        // Navigate down through AI sidebar fields
                                        focus = Focus::AiSidebar;
                                        ai_sidebar.focus_next();
                                    }
                                    _ => {}
                                }
                                continue;
                            }

                            // ── Terminal area ──
                            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                                focus = Focus::Terminal;
                            }

                            // Check if clicking on an editor pane
                            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                                let mut hit_editor = false;
                                for ep in editor_panes.iter().rev() {
                                    let r = ep.rect;
                                    if mouse.column >= r.x && mouse.column < r.x + r.width
                                        && mouse.row >= r.y && mouse.row < r.y + r.height
                                    {
                                        focus = Focus::Editor(ep.id);
                                        hit_editor = true;
                                        // Title bar drag
                                        if mouse.row == r.y {
                                            editor_dragging = Some(EditorDragState {
                                                editor_id: ep.id,
                                                offset_x: mouse.column.saturating_sub(r.x),
                                            });
                                        }
                                        break;
                                    }
                                }
                                if hit_editor {
                                    continue;
                                }
                            }

                            // Check if clicking on a floating pane title bar to start drag
                            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                                let adj_col = mouse.column.saturating_sub(sidebar_w);
                                let adj_row = mouse.row.saturating_sub(chrome_y);
                                if let Some(tab) = session.active_tab_mut() {
                                    let mut drag_started = false;
                                    // Iterate floating panes in reverse (topmost first visually)
                                    for pane in tab.panes.iter_mut().rev() {
                                        if !pane.is_floating { continue; }
                                        let r = &pane.rect;
                                        // Check if click is on the title bar (y == r.y, within width)
                                        if adj_row == r.y
                                            && adj_col >= r.x
                                            && adj_col < r.x + r.width
                                        {
                                            // Focus this pane
                                            let pid = pane.id;
                                            pane.focused = true;
                                            let offset_x = adj_col - r.x;
                                            dragging = Some(DragState {
                                                pane_id: pid,
                                                offset_x,
                                                offset_y: 0,
                                            });
                                            drag_started = true;
                                            break;
                                        }
                                        // Click anywhere inside floating pane → focus
                                        if adj_row >= r.y
                                            && adj_row < r.y + r.height
                                            && adj_col >= r.x
                                            && adj_col < r.x + r.width
                                        {
                                            let pid = pane.id;
                                            pane.focused = true;
                                            tab.active_pane = Some(pid);
                                            drag_started = true;
                                            break;
                                        }
                                    }
                                    if drag_started {
                                        // Unfocus all other panes
                                        if let Some(ref drag) = dragging {
                                            let did = drag.pane_id;
                                            for p in &mut tab.panes {
                                                if p.id != did { p.focused = false; }
                                            }
                                        }
                                    } else {
                                        // No floating pane hit — delegate to tiled pane handling
                                        handle_mouse_event(&mut session, mouse, show_tab_bar, sidebar_w, &mut split_resizing, &mut selection_dragging);
                                    }
                                }
                            } else {
                                handle_mouse_event(&mut session, mouse, show_tab_bar, sidebar_w, &mut split_resizing, &mut selection_dragging);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some(result) = test_rx.recv() => {
                // Test connection result arrived
                ai_sidebar.test_status = Some(result.clone());
                push_notification(&mut notifications, result, 4000);
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }

    // Save session
    let save_path = session.save_path();
    if let Err(e) = session.save_to_file(&save_path) {
        tracing::warn!("failed to save session: {}", e);
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    io::stdout().execute(DisableMouseCapture)?;
    io::stdout().execute(LeaveAlternateScreen)?;
    tracing::info!("uterx exited");
    Ok(())
}

// ── Helper types and functions ──────────────────────────────────────────

/// Actions triggered from the command palette.
enum PaletteAction {
    Quit,
    Help,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    SplitVertical,
    SplitHorizontal,
    NextPane,
    PrevPane,
    Broadcast,
    FileBrowser,
    ToggleFloat,
    ToggleMaximize,
    AiSidebar,
    NewAiChat,
}

/// Map a command palette entry to a PaletteAction.
fn palette_action_for(cmd: &CommandEntry) -> Option<PaletteAction> {
    match cmd.label.as_str() {
        "New Tab" => Some(PaletteAction::NewTab),
        "Close Tab" => Some(PaletteAction::CloseTab),
        "Next Tab" => Some(PaletteAction::NextTab),
        "Previous Tab" => Some(PaletteAction::PrevTab),
        "Split Vertical" => Some(PaletteAction::SplitVertical),
        "Split Horizontal" => Some(PaletteAction::SplitHorizontal),
        "Focus Next Pane" => Some(PaletteAction::NextPane),
        "Focus Previous Pane" => Some(PaletteAction::PrevPane),
        "Toggle Broadcast" => Some(PaletteAction::Broadcast),
        "Help" => Some(PaletteAction::Help),
        "File Browser" => Some(PaletteAction::FileBrowser),
        "Toggle Floating" => Some(PaletteAction::ToggleFloat),
        "Toggle Maximize" => Some(PaletteAction::ToggleMaximize),
        "AI Sidebar" => Some(PaletteAction::AiSidebar),
        "New AI Chat" => Some(PaletteAction::NewAiChat),
        "Quit" => Some(PaletteAction::Quit),
        _ => None,
    }
}

/// Execute a command palette action. Returns true if the app should quit.
#[allow(clippy::too_many_arguments)]
fn execute_palette_action(
    action: PaletteAction,
    session: &mut Session,
    overlay: &mut Overlay,
    file_browser: &mut FileBrowserState,
    ai_sidebar: &mut AiSidebarState,
    focus: &mut Focus,
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    shell: &str,
    show_tab_bar: bool,
    show_status_bar: bool,
) -> bool {
    let left_w = if file_browser.visible {
        file_browser.width
    } else {
        0
    };
    let right_w = if ai_sidebar.visible {
        ai_sidebar.width
    } else {
        0
    };
    match action {
        PaletteAction::Quit => return true,
        PaletteAction::Help => {
            *overlay = Overlay::Help;
        }
        PaletteAction::FileBrowser => {
            file_browser.toggle();
            if file_browser.visible {
                *focus = Focus::FileBrowser;
            } else {
                *focus = Focus::Terminal;
            }
            let left = if file_browser.visible {
                file_browser.width
            } else {
                0
            };
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, left, right_w);
            session.relayout_all(area);
        }
        PaletteAction::AiSidebar => {
            ai_sidebar.toggle();
            if ai_sidebar.visible {
                *focus = Focus::AiSidebar;
            } else {
                *focus = Focus::Terminal;
            }
            let right = if ai_sidebar.visible {
                ai_sidebar.width
            } else {
                0
            };
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, left_w, right);
            session.relayout_all(area);
        }
        PaletteAction::NewAiChat => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, left_w, right_w);
            spawn_ai_chat_pane(session, ai_sidebar, area);
            *focus = Focus::Terminal;
        }
        PaletteAction::NewTab => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, left_w, right_w);
            let _ = session.create_tab(shell, area);
        }
        PaletteAction::CloseTab => {
            session.close_active_tab();
            if session.tabs.is_empty() {
                return true;
            }
        }
        PaletteAction::NextTab => session.next_tab(),
        PaletteAction::PrevTab => session.prev_tab(),
        PaletteAction::SplitVertical => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, left_w, right_w);
            if let Some(tab) = session.active_tab_mut() {
                tab.relayout(area);
            }
            let _ = session.split_vertical(shell);
            if let Some(tab) = session.active_tab_mut() {
                tab.relayout(area);
            }
        }
        PaletteAction::SplitHorizontal => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, left_w, right_w);
            if let Some(tab) = session.active_tab_mut() {
                tab.relayout(area);
            }
            let _ = session.split_horizontal(shell);
            if let Some(tab) = session.active_tab_mut() {
                tab.relayout(area);
            }
        }
        PaletteAction::NextPane => {
            if let Some(tab) = session.active_tab_mut() {
                tab.focus_next();
            }
        }
        PaletteAction::PrevPane => {
            if let Some(tab) = session.active_tab_mut() {
                tab.focus_prev();
            }
        }
        PaletteAction::Broadcast => {
            if let Some(tab) = session.active_tab_mut() {
                tab.toggle_broadcast();
            }
        }
        PaletteAction::ToggleFloat => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, left_w, right_w);
            if let Some(tab) = session.active_tab_mut() {
                tab.toggle_float(area);
            }
        }
        PaletteAction::ToggleMaximize => {
            if let Some(tab) = session.active_tab_mut() {
                tab.toggle_maximize();
            }
        }
    }
    false
}

/// Filter commands by fuzzy match and prioritize recent commands.
fn filter_commands(
    all: &[CommandEntry],
    filter: &str,
    recent: &VecDeque<String>,
) -> Vec<CommandEntry> {
    // If no filter, return all with recent commands first
    if filter.is_empty() {
        let mut result = Vec::new();
        // Add recent commands first
        for recent_label in recent.iter() {
            if let Some(cmd) = all.iter().find(|c| &c.label == recent_label) {
                result.push(cmd.clone());
            }
        }
        // Add remaining commands
        for cmd in all.iter() {
            if !recent.contains(&cmd.label) {
                result.push(cmd.clone());
            }
        }
        return result;
    }

    let pat = filter.to_lowercase();

    // Simple fuzzy subsequence scorer:
    // - returns None if pattern is not a subsequence of text
    // - otherwise returns a score (higher is better), rewarding consecutive matches
    fn fuzzy_score(pat: &str, text: &str) -> Option<usize> {
        let tchars: Vec<char> = text.to_lowercase().chars().collect();
        let pchars: Vec<char> = pat.chars().collect();
        let mut score: usize = 0;
        let mut last_idx: Option<usize> = None;
        let mut idx: usize = 0;

        for &pc in &pchars {
            let mut found = false;
            while idx < tchars.len() {
                if tchars[idx] == pc {
                    // Base point for a match
                    score = score.saturating_add(1);
                    // Bonus for consecutive match
                    if let Some(prev) = last_idx {
                        if idx == prev + 1 {
                            score = score.saturating_add(2);
                        }
                    }
                    last_idx = Some(idx);
                    idx += 1;
                    found = true;
                    break;
                }
                idx += 1;
            }
            if !found {
                return None;
            }
        }
        Some(score)
    }

    // Score each command by taking the best match across label, shortcut, description.
    let mut scored: Vec<(usize, CommandEntry)> = all
        .iter()
        .filter_map(|c| {
            let mut best: Option<usize> = None;
            if let Some(s) = fuzzy_score(&pat, &c.label) {
                best = Some(best.map_or(s * 1000, |b| b.max(s * 1000)));
            }
            if let Some(s) = fuzzy_score(&pat, &c.shortcut) {
                best = Some(best.map_or(s * 800, |b| b.max(s * 800)));
            }
            if let Some(s) = fuzzy_score(&pat, &c.description) {
                best = Some(best.map_or(s * 600, |b| b.max(s * 600)));
            }

            // Boost score significantly if this is a recent command
            if let Some(score) = best {
                let recent_bonus = if recent.contains(&c.label) {
                    // Recent commands get a massive boost based on recency
                    let pos = recent.iter().position(|r| r == &c.label).unwrap_or(10);
                    (10 - pos.min(9)) * 10000 // More recent = higher bonus
                } else {
                    0
                };
                best = Some(score + recent_bonus);
            }

            best.map(|score| (score, c.clone()))
        })
        .collect();

    // If nothing matched (should be rare because subsequence is strict), fallback to substring filter.
    if scored.is_empty() {
        let f = pat.clone();
        return all
            .iter()
            .filter(|c| {
                c.label.to_lowercase().contains(&f)
                    || c.shortcut.to_lowercase().contains(&f)
                    || c.description.to_lowercase().contains(&f)
            })
            .cloned()
            .collect();
    }

    // Sort by score descending and return entries.
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, e)| e).collect()
}

/// Open a file in a floating editor pane.
/// `screen_area` is in ratatui screen coordinates (accounts for tab bar and sidebar).
fn open_file_in_editor(
    path: &std::path::Path,
    screen_area: ratatui::layout::Rect,
    editor_panes: &mut Vec<EditorPane>,
    next_id: &mut u64,
    focus: &mut Focus,
) {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let state = EditorState::new(path, content);

    // Center at 70% width × 80% height of the usable area
    let fw = (screen_area.width as f32 * 0.70) as u16;
    let fh = (screen_area.height as f32 * 0.80) as u16;
    let fx = screen_area.x + (screen_area.width.saturating_sub(fw)) / 2;
    let fy = screen_area.y + (screen_area.height.saturating_sub(fh)) / 2;

    let id = *next_id;
    *next_id += 1;

    editor_panes.push(EditorPane {
        id,
        state,
        rect: ratatui::layout::Rect {
            x: fx,
            y: fy,
            width: fw,
            height: fh,
        },
    });
    *focus = Focus::Editor(id);
}

/// Open a folder in a new terminal pane (cd + shell).
fn open_folder_in_pane(
    session: &mut Session,
    shell: &str,
    folder: &std::path::Path,
    area: MuxRect,
) {
    // Create a new tab with the pane cd'd into the folder
    if let Ok(tab_id) = session.create_tab(shell, area) {
        // Send a cd command to the newly created pane
        if let Some(tab) = session.tabs.iter_mut().find(|t| t.id == tab_id) {
            if let Some(pane) = tab.focused_pane_mut() {
                // Set title to folder name
                let folder_name = folder
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| folder.to_string_lossy().to_string());
                pane.grid.title = folder_name;

                // Send cd command
                let cd_cmd = format!("cd \"{}\"\r", folder.display());
                let _ = pane.write_to_pty(cd_cmd.as_bytes());
            }
        }
        // Switch to the new tab
        session.active_tab = Some(tab_id);
    }
}

/// Copy text to the system clipboard.
fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    use std::process::Command;
    
    // Try wl-copy first (Wayland), then xclip (X11), then pbcopy (macOS)
    #[cfg(target_os = "linux")]
    {
        // Try wl-copy first
        if Command::new("wl-copy").arg(text).spawn().is_ok() {
            return Ok(());
        }
        // Fall back to xclip
        if let Ok(mut child) = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn() 
        {
            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write;
                stdin.write_all(text.as_bytes())?;
            }
            return Ok(());
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut child) = Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn() 
        {
            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write;
                stdin.write_all(text.as_bytes())?;
            }
            return Ok(());
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        if let Ok(mut child) = Command::new("powershell")
            .args([
                "-Command",
                &format!("Set-Clipboard -Value '{}'", text.replace("'", "''"))
            ])
            .spawn() 
        {
            return Ok(());
        }
    }
    
    Err(anyhow::anyhow!("No clipboard utility available"))
}

/// Read text from the system clipboard with timeout.
fn paste_from_clipboard() -> anyhow::Result<String> {
    use std::process::{Command, Stdio};
    use std::time::Duration;
    use std::thread;
    
    // Spawn clipboard access in a separate thread with timeout
    let result = thread::spawn(|| {
        #[cfg(target_os = "linux")]
        {
            // Try wl-paste first (Wayland)
            if let Ok(output) = Command::new("wl-paste").output() {
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
            }
            // Fall back to xclip
            if let Ok(output) = Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output() 
            {
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("pbpaste").output() {
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            // Use cmd.exe to run PowerShell command - more reliable
            if let Ok(output) = Command::new("cmd.exe")
                .args([
                    "/c",
                    "powershell.exe -NoProfile -Command \"Get-Clipboard\""
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output() 
            {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout).to_string();
                    return Ok(text.trim_end().to_string());
                }
            }
        }
        
        Err(anyhow::anyhow!("No clipboard utility available"))
    })
    .join();
    
    match result {
        Ok(Ok(text)) => Ok(text),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(anyhow::anyhow!("Clipboard access timed out or panicked")),
    }
}

/// Compute the pane area accounting for chrome and both sidebars.
fn compute_pane_area(
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    show_tab_bar: bool,
    show_status_bar: bool,
    left_sidebar_width: u16,
    right_sidebar_width: u16,
) -> MuxRect {
    let size = terminal.size().unwrap_or_default();
    let chrome_rows: u16 = if show_tab_bar { 1 } else { 0 } + if show_status_bar { 1 } else { 0 };
    let total_sidebar = left_sidebar_width.saturating_add(right_sidebar_width);
    MuxRect {
        x: 0,
        y: 0,
        width: size.width.saturating_sub(total_sidebar),
        height: size.height.saturating_sub(chrome_rows),
    }
}

/// Compute the content area as screen-coordinate Rect (for editor pane placement).
fn compute_screen_area(
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    show_tab_bar: bool,
    show_status_bar: bool,
    left_sidebar_width: u16,
    right_sidebar_width: u16,
) -> ratatui::layout::Rect {
    let size = terminal.size().unwrap_or_default();
    let chrome_y: u16 = if show_tab_bar { 1 } else { 0 };
    let chrome_bot: u16 = if show_status_bar { 1 } else { 0 };
    let total_sidebar = left_sidebar_width.saturating_add(right_sidebar_width);
    ratatui::layout::Rect {
        x: left_sidebar_width,
        y: chrome_y,
        width: size.width.saturating_sub(total_sidebar),
        height: size.height.saturating_sub(chrome_y + chrome_bot),
    }
}

/// Spawn a new terminal pane/tab running the configured AI chat command.
fn spawn_ai_chat_pane(session: &mut Session, ai_sidebar: &AiSidebarState, area: MuxRect) {
    let model = if ai_sidebar.model.is_empty() {
        ai_sidebar.provider.default_model().to_string()
    } else {
        ai_sidebar.model.clone()
    };
    let tab_name = format!("AI:{}", model);
    
    // Build platform-specific command
    let shell_cmd = if cfg!(target_os = "windows") {
        // On Windows, use cmd with a simple approach - just run opencode or aichat
        // Environment variables need to be set differently on Windows
        let cmd = "opencode";
        format!("{}", cmd)
    } else {
        // On Unix/Linux/macOS, use bash
        let chat_cmd = ai_sidebar.build_chat_command();
        format!("bash -c \"{}\"", chat_cmd.replace('"', "\\\""))
    };
    
    tracing::info!("Spawning AI chat with command: {}", shell_cmd);
    
    match session.create_tab(&shell_cmd, area) {
        Ok(tab_id) => {
            if let Some(tab) = session.tabs.iter_mut().find(|t| t.id == tab_id) {
                tab.name = tab_name.clone();
                if let Some(pane) = tab.focused_pane_mut() {
                    pane.grid.title = tab_name;
                }
            }
            session.active_tab = Some(tab_id);
        }
        Err(e) => {
            tracing::error!("Failed to create AI chat tab: {}", e);
        }
    }
}

/// Convert a crossterm key event into bytes to send to the PTY.
fn key_to_bytes(key: &crossterm::event::KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let ctrl = (c as u8).wrapping_sub(b'a').wrapping_add(1);
                vec![ctrl]
            } else {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n) => match n {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            12 => b"\x1b[24~".to_vec(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Handle a mouse event — focus pane on click, scroll wheel.
/// Test AI connection based on provider and credentials.
async fn test_ai_connection(provider: UiAiProvider, api_key: &str, base_url: &str) -> String {
    use UiAiProvider::*;

    match provider {
        Anthropic => {
            if api_key.is_empty() {
                return "❌ API key required for Anthropic".to_string();
            }
            // Simple test: try to list models or make a minimal request
            let client = reqwest::Client::new();
            let res = client
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    "✓ Anthropic connection successful".to_string()
                }
                Ok(resp) => {
                    format!("❌ Anthropic error: {}", resp.status())
                }
                Err(e) => {
                    format!("❌ Anthropic connection failed: {}", e)
                }
            }
        }
        OpenAi => {
            if api_key.is_empty() {
                return "❌ API key required for OpenAI".to_string();
            }
            let client = reqwest::Client::new();
            let res = client
                .get("https://api.openai.com/v1/models")
                .header("Authorization", format!("Bearer {}", api_key))
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    "✓ OpenAI connection successful".to_string()
                }
                Ok(resp) => {
                    format!("❌ OpenAI error: {}", resp.status())
                }
                Err(e) => {
                    format!("❌ OpenAI connection failed: {}", e)
                }
            }
        }
        Ollama => {
            let url = if base_url.is_empty() {
                "http://localhost:11434/api/tags"
            } else {
                base_url
            };

            let client = reqwest::Client::new();
            let res = client
                .get(url)
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    "✓ Ollama connection successful".to_string()
                }
                Ok(resp) => {
                    format!("❌ Ollama error: {}", resp.status())
                }
                Err(_) => "❌ Ollama not reachable (is it running?)".to_string(),
            }
        }
        Custom => {
            if base_url.is_empty() {
                return "❌ Base URL required for custom provider".to_string();
            }

            let client = reqwest::Client::new();
            let test_url = format!("{}/v1/models", base_url.trim_end_matches('/'));
            let mut req = client
                .get(&test_url)
                .timeout(std::time::Duration::from_secs(5));

            if !api_key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", api_key));
            }

            let res = req.send().await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    "✓ Custom provider connection successful".to_string()
                }
                Ok(resp) => {
                    format!("❌ Custom provider error: {}", resp.status())
                }
                Err(e) => {
                    format!("❌ Custom provider failed: {}", e)
                }
            }
        }
        ZAi => {
            if api_key.is_empty() {
                return "❌ API key required for z.Ai".to_string();
            }
            let url = if base_url.is_empty() {
                "https://api.z.ai/v1/models".to_string()
            } else {
                format!("{}/v1/models", base_url.trim_end_matches('/'))
            };

            let client = reqwest::Client::new();
            let res = client
                .get(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    "✓ z.Ai connection successful".to_string()
                }
                Ok(resp) => {
                    format!("❌ z.Ai error: {}", resp.status())
                }
                Err(e) => {
                    format!("❌ z.Ai connection failed: {}", e)
                }
            }
        }
        Kimi => {
            if api_key.is_empty() {
                return "❌ API key required for Kimi".to_string();
            }
            let url = if base_url.is_empty() {
                "https://api.moonshot.cn/v1/models".to_string()
            } else {
                format!("{}/v1/models", base_url.trim_end_matches('/'))
            };

            let client = reqwest::Client::new();
            let res = client
                .get(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    "✓ Kimi connection successful".to_string()
                }
                Ok(resp) => {
                    format!("❌ Kimi error: {}", resp.status())
                }
                Err(e) => {
                    format!("❌ Kimi connection failed: {}", e)
                }
            }
        }
    }
}

fn handle_mouse_event(
    session: &mut Session,
    mouse: crossterm::event::MouseEvent,
    show_tab_bar: bool,
    sidebar_offset: u16,
    split_resizing: &mut Option<SplitResizeState>,
    selection_dragging: &mut Option<SelectionDragState>,
) {
    let tab = match session.active_tab_mut() {
        Some(t) => t,
        None => return,
    };

    // Skip if a pane is maximized
    if tab.has_maximized_pane() {
        return;
    }

    let offset_y = if show_tab_bar { 1u16 } else { 0 };
    let mx = mouse.column.saturating_sub(sidebar_offset);
    let my = mouse.row.saturating_sub(offset_y);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Check if clicking on a border between panes
            if let Some(direction) = tab.layout.split_direction() {
                let tiled_panes: Vec<_> = tab.panes.iter().filter(|p| !p.is_floating).collect();
                if tiled_panes.len() >= 2 {
                    // Check for borders between adjacent panes
                    for (i, window) in tiled_panes.windows(2).enumerate() {
                        let first = &window[0];
                        let _second = &window[1];
                        
                        let on_border = match direction {
                            SplitDirection::Vertical => {
                                // Border is at the right edge of first pane
                                let border_x = first.rect.x + first.rect.width;
                                mx >= border_x.saturating_sub(1) && mx <= border_x + 1
                                    && my >= first.rect.y && my < first.rect.y + first.rect.height
                            }
                            SplitDirection::Horizontal => {
                                // Border is at the bottom edge of first pane
                                let border_y = first.rect.y + first.rect.height;
                                my >= border_y.saturating_sub(1) && my <= border_y + 1
                                    && mx >= first.rect.x && mx < first.rect.x + first.rect.width
                            }
                        };
                        
                        if on_border {
                            // Start split resize drag
                            *split_resizing = Some(SplitResizeState {
                                pane_idx: i,
                                start_x: mouse.column,
                                start_y: mouse.row,
                            });
                            return;
                        }
                    }
                }
            }
            
            // Not on a border, handle as normal pane click (potential text selection start)
            let mut target_pane_id = None;
            let mut click_row = 0usize;
            let mut click_col = 0usize;
            
            for pane in &tab.panes {
                let r = &pane.rect;
                if mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height {
                    target_pane_id = Some(pane.id);
                    // Calculate row/col within pane (accounting for borders if any)
                    click_col = (mx - r.x) as usize;
                    click_row = (my - r.y) as usize;
                    break;
                }
            }
            
            if let Some(pid) = target_pane_id {
                for pane in &mut tab.panes {
                    pane.focused = pane.id == pid;
                    // Clear any existing selection when clicking
                    if pane.id == pid {
                        pane.clear_selection();
                    }
                }
                tab.active_pane = Some(pid);
                
                // Start selection drag
                *selection_dragging = Some(SelectionDragState {
                    pane_id: pid,
                    start_row: click_row,
                    start_col: click_col,
                });
                
                // Initialize selection in the pane
                if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pid) {
                    pane.start_selection(click_row, click_col);
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Handle text selection dragging
            if let Some(sel_drag) = selection_dragging {
                let pid = sel_drag.pane_id;
                
                // Find the pane and update selection
                if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pid) {
                    let r = &pane.rect;
                    if mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height {
                        let col = (mx - r.x) as usize;
                        let row = (my - r.y) as usize;
                        pane.update_selection(row, col);
                    }
                }
            }
        }
        MouseEventKind::ScrollUp => {
            // Find pane under mouse and scroll up (show older scrollback)
            for pane in &mut tab.panes {
                let r = &pane.rect;
                if mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height {
                    // Scroll up by 3 lines (show older content)
                    let scrollback_len = pane.grid.scrollback.len();
                    let max_scroll = scrollback_len + pane.grid.rows();
                    pane.scrollback_offset = (pane.scrollback_offset + 3).min(max_scroll);
                    break;
                }
            }
        }
        MouseEventKind::ScrollDown => {
            // Find pane under mouse and scroll down (show newer content / back to current)
            for pane in &mut tab.panes {
                let r = &pane.rect;
                if mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height {
                    // Scroll down by 3 lines
                    pane.scrollback_offset = pane.scrollback_offset.saturating_sub(3);
                    break;
                }
            }
        }
        _ => {}
    }
}
