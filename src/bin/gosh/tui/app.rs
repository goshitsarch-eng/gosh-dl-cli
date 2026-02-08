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

use crate::config::CliConfig;

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

    /// Last update timestamp (reserved for future use)
    #[allow(dead_code)]
    last_update: Instant,

    /// Global download speed
    pub download_speed: u64,

    /// Global upload speed
    pub upload_speed: u64,

    /// Should quit
    should_quit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    All,
    Active,
    Completed,
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
            last_update: Instant::now(),
            download_speed: 0,
            upload_speed: 0,
            should_quit: false,
        })
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Run the TUI event loop
    pub async fn run(&mut self) -> Result<()> {
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
                AppEvent::Resize(_, _) => {
                    // Terminal will redraw on next iteration
                }
            }

            if self.should_quit {
                break;
            }
        }

        // Restore terminal
        restore_terminal(terminal)?;

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
                        match key.code {
                            crossterm::event::KeyCode::Char(c) => {
                                input.insert(*cursor, c);
                                *cursor += 1;
                            }
                            crossterm::event::KeyCode::Backspace => {
                                if *cursor > 0 {
                                    *cursor -= 1;
                                    input.remove(*cursor);
                                }
                            }
                            crossterm::event::KeyCode::Left => {
                                if *cursor > 0 {
                                    *cursor -= 1;
                                }
                            }
                            crossterm::event::KeyCode::Right => {
                                if *cursor < input.len() {
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
            }
        }

        // Handle help overlay
        if self.show_help {
            if event::is_escape(event) || event::is_key(event, '?') || event::is_key(event, 'q') {
                self.show_help = false;
            }
            return Ok(false);
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
            for _ in 0..10 {
                self.select_prev();
            }
        } else if event::is_page_down(event) {
            for _ in 0..10 {
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

        Ok(false)
    }

    /// Handle engine events
    fn handle_engine_event(&mut self, event: DownloadEvent) {
        match event {
            DownloadEvent::Added { .. }
            | DownloadEvent::Removed { .. }
            | DownloadEvent::Completed { .. }
            | DownloadEvent::Failed { .. } => {
                self.refresh_downloads();
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
                if let Some(dl) = self.downloads.iter_mut().find(|d| d.id == id) {
                    dl.state = DownloadState::Paused;
                }
            }
            DownloadEvent::Resumed { id } => {
                if let Some(dl) = self.downloads.iter_mut().find(|d| d.id == id) {
                    dl.state = DownloadState::Downloading;
                }
            }
            _ => {}
        }
    }

    /// Update global stats
    fn update_stats(&mut self) {
        let stats = self.engine.global_stats();
        self.download_speed = stats.download_speed;
        self.upload_speed = stats.upload_speed;

        // Update speed history
        self.speed_history
            .push_back((stats.download_speed, stats.upload_speed));
        while self.speed_history.len() > 60 {
            self.speed_history.pop_front();
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
