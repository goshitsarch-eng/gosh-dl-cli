use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use gosh_dl::DownloadEngine;
use gosh_dl::{DownloadEvent, DownloadState, DownloadStatus};
use ratatui::prelude::*;
use std::collections::VecDeque;
use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::{Duration, Instant};
use throbber_widgets_tui::ThrobberState;

use crate::config::CliConfig;
use crate::util::truncate_str;

use super::event::{self, AppEvent, EventHandler};
use super::theme::Theme;
use super::ui;

/// TUI Application state
pub struct TuiApp {
    /// The download engine
    engine: Arc<DownloadEngine>,

    /// Application configuration
    config: CliConfig,

    /// Color theme
    theme: Theme,

    /// Current view mode
    pub mode: ViewMode,

    /// Cached list of downloads
    pub downloads: Vec<DownloadStatus>,

    /// Currently selected download index
    pub selected: usize,

    /// Scroll offset for download list
    pub scroll_offset: usize,

    /// Last known visible height for download list
    pub last_visible_height: usize,

    /// Speed history for graph (last 60 samples: download, upload)
    pub speed_history: VecDeque<(u64, u64)>,

    /// Whether help overlay is shown
    pub show_help: bool,

    /// Active dialog (add URL, confirm cancel, etc.)
    pub dialog: Option<DialogState>,

    /// Last frame timestamp for effect timing
    pub last_frame: Instant,

    /// Global download speed
    pub download_speed: u64,

    /// Global upload speed
    pub upload_speed: u64,

    /// Throbber state for animated spinners
    pub throbber_state: ThrobberState,

    /// Active toast notifications
    pub toasts: Vec<Toast>,

    /// Effect manager for tachyonfx animations
    pub effect_manager: tachyonfx::EffectManager<()>,

    /// Whether startup effects have been queued
    pub startup_effects_added: bool,

    /// Current layout mode based on terminal size
    pub layout_mode: LayoutMode,

    /// Terminal width
    pub terminal_width: u16,

    /// Terminal height
    pub terminal_height: u16,

    /// Right panel focus (for two-column mode)
    pub right_panel_focus: RightPanelFocus,

    /// Active search/filter state
    pub search: Option<SearchState>,

    /// Peak download speed observed
    pub peak_download_speed: u64,

    /// Peak upload speed observed
    pub peak_upload_speed: u64,

    /// Chunk states for the selected download
    pub chunk_states: Vec<ChunkState>,

    /// Number of chunks for the selected download
    pub chunk_count: usize,

    /// Activity log entries
    pub activity_log: VecDeque<ActivityEntry>,

    /// Whether activity log panel is focused/visible
    pub show_activity_log: bool,

    /// Scroll offset for activity log
    pub activity_log_scroll: usize,

    /// Should quit
    should_quit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    All,
    Active,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    TwoColumn,
    SingleColumn,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelFocus {
    Graph,
    Details,
    ChunkMap,
}

#[derive(Debug)]
pub enum DialogState {
    AddUrl {
        input: String,
        cursor: usize,
    },
    ConfirmCancel {
        id: gosh_dl::DownloadId,
        delete_files: bool,
    },
    Error {
        message: String,
    },
    Settings {
        active_tab: usize,
        selected_row: usize,
        editing: Option<String>,
        draft: Box<CliConfig>,
        dirty: bool,
    },
    BatchImport {
        phase: BatchPhase,
    },
}

#[derive(Debug)]
pub enum BatchPhase {
    Input {
        text: String,
        cursor_line: usize,
        cursor_col: usize,
    },
    Review {
        entries: Vec<BatchEntry>,
        selected: usize,
    },
}

#[derive(Debug)]
pub struct BatchEntry {
    pub url: String,
    pub valid: bool,
    pub selected: bool,
    pub kind: String,
    pub error: Option<String>,
}

pub struct SearchState {
    pub query: String,
    pub cursor: usize,
    pub scope: SearchScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    All,
    Name,
    Url,
    State,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            scope: SearchScope::All,
        }
    }
}

impl SearchScope {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Name => "Name",
            Self::Url => "URL",
            Self::State => "State",
        }
    }
    pub fn next(&self) -> Self {
        match self {
            Self::All => Self::Name,
            Self::Name => Self::Url,
            Self::Url => Self::State,
            Self::State => Self::All,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkState {
    Pending,
    Downloading,
    Complete,
    Failed,
}

pub struct ActivityEntry {
    pub timestamp: Instant,
    pub level: ActivityLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Toast notification
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created: Instant,
}

#[derive(Clone, Copy)]
pub enum ToastLevel {
    Success,
    Error,
}

impl TuiApp {
    pub async fn new(config: CliConfig) -> Result<Self> {
        // Ensure database directory exists
        if let Some(parent) = config.general.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let engine_config = config.to_engine_config();
        let engine = DownloadEngine::new(engine_config).await?;

        // Get initial download list
        let downloads = engine.list();

        let theme = Theme::from_name(&config.tui.theme);

        let (terminal_width, terminal_height) = crossterm::terminal::size().unwrap_or((80, 24));

        let layout_mode = if terminal_width >= 100 && terminal_height >= 24 {
            LayoutMode::TwoColumn
        } else if terminal_width >= 80 && terminal_height >= 20 {
            LayoutMode::SingleColumn
        } else {
            LayoutMode::Minimal
        };

        Ok(Self {
            engine,
            config,
            theme,
            mode: ViewMode::All,
            downloads,
            selected: 0,
            scroll_offset: 0,
            last_visible_height: 20,
            speed_history: VecDeque::with_capacity(60),
            show_help: false,
            dialog: None,
            last_frame: Instant::now(),
            download_speed: 0,
            upload_speed: 0,
            throbber_state: ThrobberState::default(),
            toasts: Vec::new(),
            effect_manager: tachyonfx::EffectManager::default(),
            startup_effects_added: false,
            layout_mode,
            terminal_width,
            terminal_height,
            right_panel_focus: RightPanelFocus::Details,
            search: None,
            peak_download_speed: 0,
            peak_upload_speed: 0,
            chunk_states: Vec::new(),
            chunk_count: 0,
            activity_log: VecDeque::new(),
            show_activity_log: false,
            activity_log_scroll: 0,
            should_quit: false,
        })
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    fn reorder_download(&mut self, direction: i32) {
        let len = self.downloads.len();
        if len < 2 {
            return;
        }

        let new_idx = if direction > 0 {
            if self.selected + 1 >= len {
                return;
            }
            self.selected + 1
        } else {
            if self.selected == 0 {
                return;
            }
            self.selected - 1
        };

        self.downloads.swap(self.selected, new_idx);
        self.selected = new_idx;
        self.adjust_scroll(self.last_visible_height);
    }

    fn detect_layout_mode(&mut self) {
        self.layout_mode = if self.terminal_width >= 100 && self.terminal_height >= 24 {
            LayoutMode::TwoColumn
        } else if self.terminal_width >= 80 && self.terminal_height >= 20 {
            LayoutMode::SingleColumn
        } else {
            LayoutMode::Minimal
        };
    }

    /// Run the TUI event loop
    pub async fn run(&mut self) -> Result<()> {
        // Install panic hook that restores the terminal before printing the panic
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            original_hook(panic_info);
        }));

        // Setup terminal
        let mut terminal = setup_terminal()?;

        // Create event handler
        let tick_rate = Duration::from_millis(self.config.tui.refresh_rate_ms);
        let mut event_handler = EventHandler::new(self.engine.subscribe(), tick_rate);

        // Main loop
        loop {
            // Draw UI
            terminal.draw(|frame| ui::render(frame, self))?;

            // Handle events
            match event_handler.next().await? {
                AppEvent::Terminal(event) => {
                    if self.handle_terminal_event(&event).await? {
                        break;
                    }
                }
                AppEvent::Engine(event) => {
                    self.handle_engine_event(event);
                }
                AppEvent::Tick => {
                    self.update_stats();
                }
                AppEvent::Resync => {
                    // Full resync after missed broadcast events
                    self.refresh_downloads();
                    self.update_stats();
                }
                AppEvent::Resize(w, h) => {
                    self.terminal_width = w;
                    self.terminal_height = h;
                    self.detect_layout_mode();
                }
            }

            if self.should_quit {
                break;
            }
        }

        // Restore terminal
        restore_terminal(terminal)?;

        // Restore original panic hook now that the terminal is back to normal
        let _ = std::panic::take_hook();

        // Shutdown engine
        self.engine.shutdown().await?;

        Ok(())
    }

    /// Handle terminal input events
    async fn handle_terminal_event(&mut self, event: &crossterm::event::Event) -> Result<bool> {
        // Handle dialog input first
        if let Some(ref mut dialog) = self.dialog {
            match dialog {
                DialogState::AddUrl { input, cursor } => {
                    if event::is_escape(event) {
                        self.dialog = None;
                    } else if event::is_enter(event) {
                        if !input.is_empty() {
                            let url = input.clone();
                            self.dialog = None;
                            self.add_download(&url).await?;
                        }
                    } else if let crossterm::event::Event::Key(key) = event {
                        // cursor is a *character* index, not a byte offset
                        match key.code {
                            crossterm::event::KeyCode::Char(c) => {
                                let byte_pos = input
                                    .char_indices()
                                    .nth(*cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(input.len());
                                input.insert(byte_pos, c);
                                *cursor += 1;
                            }
                            crossterm::event::KeyCode::Backspace => {
                                if *cursor > 0 {
                                    *cursor -= 1;
                                    let byte_pos = input
                                        .char_indices()
                                        .nth(*cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(input.len());
                                    input.remove(byte_pos);
                                }
                            }
                            crossterm::event::KeyCode::Left => {
                                if *cursor > 0 {
                                    *cursor -= 1;
                                }
                            }
                            crossterm::event::KeyCode::Right => {
                                if *cursor < input.chars().count() {
                                    *cursor += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                    return Ok(false);
                }
                DialogState::ConfirmCancel { id, delete_files } => {
                    if event::is_escape(event) || event::is_key(event, 'n') {
                        self.dialog = None;
                    } else if event::is_key(event, 'y') || event::is_enter(event) {
                        let id = *id;
                        let delete = *delete_files;
                        self.dialog = None;
                        if let Err(e) = self.engine.cancel(id, delete).await {
                            self.dialog = Some(DialogState::Error {
                                message: e.to_string(),
                            });
                        }
                    }
                    return Ok(false);
                }
                DialogState::Error { .. } => {
                    if event::is_escape(event)
                        || event::is_enter(event)
                        || event::is_key(event, 'q')
                    {
                        self.dialog = None;
                    }
                    return Ok(false);
                }
                DialogState::Settings {
                    active_tab,
                    selected_row,
                    editing,
                    draft,
                    dirty,
                } => {
                    if let crossterm::event::Event::Key(key) = event {
                        if editing.is_some() {
                            match key.code {
                                crossterm::event::KeyCode::Esc => {
                                    *editing = None;
                                }
                                crossterm::event::KeyCode::Enter => {
                                    if let Some(val) = editing.take() {
                                        Self::apply_settings_edit(
                                            draft,
                                            *active_tab,
                                            *selected_row,
                                            &val,
                                        );
                                        *dirty = true;
                                    }
                                }
                                crossterm::event::KeyCode::Backspace => {
                                    if let Some(ref mut buf) = editing {
                                        buf.pop();
                                    }
                                }
                                crossterm::event::KeyCode::Char(c) => {
                                    if let Some(ref mut buf) = editing {
                                        buf.push(c);
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                crossterm::event::KeyCode::Esc => {
                                    if *dirty {
                                        let new_config = *draft.clone();
                                        if new_config.validate().is_ok() {
                                            let _ = new_config.save(None);
                                            self.config = new_config;
                                            let engine_cfg = self.config.to_engine_config();
                                            let _ = self.engine.set_config(engine_cfg);
                                            self.theme = Theme::from_name(&self.config.tui.theme);
                                            self.push_toast(
                                                "Settings saved".to_string(),
                                                ToastLevel::Success,
                                            );
                                        }
                                    }
                                    self.dialog = None;
                                }
                                crossterm::event::KeyCode::Left => {
                                    if *active_tab > 0 {
                                        *active_tab -= 1;
                                        *selected_row = 0;
                                    }
                                }
                                crossterm::event::KeyCode::Right => {
                                    if *active_tab < 4 {
                                        *active_tab += 1;
                                        *selected_row = 0;
                                    }
                                }
                                crossterm::event::KeyCode::Up
                                | crossterm::event::KeyCode::Char('k') => {
                                    if *selected_row > 0 {
                                        *selected_row -= 1;
                                    }
                                }
                                crossterm::event::KeyCode::Down
                                | crossterm::event::KeyCode::Char('j') => {
                                    *selected_row += 1;
                                }
                                crossterm::event::KeyCode::Char(n @ '1'..='5') => {
                                    *active_tab = (n as usize) - ('1' as usize);
                                    *selected_row = 0;
                                }
                                crossterm::event::KeyCode::Enter
                                | crossterm::event::KeyCode::Char(' ') => {
                                    if Self::is_settings_bool(*active_tab, *selected_row) {
                                        Self::toggle_settings_bool(
                                            draft,
                                            *active_tab,
                                            *selected_row,
                                        );
                                        *dirty = true;
                                    } else {
                                        *editing = Some(Self::get_settings_value(
                                            draft,
                                            *active_tab,
                                            *selected_row,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    return Ok(false);
                }
                DialogState::BatchImport { phase } => {
                    if let crossterm::event::Event::Key(key) = event {
                        match phase {
                            BatchPhase::Input {
                                text,
                                cursor_line,
                                cursor_col,
                            } => match key.code {
                                crossterm::event::KeyCode::Esc => {
                                    self.dialog = None;
                                }
                                crossterm::event::KeyCode::Enter => {
                                    if key.modifiers == crossterm::event::KeyModifiers::CONTROL {
                                        let lines: Vec<String> = text
                                            .lines()
                                            .map(|l| l.trim().to_string())
                                            .filter(|l| !l.is_empty())
                                            .collect();
                                        let entries: Vec<BatchEntry> = lines.into_iter().map(|url| {
                                            use crate::input::url_parser::parse_input;
                                            let (valid, kind, error) = match parse_input(&url) {
                                                Ok(parsed) => {
                                                    let kind = match parsed {
                                                        crate::input::url_parser::ParsedInput::Http(_) => "HTTP",
                                                        crate::input::url_parser::ParsedInput::Magnet(_) => "Magnet",
                                                        crate::input::url_parser::ParsedInput::TorrentFile(_) => "Torrent",
                                                    };
                                                    (true, kind.to_string(), None)
                                                }
                                                Err(e) => (false, "?".to_string(), Some(e.to_string())),
                                            };
                                            BatchEntry { url, valid, selected: valid, kind, error }
                                        }).collect();
                                        if !entries.is_empty() {
                                            *phase = BatchPhase::Review {
                                                entries,
                                                selected: 0,
                                            };
                                        }
                                    } else {
                                        text.push('\n');
                                        *cursor_line += 1;
                                        *cursor_col = 0;
                                    }
                                }
                                crossterm::event::KeyCode::Char(c) => {
                                    let mut lines: Vec<&str> = text.lines().collect();
                                    if lines.is_empty() {
                                        lines.push("");
                                    }
                                    while *cursor_line >= lines.len() {
                                        text.push('\n');
                                        lines = text.lines().collect();
                                    }
                                    let line = lines[*cursor_line];
                                    let byte_pos = line
                                        .char_indices()
                                        .nth(*cursor_col)
                                        .map(|(i, _)| i)
                                        .unwrap_or(line.len());
                                    let abs_pos: usize = text
                                        .lines()
                                        .take(*cursor_line)
                                        .map(|l| l.len() + 1)
                                        .sum::<usize>()
                                        + byte_pos;
                                    if abs_pos <= text.len() {
                                        text.insert(abs_pos, c);
                                    } else {
                                        text.push(c);
                                    }
                                    *cursor_col += 1;
                                }
                                crossterm::event::KeyCode::Backspace => {
                                    if *cursor_col > 0 {
                                        *cursor_col -= 1;
                                        let lines: Vec<&str> = text.lines().collect();
                                        if *cursor_line < lines.len() {
                                            let line = lines[*cursor_line];
                                            let byte_pos = line
                                                .char_indices()
                                                .nth(*cursor_col)
                                                .map(|(i, _)| i)
                                                .unwrap_or(line.len());
                                            let abs_pos: usize = text
                                                .lines()
                                                .take(*cursor_line)
                                                .map(|l| l.len() + 1)
                                                .sum::<usize>()
                                                + byte_pos;
                                            if abs_pos < text.len() {
                                                text.remove(abs_pos);
                                            }
                                        }
                                    } else if *cursor_line > 0 {
                                        let lines: Vec<&str> = text.lines().collect();
                                        let prev_col = lines[*cursor_line - 1].chars().count();
                                        let abs_pos: usize = text
                                            .lines()
                                            .take(*cursor_line)
                                            .map(|l| l.len() + 1)
                                            .sum::<usize>()
                                            - 1;
                                        if abs_pos < text.len() {
                                            text.remove(abs_pos);
                                        }
                                        *cursor_line -= 1;
                                        *cursor_col = prev_col;
                                    }
                                }
                                _ => {}
                            },
                            BatchPhase::Review { entries, selected } => match key.code {
                                crossterm::event::KeyCode::Esc => {
                                    let text = entries
                                        .iter()
                                        .map(|e| e.url.as_str())
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    *phase = BatchPhase::Input {
                                        text,
                                        cursor_line: 0,
                                        cursor_col: 0,
                                    };
                                }
                                crossterm::event::KeyCode::Up
                                | crossterm::event::KeyCode::Char('k') => {
                                    if *selected > 0 {
                                        *selected -= 1;
                                    }
                                }
                                crossterm::event::KeyCode::Down
                                | crossterm::event::KeyCode::Char('j') => {
                                    if *selected + 1 < entries.len() {
                                        *selected += 1;
                                    }
                                }
                                crossterm::event::KeyCode::Char(' ') => {
                                    if let Some(e) = entries.get_mut(*selected) {
                                        e.selected = !e.selected;
                                    }
                                }
                                crossterm::event::KeyCode::Enter => {
                                    let urls: Vec<String> = entries
                                        .iter()
                                        .filter(|e| e.selected && e.valid)
                                        .map(|e| e.url.clone())
                                        .collect();
                                    self.dialog = None;
                                    let count = urls.len();
                                    for url in urls {
                                        let _ = self.add_download(&url).await;
                                    }
                                    if count > 0 {
                                        self.push_toast(
                                            format!("Added {} downloads", count),
                                            ToastLevel::Success,
                                        );
                                    }
                                    return Ok(false);
                                }
                                _ => {}
                            },
                        }
                    }
                    return Ok(false);
                }
            }
        }

        // Handle help overlay — any key closes it
        if self.show_help {
            if matches!(event, crossterm::event::Event::Key(_)) {
                self.show_help = false;
            }
            return Ok(false);
        }

        // Handle search input mode
        if let Some(ref mut search) = self.search {
            if let crossterm::event::Event::Key(key) = event {
                match key.code {
                    crossterm::event::KeyCode::Esc => {
                        self.search = None;
                        return Ok(false);
                    }
                    crossterm::event::KeyCode::Enter => {
                        if search.query.is_empty() {
                            self.search = None;
                        }
                        return Ok(false);
                    }
                    crossterm::event::KeyCode::Char(c) => {
                        if key.modifiers == crossterm::event::KeyModifiers::CONTROL && c == 's' {
                            search.scope = search.scope.next();
                        } else if key.modifiers == crossterm::event::KeyModifiers::NONE
                            || key.modifiers == crossterm::event::KeyModifiers::SHIFT
                        {
                            let byte_pos = search
                                .query
                                .char_indices()
                                .nth(search.cursor)
                                .map(|(i, _)| i)
                                .unwrap_or(search.query.len());
                            search.query.insert(byte_pos, c);
                            search.cursor += 1;
                        }
                        return Ok(false);
                    }
                    crossterm::event::KeyCode::Backspace => {
                        if search.cursor > 0 {
                            search.cursor -= 1;
                            let byte_pos = search
                                .query
                                .char_indices()
                                .nth(search.cursor)
                                .map(|(i, _)| i)
                                .unwrap_or(search.query.len());
                            search.query.remove(byte_pos);
                        }
                        return Ok(false);
                    }
                    _ => {}
                }
            }
        }

        // Handle global keys
        if event::is_ctrl_c(event) || event::is_key(event, 'q') {
            return Ok(true); // Quit
        }

        if event::is_key(event, '?') {
            self.show_help = true;
            return Ok(false);
        }

        // Navigation
        if event::is_up(event) || event::is_key(event, 'k') {
            self.select_prev();
        } else if event::is_down(event) || event::is_key(event, 'j') {
            self.select_next();
        } else if event::is_page_up(event) {
            for _ in 0..self.last_visible_height {
                self.select_prev();
            }
        } else if event::is_page_down(event) {
            for _ in 0..self.last_visible_height {
                self.select_next();
            }
        }

        // Actions
        if event::is_key(event, 'a') {
            // Add download
            self.dialog = Some(DialogState::AddUrl {
                input: String::new(),
                cursor: 0,
            });
        } else if event::is_key(event, 'p') {
            // Pause selected
            self.pause_selected().await?;
        } else if event::is_key(event, 'r') {
            // Resume selected
            self.resume_selected().await?;
        } else if event::is_key(event, 'c') || event::is_key(event, 'd') {
            // Cancel selected (with confirmation)
            if let Some(dl) = self.selected_download() {
                self.dialog = Some(DialogState::ConfirmCancel {
                    id: dl.id,
                    delete_files: event::is_key(event, 'd'),
                });
            }
        }

        // View mode
        if event::is_key(event, '1') {
            self.mode = ViewMode::All;
            self.refresh_downloads();
        } else if event::is_key(event, '2') {
            self.mode = ViewMode::Active;
            self.refresh_downloads();
        } else if event::is_key(event, '3') {
            self.mode = ViewMode::Completed;
            self.refresh_downloads();
        }

        // Tab cycles right panel focus
        if event::is_tab(event) {
            self.right_panel_focus = match self.right_panel_focus {
                RightPanelFocus::Graph => RightPanelFocus::Details,
                RightPanelFocus::Details => RightPanelFocus::ChunkMap,
                RightPanelFocus::ChunkMap => RightPanelFocus::Graph,
            };
        }

        // Toggle activity log
        if event::is_upper_key(event, 'L') {
            self.show_activity_log = !self.show_activity_log;
        }

        // Search
        if event::is_key(event, '/') {
            self.search = Some(SearchState::default());
        }

        // Settings (Shift+S)
        if event::is_upper_key(event, 'S') {
            self.dialog = Some(DialogState::Settings {
                active_tab: 0,
                selected_row: 0,
                editing: None,
                draft: Box::new(self.config.clone()),
                dirty: false,
            });
        }

        // Batch import (Shift+A)
        if event::is_upper_key(event, 'A') {
            self.dialog = Some(DialogState::BatchImport {
                phase: BatchPhase::Input {
                    text: String::new(),
                    cursor_line: 0,
                    cursor_col: 0,
                },
            });
        }

        // Queue reordering (Shift+J / Shift+K)
        if event::is_upper_key(event, 'J') {
            self.reorder_download(1);
        } else if event::is_upper_key(event, 'K') {
            self.reorder_download(-1);
        }

        Ok(false)
    }

    /// Handle engine events
    fn handle_engine_event(&mut self, event: DownloadEvent) {
        match event {
            DownloadEvent::Added { .. } | DownloadEvent::Removed { .. } => {
                self.push_activity(ActivityLevel::Info, "Download added".to_string());
                self.refresh_downloads();
            }
            DownloadEvent::Completed { id } => {
                let name = self
                    .downloads
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| d.metadata.name.clone());
                self.refresh_downloads();
                if let Some(ref name) = name {
                    self.push_toast(truncate_str(name, 40), ToastLevel::Success);
                    self.push_activity(
                        ActivityLevel::Success,
                        format!("Completed: {}", truncate_str(name, 50)),
                    );
                }
            }
            DownloadEvent::Failed { error, .. } => {
                self.refresh_downloads();
                self.push_toast(truncate_str(&error, 40), ToastLevel::Error);
                self.push_activity(
                    ActivityLevel::Error,
                    format!("Failed: {}", truncate_str(&error, 50)),
                );
            }
            DownloadEvent::Progress { id, progress } => {
                if let Some(dl) = self.downloads.iter_mut().find(|d| d.id == id) {
                    dl.progress = progress;
                }
            }
            DownloadEvent::StateChanged { id, new_state, .. } => {
                if let Some(dl) = self.downloads.iter_mut().find(|d| d.id == id) {
                    dl.state = new_state;
                }
            }
            DownloadEvent::Paused { id } => {
                let name = self
                    .downloads
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| d.metadata.name.clone());
                if let Some(dl) = self.downloads.iter_mut().find(|d| d.id == id) {
                    dl.state = DownloadState::Paused;
                }
                if let Some(name) = name {
                    self.push_activity(
                        ActivityLevel::Warning,
                        format!("Paused: {}", truncate_str(&name, 50)),
                    );
                }
            }
            DownloadEvent::Resumed { .. } => {
                // Don't hardcode state; let StateChanged events update it
                // (engine sends Connecting first, then Downloading)
                self.push_activity(ActivityLevel::Info, "Resumed".to_string());
                self.refresh_downloads();
            }
            _ => {}
        }
    }

    /// Update global stats
    fn update_stats(&mut self) {
        let stats = self.engine.global_stats();
        self.download_speed = stats.download_speed;
        self.upload_speed = stats.upload_speed;

        // Track peak speeds
        self.peak_download_speed = self.peak_download_speed.max(stats.download_speed);
        self.peak_upload_speed = self.peak_upload_speed.max(stats.upload_speed);

        // Update speed history
        self.speed_history
            .push_back((stats.download_speed, stats.upload_speed));
        while self.speed_history.len() > 60 {
            self.speed_history.pop_front();
        }

        // Update chunk states for selected download
        self.compute_chunk_states();

        // Advance throbber animation
        self.throbber_state.calc_next();

        // Expire old toasts (4 second lifetime)
        self.toasts
            .retain(|t| t.created.elapsed() < Duration::from_secs(4));
    }

    /// Push a toast notification
    fn push_toast(&mut self, message: String, level: ToastLevel) {
        self.toasts.push(Toast {
            message,
            level,
            created: Instant::now(),
        });
        // Keep at most 5 toasts
        while self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    pub fn compute_chunk_states(&mut self) {
        if let Some(dl) = self.selected_download() {
            let total = dl.progress.total_size.unwrap_or(0);
            if total == 0 {
                self.chunk_states.clear();
                self.chunk_count = 0;
                return;
            }

            let count = if let Some(ref ti) = dl.torrent_info {
                ti.pieces_count.min(256)
            } else {
                let seg_size = 1024 * 1024_u64;
                ((total / seg_size) as usize).clamp(1, 256)
            };

            let progress_ratio = dl.progress.completed_size as f64 / total as f64;
            let completed_chunks = (count as f64 * progress_ratio) as usize;

            let is_active = matches!(
                dl.state,
                DownloadState::Downloading | DownloadState::Connecting
            );

            self.chunk_count = count;
            self.chunk_states = (0..count)
                .map(|i| {
                    if i < completed_chunks {
                        ChunkState::Complete
                    } else if i < completed_chunks + 3 && is_active {
                        ChunkState::Downloading
                    } else {
                        ChunkState::Pending
                    }
                })
                .collect();
        } else {
            self.chunk_states.clear();
            self.chunk_count = 0;
        }
    }

    pub fn push_activity(&mut self, level: ActivityLevel, message: String) {
        self.activity_log.push_back(ActivityEntry {
            timestamp: Instant::now(),
            level,
            message,
        });
        while self.activity_log.len() > 500 {
            self.activity_log.pop_front();
        }
    }

    /// Refresh download list from engine
    fn refresh_downloads(&mut self) {
        self.downloads = match self.mode {
            ViewMode::All => self.engine.list(),
            ViewMode::Active => self.engine.active(),
            ViewMode::Completed => self
                .engine
                .stopped()
                .into_iter()
                .filter(|d| matches!(d.state, DownloadState::Completed))
                .collect(),
        };

        // Adjust selection if needed
        if self.selected >= self.downloads.len() && !self.downloads.is_empty() {
            self.selected = self.downloads.len() - 1;
        }
    }

    /// Get currently selected download
    pub fn selected_download(&self) -> Option<&DownloadStatus> {
        self.downloads.get(self.selected)
    }

    /// Adjust scroll offset to keep selected item visible
    pub fn adjust_scroll(&mut self, visible_height: usize) {
        let total = self.downloads.len();
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected - visible_height + 1;
        }
        if total <= visible_height {
            self.scroll_offset = 0;
        } else if self.scroll_offset > total - visible_height {
            self.scroll_offset = total - visible_height;
        }
    }

    /// Select next item
    fn select_next(&mut self) {
        if !self.downloads.is_empty() {
            self.selected = (self.selected + 1).min(self.downloads.len() - 1);
            self.adjust_scroll(self.last_visible_height);
        }
    }

    /// Select previous item
    fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.adjust_scroll(self.last_visible_height);
        }
    }

    /// Add a new download
    async fn add_download(&mut self, url: &str) -> Result<()> {
        use crate::input::url_parser::{parse_input, ParsedInput};

        let input = parse_input(url)?;
        let options = gosh_dl::DownloadOptions::default();

        let result = match input {
            ParsedInput::Http(url) => self.engine.add_http(&url, options).await,
            ParsedInput::Magnet(uri) => self.engine.add_magnet(&uri, options).await,
            ParsedInput::TorrentFile(path) => {
                let data = tokio::fs::read(&path).await?;
                self.engine.add_torrent(&data, options).await
            }
        };

        if let Err(e) = result {
            self.dialog = Some(DialogState::Error {
                message: e.to_string(),
            });
        }

        Ok(())
    }

    /// Pause selected download
    async fn pause_selected(&mut self) -> Result<()> {
        if let Some(dl) = self.selected_download() {
            let id = dl.id;
            if let Err(e) = self.engine.pause(id).await {
                self.dialog = Some(DialogState::Error {
                    message: e.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Resume selected download
    async fn resume_selected(&mut self) -> Result<()> {
        if let Some(dl) = self.selected_download() {
            let id = dl.id;
            if let Err(e) = self.engine.resume(id).await {
                self.dialog = Some(DialogState::Error {
                    message: e.to_string(),
                });
            }
        }
        Ok(())
    }

    // Settings helper: check if a row in a tab is a boolean setting
    pub fn is_settings_bool(tab: usize, row: usize) -> bool {
        match tab {
            1 => row == 10,            // accept_invalid_certs
            2 => matches!(row, 0..=4), // enable_dht, enable_pex, enable_lpd, max_peers is not bool but seed_ratio is not
            3 => matches!(row, 2 | 3), // show_speed_graph, show_peers
            _ => false,
        }
    }

    // Settings helper: toggle a boolean setting
    pub fn toggle_settings_bool(draft: &mut CliConfig, tab: usize, row: usize) {
        match tab {
            1 => {
                if row == 10 {
                    draft.engine.accept_invalid_certs = !draft.engine.accept_invalid_certs;
                }
            }
            2 => match row {
                0 => draft.engine.enable_dht = !draft.engine.enable_dht,
                1 => draft.engine.enable_pex = !draft.engine.enable_pex,
                2 => draft.engine.enable_lpd = !draft.engine.enable_lpd,
                _ => {}
            },
            3 => match row {
                2 => draft.tui.show_speed_graph = !draft.tui.show_speed_graph,
                3 => draft.tui.show_peers = !draft.tui.show_peers,
                _ => {}
            },
            _ => {}
        }
    }

    // Settings helper: get current value as string
    pub fn get_settings_value(draft: &CliConfig, tab: usize, row: usize) -> String {
        match tab {
            0 => match row {
                0 => draft.general.download_dir.display().to_string(),
                1 => draft.general.database_path.display().to_string(),
                2 => draft.general.log_level.clone(),
                _ => String::new(),
            },
            1 => match row {
                0 => draft.engine.max_concurrent_downloads.to_string(),
                1 => draft.engine.max_connections_per_download.to_string(),
                2 => draft.engine.min_segment_size.to_string(),
                3 => draft
                    .engine
                    .global_download_limit
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                4 => draft
                    .engine
                    .global_upload_limit
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                5 => draft.engine.user_agent.clone(),
                6 => draft.engine.proxy_url.clone().unwrap_or_default(),
                7 => draft.engine.connect_timeout.to_string(),
                8 => draft.engine.read_timeout.to_string(),
                9 => draft.engine.max_retries.to_string(),
                10 => {
                    if draft.engine.accept_invalid_certs {
                        "ON".to_string()
                    } else {
                        "OFF".to_string()
                    }
                }
                _ => String::new(),
            },
            2 => match row {
                0 => {
                    if draft.engine.enable_dht {
                        "ON".to_string()
                    } else {
                        "OFF".to_string()
                    }
                }
                1 => {
                    if draft.engine.enable_pex {
                        "ON".to_string()
                    } else {
                        "OFF".to_string()
                    }
                }
                2 => {
                    if draft.engine.enable_lpd {
                        "ON".to_string()
                    } else {
                        "OFF".to_string()
                    }
                }
                3 => draft.engine.max_peers.to_string(),
                4 => format!("{:.1}", draft.engine.seed_ratio),
                _ => String::new(),
            },
            3 => match row {
                0 => draft.tui.refresh_rate_ms.to_string(),
                1 => draft.tui.theme.clone(),
                2 => {
                    if draft.tui.show_speed_graph {
                        "ON".to_string()
                    } else {
                        "OFF".to_string()
                    }
                }
                3 => {
                    if draft.tui.show_peers {
                        "ON".to_string()
                    } else {
                        "OFF".to_string()
                    }
                }
                _ => String::new(),
            },
            _ => String::new(),
        }
    }

    // Settings helper: get label for a row
    pub fn get_settings_label(tab: usize, row: usize) -> &'static str {
        match tab {
            0 => match row {
                0 => "Download Directory",
                1 => "Database Path",
                2 => "Log Level",
                _ => "",
            },
            1 => match row {
                0 => "Max Concurrent Downloads",
                1 => "Max Connections/Download",
                2 => "Min Segment Size",
                3 => "Global Download Limit",
                4 => "Global Upload Limit",
                5 => "User Agent",
                6 => "Proxy URL",
                7 => "Connect Timeout (sec)",
                8 => "Read Timeout (sec)",
                9 => "Max Retries",
                10 => "Accept Invalid Certs",
                _ => "",
            },
            2 => match row {
                0 => "Enable DHT",
                1 => "Enable PEX",
                2 => "Enable LPD",
                3 => "Max Peers",
                4 => "Seed Ratio",
                _ => "",
            },
            3 => match row {
                0 => "Refresh Rate (ms)",
                1 => "Theme",
                2 => "Show Speed Graph",
                3 => "Show Peers",
                _ => "",
            },
            4 => "Schedule Rules (read-only)",
            _ => "",
        }
    }

    // Settings helper: how many rows per tab
    pub fn settings_row_count(tab: usize) -> usize {
        match tab {
            0 => 3,
            1 => 11,
            2 => 5,
            3 => 4,
            4 => 1,
            _ => 0,
        }
    }

    // Settings helper: tab names
    pub fn settings_tab_names() -> &'static [&'static str] {
        &["General", "Network", "BitTorrent", "Interface", "Schedule"]
    }

    // Settings helper: apply edit value to draft config
    fn apply_settings_edit(draft: &mut CliConfig, tab: usize, row: usize, val: &str) {
        match tab {
            0 => match row {
                0 => draft.general.download_dir = std::path::PathBuf::from(val),
                1 => draft.general.database_path = std::path::PathBuf::from(val),
                2 => draft.general.log_level = val.to_string(),
                _ => {}
            },
            1 => match row {
                0 => {
                    if let Ok(v) = val.parse() {
                        draft.engine.max_concurrent_downloads = v;
                    }
                }
                1 => {
                    if let Ok(v) = val.parse() {
                        draft.engine.max_connections_per_download = v;
                    }
                }
                2 => {
                    if let Ok(v) = val.parse() {
                        draft.engine.min_segment_size = v;
                    }
                }
                3 => {
                    draft.engine.global_download_limit = val.parse().ok().filter(|&v: &u64| v > 0);
                }
                4 => {
                    draft.engine.global_upload_limit = val.parse().ok().filter(|&v: &u64| v > 0);
                }
                5 => {
                    draft.engine.user_agent = val.to_string();
                }
                6 => {
                    draft.engine.proxy_url = if val.is_empty() {
                        None
                    } else {
                        Some(val.to_string())
                    };
                }
                7 => {
                    if let Ok(v) = val.parse() {
                        draft.engine.connect_timeout = v;
                    }
                }
                8 => {
                    if let Ok(v) = val.parse() {
                        draft.engine.read_timeout = v;
                    }
                }
                9 => {
                    if let Ok(v) = val.parse() {
                        draft.engine.max_retries = v;
                    }
                }
                _ => {}
            },
            2 => match row {
                3 => {
                    if let Ok(v) = val.parse() {
                        draft.engine.max_peers = v;
                    }
                }
                4 => {
                    if let Ok(v) = val.parse() {
                        draft.engine.seed_ratio = v;
                    }
                }
                _ => {}
            },
            3 => match row {
                0 => {
                    if let Ok(v) = val.parse() {
                        draft.tui.refresh_rate_ms = v;
                    }
                }
                1 => {
                    draft.tui.theme = val.to_string();
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// Setup terminal for TUI
fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal to normal mode
fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
