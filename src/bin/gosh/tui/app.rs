use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use gosh_dl::{
    BatchResult, DownloadEngine, DownloadEvent, DownloadId, DownloadState, RecursiveJobEvent,
    RecursiveJobState, VerifyReport,
};
use ratatui::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::{Duration, Instant};
use throbber_widgets_tui::ThrobberState;

use crate::config::CliConfig;
use crate::input::url_parser::{parse_input, ParsedInput};
use crate::util::truncate_str;

use super::event::{self, AppEvent, EventHandler};
use super::model::{build_rows, Row, RowId, RowState, ViewMode};
use super::settings;
use super::theme::Theme;
use super::ui;

/// Samples kept for the network graph.
pub const SPEED_HISTORY: usize = 160;

type IntegrityTask = tokio::task::JoinHandle<(bool, DownloadId, gosh_dl::Result<VerifyReport>)>;
type MirrorTask = tokio::task::JoinHandle<(String, gosh_dl::Result<gosh_dl::RecursiveJob>)>;

/// TUI Application state
pub struct TuiApp {
    pub(super) engine: Arc<DownloadEngine>,
    pub(super) config: CliConfig,
    pub(super) config_path: Option<std::path::PathBuf>,
    pub(super) integrity_task: Option<IntegrityTask>,
    /// Mirror discoveries running in the background (crawling a directory
    /// tree can take a while and must not freeze the interface)
    pub(super) mirror_tasks: Vec<MirrorTask>,
    theme: Theme,
    /// Current view (tab)
    pub mode: ViewMode,
    /// Rows of the current view, after the search filter
    pub rows: Vec<Row>,
    /// Row counts per view, in `ViewMode::ALL` order
    pub counts: [usize; 4],
    /// Files finished / files in total, over every mirror job
    pub mirror_files: (usize, usize),
    /// Currently selected row index
    pub selected: usize,
    /// Scroll offset for the download list
    pub scroll_offset: usize,
    /// Rows the list could show at the last render
    pub last_visible_height: usize,
    /// Speed history for the graph (download, upload)
    pub speed_history: VecDeque<(u64, u64)>,
    /// Whether help overlay is shown
    pub show_help: bool,
    /// Active dialog (add URL, confirm cancel, etc.)
    pub dialog: Option<DialogState>,
    /// Last frame timestamp for effect timing
    pub last_frame: Instant,
    pub download_speed: u64,
    pub upload_speed: u64,
    pub peak_download_speed: u64,
    pub peak_upload_speed: u64,
    /// Throbber state for animated spinners
    pub throbber_state: ThrobberState,
    /// Active toast notifications
    pub toasts: Vec<Toast>,
    /// Effect manager for tachyonfx animations
    pub effect_manager: tachyonfx::EffectManager<()>,
    /// Whether startup effects have been queued
    pub startup_effects_added: bool,
    pub layout_mode: LayoutMode,
    pub terminal_width: u16,
    pub terminal_height: u16,
    /// Active search/filter state
    pub search: Option<SearchState>,
    /// Activity log entries
    pub activity_log: VecDeque<ActivityEntry>,
    /// Whether the activity panel is shown
    pub show_activity_log: bool,
    /// How many entries the activity panel is scrolled back
    pub activity_log_scroll: usize,
    /// The user's visual row order (new rows are appended)
    order: Vec<RowId>,
    /// Downloads that belong to a mirror job (no per-file notifications)
    mirror_children: HashSet<DownloadId>,
    /// Last known state per mirror job, for transition messages
    mirror_states: HashMap<uuid::Uuid, RecursiveJobState>,
    /// The engine's download set changed since the rows were built
    rows_dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// List, graph, details and activity side by side (100x24 and up)
    Wide,
    /// One-line rows above a short details box (down to 60x12)
    Compact,
    /// Just the list
    Minimal,
}

/// What the text in the add dialog was recognised as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddKind {
    Empty,
    Invalid,
    Http,
    /// An HTTP(S) URL ending in `/`: mirrored recursively
    Directory,
    Magnet,
    Torrent,
    Metalink,
}

impl AddKind {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Empty | Self::Invalid => "?",
            Self::Http => "http",
            Self::Directory => "mirror",
            Self::Magnet => "magnet",
            Self::Torrent => "torrent",
            Self::Metalink => "metalink",
        }
    }
}

/// Classify add-dialog input. `single_file` turns a trailing-slash URL back
/// into an ordinary download.
pub fn detect_input(text: &str, single_file: bool) -> (AddKind, String) {
    let text = text.trim();
    if text.is_empty() {
        return (AddKind::Empty, "Waiting for input".to_string());
    }
    match parse_input(text) {
        Ok(ParsedInput::Http(url)) => {
            let is_dir = url
                .split(['?', '#'])
                .next()
                .is_some_and(|path| path.ends_with('/'));
            if is_dir && !single_file {
                (
                    AddKind::Directory,
                    "Directory listing \u{b7} mirrored recursively (wget -r style)".to_string(),
                )
            } else if is_dir {
                (
                    AddKind::Http,
                    "HTTP(S) \u{b7} directory URL saved as a single file".to_string(),
                )
            } else {
                (
                    AddKind::Http,
                    "HTTP(S) \u{b7} multi-connection \u{b7} resumable".to_string(),
                )
            }
        }
        Ok(ParsedInput::Magnet(_)) => (
            AddKind::Magnet,
            "Magnet link \u{b7} metadata via DHT / PEX / LPD".to_string(),
        ),
        Ok(ParsedInput::TorrentFile(_)) => (
            AddKind::Torrent,
            "Torrent file \u{b7} parsed locally".to_string(),
        ),
        Ok(ParsedInput::Metalink(_)) => (
            AddKind::Metalink,
            "Metalink \u{b7} one download per file, with mirrors and checksums".to_string(),
        ),
        Err(e) => (AddKind::Invalid, e.to_string()),
    }
}

/// Outcome shown in the verify dialog.
#[derive(Debug, Clone)]
pub enum VerifyOutcome {
    Report(VerifyReport),
    Failed(String),
}

#[derive(Debug)]
pub enum DialogState {
    AddUrl {
        input: String,
        /// Cursor as a *character* index, not a byte offset
        cursor: usize,
        /// Treat a trailing-slash URL as one file instead of a mirror
        single_file: bool,
    },
    ConfirmCancel {
        target: RowId,
        name: String,
        delete_files: bool,
    },
    ConfirmCancelAll,
    ConfirmRepair {
        id: DownloadId,
        name: String,
    },
    Verify {
        id: DownloadId,
        name: String,
        tag: String,
        repair: bool,
        outcome: Option<VerifyOutcome>,
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
    pub kind: AddKind,
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

pub struct ActivityEntry {
    /// Wall-clock time, shown as HH:MM:SS
    pub time: chrono::DateTime<chrono::Local>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl From<ActivityLevel> for ToastLevel {
    fn from(level: ActivityLevel) -> Self {
        match level {
            ActivityLevel::Info => Self::Info,
            ActivityLevel::Success => Self::Success,
            ActivityLevel::Warning => Self::Warning,
            ActivityLevel::Error => Self::Error,
        }
    }
}

fn layout_for(width: u16, height: u16) -> LayoutMode {
    if width >= 100 && height >= 24 {
        LayoutMode::Wide
    } else if width >= 60 && height >= 12 {
        LayoutMode::Compact
    } else {
        LayoutMode::Minimal
    }
}

impl TuiApp {
    pub async fn new(config: CliConfig, config_path: Option<std::path::PathBuf>) -> Result<Self> {
        let engine = crate::app::create_engine(&config).await?;
        let theme = Theme::from_name(&config.tui.theme);
        let (terminal_width, terminal_height) = crossterm::terminal::size().unwrap_or((80, 24));

        let mut app = Self {
            engine,
            config,
            config_path,
            integrity_task: None,
            mirror_tasks: Vec::new(),
            theme,
            mode: ViewMode::All,
            rows: Vec::new(),
            counts: [0; 4],
            mirror_files: (0, 0),
            selected: 0,
            scroll_offset: 0,
            last_visible_height: 10,
            speed_history: VecDeque::with_capacity(SPEED_HISTORY),
            show_help: false,
            dialog: None,
            last_frame: Instant::now(),
            download_speed: 0,
            upload_speed: 0,
            peak_download_speed: 0,
            peak_upload_speed: 0,
            throbber_state: ThrobberState::default(),
            toasts: Vec::new(),
            effect_manager: tachyonfx::EffectManager::default(),
            startup_effects_added: false,
            layout_mode: layout_for(terminal_width, terminal_height),
            terminal_width,
            terminal_height,
            search: None,
            activity_log: VecDeque::new(),
            show_activity_log: true,
            activity_log_scroll: 0,
            order: Vec::new(),
            mirror_children: HashSet::new(),
            mirror_states: HashMap::new(),
            rows_dirty: false,
        };
        app.refresh_rows();
        let restored = app.counts[0];
        app.push_activity(
            ActivityLevel::Info,
            format!(
                "gosh {} \u{b7} {} \u{b7} {} download{} restored",
                env!("CARGO_PKG_VERSION"),
                app.storage_label(),
                restored,
                if restored == 1 { "" } else { "s" }
            ),
        );
        Ok(app)
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn config(&self) -> &CliConfig {
        &self.config
    }

    pub fn config_path(&self) -> std::path::PathBuf {
        self.config_path
            .clone()
            .unwrap_or_else(CliConfig::default_path)
    }

    pub fn show_speed_graph(&self) -> bool {
        self.config.tui.show_speed_graph
    }

    pub fn show_peers(&self) -> bool {
        self.config.tui.show_peers
    }

    pub fn storage_label(&self) -> &'static str {
        match self.config.general.storage_backend {
            crate::config::StorageBackend::Sqlite => "sqlite",
            crate::config::StorageBackend::File => "file",
            crate::config::StorageBackend::None => "no storage",
        }
    }

    /// Whether an integrity check is running right now.
    pub fn integrity_running(&self) -> bool {
        self.integrity_task.is_some()
    }

    /// Mirror discoveries still crawling.
    pub fn pending_mirrors(&self) -> usize {
        self.mirror_tasks.len()
    }

    fn save_settings(&mut self, config: CliConfig) -> Result<bool> {
        config.validate()?;
        std::fs::create_dir_all(&config.general.download_dir)?;
        let engine_config = config.to_engine_config();
        engine_config.validate()?;
        config.save(self.config_path.as_deref())?;
        self.engine.set_config(engine_config)?;
        self.engine
            .set_file_conflict_policy(config.general.file_conflict.to_engine());
        let restart = settings::restart_needed(&self.config, &config);
        self.theme = Theme::from_name(&config.tui.theme);
        self.config = config;
        Ok(restart)
    }

    fn start_integrity(&mut self, id: DownloadId, repair: bool) -> bool {
        if self.integrity_task.is_some() {
            self.push_toast(
                "An integrity check is already running".into(),
                ToastLevel::Info,
            );
            return false;
        }
        let engine = self.engine.clone();
        self.integrity_task = Some(tokio::spawn(async move {
            let result = if repair {
                engine.repair(id).await
            } else {
                engine.verify(id).await
            };
            (repair, id, result)
        }));
        true
    }

    pub(super) async fn finish_integrity(&mut self) {
        if !self
            .integrity_task
            .as_ref()
            .is_some_and(|task| task.is_finished())
        {
            return;
        }
        let Some(task) = self.integrity_task.take() else {
            return;
        };
        let (repair, id, result) = match task.await {
            Ok(done) => done,
            Err(e) => {
                self.push_activity(ActivityLevel::Error, format!("integrity check failed: {e}"));
                self.push_toast("Integrity check failed".into(), ToastLevel::Error);
                return;
            }
        };
        let name = self
            .engine
            .status(id)
            .map(|s| s.metadata.name)
            .unwrap_or_else(|| id.to_gid());
        let (message, level, outcome) = match result {
            Ok(report) if report.valid => (
                format!("verify {name} \u{b7} data ok \u{b7} {}", report.detail),
                ActivityLevel::Success,
                VerifyOutcome::Report(report),
            ),
            Ok(report) if repair => (
                format!("repair queued \u{b7} {name} \u{b7} {}", report.detail),
                ActivityLevel::Warning,
                VerifyOutcome::Report(report),
            ),
            Ok(report) => (
                format!("verify {name} \u{b7} NOT valid \u{b7} {}", report.detail),
                ActivityLevel::Error,
                VerifyOutcome::Report(report),
            ),
            Err(e) => (
                format!("verify {name} \u{b7} {e}"),
                ActivityLevel::Error,
                VerifyOutcome::Failed(e.to_string()),
            ),
        };
        self.push_activity(level, message.clone());

        // Show the result in the dialog if it is still open for this
        // download; otherwise as a toast.
        let shown_in_dialog = match &mut self.dialog {
            Some(DialogState::Verify {
                id: open_id,
                outcome: slot,
                ..
            }) if *open_id == id => {
                *slot = Some(outcome);
                true
            }
            _ => false,
        };
        if !shown_in_dialog {
            self.push_toast(message, level.into());
        }
        self.refresh_rows();
    }

    /// Collect finished mirror discoveries.
    async fn finish_mirror_tasks(&mut self) {
        let mut index = 0;
        while index < self.mirror_tasks.len() {
            if !self.mirror_tasks[index].is_finished() {
                index += 1;
                continue;
            }
            let task = self.mirror_tasks.swap_remove(index);
            match task.await {
                Ok((url, Ok(job))) => {
                    let files = job.child_ids.len();
                    let (level, message) = if files == 0 {
                        (
                            ActivityLevel::Warning,
                            format!("mirror {url} \u{b7} no downloadable files found"),
                        )
                    } else {
                        (
                            ActivityLevel::Info,
                            format!("mirror {url} \u{b7} {files} files queued"),
                        )
                    };
                    self.push_activity(level, message.clone());
                    self.push_toast(message, level.into());
                }
                Ok((url, Err(e))) => {
                    self.push_activity(ActivityLevel::Error, format!("mirror {url} \u{b7} {e}"));
                    // Never replace a dialog the user is typing into.
                    if self.dialog.is_none() {
                        self.dialog = Some(DialogState::Error {
                            message: format!("Could not mirror {url}: {e}"),
                        });
                    } else {
                        self.push_toast(format!("Mirror failed: {e}"), ToastLevel::Error);
                    }
                }
                Err(e) => {
                    self.push_activity(ActivityLevel::Error, format!("mirror task failed: {e}"));
                }
            }
            self.rows_dirty = true;
        }
    }

    fn reorder_selected(&mut self, direction: i32) {
        let len = self.rows.len();
        if len < 2 {
            return;
        }
        let neighbour = if direction > 0 {
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
        let a = self.rows[self.selected].id();
        let b = self.rows[neighbour].id();
        let (Some(ia), Some(ib)) = (
            self.order.iter().position(|id| *id == a),
            self.order.iter().position(|id| *id == b),
        ) else {
            return;
        };
        self.order.swap(ia, ib);
        self.rows.swap(self.selected, neighbour);
        self.selected = neighbour;
        self.adjust_scroll(self.last_visible_height);
    }

    pub(super) fn detect_layout_mode(&mut self) {
        self.layout_mode = layout_for(self.terminal_width, self.terminal_height);
    }

    /// Run the TUI event loop
    pub async fn run(&mut self) -> Result<()> {
        let result = self.run_terminal().await;

        // Let verification/repair finish before shutting its engine down.
        if let Some(task) = self.integrity_task.take() {
            let _ = task.await;
        }
        // A discovery still crawling would enqueue downloads into an engine
        // that is about to stop.
        for task in self.mirror_tasks.drain(..) {
            task.abort();
        }
        let shutdown = self.engine.shutdown().await;
        result?;
        shutdown?;
        Ok(())
    }

    async fn run_terminal(&mut self) -> Result<()> {
        let _guard = TerminalGuard::new();
        let mut terminal = setup_terminal()?;

        let tick_rate = Duration::from_millis(self.config.tui.refresh_rate_ms);
        let mut event_handler = EventHandler::new(
            self.engine.subscribe(),
            self.engine.subscribe_recursive_jobs(),
            tick_rate,
        );

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
                    // Act on what is really there, not on a stale list.
                    if self.rows_dirty {
                        self.refresh_rows();
                    }
                    if self.handle_terminal_event(&event).await? {
                        break;
                    }
                    // Input latency matters: draw immediately
                    draw = true;
                }
                AppEvent::Engine(event) => self.handle_engine_event(event),
                AppEvent::RecursiveJob(event) => self.handle_recursive_event(event),
                AppEvent::Tick | AppEvent::Resync => {
                    self.on_tick().await;
                    draw = true;
                }
                AppEvent::Resize(w, h) => {
                    self.terminal_width = w;
                    self.terminal_height = h;
                    self.detect_layout_mode();
                    draw = true;
                }
            }

            if draw {
                terminal.draw(|frame| ui::render(frame, self))?;
            }
        }

        Ok(())
    }

    /// Periodic work: collect background results and refresh everything.
    pub(super) async fn on_tick(&mut self) {
        self.finish_integrity().await;
        self.finish_mirror_tasks().await;
        self.refresh_rows();
        self.update_stats();
    }

    /// Handle terminal input events. Returns true to quit.
    pub(super) async fn handle_terminal_event(&mut self, event: &Event) -> Result<bool> {
        // Ctrl+C works even while a dialog or search field has focus.
        if event::is_ctrl_c(event) {
            return Ok(true);
        }

        if let Some(dialog) = self.dialog.take() {
            if let Some(keep) = self.handle_dialog_event(dialog, event).await? {
                self.dialog = Some(keep);
            }
            return Ok(false);
        }

        // Help overlay: any key closes it
        if self.show_help {
            if matches!(event, Event::Key(_)) {
                self.show_help = false;
            }
            return Ok(false);
        }

        // Search input mode (only while the bar is capturing keys; a
        // committed filter releases the keyboard back to normal bindings)
        if self.search.as_ref().is_some_and(|s| s.input_active) {
            self.handle_search_event(event);
            return Ok(false);
        }

        let Event::Key(key) = event else {
            return Ok(false);
        };
        // Ctrl/Alt chords are not bindings; ignoring them keeps e.g. Ctrl+A
        // from opening the add dialog.
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return Ok(false);
        }
        // Uppercase shortcuts: terminals disagree on whether Shift is
        // reported as a modifier or folded into the character.
        let upper = |c: char| event::is_upper_key(event, c);

        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Esc => {
                // Esc clears a committed search filter
                if self.search.take().is_some() {
                    self.refresh_rows();
                }
            }
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = true,
            KeyCode::Up => self.select_prev(),
            KeyCode::Down => self.select_next(),
            KeyCode::PageUp => {
                for _ in 0..self.last_visible_height.max(1) {
                    self.select_prev();
                }
            }
            KeyCode::PageDown => {
                for _ in 0..self.last_visible_height.max(1) {
                    self.select_next();
                }
            }
            KeyCode::Home => {
                self.selected = 0;
                self.adjust_scroll(self.last_visible_height);
            }
            KeyCode::End => {
                self.selected = self.rows.len().saturating_sub(1);
                self.adjust_scroll(self.last_visible_height);
            }
            KeyCode::Tab => {
                let index = ViewMode::ALL
                    .iter()
                    .position(|m| *m == self.mode)
                    .unwrap_or(0);
                self.mode = ViewMode::ALL[(index + 1) % ViewMode::ALL.len()];
                self.refresh_rows();
            }
            _ if upper('A') => {
                self.dialog = Some(DialogState::BatchImport {
                    phase: BatchPhase::Input {
                        text: String::new(),
                    },
                });
            }
            _ if upper('P') => {
                let result = self.engine.pause_all().await;
                self.report_batch("pause-all", "paused", &result);
            }
            _ if upper('R') => {
                let result = self.engine.resume_all().await;
                self.report_batch("resume-all", "resumed", &result);
            }
            _ if upper('C') => {
                if self.counts[0] > 0 {
                    self.dialog = Some(DialogState::ConfirmCancelAll);
                }
            }
            _ if upper('V') => self.ask_repair(),
            _ if upper('S') => self.open_settings(0),
            _ if upper('L') => {
                self.show_activity_log = !self.show_activity_log;
                self.activity_log_scroll = 0;
            }
            _ if upper('J') => self.reorder_selected(1),
            _ if upper('K') => self.reorder_selected(-1),
            KeyCode::Char('k') => self.select_prev(),
            KeyCode::Char('j') => self.select_next(),
            KeyCode::Char('a') => {
                self.dialog = Some(DialogState::AddUrl {
                    input: String::new(),
                    cursor: 0,
                    single_file: false,
                });
            }
            KeyCode::Char('p') => self.pause_selected().await,
            KeyCode::Char('r') => self.resume_selected().await,
            KeyCode::Char('v') => self.verify_selected(),
            KeyCode::Char('c') | KeyCode::Char('d') => {
                if let Some(row) = self.selected_row() {
                    self.dialog = Some(DialogState::ConfirmCancel {
                        target: row.id(),
                        name: row.name(),
                        delete_files: key.code == KeyCode::Char('d'),
                    });
                }
            }
            KeyCode::Char(c @ '1'..='4') => {
                self.mode = ViewMode::ALL[(c as usize) - ('1' as usize)];
                self.refresh_rows();
            }
            KeyCode::Char('[') => {
                if self.show_activity_log {
                    self.activity_log_scroll = (self.activity_log_scroll + 1)
                        .min(self.activity_log.len().saturating_sub(1));
                }
            }
            KeyCode::Char(']') => {
                self.activity_log_scroll = self.activity_log_scroll.saturating_sub(1);
            }
            KeyCode::Char('/') => match self.search.as_mut() {
                Some(s) => s.input_active = true,
                None => self.search = Some(SearchState::default()),
            },
            _ => {}
        }

        Ok(false)
    }

    /// Keys typed into the search bar.
    fn handle_search_event(&mut self, event: &Event) {
        let Some(search) = self.search.as_mut() else {
            return;
        };
        let mut changed = false;
        let mut close = false;
        match event {
            Event::Paste(text) => {
                let clean: String = text.chars().filter(|c| !c.is_control()).collect();
                let byte_pos = char_to_byte(&search.query, search.cursor);
                search.query.insert_str(byte_pos, &clean);
                search.cursor += clean.chars().count();
                changed = true;
            }
            Event::Key(key) => match key.code {
                KeyCode::Esc => {
                    close = true;
                    changed = true;
                }
                KeyCode::Enter => {
                    if search.query.is_empty() {
                        close = true;
                    } else {
                        // Commit: filter stays active, keys fall through
                        search.input_active = false;
                    }
                }
                KeyCode::Tab => {
                    search.scope = search.scope.next();
                    changed = true;
                }
                KeyCode::Char('s') if key.modifiers == KeyModifiers::CONTROL => {
                    search.scope = search.scope.next();
                    changed = true;
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    let byte_pos = char_to_byte(&search.query, search.cursor);
                    search.query.insert(byte_pos, c);
                    search.cursor += 1;
                    changed = true;
                }
                KeyCode::Backspace if search.cursor > 0 => {
                    search.cursor -= 1;
                    let byte_pos = char_to_byte(&search.query, search.cursor);
                    search.query.remove(byte_pos);
                    changed = true;
                }
                KeyCode::Left if search.cursor > 0 => search.cursor -= 1,
                KeyCode::Right if search.cursor < search.query.chars().count() => {
                    search.cursor += 1;
                }
                _ => {}
            },
            _ => {}
        }
        if close {
            self.search = None;
        }
        if changed {
            self.refresh_rows();
        }
    }

    /// Handle input for an open dialog. Returns the dialog to keep it open.
    /// A handler that wants a *different* dialog sets `self.dialog` itself
    /// and returns `None`.
    async fn handle_dialog_event(
        &mut self,
        dialog: DialogState,
        event: &Event,
    ) -> Result<Option<DialogState>> {
        match dialog {
            DialogState::AddUrl {
                mut input,
                mut cursor,
                mut single_file,
            } => {
                if event::is_escape(event) {
                    return Ok(None);
                }
                if let Event::Key(key) = event {
                    match key.code {
                        // Shift+Enter is only reported by terminals with the
                        // kitty keyboard protocol; F3 works everywhere.
                        KeyCode::Enter | KeyCode::F(3) => {
                            let paused = key.code == KeyCode::F(3)
                                || key.modifiers.contains(KeyModifiers::SHIFT);
                            if !input.trim().is_empty() {
                                self.add_input(&input, single_file, paused, true).await;
                                return Ok(None);
                            }
                        }
                        KeyCode::Tab => single_file = !single_file,
                        KeyCode::Char(c)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            let byte_pos = char_to_byte(&input, cursor);
                            input.insert(byte_pos, c);
                            cursor += 1;
                        }
                        KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                            input.clear();
                            cursor = 0;
                        }
                        KeyCode::Backspace if cursor > 0 => {
                            cursor -= 1;
                            let byte_pos = char_to_byte(&input, cursor);
                            input.remove(byte_pos);
                        }
                        KeyCode::Delete if cursor < input.chars().count() => {
                            let byte_pos = char_to_byte(&input, cursor);
                            input.remove(byte_pos);
                        }
                        KeyCode::Left if cursor > 0 => cursor -= 1,
                        KeyCode::Right if cursor < input.chars().count() => cursor += 1,
                        KeyCode::Home => cursor = 0,
                        KeyCode::End => cursor = input.chars().count(),
                        _ => {}
                    }
                } else if let Event::Paste(text) = event {
                    // A pasted URL is one edit, not a stream of key presses.
                    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
                    let byte_pos = char_to_byte(&input, cursor);
                    input.insert_str(byte_pos, &clean);
                    cursor += clean.chars().count();
                }
                Ok(Some(DialogState::AddUrl {
                    input,
                    cursor,
                    single_file,
                }))
            }

            DialogState::ConfirmCancel {
                target,
                name,
                delete_files,
            } => {
                if event::is_escape(event) || event::is_key(event, 'n') {
                    return Ok(None);
                }
                if event::is_key(event, 'y') || event::is_enter(event) {
                    self.cancel_row(target, &name, delete_files).await;
                    return Ok(None);
                }
                Ok(Some(DialogState::ConfirmCancel {
                    target,
                    name,
                    delete_files,
                }))
            }

            DialogState::ConfirmCancelAll => {
                if event::is_escape(event) || event::is_key(event, 'n') {
                    return Ok(None);
                }
                if event::is_key(event, 'y') || event::is_enter(event) {
                    let result = self.engine.cancel_all(false).await;
                    // Cancelling every child leaves empty mirror records.
                    for job in self.engine.list_recursive_jobs() {
                        let _ = self.engine.remove_recursive_job(job.id, false).await;
                    }
                    self.report_batch("cancel-all", "removed \u{b7} files kept", &result);
                    return Ok(None);
                }
                Ok(Some(DialogState::ConfirmCancelAll))
            }

            DialogState::ConfirmRepair { id, name } => {
                if event::is_escape(event) || event::is_key(event, 'n') {
                    return Ok(None);
                }
                if event::is_key(event, 'y') || event::is_enter(event) {
                    self.open_verify(id, true);
                    return Ok(None);
                }
                Ok(Some(DialogState::ConfirmRepair { id, name }))
            }

            DialogState::Verify {
                id,
                name,
                tag,
                repair,
                outcome,
            } => {
                if event::is_escape(event) || event::is_enter(event) || event::is_key(event, 'q') {
                    return Ok(None);
                }
                let can_repair = !repair
                    && matches!(&outcome, Some(VerifyOutcome::Report(report)) if !report.valid);
                if event::is_upper_key(event, 'V') && can_repair {
                    self.dialog = Some(DialogState::ConfirmRepair { id, name });
                    return Ok(None);
                }
                Ok(Some(DialogState::Verify {
                    id,
                    name,
                    tag,
                    repair,
                    outcome,
                }))
            }

            DialogState::Error { message } => {
                if event::is_escape(event) || event::is_enter(event) || event::is_key(event, 'q') {
                    return Ok(None);
                }
                Ok(Some(DialogState::Error { message }))
            }

            DialogState::Settings {
                active_tab,
                selected_row,
                editing,
                draft,
                dirty,
            } => Ok(self.handle_settings_event(
                event,
                active_tab,
                selected_row,
                editing,
                draft,
                dirty,
            )),

            DialogState::BatchImport { phase } => self.handle_batch_event(event, phase).await,
        }
    }

    fn handle_settings_event(
        &mut self,
        event: &Event,
        mut active_tab: usize,
        mut selected_row: usize,
        mut editing: Option<String>,
        mut draft: Box<CliConfig>,
        mut dirty: bool,
    ) -> Option<DialogState> {
        let tab_count = settings::TAB_NAMES.len();

        match event {
            Event::Paste(text) => {
                if let Some(buffer) = editing.as_mut() {
                    buffer.extend(text.chars().filter(|c| !c.is_control()));
                }
            }
            Event::Key(key) => {
                if let Some(mut buffer) = editing.take() {
                    match key.code {
                        KeyCode::Esc => {}
                        KeyCode::Enter => {
                            if let Some(setting) = settings::rows(active_tab).get(selected_row) {
                                match settings::apply(&mut draft, setting, &buffer) {
                                    Ok(()) => dirty = true,
                                    Err(e) => {
                                        self.push_toast(
                                            format!("Invalid setting: {e}"),
                                            ToastLevel::Error,
                                        );
                                        editing = Some(buffer);
                                    }
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            buffer.pop();
                            editing = Some(buffer);
                        }
                        KeyCode::Char(c)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            buffer.push(c);
                            editing = Some(buffer);
                        }
                        _ => editing = Some(buffer),
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            if !dirty {
                                return None;
                            }
                            match self.save_settings((*draft).clone()) {
                                Ok(restart) => {
                                    if restart {
                                        self.push_toast(
                                            "Saved \u{b7} restart gosh to apply the \u{21bb} settings"
                                                .into(),
                                            ToastLevel::Warning,
                                        );
                                    } else {
                                        self.push_toast(
                                            "Settings saved".into(),
                                            ToastLevel::Success,
                                        );
                                    }
                                    self.push_activity(
                                        ActivityLevel::Info,
                                        format!(
                                            "settings saved to {}",
                                            self.config_path().display()
                                        ),
                                    );
                                    return None;
                                }
                                Err(e) => self.push_toast(
                                    format!("Settings not saved: {e}"),
                                    ToastLevel::Error,
                                ),
                            }
                        }
                        KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => {
                            active_tab = (active_tab + tab_count - 1) % tab_count;
                            selected_row = 0;
                        }
                        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                            active_tab = (active_tab + 1) % tab_count;
                            selected_row = 0;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            selected_row = selected_row.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let max_row = settings::row_count(active_tab, &draft).saturating_sub(1);
                            selected_row = (selected_row + 1).min(max_row);
                        }
                        KeyCode::Char(n @ '1'..='5') => {
                            active_tab = (n as usize) - ('1' as usize);
                            selected_row = 0;
                        }
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            if active_tab == settings::SCHEDULE_TAB {
                                if let Some(rule) = draft.schedule.rules.get_mut(selected_row) {
                                    rule.enabled = !rule.enabled;
                                    dirty = true;
                                }
                            } else if let Some(setting) =
                                settings::rows(active_tab).get(selected_row)
                            {
                                match setting.editor {
                                    settings::Editor::Text => {
                                        editing = Some(settings::edit_value(&draft, setting));
                                    }
                                    _ => match settings::activate(&mut draft, setting) {
                                        Ok(()) => dirty = true,
                                        Err(e) => self.push_toast(
                                            format!("Invalid setting: {e}"),
                                            ToastLevel::Error,
                                        ),
                                    },
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        Some(DialogState::Settings {
            active_tab,
            selected_row,
            editing,
            draft,
            dirty,
        })
    }

    async fn handle_batch_event(
        &mut self,
        event: &Event,
        phase: BatchPhase,
    ) -> Result<Option<DialogState>> {
        let keep = |phase| Ok(Some(DialogState::BatchImport { phase }));
        let key = match event {
            Event::Key(key) => key,
            Event::Paste(pasted) => {
                // A pasted list arrives as one event; keep its line breaks.
                return match phase {
                    BatchPhase::Input { mut text } => {
                        text.push_str(&pasted.replace("\r\n", "\n").replace('\r', "\n"));
                        keep(BatchPhase::Input { text })
                    }
                    other => keep(other),
                };
            }
            _ => return keep(phase),
        };

        match phase {
            BatchPhase::Input { mut text } => {
                match key.code {
                    KeyCode::Esc => return Ok(None),
                    // Ctrl+Enter is not reported by every terminal; F2 is.
                    KeyCode::F(2) => return keep(Self::review_batch(text)),
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return keep(Self::review_batch(text));
                    }
                    KeyCode::Enter => text.push('\n'),
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        text.push(c);
                    }
                    KeyCode::Backspace => {
                        text.pop();
                    }
                    _ => {}
                }
                keep(BatchPhase::Input { text })
            }
            BatchPhase::Review {
                mut entries,
                mut selected,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        let text = entries
                            .iter()
                            .map(|e| e.url.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                        return keep(BatchPhase::Input { text });
                    }
                    KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') if selected + 1 < entries.len() => {
                        selected += 1;
                    }
                    KeyCode::Char(' ') => {
                        if let Some(entry) = entries.get_mut(selected) {
                            if entry.valid {
                                entry.selected = !entry.selected;
                            }
                        }
                    }
                    KeyCode::Enter => {
                        let urls: Vec<String> = entries
                            .iter()
                            .filter(|e| e.selected && e.valid)
                            .map(|e| e.url.clone())
                            .collect();
                        let total = urls.len();
                        let mut count = 0;
                        for url in urls {
                            if self.add_input(&url, false, false, false).await {
                                count += 1;
                            }
                        }
                        let level = if count == total {
                            ActivityLevel::Success
                        } else {
                            ActivityLevel::Error
                        };
                        self.push_activity(
                            level,
                            format!(
                                "batch import \u{b7} {count} added \u{b7} {} skipped",
                                entries.len() - count
                            ),
                        );
                        self.push_toast(format!("Added {count}/{total} downloads"), level.into());
                        return Ok(None);
                    }
                    _ => {}
                }
                keep(BatchPhase::Review { entries, selected })
            }
        }
    }

    /// Turn pasted text into a reviewable list (blank lines and `#`
    /// comments are skipped). Stays in the input phase if nothing is left.
    fn review_batch(text: String) -> BatchPhase {
        let entries: Vec<BatchEntry> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|url| {
                let (kind, label) = detect_input(url, false);
                let valid = !matches!(kind, AddKind::Invalid | AddKind::Empty);
                BatchEntry {
                    url: url.to_string(),
                    valid,
                    selected: valid,
                    kind,
                    error: (!valid).then_some(label),
                }
            })
            .collect();
        if entries.is_empty() {
            BatchPhase::Input { text }
        } else {
            BatchPhase::Review {
                entries,
                selected: 0,
            }
        }
    }

    fn open_settings(&mut self, tab: usize) {
        self.dialog = Some(DialogState::Settings {
            active_tab: tab,
            selected_row: 0,
            editing: None,
            draft: Box::new(self.config.clone()),
            dirty: false,
        });
    }

    /// Handle engine events
    fn handle_engine_event(&mut self, event: DownloadEvent) {
        match event {
            // Progress is picked up by the next tick. A mirror adds hundreds
            // of downloads in a burst; rebuilding the list once is enough.
            DownloadEvent::Progress { .. } => {}
            DownloadEvent::Completed { id } => {
                self.refresh_rows();
                if self.mirror_children.contains(&id) {
                    return;
                }
                if let Some(name) = self.engine.status(id).map(|s| s.metadata.name) {
                    self.push_toast(format!("\u{2713} {name}"), ToastLevel::Success);
                    self.push_activity(ActivityLevel::Success, format!("complete {name}"));
                }
            }
            DownloadEvent::Failed { id, error, .. } => {
                self.refresh_rows();
                let name = self
                    .engine
                    .status(id)
                    .map(|s| s.metadata.name)
                    .unwrap_or_else(|| id.to_gid());
                self.push_activity(
                    ActivityLevel::Error,
                    format!("{name} \u{b7} {}", truncate_str(&error, 120)),
                );
                if !self.mirror_children.contains(&id) {
                    self.push_toast(format!("\u{2717} {name}"), ToastLevel::Error);
                }
            }
            DownloadEvent::Paused { id } => {
                self.refresh_rows();
                if !self.mirror_children.contains(&id) {
                    if let Some(status) = self.engine.status(id) {
                        self.push_activity(
                            ActivityLevel::Warning,
                            format!("paused {}", status.metadata.name),
                        );
                    }
                }
            }
            DownloadEvent::Resumed { id } => {
                self.refresh_rows();
                if !self.mirror_children.contains(&id) {
                    if let Some(status) = self.engine.status(id) {
                        self.push_activity(
                            ActivityLevel::Info,
                            format!("resume {}", status.metadata.name),
                        );
                    }
                }
            }
            _ => self.rows_dirty = true,
        }
    }

    /// Handle recursive mirror job events
    fn handle_recursive_event(&mut self, event: RecursiveJobEvent) {
        match event {
            RecursiveJobEvent::Added { job, status } => {
                self.mirror_states.insert(job.id, status.state);
                self.refresh_rows();
            }
            RecursiveJobEvent::Updated { job, status } => {
                let previous = self.mirror_states.insert(job.id, status.state);
                if previous == Some(status.state) {
                    return;
                }
                let url = job.root_url;
                let progress = status.progress;
                match status.state {
                    RecursiveJobState::Completed => {
                        self.push_toast(
                            format!("Mirror complete ({} files)", progress.total_children),
                            ToastLevel::Success,
                        );
                        self.push_activity(
                            ActivityLevel::Success,
                            format!(
                                "mirror complete \u{b7} {} files \u{b7} {url}",
                                progress.total_children
                            ),
                        );
                    }
                    RecursiveJobState::Failed => {
                        self.push_toast("Mirror failed".into(), ToastLevel::Error);
                        self.push_activity(
                            ActivityLevel::Error,
                            format!("mirror failed \u{b7} {url}"),
                        );
                    }
                    RecursiveJobState::Partial => {
                        self.push_activity(
                            ActivityLevel::Warning,
                            format!(
                                "mirror finished with failures \u{b7} {}/{} files \u{b7} {url}",
                                progress.completed_children, progress.total_children
                            ),
                        );
                    }
                    _ => {}
                }
            }
            RecursiveJobEvent::Removed { id } => {
                self.mirror_states.remove(&id);
                self.refresh_rows();
            }
        }
    }

    /// Report a batch-operation outcome as a toast + activity entry
    fn report_batch(&mut self, name: &str, verb: &str, result: &BatchResult) {
        if result.succeeded.is_empty() && result.skipped.is_empty() && result.failed.is_empty() {
            self.push_toast("No downloads to act on".to_string(), ToastLevel::Info);
            return;
        }

        let mut msg = format!("{name} \u{b7} {} {verb}", result.succeeded.len());
        if !result.skipped.is_empty() {
            msg.push_str(&format!(" \u{b7} {} skipped", result.skipped.len()));
        }
        if !result.failed.is_empty() {
            msg.push_str(&format!(" \u{b7} {} failed", result.failed.len()));
        }

        let level = if result.failed.is_empty() {
            ActivityLevel::Info
        } else {
            ActivityLevel::Error
        };
        self.push_toast(msg.clone(), level.into());
        self.push_activity(level, msg);
        self.refresh_rows();
    }

    /// Update global stats
    pub(super) fn update_stats(&mut self) {
        let stats = self.engine.global_stats();
        self.download_speed = stats.download_speed;
        self.upload_speed = stats.upload_speed;
        self.peak_download_speed = self.peak_download_speed.max(stats.download_speed);
        self.peak_upload_speed = self.peak_upload_speed.max(stats.upload_speed);

        self.speed_history
            .push_back((stats.download_speed, stats.upload_speed));
        while self.speed_history.len() > SPEED_HISTORY {
            self.speed_history.pop_front();
        }

        self.throbber_state.calc_next();

        // Expire old toasts (4 second lifetime)
        self.toasts
            .retain(|t| t.created.elapsed() < Duration::from_secs(4));
    }

    pub(crate) fn push_toast(&mut self, message: String, level: ToastLevel) {
        self.toasts.push(Toast {
            message,
            level,
            created: Instant::now(),
        });
        // Keep at most 4 toasts
        while self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    pub fn push_activity(&mut self, level: ActivityLevel, message: String) {
        self.activity_log.push_back(ActivityEntry {
            time: chrono::Local::now(),
            level,
            message,
        });
        while self.activity_log.len() > 500 {
            self.activity_log.pop_front();
        }
    }

    /// Rebuild the row list from the engine.
    pub(crate) fn refresh_rows(&mut self) {
        self.rows_dirty = false;
        let keep = self.selected_row().map(|r| r.id());

        let jobs: Vec<_> = self
            .engine
            .list_recursive_jobs()
            .into_iter()
            .map(|job| {
                let status = self.engine.recursive_job_status(&job.as_job());
                (job, status)
            })
            .collect();
        self.mirror_children = jobs
            .iter()
            .flat_map(|(job, _)| job.child_ids.iter().copied())
            .collect();
        let mut rows = build_rows(self.engine.list(), jobs);

        // Keep the user's visual order; rows seen for the first time go last
        // (build_rows already sorted them by creation time).
        let position: HashMap<RowId, usize> = self
            .order
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();
        rows.sort_by_key(|row| position.get(&row.id()).copied().unwrap_or(usize::MAX));
        self.order = rows.iter().map(|row| row.id()).collect();

        for (slot, view) in self.counts.iter_mut().zip(ViewMode::ALL) {
            *slot = rows.iter().filter(|row| view.includes(row)).count();
        }

        self.mirror_files = rows.iter().fold((0, 0), |(done, total), row| match row {
            Row::Mirror(m) => (
                done + m.status.progress.completed_children,
                total + m.status.progress.total_children,
            ),
            Row::Download(_) => (done, total),
        });

        let mode = self.mode;
        rows.retain(|row| mode.includes(row));
        if let Some(search) = self.search.as_ref().filter(|s| !s.query.is_empty()) {
            let query = search.query.to_lowercase();
            let scope = search.scope;
            rows.retain(|row| Self::matches_search(row, &query, scope));
        }
        self.rows = rows;

        // Re-find the previously selected row; fall back to clamping
        self.selected = keep
            .and_then(|id| self.rows.iter().position(|row| row.id() == id))
            .unwrap_or_else(|| self.selected.min(self.rows.len().saturating_sub(1)));
        self.adjust_scroll(self.last_visible_height);
    }

    fn matches_search(row: &Row, query: &str, scope: SearchScope) -> bool {
        let (name, url, state) = row.search_text();
        match scope {
            SearchScope::Name => name.contains(query),
            SearchScope::Url => url.contains(query),
            SearchScope::State => state.contains(query),
            SearchScope::All => {
                name.contains(query) || url.contains(query) || state.contains(query)
            }
        }
    }

    /// Currently selected row
    pub fn selected_row(&self) -> Option<&Row> {
        self.rows.get(self.selected)
    }

    /// Adjust scroll offset to keep selected item visible
    pub fn adjust_scroll(&mut self, visible_height: usize) {
        let visible_height = visible_height.max(1);
        let total = self.rows.len();

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

    fn select_next(&mut self) {
        if !self.rows.is_empty() {
            self.selected = (self.selected + 1).min(self.rows.len() - 1);
            self.adjust_scroll(self.last_visible_height);
        }
    }

    fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.adjust_scroll(self.last_visible_height);
        }
    }

    fn select_id(&mut self, id: RowId) {
        if let Some(index) = self.rows.iter().position(|row| row.id() == id) {
            self.selected = index;
            self.adjust_scroll(self.last_visible_height);
        }
    }

    /// Add whatever the user typed: a download, a torrent, a Metalink
    /// document, or (for a directory URL) a recursive mirror. Returns
    /// whether something was started. `report` controls the per-item toast
    /// (batch import reports once for the whole batch).
    pub(super) async fn add_input(
        &mut self,
        text: &str,
        single_file: bool,
        paused: bool,
        report: bool,
    ) -> bool {
        let (kind, _) = detect_input(text, single_file);
        let options = gosh_dl::DownloadOptions {
            start_paused: paused,
            ..Default::default()
        };

        if kind == AddKind::Directory {
            let Ok(ParsedInput::Http(url)) = parse_input(text.trim()) else {
                return false;
            };
            let engine = self.engine.clone();
            let task_url = url.clone();
            self.mirror_tasks.push(tokio::spawn(async move {
                let result = engine
                    .add_http_recursive(&task_url, options, gosh_dl::RecursiveOptions::default())
                    .await;
                (task_url, result)
            }));
            self.push_activity(
                ActivityLevel::Info,
                format!("mirror {url} \u{b7} discovering files\u{2026}"),
            );
            if report {
                self.push_toast(format!("Discovering files under {url}"), ToastLevel::Info);
            }
            return true;
        }

        let result: Result<Vec<DownloadId>> = async {
            let input = parse_input(text.trim())?;
            crate::util::submit_input(&self.engine, &input, options).await
        }
        .await;

        match result {
            Ok(ids) => {
                // Show what was just added, whatever view or filter was on.
                if report {
                    self.mode = ViewMode::All;
                    self.search = None;
                }
                self.refresh_rows();
                if let Some(first) = ids.first() {
                    self.select_id(RowId::Download(*first));
                }
                let what = if paused { "saved paused" } else { "added" };
                let message = match ids.as_slice() {
                    [id] => {
                        let name = self
                            .engine
                            .status(*id)
                            .map(|s| s.metadata.name)
                            .unwrap_or_default();
                        format!("{what} {name}")
                    }
                    many => format!("{what} {} downloads", many.len()),
                };
                self.push_activity(ActivityLevel::Info, message.clone());
                if report {
                    self.push_toast(message, ToastLevel::Info);
                }
                true
            }
            Err(e) => {
                let message = format!("{e:#}");
                self.push_activity(
                    ActivityLevel::Error,
                    format!("could not add {} \u{b7} {message}", truncate_str(text, 60)),
                );
                self.dialog = Some(DialogState::Error { message });
                false
            }
        }
    }

    async fn pause_selected(&mut self) {
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        match row {
            Row::Download(d) => {
                if self.engine.pause(d.id).await.is_err() {
                    self.push_toast(
                        format!(
                            "Nothing to pause: {} is {}",
                            d.metadata.name,
                            RowState::from_download(&d.state).label().to_lowercase()
                        ),
                        ToastLevel::Info,
                    );
                }
            }
            Row::Mirror(m) => {
                let mut paused = 0;
                for child in &m.children {
                    if matches!(
                        child.state,
                        DownloadState::Queued
                            | DownloadState::Connecting
                            | DownloadState::Downloading
                    ) && self.engine.pause(child.id).await.is_ok()
                    {
                        paused += 1;
                    }
                }
                if paused == 0 {
                    self.push_toast("Nothing to pause".into(), ToastLevel::Info);
                } else {
                    self.push_activity(
                        ActivityLevel::Warning,
                        format!("paused mirror {} \u{b7} {paused} files", m.display_name()),
                    );
                }
            }
        }
        self.refresh_rows();
    }

    async fn resume_selected(&mut self) {
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        match row {
            Row::Download(d) => {
                if matches!(d.state, DownloadState::Paused | DownloadState::Error { .. }) {
                    if let Err(e) = self.engine.resume(d.id).await {
                        self.dialog = Some(DialogState::Error {
                            message: e.to_string(),
                        });
                    }
                } else {
                    self.push_toast("Nothing to resume".into(), ToastLevel::Info);
                }
            }
            Row::Mirror(m) => {
                let mut resumed = 0;
                for child in &m.children {
                    if matches!(
                        child.state,
                        DownloadState::Paused | DownloadState::Error { .. }
                    ) && self.engine.resume(child.id).await.is_ok()
                    {
                        resumed += 1;
                    }
                }
                if resumed == 0 {
                    self.push_toast("Nothing to resume".into(), ToastLevel::Info);
                } else {
                    self.push_activity(
                        ActivityLevel::Info,
                        format!("resume mirror {} \u{b7} {resumed} files", m.display_name()),
                    );
                }
            }
        }
        self.refresh_rows();
    }

    async fn cancel_row(&mut self, target: RowId, name: &str, delete_files: bool) {
        let result = match target {
            RowId::Download(id) => self.engine.cancel(id, delete_files).await,
            RowId::Mirror(id) => self.engine.remove_recursive_job(id, delete_files).await,
        };
        match result {
            Ok(()) => self.push_activity(
                ActivityLevel::Warning,
                format!(
                    "cancelled {name} \u{b7} {}",
                    if delete_files {
                        "files deleted"
                    } else {
                        "files kept"
                    }
                ),
            ),
            Err(e) => {
                self.dialog = Some(DialogState::Error {
                    message: e.to_string(),
                });
            }
        }
        self.refresh_rows();
    }

    /// The selected download if it can be verified now, with a hint if not.
    fn verifiable_selection(&mut self, action: &str) -> Option<(DownloadId, String)> {
        match self.selected_row() {
            Some(Row::Download(d)) => {
                if matches!(
                    d.state,
                    DownloadState::Queued | DownloadState::Connecting | DownloadState::Downloading
                ) {
                    self.push_toast(
                        format!("Pause the download before {action} it"),
                        ToastLevel::Warning,
                    );
                    return None;
                }
                Some((d.id, d.metadata.name.clone()))
            }
            Some(Row::Mirror(_)) => {
                self.push_toast(
                    "Mirrored files are checked one at a time: gosh verify <id>".into(),
                    ToastLevel::Info,
                );
                None
            }
            None => None,
        }
    }

    fn verify_selected(&mut self) {
        if let Some((id, _)) = self.verifiable_selection("verifying") {
            self.open_verify(id, false);
        }
    }

    fn ask_repair(&mut self) {
        if let Some((id, name)) = self.verifiable_selection("repairing") {
            self.dialog = Some(DialogState::ConfirmRepair { id, name });
        }
    }

    fn open_verify(&mut self, id: DownloadId, repair: bool) {
        let Some(status) = self.engine.status(id) else {
            return;
        };
        if !self.start_integrity(id, repair) {
            return;
        }
        let name = status.metadata.name.clone();
        self.push_activity(
            ActivityLevel::Info,
            format!(
                "{} started \u{b7} {name}",
                if repair { "repair" } else { "verify" },
            ),
        );
        self.dialog = Some(DialogState::Verify {
            id,
            tag: Row::Download(Box::new(status)).tag(),
            name,
            repair,
            outcome: None,
        });
    }
}

/// Byte offset of the `index`-th character (or the end of the string).
pub(super) fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}

/// Setup terminal for TUI
fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

type PanicHook = std::sync::Arc<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;

/// Restores the terminal on every return path, including I/O errors and
/// panics.
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
    let _ = execute!(
        io::stdout(),
        crossterm::event::DisableBracketedPaste,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    );
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
