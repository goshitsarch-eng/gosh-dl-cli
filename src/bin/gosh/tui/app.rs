use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use gosh_dl::DownloadEngine;
use gosh_dl::{
    BatchResult, DownloadEvent, DownloadState, DownloadStatus, RecursiveJobEvent,
    RecursiveJobState, RecursiveJobStatus,
};
use ratatui::prelude::*;
use std::collections::{HashMap, VecDeque};
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
    config_path: Option<std::path::PathBuf>,
    integrity_task: Option<tokio::task::JoinHandle<(bool, gosh_dl::Result<gosh_dl::VerifyReport>)>>,

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

    /// Active recursive mirror jobs, keyed by tracked job ID
    pub recursive_jobs: HashMap<uuid::Uuid, RecursiveJobStatus>,

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
    ConfirmCancelAll,
    ConfirmRepair {
        id: gosh_dl::DownloadId,
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
    /// True while the search bar is capturing keystrokes; false once the
    /// filter is committed with Enter (filter stays active, keys fall through)
    pub input_active: bool,
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
            input_active: true,
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
    Info,
    Success,
    Error,
}

impl TuiApp {
    pub async fn new(config: CliConfig, config_path: Option<std::path::PathBuf>) -> Result<Self> {
        let engine = crate::app::create_engine(&config).await?;

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
            config_path,
            integrity_task: None,
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
            recursive_jobs: HashMap::new(),
            should_quit: false,
        })
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn show_speed_graph(&self) -> bool {
        self.config.tui.show_speed_graph
    }

    pub fn show_peers(&self) -> bool {
        self.config.tui.show_peers
    }

    fn save_settings(&mut self, config: CliConfig) -> Result<()> {
        config.validate()?;
        std::fs::create_dir_all(&config.general.download_dir)?;
        let engine_config = config.to_engine_config();
        engine_config.validate()?;
        config.save(self.config_path.as_deref())?;
        self.engine.set_config(engine_config)?;
        self.theme = Theme::from_name(&config.tui.theme);
        self.config = config;
        Ok(())
    }

    fn start_integrity(&mut self, id: gosh_dl::DownloadId, repair: bool) {
        if self.integrity_task.is_some() {
            self.push_toast(
                "An integrity check is already running".into(),
                ToastLevel::Info,
            );
            return;
        }
        let engine = self.engine.clone();
        self.integrity_task = Some(tokio::spawn(async move {
            let result = if repair {
                engine.repair(id).await
            } else {
                engine.verify(id).await
            };
            (repair, result)
        }));
        self.push_toast("Checking data on disk…".into(), ToastLevel::Info);
    }

    async fn finish_integrity(&mut self) {
        if !self
            .integrity_task
            .as_ref()
            .is_some_and(|task| task.is_finished())
        {
            return;
        }
        let result = self.integrity_task.take().unwrap().await;
        let (message, failed) = match result {
            Ok((repair, Ok(report))) => {
                let action = if repair && !report.valid {
                    "Repair queued; watch download progress. "
                } else if report.valid {
                    "Verification passed. "
                } else {
                    "Verification failed. Use Shift+V to repair. "
                };
                (
                    format!("{action}{}", report.detail),
                    !report.valid && !repair,
                )
            }
            Ok((_, Err(e))) => (e.to_string(), true),
            Err(e) => (format!("Integrity check failed: {e}"), true),
        };
        self.push_activity(
            if failed {
                ActivityLevel::Error
            } else {
                ActivityLevel::Info
            },
            message.clone(),
        );
        self.push_toast(
            message,
            if failed {
                ToastLevel::Error
            } else {
                ToastLevel::Success
            },
        );
        self.refresh_downloads();
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
        let result = self.run_terminal().await;
        // Let verification/repair finish before shutting its engine down.
        if let Some(task) = self.integrity_task.take() {
            let _ = task.await;
        }
        let shutdown = self.engine.shutdown().await;
        result?;
        shutdown?;
        Ok(())
    }

    async fn run_terminal(&mut self) -> Result<()> {
        let _guard = TerminalGuard::new();
        let mut terminal = setup_terminal()?;

        // Create event handler
        let tick_rate = Duration::from_millis(self.config.tui.refresh_rate_ms);
        let mut event_handler = EventHandler::new(
            self.engine.subscribe(),
            self.engine.subscribe_recursive_jobs(),
            tick_rate,
        );

        // Seed mirror job state from the engine
        for job in self.engine.list_recursive_jobs() {
            let status = self.engine.recursive_job_status(&job.as_job());
            self.recursive_jobs.insert(job.id, status);
        }

        // Initial frame
        terminal.draw(|frame| ui::render(frame, self))?;

        // Main loop. Engine events only mark state dirty; the actual redraw
        // happens on the next tick (or input event), so progress-event floods
        // cost at most one draw per refresh interval.
        loop {
            let mut draw = false;

            event_handler.set_tick_rate(Duration::from_millis(self.config.tui.refresh_rate_ms));
            match event_handler.next().await? {
                AppEvent::Terminal(event) => {
                    if self.handle_terminal_event(&event).await? {
                        break;
                    }
                    // Input latency matters: draw immediately
                    draw = true;
                }
                AppEvent::Engine(event) => {
                    self.handle_engine_event(event);
                }
                AppEvent::RecursiveJob(event) => {
                    self.handle_recursive_event(event);
                }
                AppEvent::Tick => {
                    self.finish_integrity().await;
                    self.refresh_downloads();
                    self.update_stats();
                    draw = true;
                }
                AppEvent::Resync => {
                    // Full resync after missed broadcast events
                    self.refresh_downloads();
                    self.finish_integrity().await;
                    self.refresh_downloads();
                    self.update_stats();
                    draw = true;
                }
                AppEvent::Resize(w, h) => {
                    self.terminal_width = w;
                    self.terminal_height = h;
                    self.detect_layout_mode();
                    draw = true;
                }
            }

            if self.should_quit {
                break;
            }

            if draw {
                terminal.draw(|frame| ui::render(frame, self))?;
            }
        }

        Ok(())
    }

    /// Handle terminal input events
    async fn handle_terminal_event(&mut self, event: &crossterm::event::Event) -> Result<bool> {
        // Ctrl+C works even while a dialog or search field has focus.
        if event::is_ctrl_c(event) {
            return Ok(true);
        }
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
                            crossterm::event::KeyCode::Backspace if *cursor > 0 => {
                                *cursor -= 1;
                                let byte_pos = input
                                    .char_indices()
                                    .nth(*cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(input.len());
                                input.remove(byte_pos);
                            }
                            crossterm::event::KeyCode::Left if *cursor > 0 => {
                                *cursor -= 1;
                            }
                            crossterm::event::KeyCode::Right if *cursor < input.chars().count() => {
                                *cursor += 1;
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
                DialogState::ConfirmCancelAll => {
                    if event::is_escape(event) || event::is_key(event, 'n') {
                        self.dialog = None;
                    } else if event::is_key(event, 'y') || event::is_enter(event) {
                        self.dialog = None;
                        let result = self.engine.cancel_all(false).await;
                        self.report_batch("Cancelled", &result);
                    }
                    return Ok(false);
                }
                DialogState::ConfirmRepair { id } => {
                    if event::is_escape(event) || event::is_key(event, 'n') {
                        self.dialog = None;
                    } else if event::is_key(event, 'y') || event::is_enter(event) {
                        let id = *id;
                        self.dialog = None;
                        self.start_integrity(id, true);
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
                                        match Self::apply_settings_edit(
                                            draft,
                                            *active_tab,
                                            *selected_row,
                                            &val,
                                        ) {
                                            Ok(()) => *dirty = true,
                                            Err(e) => {
                                                *editing = Some(val);
                                                self.push_toast(
                                                    format!("Invalid setting: {e}"),
                                                    ToastLevel::Error,
                                                );
                                            }
                                        }
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
                                        match self.save_settings(new_config) {
                                            Ok(()) => self.push_toast(
                                                "Settings saved; network/storage changes need restart".into(),
                                                ToastLevel::Success,
                                            ),
                                            Err(e) => {
                                                self.push_toast(format!("Settings not saved: {e}"), ToastLevel::Error);
                                                return Ok(false);
                                            }
                                        }
                                    }
                                    self.dialog = None;
                                }
                                crossterm::event::KeyCode::Left if *active_tab > 0 => {
                                    *active_tab -= 1;
                                    *selected_row = 0;
                                }
                                crossterm::event::KeyCode::Right if *active_tab < 4 => {
                                    *active_tab += 1;
                                    *selected_row = 0;
                                }
                                crossterm::event::KeyCode::Up
                                | crossterm::event::KeyCode::Char('k')
                                    if *selected_row > 0 =>
                                {
                                    *selected_row -= 1;
                                }
                                crossterm::event::KeyCode::Down
                                | crossterm::event::KeyCode::Char('j') => {
                                    let max_row =
                                        Self::settings_row_count(*active_tab).saturating_sub(1);
                                    *selected_row = (*selected_row + 1).min(max_row);
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
                                    } else if *active_tab != 4 {
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
                                crossterm::event::KeyCode::Enter
                                | crossterm::event::KeyCode::F(2) => {
                                    if key.modifiers == crossterm::event::KeyModifiers::CONTROL
                                        || key.code == crossterm::event::KeyCode::F(2)
                                    {
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
                                | crossterm::event::KeyCode::Char('k')
                                    if *selected > 0 =>
                                {
                                    *selected -= 1;
                                }
                                crossterm::event::KeyCode::Down
                                | crossterm::event::KeyCode::Char('j')
                                    if *selected + 1 < entries.len() =>
                                {
                                    *selected += 1;
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
                                    let total = urls.len();
                                    let mut count = 0;
                                    for url in urls {
                                        if self.add_download(&url).await? {
                                            count += 1;
                                        }
                                    }
                                    self.push_toast(
                                        format!("Added {count}/{total} downloads"),
                                        if count == total {
                                            ToastLevel::Success
                                        } else {
                                            ToastLevel::Error
                                        },
                                    );
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

        // Handle search input mode (only while the bar is capturing keys —
        // a committed filter releases the keyboard back to normal bindings)
        if self.search.as_ref().is_some_and(|s| s.input_active) {
            if let crossterm::event::Event::Key(key) = event {
                let mut changed = false;
                let mut close = false;
                let mut handled = true;
                if let Some(ref mut search) = self.search {
                    match key.code {
                        crossterm::event::KeyCode::Esc => {
                            close = true;
                            changed = true;
                        }
                        crossterm::event::KeyCode::Enter => {
                            if search.query.is_empty() {
                                close = true;
                            } else {
                                // Commit: filter stays active, keys fall through
                                search.input_active = false;
                            }
                        }
                        crossterm::event::KeyCode::Char(c) => {
                            if key.modifiers == crossterm::event::KeyModifiers::CONTROL && c == 's'
                            {
                                search.scope = search.scope.next();
                                changed = true;
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
                                changed = true;
                            } else {
                                handled = false;
                            }
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
                                changed = true;
                            }
                        }
                        _ => handled = false,
                    }
                }
                if close {
                    self.search = None;
                }
                if changed {
                    self.refresh_downloads();
                }
                if handled {
                    return Ok(false);
                }
            }
        }

        // Handle global keys
        if event::is_ctrl_c(event) || event::is_key(event, 'q') {
            return Ok(true); // Quit
        }

        // Esc clears a committed search filter
        if event::is_escape(event) && self.search.is_some() {
            self.search = None;
            self.refresh_downloads();
            return Ok(false);
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
        } else if event::is_key(event, 'v') {
            if let Some(dl) = self.selected_download() {
                self.start_integrity(dl.id, false);
            }
        } else if event::is_upper_key(event, 'V') {
            if let Some(dl) = self.selected_download() {
                self.dialog = Some(DialogState::ConfirmRepair { id: dl.id });
            }
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

        // Batch operations
        if event::is_upper_key(event, 'P') {
            let result = self.engine.pause_all().await;
            self.report_batch("Paused", &result);
        } else if event::is_upper_key(event, 'R') {
            let result = self.engine.resume_all().await;
            self.report_batch("Resumed", &result);
        } else if event::is_upper_key(event, 'C') && !self.downloads.is_empty() {
            self.dialog = Some(DialogState::ConfirmCancelAll);
        }

        // Toggle activity log
        if event::is_upper_key(event, 'L') {
            self.show_activity_log = !self.show_activity_log;
            self.activity_log_scroll = 0;
        }

        // Scroll activity log ([ = older, ] = newer)
        if event::is_key(event, '[') {
            if self.show_activity_log {
                self.activity_log_scroll =
                    (self.activity_log_scroll + 1).min(self.activity_log.len().saturating_sub(1));
            }
        } else if event::is_key(event, ']') {
            self.activity_log_scroll = self.activity_log_scroll.saturating_sub(1);
        }

        // Search: open the bar, or re-edit a committed filter
        if event::is_key(event, '/') {
            match self.search.as_mut() {
                Some(s) => s.input_active = true,
                None => self.search = Some(SearchState::default()),
            }
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
            DownloadEvent::Added { .. } => {
                self.push_activity(ActivityLevel::Info, "Download added".to_string());
                self.refresh_downloads();
            }
            DownloadEvent::Removed { .. } => {
                self.push_activity(ActivityLevel::Info, "Download removed".to_string());
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

    /// Handle recursive mirror job events
    fn handle_recursive_event(&mut self, event: RecursiveJobEvent) {
        match event {
            RecursiveJobEvent::Added { job, status } => {
                self.push_activity(
                    ActivityLevel::Info,
                    format!("Mirror started: {}", truncate_str(&job.root_url, 50)),
                );
                self.recursive_jobs.insert(job.id, status);
            }
            RecursiveJobEvent::Updated { job, status } => {
                let prev_state = self.recursive_jobs.get(&job.id).map(|s| s.state);
                if prev_state != Some(status.state) {
                    match status.state {
                        RecursiveJobState::Completed => {
                            self.push_toast(
                                format!(
                                    "Mirror complete ({} files)",
                                    status.progress.total_children
                                ),
                                ToastLevel::Success,
                            );
                            self.push_activity(
                                ActivityLevel::Success,
                                format!("Mirror complete: {}", truncate_str(&job.root_url, 50)),
                            );
                        }
                        RecursiveJobState::Failed => {
                            self.push_activity(
                                ActivityLevel::Error,
                                format!("Mirror failed: {}", truncate_str(&job.root_url, 50)),
                            );
                        }
                        _ => {}
                    }
                }
                self.recursive_jobs.insert(job.id, status);
            }
            RecursiveJobEvent::Removed { id } => {
                self.recursive_jobs.remove(&id);
            }
        }
    }

    /// Report a batch-operation outcome as a toast + activity entry
    fn report_batch(&mut self, verb: &str, result: &BatchResult) {
        if result.succeeded.is_empty() && result.skipped.is_empty() && result.failed.is_empty() {
            self.push_toast("No downloads to act on".to_string(), ToastLevel::Success);
            return;
        }
        let mut msg = format!("{} {}", verb, result.succeeded.len());
        if !result.skipped.is_empty() {
            msg.push_str(&format!(", skipped {}", result.skipped.len()));
        }
        if !result.failed.is_empty() {
            msg.push_str(&format!(", failed {}", result.failed.len()));
        }
        let level = if result.failed.is_empty() {
            ToastLevel::Success
        } else {
            ToastLevel::Error
        };
        self.push_toast(msg.clone(), level);
        let activity_level = if result.failed.is_empty() {
            ActivityLevel::Info
        } else {
            ActivityLevel::Error
        };
        self.push_activity(activity_level, msg);
        self.refresh_downloads();
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
        // Preserve the selected download across the rebuild
        let keep = self.selected_download().map(|d| d.id);

        let order: HashMap<_, _> = self
            .downloads
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id, i))
            .collect();
        self.downloads = match self.mode {
            ViewMode::All => self.engine.list(),
            ViewMode::Active => self.engine.active(),
            ViewMode::Completed => self
                .engine
                .list()
                .into_iter()
                .filter(|d| matches!(d.state, DownloadState::Completed | DownloadState::Seeding))
                .collect(),
        };

        self.downloads
            .sort_by_key(|d| order.get(&d.id).copied().unwrap_or(usize::MAX));

        // Apply the search filter
        if let Some(ref search) = self.search {
            if !search.query.is_empty() {
                let query = search.query.to_lowercase();
                let scope = search.scope;
                self.downloads
                    .retain(|d| Self::matches_search(d, &query, scope));
            }
        }

        // Re-find the previously selected download; fall back to clamping
        self.selected = keep
            .and_then(|id| self.downloads.iter().position(|d| d.id == id))
            .unwrap_or_else(|| self.selected.min(self.downloads.len().saturating_sub(1)));
        self.adjust_scroll(self.last_visible_height);
    }

    /// Check whether a download matches the search query within a scope
    fn matches_search(d: &DownloadStatus, query: &str, scope: SearchScope) -> bool {
        let name_match = || d.metadata.name.to_lowercase().contains(query);
        let url_match = || {
            d.metadata
                .url
                .as_deref()
                .is_some_and(|u| u.to_lowercase().contains(query))
                || d.metadata
                    .magnet_uri
                    .as_deref()
                    .is_some_and(|u| u.to_lowercase().contains(query))
        };
        let state_match = || {
            crate::format::format_state(&d.state)
                .to_lowercase()
                .contains(query)
        };
        match scope {
            SearchScope::Name => name_match(),
            SearchScope::Url => url_match(),
            SearchScope::State => state_match(),
            SearchScope::All => name_match() || url_match() || state_match(),
        }
    }

    /// Get currently selected download
    pub fn selected_download(&self) -> Option<&DownloadStatus> {
        self.downloads.get(self.selected)
    }

    /// Adjust scroll offset to keep selected item visible
    pub fn adjust_scroll(&mut self, visible_height: usize) {
        let visible_height = visible_height.max(1);
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
    async fn add_download(&mut self, url: &str) -> Result<bool> {
        use crate::input::url_parser::{parse_input, ParsedInput};

        let result: Result<_> = async {
            let input = parse_input(url.trim())?;
            let options = gosh_dl::DownloadOptions::default();

            let result = match input {
                ParsedInput::Http(url) => self.engine.add_http(&url, options).await,
                ParsedInput::Magnet(uri) => self.engine.add_magnet(&uri, options).await,
                ParsedInput::TorrentFile(path) => {
                    let data = tokio::fs::read(&path).await?;
                    self.engine.add_torrent(&data, options).await
                }
            };

            Ok(result?)
        }
        .await;
        match result {
            Ok(_) => Ok(true),
            Err(e) => {
                self.push_activity(ActivityLevel::Error, format!("Could not add download: {e}"));
                self.dialog = Some(DialogState::Error {
                    message: e.to_string(),
                });
                Ok(false)
            }
        }
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
            2 => matches!(row, 0..=2), // enable_dht, enable_pex, enable_lpd (max_peers/seed_ratio are editable values)
            3 => matches!(row, 2 | 3), // show_speed_graph, show_peers
            _ => false,
        }
    }

    // Settings helper: toggle a boolean setting
    pub fn toggle_settings_bool(draft: &mut CliConfig, tab: usize, row: usize) {
        match tab {
            1 if row == 10 => {
                draft.engine.accept_invalid_certs = !draft.engine.accept_invalid_certs;
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
                2 => "Log Level (restart)",
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
                9 => "Max HTTP Attempts",
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
    fn apply_settings_edit(draft: &mut CliConfig, tab: usize, row: usize, val: &str) -> Result<()> {
        match tab {
            0 => match row {
                0 => draft.general.download_dir = std::path::PathBuf::from(val),
                1 => draft.general.database_path = std::path::PathBuf::from(val),
                2 => draft.general.log_level = val.to_string(),
                _ => {}
            },
            1 => match row {
                0 => {
                    draft.engine.max_concurrent_downloads = val.parse()?;
                }
                1 => {
                    draft.engine.max_connections_per_download = val.parse()?;
                }
                2 => {
                    draft.engine.min_segment_size = val.parse()?;
                }
                3 => {
                    draft.engine.global_download_limit =
                        Some(val.parse::<u64>()?).filter(|&v| v > 0);
                }
                4 => {
                    draft.engine.global_upload_limit = Some(val.parse::<u64>()?).filter(|&v| v > 0);
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
                    draft.engine.connect_timeout = val.parse()?;
                }
                8 => {
                    draft.engine.read_timeout = val.parse()?;
                }
                9 => {
                    draft.engine.max_retries = val.parse()?;
                }
                _ => {}
            },
            2 => match row {
                3 => {
                    draft.engine.max_peers = val.parse()?;
                }
                4 => {
                    draft.engine.seed_ratio = val.parse()?;
                }
                _ => {}
            },
            3 => match row {
                0 => {
                    draft.tui.refresh_rate_ms = val.parse()?;
                }
                1 => {
                    draft.tui.theme = val.to_string();
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
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

/// Restores the terminal on every return path, including I/O errors.
type PanicHook = std::sync::Arc<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;

struct TerminalGuard {
    original_hook: PanicHook,
}

impl TerminalGuard {
    fn new() -> Self {
        let original_hook: PanicHook = std::panic::take_hook().into();
        let hook = original_hook.clone();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            hook(info);
        }));
        Self { original_hook }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
        if !std::thread::panicking() {
            let hook = self.original_hook.clone();
            std::panic::set_hook(Box::new(move |info| hook(info)));
        }
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use tempfile::TempDir;

    async fn app() -> (TuiApp, TempDir) {
        let dir = TempDir::new().unwrap();
        let mut config = CliConfig::default();
        config.general.storage_backend = crate::config::StorageBackend::None;
        config.general.download_dir = dir.path().join("downloads");
        config.engine.enable_dht = false;
        config.engine.enable_lpd = false;
        config.engine.max_retries = 1;
        let app = TuiApp::new(config, Some(dir.path().join("custom.toml")))
            .await
            .unwrap();
        (app, dir)
    }

    async fn key(app: &mut TuiApp, code: KeyCode) -> bool {
        app.handle_terminal_event(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
            .await
            .unwrap()
    }

    fn render(app: &mut TuiApp, width: u16, height: u16) -> String {
        app.terminal_width = width;
        app.terminal_height = height;
        app.detect_layout_mode();
        app.startup_effects_added = true;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| ui::render(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[tokio::test]
    async fn invalid_add_input_stays_in_tui_and_ctrl_c_always_quits() {
        let (mut app, _dir) = app().await;
        key(&mut app, KeyCode::Char('a')).await;
        for c in "this is not a URL".chars() {
            key(&mut app, KeyCode::Char(c)).await;
        }
        assert!(!key(&mut app, KeyCode::Enter).await);
        assert!(matches!(app.dialog, Some(DialogState::Error { .. })));
        assert!(app
            .handle_terminal_event(&Event::Key(KeyEvent::new(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL
            )))
            .await
            .unwrap());
        app.engine.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn unicode_add_editing_and_dialog_at_80x24() {
        let (mut app, _dir) = app().await;
        key(&mut app, KeyCode::Char('a')).await;
        for c in "雪éx".chars() {
            key(&mut app, KeyCode::Char(c)).await;
        }
        key(&mut app, KeyCode::Left).await;
        key(&mut app, KeyCode::Backspace).await;
        assert!(
            matches!(&app.dialog, Some(DialogState::AddUrl { input, cursor }) if input == "雪x" && *cursor == 1)
        );
        let screen = render(&mut app, 80, 24);
        assert!(screen.contains("Enter URL"));
        assert!(screen.contains("Cancel"));
        key(&mut app, KeyCode::Esc).await;
        key(&mut app, KeyCode::Char('?')).await;
        let help = render(&mut app, 80, 24);
        assert!(help.contains("verify and repair"));
        assert!(help.contains("Press any key to close"));
        app.engine.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn settings_save_custom_path_and_keep_draft_on_failure() {
        let (mut app, dir) = app().await;
        key(&mut app, KeyCode::Char('S')).await;
        if let Some(DialogState::Settings { draft, dirty, .. }) = &mut app.dialog {
            draft.tui.theme = "light".into();
            *dirty = true;
        } else {
            panic!("uppercase shortcut did not open settings");
        }
        key(&mut app, KeyCode::Esc).await;
        assert!(dir.path().join("custom.toml").exists());
        assert_eq!(
            CliConfig::load(app.config_path.as_deref())
                .unwrap()
                .tui
                .theme,
            "light"
        );
        app.config_path = Some(dir.path().to_path_buf()); // writing a directory must fail
        key(&mut app, KeyCode::Char('S')).await;
        if let Some(DialogState::Settings { draft, dirty, .. }) = &mut app.dialog {
            draft.tui.theme = "dark".into();
            *dirty = true;
        }
        key(&mut app, KeyCode::Esc).await;
        assert!(matches!(
            app.dialog,
            Some(DialogState::Settings { dirty: true, .. })
        ));
        assert_eq!(app.config.tui.theme, "light");
        assert!(app.toasts.last().unwrap().message.contains("not saved"));
        app.engine.shutdown().await.unwrap();
    }

    #[test]
    fn invalid_settings_edits_do_not_disable_limits() {
        let mut draft = CliConfig::default();
        draft.engine.global_download_limit = Some(1024);
        assert!(TuiApp::apply_settings_edit(&mut draft, 1, 3, "typo").is_err());
        assert_eq!(draft.engine.global_download_limit, Some(1024));
        assert!(TuiApp::apply_settings_edit(&mut draft, 1, 0, "typo").is_err());
    }

    #[tokio::test]
    async fn dialogs_render_at_small_and_normal_sizes_and_settings_scroll() {
        let (mut app, _dir) = app().await;
        for (width, height) in [(1, 1), (20, 6), (79, 19), (80, 24), (120, 40)] {
            for c in ['a', 'S', 'A', '?'] {
                app.dialog = None;
                app.show_help = false;
                key(&mut app, KeyCode::Char(c)).await;
                render(&mut app, width, height);
            }
        }
        app.show_help = false;
        app.dialog = Some(DialogState::Settings {
            active_tab: 1,
            selected_row: 10,
            editing: None,
            draft: Box::new(app.config.clone()),
            dirty: false,
        });
        assert!(render(&mut app, 80, 24).contains("Accept Invalid Certs"));
        app.engine.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn batch_review_is_portable_and_does_not_report_failed_imports_as_success() {
        let (mut app, dir) = app().await;
        let torrent = dir.path().join("broken.torrent");
        std::fs::write(&torrent, "not valid torrent metadata").unwrap();
        key(&mut app, KeyCode::Char('A')).await;
        if let Some(DialogState::BatchImport {
            phase: BatchPhase::Input { text, .. },
        }) = &mut app.dialog
        {
            *text = torrent.display().to_string();
        }
        key(&mut app, KeyCode::F(2)).await;
        assert!(
            matches!(&app.dialog, Some(DialogState::BatchImport { phase: BatchPhase::Review { entries, .. } }) if entries.len() == 1)
        );
        key(&mut app, KeyCode::Enter).await;
        assert_eq!(app.toasts.last().unwrap().message, "Added 0/1 downloads");
        assert!(app.engine.list().is_empty());
        assert!(matches!(app.dialog, Some(DialogState::Error { .. })));
        app.engine.shutdown().await.unwrap();
    }

    async fn completed_http(
        app: &mut TuiApp,
    ) -> (gosh_dl::DownloadId, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/test.bin", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut buf = [0; 4096];
                    let n = stream.read(&mut buf).await.unwrap();
                    let head = buf[..n].starts_with(b"HEAD ");
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n",
                        )
                        .await
                        .unwrap();
                    if !head {
                        let _ = stream.write_all(b"test").await;
                    }
                });
            }
        });
        app.add_download(&url).await.unwrap();
        assert!(app.dialog.is_none());
        let id = app.engine.list()[0].id;
        tokio::time::timeout(Duration::from_secs(5), async {
            while !matches!(
                app.engine.status(id).unwrap().state,
                DownloadState::Completed
            ) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        app.refresh_downloads();
        (id, server)
    }

    async fn finish_check(app: &mut TuiApp) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !app.integrity_task.as_ref().unwrap().is_finished() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        app.finish_integrity().await;
    }

    #[tokio::test]
    async fn verify_repair_real_file_and_search_actions() {
        let (mut app, _dir) = app().await;
        let (id, server) = completed_http(&mut app).await;
        key(&mut app, KeyCode::Char('v')).await;
        finish_check(&mut app).await;
        assert!(app
            .activity_log
            .back()
            .unwrap()
            .message
            .contains("Verification passed"));
        let path = app.config.general.download_dir.join("test.bin");
        tokio::fs::write(&path, b"bad").await.unwrap();
        key(&mut app, KeyCode::Char('v')).await;
        finish_check(&mut app).await;
        assert!(app
            .activity_log
            .back()
            .unwrap()
            .message
            .contains("Verification failed"));
        key(&mut app, KeyCode::Char('V')).await;
        assert!(matches!(
            app.dialog,
            Some(DialogState::ConfirmRepair { .. })
        ));
        key(&mut app, KeyCode::Char('y')).await;
        finish_check(&mut app).await;
        assert!(app
            .activity_log
            .back()
            .unwrap()
            .message
            .contains("Repair queued"));
        tokio::time::timeout(Duration::from_secs(5), async {
            while !matches!(
                app.engine.status(id).unwrap().state,
                DownloadState::Completed
            ) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(tokio::fs::read(path).await.unwrap(), b"test");
        key(&mut app, KeyCode::Char('/')).await;
        for c in "test".chars() {
            key(&mut app, KeyCode::Char(c)).await;
        }
        key(&mut app, KeyCode::Enter).await;
        assert_eq!(app.selected_download().unwrap().id, id);
        key(&mut app, KeyCode::Char('d')).await;
        key(&mut app, KeyCode::Char('n')).await;
        assert!(app.engine.status(id).is_some());
        app.config.tui.show_speed_graph = false;
        app.config.tui.show_peers = false;
        app.toasts.clear();
        let screen = render(&mut app, 120, 40);
        assert!(!screen.contains("Peers:"));
        assert!(screen.contains("Priority:"));
        app.engine.shutdown().await.unwrap();
        server.abort();
    }
}
