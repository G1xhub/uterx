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
    event::{
        Event, EventStream, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
        EnableMouseCapture, DisableMouseCapture,
    },
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders},
    Terminal,
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uterx_plugin::{PluginInstance, PluginManager};
use uterx_mux::{Layout as MuxLayout, Rect as MuxRect, Session};
use uterx_ui::input::{Action, InputHandler};
use uterx_ui::widgets::command_palette::{default_commands, CommandEntry, CommandPalette};
use uterx_ui::widgets::editor::{EditorState, EditorWidget, EditorMode};
use uterx_ui::widgets::file_browser::{FileBrowserState, FileBrowserWidget};
use uterx_ui::widgets::help_overlay::HelpOverlay;
use uterx_ui::widgets::status_bar::StatusBar;
use uterx_ui::widgets::tab_bar::{TabBar, TabBarHover, TabInfo};
use uterx_ui::TerminalView;

/// UI overlay state.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Overlay {
    None,
    Help,
    CommandPalette,
}

/// Where keyboard input is currently directed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Focus {
    Terminal,
    FileBrowser,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitResizeOrientation {
    Horizontal,
    Vertical,
}

/// State for mouse-dragging a tiled split border.
struct SplitResizeDragState {
    boundary_index: usize,
    orientation: SplitResizeOrientation,
    last_col: u16,
    last_row: u16,
}

/// State for drag-and-drop reordering of tiled panes.
struct TiledReorderDragState {
    pane_id: uterx_mux::PaneId,
}

struct UterxAiRuntime {
    instance: Option<PluginInstance>,
    pane_id: Option<uterx_mux::PaneId>,
    last_io_write_count: usize,
    prompt_buffer: String,
    status: String,
    transcript: Vec<String>,
    settings_path: Option<PathBuf>,
    api_key: String,
    api_url: String,
    api_model: String,
    setup_mode: bool,
    setup_field: UterxAiSetupField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UterxAiSetupField {
    ApiKey,
    ApiUrl,
    ApiModel,
}

impl UterxAiSetupField {
    fn next(self) -> Self {
        match self {
            Self::ApiKey => Self::ApiUrl,
            Self::ApiUrl => Self::ApiModel,
            Self::ApiModel => Self::ApiModel,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::ApiKey => Self::ApiKey,
            Self::ApiUrl => Self::ApiKey,
            Self::ApiModel => Self::ApiUrl,
        }
    }
}

/// Run the main terminal event loop.
pub async fn run(mut cfg: AppConfig, restore_on_launch: bool) -> anyhow::Result<()> {
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

    let pane_area = MuxRect {
        x: 0,
        y: 0,
        width: cols,
        height: rows,
    };
    let mut session = build_initial_session(&shell, pane_area, restore_on_launch)?;
    normalize_focus_state(&mut session);
    let mut uterxai = init_uterxai_runtime(&cfg);

    let input_handler = InputHandler::new();

    // Command palette state
    let commands = default_commands();
    let mut palette_selected: usize = 0;
    let mut palette_filter = String::new();
    let mut filtered_commands: Vec<CommandEntry> = commands.clone();

    // File browser state
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let mut file_browser = FileBrowserState::new(&home);
    file_browser.width = 32;

    // Focus state
    let mut focus = Focus::Terminal;

    // Floating pane drag state
    let mut dragging: Option<DragState> = None;

    // Editor panes
    let mut editor_panes: Vec<EditorPane> = Vec::new();
    let mut next_editor_id: u64 = 1;
    let mut editor_dragging: Option<EditorDragState> = None;
    let mut split_resize_dragging: Option<SplitResizeDragState> = None;
    let mut tiled_reorder_dragging: Option<TiledReorderDragState> = None;
    let mut tab_bar_hover: Option<TabBarHover> = None;
    let mut file_browser_hover_entry: Option<usize> = None;

    // crossterm event stream
    let mut event_stream = EventStream::new();
    let mut file_browser_last_click: Option<(usize, Instant)> = None;

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
        sync_uterxai_output_into_pane(&mut session, &mut uterxai);

        // Render
        let fb_visible = file_browser.visible;
        let fb_width = file_browser.width;
        let fb_focused = focus == Focus::FileBrowser;

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
                let broadcast = session
                    .active_tab()
                    .map(|t| t.broadcast)
                    .unwrap_or(false);
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
                let tab_bar = TabBar::new(&tabs)
                    .broadcast(broadcast)
                    .hover(tab_bar_hover);
                frame.render_widget(tab_bar, v_chunks[v_idx]);
                v_idx += 1;
            }

            // ── Main area (sidebar + terminal) ──
            let main_area = v_chunks[v_idx];
            v_idx += 1;

            // Horizontal split: file browser sidebar + terminal area
            let (sidebar_area, term_area) = if fb_visible {
                let h_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Length(fb_width),
                        Constraint::Min(1),
                    ])
                    .split(main_area);
                (Some(h_chunks[0]), h_chunks[1])
            } else {
                (None, main_area)
            };

            // ── File browser sidebar ──
            if let Some(sb_area) = sidebar_area {
                let fb_widget = FileBrowserWidget::new(&file_browser, fb_focused)
                    .hovered_entry(file_browser_hover_entry);
                frame.render_widget(fb_widget, sb_area);
            }

            // ── Terminal panes ──
            if let Some(tab) = session.active_tab() {
                // Count only tiled panes for layout
                let tiled_count = tab.panes.iter().filter(|p| !p.is_floating).count();

                if tiled_count == 1 && tab.floating_panes().count() == 0 {
                    // Single tiled pane — fill the area
                    if let Some(pane) = tab.tiled_panes().next() {
                        let view = TerminalView::new(&pane.grid);
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
                            .title(Span::styled(
                                format!(" {} ", pane_title),
                                title_style,
                            ));
                        let inner = block.inner(pane_area);
                        frame.render_widget(block, pane_area);

                        let view = TerminalView::new(&pane.grid)
                            .show_cursor(pane.focused);
                        frame.render_widget(view, inner);
                    }
                }

                // ── Floating panes (rendered on TOP of tiled panes) ──
                // Collect floating pane info first to avoid borrow issues
                let floating_info: Vec<_> = tab.floating_panes().map(|pane| {
                    let chrome_y = if show_tab_bar { 1u16 } else { 0 };
                    let sidebar_x = if fb_visible { fb_width } else { 0 };
                    let float_area = ratatui::layout::Rect {
                        x: pane.rect.x.saturating_add(sidebar_x),
                        y: pane.rect.y.saturating_add(chrome_y),
                        width: pane.rect.width.min(area.width.saturating_sub(pane.rect.x + sidebar_x)),
                        height: pane.rect.height.min(area.height.saturating_sub(pane.rect.y + chrome_y)),
                    };
                    (float_area, pane.focused, pane.grid.title.clone())
                }).collect();

                // Draw shadows
                {
                    let buf = frame.buffer_mut();
                    let shadow_style = Style::default().bg(Color::Rgb(17, 17, 27));
                    for (float_area, _, _) in &floating_info {
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
                    if float_pane_idx >= floating_info.len() { break; }
                    let (float_area, focused, _) = &floating_info[float_pane_idx];
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
                        .title(Span::styled(
                            format!(" {} ", pane_title),
                            title_style,
                        ));
                    let inner = block.inner(*float_area);
                    frame.render_widget(block, *float_area);

                    let view = TerminalView::new(&pane.grid)
                        .show_cursor(pane.focused);
                    frame.render_widget(view, inner);
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
                let fname = ep.state.path.file_name()
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
                let status = StatusBar {
                    session_name: &session.name,
                    pane_title,
                    pane_count,
                    tab_count: session.tabs.len(),
                    broadcast,
                    focused_index: focused_idx,
                };
                frame.render_widget(status, v_chunks[v_idx]);
            }

            // ── Overlays ──
            match &overlay {
                Overlay::Help => {
                    let help = HelpOverlay::new();
                    frame.render_widget(help, area);
                }
                Overlay::CommandPalette => {
                    let palette = CommandPalette::new(
                        &filtered_commands,
                        palette_selected,
                        &palette_filter,
                    );
                    frame.render_widget(palette, area);
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
                                            let action = palette_action_for(cmd);
                                            overlay = Overlay::None;
                                            palette_filter.clear();
                                            palette_selected = 0;
                                            filtered_commands = commands.clone();

                                            if let Some(a) = action {
                                                if execute_palette_action(
                                                    a,
                                                    &mut session,
                                                    &mut uterxai,
                                                    &mut overlay,
                                                    &mut file_browser,
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
                                        filtered_commands = filter_commands(&commands, &palette_filter);
                                    }
                                    KeyCode::Char(c) => {
                                        palette_filter.push(c);
                                        palette_selected = 0;
                                        filtered_commands = filter_commands(&commands, &palette_filter);
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                            Overlay::None => {}
                        }

                        // ── Global keybindings (always processed) ──
                        if overlay == Overlay::None
                            && focus == Focus::Terminal
                            && handle_uterxai_prompt_key(&key, &mut session, &mut uterxai)
                        {
                            continue;
                        }

                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.modifiers.contains(KeyModifiers::SHIFT)
                            && matches!(key.code, KeyCode::Char('a') | KeyCode::Char('A'))
                        {
                            let area = compute_pane_area(
                                &terminal,
                                show_tab_bar,
                                show_status_bar,
                                if file_browser.visible { file_browser.width } else { 0 },
                            );
                            open_or_focus_uterxai_pane(&mut session, &mut uterxai, area);
                            focus = Focus::Terminal;
                            continue;
                        }

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
                                    filtered_commands = commands.clone();
                                    continue;
                                }
                                Action::ToggleFileBrowser => {
                                    file_browser.toggle();
                                    if file_browser.visible {
                                        focus = Focus::FileBrowser;
                                    } else {
                                        focus = Focus::Terminal;
                                    }
                                    // Relayout panes for new available width
                                    let area = compute_pane_area(
                                        &terminal,
                                        show_tab_bar,
                                        show_status_bar,
                                        if file_browser.visible { file_browser.width } else { 0 },
                                    );
                                    session.relayout_all(area);
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
                                                    if file_browser.visible { file_browser.width } else { 0 });
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
                                }
                                KeyCode::Down => {
                                    file_browser.cursor_down();
                                    let h = terminal.size().map(|s| s.height.saturating_sub(4) as usize).unwrap_or(20);
                                    file_browser.adjust_scroll(h);
                                }
                                KeyCode::Enter => {
                                    if let Some(entry) = file_browser.selected_entry() {
                                        if entry.is_dir {
                                            let path = entry.path.clone();
                                            let entry_name = entry.name.clone();
                                            if entry_name == ".." {
                                                file_browser.navigate_to(path);
                                            } else {
                                                file_browser.toggle_expand();
                                            }
                                        } else {
                                            // ── Open file in editor ──
                                            let path = entry.path.clone();
                                            let screen_area = compute_screen_area(&terminal, show_tab_bar, show_status_bar,
                                                if file_browser.visible { file_browser.width } else { 0 });
                                            open_file_in_editor(&path, screen_area, &mut editor_panes, &mut next_editor_id, &mut focus);
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
                                    }
                                }
                                KeyCode::Esc | KeyCode::Tab => {
                                    // Return focus to terminal
                                    focus = Focus::Terminal;
                                }
                                KeyCode::Char('r') => {
                                    file_browser.refresh_root();
                                }
                                KeyCode::Char('/') => {
                                    file_browser.enter_search();
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
                                    );
                                    if let Some(tab) = session.active_tab_mut() {
                                        tab.toggle_float(area);
                                    }
                                }
                                Action::Search | Action::Fullscreen => {
                                    tracing::debug!("action {:?} not yet implemented", action);
                                }
                                Action::RawInput(data) => {
                                    if let Some(tab) = session.active_tab_mut() {
                                        let _ = tab.write_input(data);
                                    }
                                }
                                // Already handled above
                                Action::Quit | Action::ShowHelp | Action::CommandPalette
                                | Action::ToggleFileBrowser | Action::OpenFolder(_) => {}
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
                        let sidebar_w = if file_browser.visible { file_browser.width } else { 0 };
                        let chrome_rows: u16 =
                            if show_tab_bar { 1 } else { 0 } + if show_status_bar { 1 } else { 0 };
                        let area = MuxRect {
                            x: 0,
                            y: 0,
                            width: w.saturating_sub(sidebar_w),
                            height: h.saturating_sub(chrome_rows),
                        };
                        session.relayout_all(area);
                    }
                    Event::Mouse(mouse) => {
                        if overlay == Overlay::None {
                            let sidebar_w = if file_browser.visible { file_browser.width } else { 0 };
                            let chrome_y = if show_tab_bar { 1u16 } else { 0 };

                            let term_width = terminal.size().map(|s| s.width).unwrap_or(80);
                            tab_bar_hover = if show_tab_bar && mouse.row == 0 {
                                tab_bar_hover_at(mouse.column, term_width, &session)
                            } else {
                                None
                            };

                            file_browser_hover_entry = if file_browser.visible && mouse.column < sidebar_w {
                                let header_offset = chrome_y + 2;
                                if mouse.row >= header_offset {
                                    let hovered_idx = file_browser.scroll_offset
                                        + (mouse.row - header_offset) as usize;
                                    if hovered_idx < file_browser.entries.len() {
                                        Some(hovered_idx)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            // ── Handle active drag of a floating pane ──
                            match mouse.kind {
                                MouseEventKind::Drag(MouseButton::Left) => {
                                    if tiled_reorder_dragging.is_some() {
                                        continue;
                                    }
                                    if let Some(ref mut split_drag) = split_resize_dragging {
                                        let delta = match split_drag.orientation {
                                            SplitResizeOrientation::Vertical => {
                                                mouse.column as i16 - split_drag.last_col as i16
                                            }
                                            SplitResizeOrientation::Horizontal => {
                                                mouse.row as i16 - split_drag.last_row as i16
                                            }
                                        };
                                        if delta != 0 {
                                            let area = compute_pane_area(
                                                &terminal,
                                                show_tab_bar,
                                                show_status_bar,
                                                sidebar_w,
                                            );
                                            if let Some(tab) = session.active_tab_mut() {
                                                let changed = tab.adjust_split_boundary(
                                                    area,
                                                    split_drag.boundary_index,
                                                    delta,
                                                    10,
                                                    4,
                                                );
                                                if changed {
                                                    split_drag.last_col = mouse.column;
                                                    split_drag.last_row = mouse.row;
                                                }
                                            }
                                        }
                                        continue;
                                    }
                                    // Editor pane drag takes priority
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
                                    if let Some(reorder_drag) = tiled_reorder_dragging.take() {
                                        let adj_col = mouse.column.saturating_sub(sidebar_w);
                                        let adj_row = mouse.row.saturating_sub(chrome_y);
                                        let area = compute_pane_area(
                                            &terminal,
                                            show_tab_bar,
                                            show_status_bar,
                                            sidebar_w,
                                        );
                                        if let Some(tab) = session.active_tab_mut() {
                                            if let Some(target_id) = hit_tiled_pane(tab, adj_col, adj_row) {
                                                if tab.reorder_tiled_panes(reorder_drag.pane_id, target_id) {
                                                    tab.relayout(area);
                                                }
                                            }
                                        }
                                    }
                                    dragging = None;
                                    editor_dragging = None;
                                    split_resize_dragging = None;
                                    continue;
                                }
                                _ => {}
                            }

                            // ── Tab bar click (row 0 when tab bar visible) ──
                            if show_tab_bar && mouse.row == 0 {
                                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                                    // Compute tab hit zones (same layout as TabBar widget)
                                    // Brand: " uterx " (7) + separator (1) = offset 8
                                    match tab_bar_hover {
                                        Some(TabBarHover::Plus) => {
                                            let area = compute_pane_area(&terminal, show_tab_bar, show_status_bar, sidebar_w);
                                            let _ = session.create_tab(&shell, area);
                                        }
                                        Some(TabBarHover::Help) => {
                                            overlay = Overlay::Help;
                                        }
                                        Some(TabBarHover::Tab(tab_index)) => {
                                            if let Some(tab) = session.tabs.get(tab_index) {
                                                session.active_tab = Some(tab.id);
                                            }
                                        }
                                        None => {}
                                    }
                                }
                                continue;
                            }

                            // ── File browser area ──
                            if file_browser.visible && mouse.column < sidebar_w {
                                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                                    focus = Focus::FileBrowser;
                                    let header_offset = chrome_y + 2;
                                    if mouse.row >= header_offset {
                                        let clicked_idx = file_browser.scroll_offset
                                            + (mouse.row - header_offset) as usize;
                                        if clicked_idx < file_browser.entries.len() {
                                            file_browser.cursor = clicked_idx;
                                            let now = Instant::now();
                                            let is_double_click = file_browser_last_click
                                                .map(|(last_idx, last_ts)| {
                                                    last_idx == clicked_idx
                                                        && now.duration_since(last_ts)
                                                            <= Duration::from_millis(350)
                                                })
                                                .unwrap_or(false);

                                            if is_double_click {
                                                if file_browser.entries[clicked_idx].is_dir {
                                                    file_browser.toggle_expand();
                                                } else if let Some(path) = file_browser.selected_path() {
                                                    let screen_area = compute_screen_area(
                                                        &terminal,
                                                        show_tab_bar,
                                                        show_status_bar,
                                                        sidebar_w,
                                                    );
                                                    open_file_in_editor(
                                                        &path,
                                                        screen_area,
                                                        &mut editor_panes,
                                                        &mut next_editor_id,
                                                        &mut focus,
                                                    );
                                                }
                                                file_browser_last_click = None;
                                            } else {
                                                file_browser_last_click = Some((clicked_idx, now));
                                            }
                                        } else {
                                            file_browser_last_click = None;
                                        }
                                    } else {
                                        file_browser_last_click = None;
                                    }
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

                                if let Some(tab) = session.active_tab() {
                                    if let Some((orientation, boundary_index)) =
                                        hit_split_boundary(tab, adj_col, adj_row)
                                    {
                                        split_resize_dragging = Some(SplitResizeDragState {
                                            boundary_index,
                                            orientation,
                                            last_col: mouse.column,
                                            last_row: mouse.row,
                                        });
                                        continue;
                                    }
                                }

                                if let Some(tab) = session.active_tab_mut() {
                                    if let Some(pane_id) = hit_tiled_title_bar(tab, adj_col, adj_row) {
                                        tiled_reorder_dragging = Some(TiledReorderDragState { pane_id });
                                        for pane in &mut tab.panes {
                                            pane.focused = pane.id == pane_id;
                                        }
                                        tab.active_pane = Some(pane_id);
                                        continue;
                                    }
                                }

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
                                        handle_mouse_event(&mut session, mouse, show_tab_bar, sidebar_w);
                                    }
                                }
                            } else {
                                handle_mouse_event(&mut session, mouse, show_tab_bar, sidebar_w);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(4)) => {}
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

fn init_uterxai_runtime(cfg: &AppConfig) -> UterxAiRuntime {
    let default_url = "https://api.z.ai/v1/chat/completions".to_string();
    let default_model = "glm-4.5-air".to_string();

    if !cfg.plugins.enabled {
        return UterxAiRuntime {
            instance: None,
            pane_id: None,
            last_io_write_count: 0,
            prompt_buffer: String::new(),
            status: "UterxAI disabled by config".to_string(),
            transcript: Vec::new(),
            settings_path: None,
            api_key: String::new(),
            api_url: default_url,
            api_model: default_model,
            setup_mode: false,
            setup_field: UterxAiSetupField::ApiKey,
        };
    }

    let mut manager = PluginManager::new(cfg.plugins_dir());
    if let Err(err) = manager.scan() {
        return UterxAiRuntime {
            instance: None,
            pane_id: None,
            last_io_write_count: 0,
            prompt_buffer: String::new(),
            status: format!("UterxAI scan failed: {}", err),
            transcript: Vec::new(),
            settings_path: None,
            api_key: String::new(),
            api_url: default_url,
            api_model: default_model,
            setup_mode: true,
            setup_field: UterxAiSetupField::ApiKey,
        };
    }

    let Some(manifest) = manager.get("uterxai").cloned() else {
        return UterxAiRuntime {
            instance: None,
            pane_id: None,
            last_io_write_count: 0,
            prompt_buffer: String::new(),
            status: "UterxAI not installed (run: uterx plugin add uterxai)".to_string(),
            transcript: Vec::new(),
            settings_path: None,
            api_key: String::new(),
            api_url: default_url,
            api_model: default_model,
            setup_mode: true,
            setup_field: UterxAiSetupField::ApiKey,
        };
    };

    let Some(wasm_path) = manager.wasm_path("uterxai") else {
        return UterxAiRuntime {
            instance: None,
            pane_id: None,
            last_io_write_count: 0,
            prompt_buffer: String::new(),
            status: "UterxAI wasm path missing".to_string(),
            transcript: Vec::new(),
            settings_path: None,
            api_key: String::new(),
            api_url: default_url,
            api_model: default_model,
            setup_mode: true,
            setup_field: UterxAiSetupField::ApiKey,
        };
    };

    let settings_path = wasm_path
        .parent()
        .map(|plugin_dir| {
            plugin_dir
                .join("plugin_fs")
                .join("uterxai")
                .join("provider.toml")
        });

    let (api_key, api_url, api_model) = settings_path
        .as_deref()
        .and_then(load_uterxai_settings)
        .unwrap_or_else(|| (String::new(), default_url.clone(), default_model.clone()));
    let setup_mode = api_key.trim().is_empty();

    let granted_permissions = manifest.permissions.clone();
    match PluginInstance::load(&wasm_path, manifest, granted_permissions) {
        Ok(mut instance) => {
            if let Err(err) = instance.start() {
                UterxAiRuntime {
                    instance: None,
                    pane_id: None,
                    last_io_write_count: 0,
                    prompt_buffer: String::new(),
                    status: format!("UterxAI start failed: {}", err),
                    transcript: Vec::new(),
                    settings_path,
                    api_key,
                    api_url,
                    api_model,
                    setup_mode,
                    setup_field: UterxAiSetupField::ApiKey,
                }
            } else {
                UterxAiRuntime {
                    instance: Some(instance),
                    pane_id: None,
                    last_io_write_count: 0,
                    prompt_buffer: String::new(),
                    status: if setup_mode {
                        "UterxAI setup required (Ctrl+Shift+A)".to_string()
                    } else {
                        "UterxAI runtime ready (Ctrl+Shift+A)".to_string()
                    },
                    transcript: Vec::new(),
                    settings_path,
                    api_key,
                    api_url,
                    api_model,
                    setup_mode,
                    setup_field: UterxAiSetupField::ApiKey,
                }
            }
        }
        Err(err) => UterxAiRuntime {
            instance: None,
            pane_id: None,
            last_io_write_count: 0,
            prompt_buffer: String::new(),
            status: format!("UterxAI load failed: {}", err),
            transcript: Vec::new(),
            settings_path,
            api_key,
            api_url,
            api_model,
            setup_mode,
            setup_field: UterxAiSetupField::ApiKey,
        },
    }
}

fn load_uterxai_settings(path: &Path) -> Option<(String, String, String)> {
    #[derive(serde::Deserialize)]
    struct ProviderSettings {
        api_key: Option<String>,
        api_url: Option<String>,
        api_model: Option<String>,
    }

    let raw = fs::read_to_string(path).ok()?;
    let parsed: ProviderSettings = toml::from_str(&raw).ok()?;
    let key = parsed.api_key.unwrap_or_default().trim().to_string();
    let url = parsed
        .api_url
        .unwrap_or_else(|| "https://api.z.ai/v1/chat/completions".to_string())
        .trim()
        .to_string();
    let model = parsed
        .api_model
        .unwrap_or_else(|| "glm-4.5-air".to_string())
        .trim()
        .to_string();
    Some((key, url, model))
}

fn save_uterxai_settings(path: &Path, key: &str, url: &str, model: &str) -> anyhow::Result<()> {
    #[derive(serde::Serialize)]
    struct ProviderSettings<'a> {
        api_key: &'a str,
        api_url: &'a str,
        api_model: &'a str,
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let body = toml::to_string_pretty(&ProviderSettings {
        api_key: key,
        api_url: url,
        api_model: model,
    })?;
    fs::write(path, body)?;
    Ok(())
}

fn normalize_uterxai_setup_values(api_key: &str, api_url: &str, api_model: &str) -> (String, String, String) {
    let key_text = api_key.trim().to_string();
    let url_text = if api_url.trim().is_empty() {
        "https://api.z.ai/v1/chat/completions".to_string()
    } else {
        api_url.trim().to_string()
    };
    let model_text = if api_model.trim().is_empty() {
        "glm-4.5-air".to_string()
    } else {
        api_model.trim().to_string()
    };
    (key_text, url_text, model_text)
}

fn validate_uterxai_setup(api_key: &str, api_url: &str, api_model: &str) -> Vec<String> {
    let mut errors = Vec::new();

    if api_key.is_empty() {
        errors.push("API key is required".to_string());
    } else if api_key.chars().count() < 10 {
        errors.push("API key seems too short".to_string());
    }

    if !(api_url.starts_with("http://") || api_url.starts_with("https://")) {
        errors.push("API URL must start with http:// or https://".to_string());
    }

    if api_model.is_empty() {
        errors.push("API model is required".to_string());
    }

    errors
}

fn uterxai_setup_field_hint(field: UterxAiSetupField) -> &'static str {
    match field {
        UterxAiSetupField::ApiKey => {
            "Hint: Paste your z.ai API key (hidden in UI, stored in provider.toml)."
        }
        UterxAiSetupField::ApiUrl => {
            "Hint: z.ai endpoint, e.g. https://api.z.ai/v1/chat/completions"
        }
        UterxAiSetupField::ApiModel => {
            "Hint: Model name, e.g. glm-4.5-air"
        }
    }
}

fn uterxai_setup_field_label(field: UterxAiSetupField) -> &'static str {
    match field {
        UterxAiSetupField::ApiKey => "API Key",
        UterxAiSetupField::ApiUrl => "API URL",
        UterxAiSetupField::ApiModel => "API Model",
    }
}

fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "[not set]".to_string();
    }
    let count = trimmed.chars().count();
    if count <= 6 {
        return "*".repeat(count.max(1));
    }
    let head: String = trimmed.chars().take(3).collect();
    let tail: String = trimmed
        .chars()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}{}{}", head, "*".repeat(count - 6), tail)
}

fn render_uterxai_pane(session: &mut Session, runtime: &UterxAiRuntime) {
    let Some(pane_id) = runtime.pane_id else {
        return;
    };
    let Some(tab) = session.active_tab_mut() else {
        return;
    };
    let Some(pane) = tab.panes.iter_mut().find(|pane| pane.id == pane_id) else {
        return;
    };

    pane.grid.clear();
    pane.grid.title = "UterxAI".to_string();

    append_text_to_grid(&mut pane.grid, "UterxAI");
    append_text_to_grid(&mut pane.grid, &runtime.status);

    if runtime.setup_mode {
        append_text_to_grid(&mut pane.grid, "Setup: configure provider.toml in-app");
        append_text_to_grid(&mut pane.grid, "Use Up/Down/Tab to move, Enter to continue/save.");
        append_text_to_grid(
            &mut pane.grid,
            &format!("[editing {}]", uterxai_setup_field_label(runtime.setup_field)),
        );

        let key_prefix = if runtime.setup_field == UterxAiSetupField::ApiKey {
            ">"
        } else {
            " "
        };
        let url_prefix = if runtime.setup_field == UterxAiSetupField::ApiUrl {
            ">"
        } else {
            " "
        };
        let model_prefix = if runtime.setup_field == UterxAiSetupField::ApiModel {
            ">"
        } else {
            " "
        };

        append_text_to_grid(
            &mut pane.grid,
            &format!("{} API Key  : {}", key_prefix, mask_secret(&runtime.api_key)),
        );
        append_text_to_grid(
            &mut pane.grid,
            &format!("{} API URL  : {}", url_prefix, runtime.api_url),
        );
        append_text_to_grid(
            &mut pane.grid,
            &format!("{} API Model: {}", model_prefix, runtime.api_model),
        );
        append_text_to_grid(
            &mut pane.grid,
            uterxai_setup_field_hint(runtime.setup_field),
        );

        let (key_text, url_text, model_text) = normalize_uterxai_setup_values(
            &runtime.api_key,
            &runtime.api_url,
            &runtime.api_model,
        );
        let errors = validate_uterxai_setup(&key_text, &url_text, &model_text);
        if errors.is_empty() {
            append_text_to_grid(&mut pane.grid, "Validation: ✅ ready");
        } else {
            append_text_to_grid(
                &mut pane.grid,
                &format!("Validation: ❌ {} issue(s)", errors.len()),
            );
            for error in errors {
                append_text_to_grid(&mut pane.grid, &format!(" - {}", error));
            }
        }
        return;
    }

    append_text_to_grid(&mut pane.grid, "Chat transcript:");
    for line in &runtime.transcript {
        append_text_to_grid(&mut pane.grid, line);
    }
    append_text_to_grid(&mut pane.grid, &format!("> {}", runtime.prompt_buffer));
}

fn open_or_focus_uterxai_pane(session: &mut Session, runtime: &mut UterxAiRuntime, area: MuxRect) {
    if let Some(existing_id) = runtime.pane_id {
        if focus_pane_by_id(session, existing_id) {
            render_uterxai_pane(session, runtime);
            return;
        }
        runtime.pane_id = None;
    }

    let pane_id = uterx_mux::PaneId(session.next_id());
    let pane_rect = uterx_mux::Rect {
        x: area.x,
        y: area.y,
        width: area.width.max(20),
        height: area.height.max(5),
    };
    let mut pane = uterx_mux::Pane::new_bare(pane_id, pane_rect);
    pane.grid.title = "UterxAI".to_string();

    if let Some(tab) = session.active_tab_mut() {
        for p in &mut tab.panes {
            p.focused = false;
        }
        pane.focused = true;
        tab.active_pane = Some(pane_id);
        tab.add_pane(pane);
        tab.relayout(area);
    }
    runtime.pane_id = Some(pane_id);
    render_uterxai_pane(session, runtime);
}

fn focus_pane_by_id(session: &mut Session, pane_id: uterx_mux::PaneId) -> bool {
    let Some(tab) = session.active_tab_mut() else {
        return false;
    };

    if !tab.panes.iter().any(|pane| pane.id == pane_id) {
        return false;
    }
    for pane in &mut tab.panes {
        pane.focused = pane.id == pane_id;
    }
    tab.active_pane = Some(pane_id);
    true
}

fn sync_uterxai_output_into_pane(session: &mut Session, runtime: &mut UterxAiRuntime) {
    let Some(instance) = runtime.instance.as_ref() else {
        return;
    };
    let Some(pane_id) = runtime.pane_id else {
        return;
    };

    let io_calls = instance.io_write_calls();
    if runtime.last_io_write_count >= io_calls.len() {
        return;
    }

    for call in &io_calls[runtime.last_io_write_count..] {
        let text = String::from_utf8_lossy(&call.bytes);
        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                runtime.transcript.push(trimmed.to_string());
            }
        }
    }
    runtime.last_io_write_count = io_calls.len();
    if !focus_pane_by_id(session, pane_id) {
        runtime.pane_id = None;
        return;
    }
    render_uterxai_pane(session, runtime);
}

fn handle_uterxai_prompt_key(
    key: &crossterm::event::KeyEvent,
    session: &mut Session,
    runtime: &mut UterxAiRuntime,
) -> bool {
    let Some(ai_pane_id) = runtime.pane_id else {
        return false;
    };

    let Some(active_tab) = session.active_tab() else {
        return false;
    };
    if active_tab.active_pane != Some(ai_pane_id) {
        return false;
    }

    if runtime.setup_mode {
        let mut consumed = true;
        match key.code {
            KeyCode::Up => {
                runtime.setup_field = runtime.setup_field.prev();
            }
            KeyCode::Down | KeyCode::Tab => {
                runtime.setup_field = runtime.setup_field.next();
            }
            KeyCode::Backspace => match runtime.setup_field {
                UterxAiSetupField::ApiKey => {
                    runtime.api_key.pop();
                }
                UterxAiSetupField::ApiUrl => {
                    runtime.api_url.pop();
                }
                UterxAiSetupField::ApiModel => {
                    runtime.api_model.pop();
                }
            },
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                match runtime.setup_field {
                    UterxAiSetupField::ApiKey => runtime.api_key.push(c),
                    UterxAiSetupField::ApiUrl => runtime.api_url.push(c),
                    UterxAiSetupField::ApiModel => runtime.api_model.push(c),
                }
            }
            KeyCode::Enter => {
                if runtime.setup_field != UterxAiSetupField::ApiModel {
                    runtime.setup_field = runtime.setup_field.next();
                } else {
                    let (key_text, url_text, model_text) = normalize_uterxai_setup_values(
                        &runtime.api_key,
                        &runtime.api_url,
                        &runtime.api_model,
                    );

                    runtime.api_key = key_text.clone();
                    runtime.api_url = url_text.clone();
                    runtime.api_model = model_text.clone();

                    let errors = validate_uterxai_setup(&key_text, &url_text, &model_text);
                    if !errors.is_empty() {
                        runtime.setup_mode = true;
                        runtime.status = format!("UterxAI setup invalid: {}", errors[0]);
                        render_uterxai_pane(session, runtime);
                        return true;
                    }

                    match runtime.settings_path.as_deref() {
                        Some(path) => {
                            match save_uterxai_settings(path, &key_text, &url_text, &model_text) {
                                Ok(()) => {
                                    runtime.setup_mode = false;
                                    runtime.status = "UterxAI setup saved".to_string();
                                    runtime.transcript.push("UterxAI setup completed.".to_string());
                                }
                                Err(err) => {
                                    runtime.status = format!("UterxAI setup save failed: {}", err);
                                }
                            }
                        }
                        None => {
                            runtime.status = "UterxAI setup path unavailable".to_string();
                        }
                    }
                }
            }
            _ => {
                consumed = false;
            }
        }

        if consumed {
            render_uterxai_pane(session, runtime);
        }
        return consumed;
    }

    match key.code {
        KeyCode::Enter => {
            let prompt = runtime.prompt_buffer.trim().to_string();
            if prompt.is_empty() {
                render_uterxai_pane(session, runtime);
                return true;
            }
            runtime.prompt_buffer.clear();
            runtime.transcript.push(format!("> {}", prompt));
            runtime.transcript.push("UterxAI: prompt accepted (stream pending)".to_string());

            if let Some(instance) = runtime.instance.as_mut() {
                instance.push_ai_stream_chunk(1, prompt.as_bytes());
                instance.push_ai_stream_chunk(1, b"\n");

                if let Err(err) = instance.call_i32_func("uterxai_poll") {
                    runtime
                        .transcript
                        .push(format!("UterxAI poll failed: {}", err));
                }
            } else {
                runtime
                    .transcript
                    .push("UterxAI runtime unavailable".to_string());
            }
            render_uterxai_pane(session, runtime);
            true
        }
        KeyCode::Backspace => {
            runtime.prompt_buffer.pop();
            render_uterxai_pane(session, runtime);
            true
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            runtime.prompt_buffer.push(c);
            render_uterxai_pane(session, runtime);
            true
        }
        _ => false,
    }
}

fn append_text_to_grid(grid: &mut uterx_core::Grid, text: &str) {
    for ch in text.chars() {
        if ch == '\n' {
            grid.cursor_col = 0;
            grid.newline();
        } else if ch == '\r' {
            grid.cursor_col = 0;
        } else {
            grid.write_char(ch);
        }
    }
    grid.cursor_col = 0;
    grid.newline();
}

fn build_initial_session(
    shell: &str,
    pane_area: MuxRect,
    restore_on_launch: bool,
) -> anyhow::Result<Session> {
    if restore_on_launch {
        let restore_path = Session::sessions_dir().join("main.toml");
        if restore_path.exists() {
            match Session::load_state(&restore_path)
                .and_then(|state| Session::restore(&state, shell, pane_area))
            {
                Ok(session) if !session.tabs.is_empty() => {
                    tracing::info!("restored session from {}", restore_path.display());
                    return Ok(session);
                }
                Ok(_) => {
                    tracing::warn!(
                        "restored session was empty, starting fresh session"
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        "failed to restore session from {}: {}",
                        restore_path.display(),
                        err
                    );
                }
            }
        }
    }

    let mut session = Session::new("main");
    session.create_tab(shell, pane_area)?;
    Ok(session)
}

fn normalize_focus_state(session: &mut Session) {
    let active_tab_id = session.active_tab;
    for tab in &mut session.tabs {
        for pane in &mut tab.panes {
            pane.focused = false;
        }
    }

    if let Some(tab_id) = active_tab_id {
        if let Some(tab) = session.tabs.iter_mut().find(|t| t.id == tab_id) {
            let pane_id = tab.active_pane.or_else(|| tab.panes.first().map(|p| p.id));
            tab.active_pane = pane_id;
            if let Some(pid) = pane_id {
                if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pid) {
                    pane.focused = true;
                    return;
                }
            }
        }
    }

    if let Some(first_tab) = session.tabs.first_mut() {
        let pane_id = first_tab.active_pane.or_else(|| first_tab.panes.first().map(|p| p.id));
        first_tab.active_pane = pane_id;
        session.active_tab = Some(first_tab.id);
        if let Some(pid) = pane_id {
            if let Some(pane) = first_tab.panes.iter_mut().find(|p| p.id == pid) {
                pane.focused = true;
            }
        }
    }
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
    UterxAi,
    ToggleFloat,
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
        "UterxAI" => Some(PaletteAction::UterxAi),
        "Toggle Floating" => Some(PaletteAction::ToggleFloat),
        "Quit" => Some(PaletteAction::Quit),
        _ => None,
    }
}

/// Execute a command palette action. Returns true if the app should quit.
fn execute_palette_action(
    action: PaletteAction,
    session: &mut Session,
    uterxai: &mut UterxAiRuntime,
    overlay: &mut Overlay,
    file_browser: &mut FileBrowserState,
    focus: &mut Focus,
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    shell: &str,
    show_tab_bar: bool,
    show_status_bar: bool,
) -> bool {
    let sidebar_w = if file_browser.visible { file_browser.width } else { 0 };
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
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar,
                if file_browser.visible { file_browser.width } else { 0 });
            session.relayout_all(area);
        }
        PaletteAction::UterxAi => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, sidebar_w);
            open_or_focus_uterxai_pane(session, uterxai, area);
            *focus = Focus::Terminal;
        }
        PaletteAction::NewTab => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, sidebar_w);
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
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, sidebar_w);
            if let Some(tab) = session.active_tab_mut() { tab.relayout(area); }
            let _ = session.split_vertical(shell);
            if let Some(tab) = session.active_tab_mut() { tab.relayout(area); }
        }
        PaletteAction::SplitHorizontal => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, sidebar_w);
            if let Some(tab) = session.active_tab_mut() { tab.relayout(area); }
            let _ = session.split_horizontal(shell);
            if let Some(tab) = session.active_tab_mut() { tab.relayout(area); }
        }
        PaletteAction::NextPane => {
            if let Some(tab) = session.active_tab_mut() { tab.focus_next(); }
        }
        PaletteAction::PrevPane => {
            if let Some(tab) = session.active_tab_mut() { tab.focus_prev(); }
        }
        PaletteAction::Broadcast => {
            if let Some(tab) = session.active_tab_mut() { tab.toggle_broadcast(); }
        }
        PaletteAction::ToggleFloat => {
            let area = compute_pane_area(terminal, show_tab_bar, show_status_bar, sidebar_w);
            if let Some(tab) = session.active_tab_mut() { tab.toggle_float(area); }
        }
    }
    false
}

/// Filter commands by substring match.
fn filter_commands(all: &[CommandEntry], filter: &str) -> Vec<CommandEntry> {
    if filter.is_empty() {
        return all.to_vec();
    }
    let f = filter.to_lowercase();
    all.iter()
        .filter(|c| {
            c.label.to_lowercase().contains(&f)
                || c.shortcut.to_lowercase().contains(&f)
                || c.description.to_lowercase().contains(&f)
        })
        .cloned()
        .collect()
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
        rect: ratatui::layout::Rect { x: fx, y: fy, width: fw, height: fh },
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

/// Compute the pane area accounting for chrome and sidebar.
fn compute_pane_area(
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    show_tab_bar: bool,
    show_status_bar: bool,
    sidebar_width: u16,
) -> MuxRect {
    let size = terminal.size().unwrap_or_default();
    let chrome_rows: u16 =
        if show_tab_bar { 1 } else { 0 } + if show_status_bar { 1 } else { 0 };
    MuxRect {
        x: 0,
        y: 0,
        width: size.width.saturating_sub(sidebar_width),
        height: size.height.saturating_sub(chrome_rows),
    }
}

/// Compute the content area as screen-coordinate Rect (for editor pane placement).
fn compute_screen_area(
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    show_tab_bar: bool,
    show_status_bar: bool,
    sidebar_width: u16,
) -> ratatui::layout::Rect {
    let size = terminal.size().unwrap_or_default();
    let chrome_y: u16 = if show_tab_bar { 1 } else { 0 };
    let chrome_bot: u16 = if show_status_bar { 1 } else { 0 };
    ratatui::layout::Rect {
        x: sidebar_width,
        y: chrome_y,
        width: size.width.saturating_sub(sidebar_width),
        height: size.height.saturating_sub(chrome_y + chrome_bot),
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

/// Hit-test tiled split boundaries in pane-local coordinates.
fn hit_split_boundary(
    tab: &uterx_mux::tab::Tab,
    mx: u16,
    my: u16,
) -> Option<(SplitResizeOrientation, usize)> {
    const HIT_TOLERANCE: u16 = 1;

    let tiled: Vec<_> = tab.panes.iter().filter(|p| !p.is_floating).collect();
    if tiled.len() < 2 {
        return None;
    }

    match &tab.layout {
        MuxLayout::VerticalSplit { .. } => {
            for i in 1..tiled.len() {
                let boundary_x = tiled[i].rect.x;
                let top = tiled[i].rect.y;
                let bottom = tiled[i].rect.y + tiled[i].rect.height;
                if my >= top && my < bottom && mx.abs_diff(boundary_x) <= HIT_TOLERANCE {
                    return Some((SplitResizeOrientation::Vertical, i - 1));
                }
            }
            None
        }
        MuxLayout::HorizontalSplit { .. } => {
            for i in 1..tiled.len() {
                let boundary_y = tiled[i].rect.y;
                let left = tiled[i].rect.x;
                let right = tiled[i].rect.x + tiled[i].rect.width;
                if mx >= left && mx < right && my.abs_diff(boundary_y) <= HIT_TOLERANCE {
                    return Some((SplitResizeOrientation::Horizontal, i - 1));
                }
            }
            None
        }
        _ => None,
    }
}

fn tab_bar_hover_at(column: u16, term_width: u16, session: &Session) -> Option<TabBarHover> {
    let brand_width: u16 = 8; // " uterx │"
    let right_section_width: u16 = 12;
    let plus_x = term_width.saturating_sub(right_section_width);
    let help_x = plus_x + 5;

    if column >= plus_x && column < plus_x + 5 {
        return Some(TabBarHover::Plus);
    }
    if column >= help_x && column < help_x + 5 {
        return Some(TabBarHover::Help);
    }
    if column < brand_width {
        return None;
    }

    let mut x = brand_width;
    let max_tab_x = term_width.saturating_sub(right_section_width);
    for (i, t) in session.tabs.iter().enumerate() {
        let display_name = if let Some(pane) = t.focused_pane() {
            if pane.grid.title.is_empty() {
                t.name.clone()
            } else {
                pane.grid.title.clone()
            }
        } else {
            t.name.clone()
        };
        let label = format!(" {}:{} ", i + 1, display_name);
        let tab_w = label.len() as u16;
        if x + tab_w > max_tab_x {
            break;
        }
        if column >= x && column < x + tab_w {
            return Some(TabBarHover::Tab(i));
        }
        x += tab_w + 1;
    }
    None
}

fn hit_tiled_title_bar(
    tab: &uterx_mux::tab::Tab,
    mx: u16,
    my: u16,
) -> Option<uterx_mux::PaneId> {
    let tiled_count = tab.panes.iter().filter(|p| !p.is_floating).count();
    if tiled_count < 2 {
        return None;
    }

    for pane in tab.panes.iter().filter(|p| !p.is_floating) {
        let r = pane.rect;
        if my == r.y && mx >= r.x && mx < r.x + r.width {
            return Some(pane.id);
        }
    }
    None
}

fn hit_tiled_pane(
    tab: &uterx_mux::tab::Tab,
    mx: u16,
    my: u16,
) -> Option<uterx_mux::PaneId> {
    for pane in tab.panes.iter().filter(|p| !p.is_floating) {
        let r = pane.rect;
        if mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height {
            return Some(pane.id);
        }
    }
    None
}

/// Handle a mouse event — focus pane on click, scroll wheel.
fn handle_mouse_event(
    session: &mut Session,
    mouse: crossterm::event::MouseEvent,
    show_tab_bar: bool,
    sidebar_offset: u16,
) {
    let tab = match session.active_tab_mut() {
        Some(t) => t,
        None => return,
    };

    let offset_y = if show_tab_bar { 1u16 } else { 0 };
    let mx = mouse.column.saturating_sub(sidebar_offset);
    let my = mouse.row.saturating_sub(offset_y);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let mut target_pane_id = None;
            for pane in &tab.panes {
                let r = &pane.rect;
                if mx >= r.x && mx < r.x + r.width && my >= r.y && my < r.y + r.height {
                    target_pane_id = Some(pane.id);
                    break;
                }
            }
            if let Some(pid) = target_pane_id {
                for pane in &mut tab.panes {
                    pane.focused = pane.id == pid;
                }
                tab.active_pane = Some(pid);
            }
        }
        MouseEventKind::ScrollUp => {}
        MouseEventKind::ScrollDown => {}
        _ => {}
    }
}
