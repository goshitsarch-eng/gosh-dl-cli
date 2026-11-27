use super::*;
use crate::tui::model::tests::download;
use crate::tui::model::MirrorRow;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use gosh_dl::{DownloadKind, DownloadStatus, RecursiveJobProgress, RecursiveJobStatus};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn app() -> (TuiApp, TempDir) {
    app_with(|_| {}).await
}

async fn app_with(tweak: impl FnOnce(&mut CliConfig)) -> (TuiApp, TempDir) {
    let dir = TempDir::new().unwrap();
    let mut config = CliConfig::default();
    config.general.storage_backend = crate::config::StorageBackend::None;
    config.general.download_dir = dir.path().join("downloads");
    config.engine.enable_dht = false;
    config.engine.enable_lpd = false;
    config.engine.max_retries = 1;
    tweak(&mut config);
    let app = TuiApp::new(config, Some(dir.path().join("custom.toml")))
        .await
        .unwrap();
    (app, dir)
}

async fn press(app: &mut TuiApp, code: KeyCode, modifiers: KeyModifiers) -> bool {
    app.handle_terminal_event(&Event::Key(KeyEvent::new(code, modifiers)))
        .await
        .unwrap()
}

async fn key(app: &mut TuiApp, code: KeyCode) -> bool {
    press(app, code, KeyModifiers::NONE).await
}

async fn type_text(app: &mut TuiApp, text: &str) {
    for c in text.chars() {
        key(app, KeyCode::Char(c)).await;
    }
}

fn draw(app: &mut TuiApp, width: u16, height: u16) -> Buffer {
    app.terminal_width = width;
    app.terminal_height = height;
    app.detect_layout_mode();
    app.startup_effects_added = true;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| ui::render(f, app)).unwrap();
    terminal.backend().buffer().clone()
}

/// The screen as text, one line per row.
fn render(app: &mut TuiApp, width: u16, height: u16) -> String {
    let buffer = draw(app, width, height);
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

/// Wait (with ticks) until `done` holds.
async fn tick_until(app: &mut TuiApp, what: &str, mut done: impl FnMut(&TuiApp) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.on_tick().await;
        if done(app) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// A tiny HTTP server: `(path, body)` pairs, 404 for everything else.
async fn serve(routes: Vec<(&'static str, &'static str)>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes = Arc::new(routes);
    let server = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let routes = routes.clone();
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buf = [0; 2048];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => request.extend_from_slice(&buf[..n]),
                    }
                }
                let request = String::from_utf8_lossy(&request).to_string();
                let head = request.starts_with("HEAD ");
                let path = request.split_whitespace().nth(1).unwrap_or("");
                let reply = match routes.iter().find(|(route, _)| *route == path) {
                    Some((_, body)) => {
                        let kind = if body.starts_with('<') {
                            "text/html"
                        } else {
                            "application/octet-stream"
                        };
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            if head { "" } else { body }
                        )
                    }
                    None => {
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_string()
                    }
                };
                let _ = stream.write_all(reply.as_bytes()).await;
            });
        }
    });
    (base, server)
}

fn state_of(app: &TuiApp, id: DownloadId) -> DownloadState {
    app.engine.status(id).unwrap().state
}

/// A populated screen without a network: one row of every kind and state.
fn load_demo(app: &mut TuiApp) {
    use gosh_dl::http::ExpectedChecksum;
    let gib = 1024 * 1024 * 1024u64;
    let mib = 1024 * 1024u64;
    let mut rows = Vec::new();

    let mut d = download(
        "ubuntu-26.04-desktop-amd64.iso",
        DownloadKind::Http,
        DownloadState::Downloading,
    );
    d.progress.total_size = Some(6 * gib);
    d.progress.completed_size = 4 * gib + 300 * mib;
    d.progress.download_speed = 42 * mib;
    d.progress.connections = 8;
    d.progress.eta_seconds = Some(41);
    d.metadata.url =
        Some("https://releases.ubuntu.com/26.04/ubuntu-26.04-desktop-amd64.iso".into());
    d.metadata.checksum = Some(ExpectedChecksum::sha256("9f2c".repeat(16)));
    d.metadata.mirrors = vec!["https://mirror.example.org/ubuntu.iso".into()];
    d.metadata.etag = Some("\"abc\"".into());
    rows.push(Row::Download(Box::new(d)));

    let mut d = download(
        "debian-13.2.0-amd64-netinst.iso",
        DownloadKind::Torrent,
        DownloadState::Seeding,
    );
    d.progress.total_size = Some(700 * mib);
    d.progress.completed_size = 700 * mib;
    d.progress.download_speed = 0;
    d.progress.upload_speed = 900 * 1024;
    d.progress.peers = 23;
    d.progress.seeders = 11;
    d.metadata.url = None;
    d.metadata.info_hash = Some("6a9759bffd5c0af65319979fb7832189f4f3c35d".into());
    d.torrent_info = Some(gosh_dl::TorrentStatusInfo {
        files: Vec::new(),
        piece_length: 256 * 1024,
        pieces_count: 2800,
        private: false,
    });
    rows.push(Row::Download(Box::new(d)));

    let children: Vec<DownloadStatus> = [
        ("coreutils-9.7.tar.xz", DownloadState::Completed),
        ("coreutils-9.7.tar.xz.sig", DownloadState::Completed),
        ("coreutils-9.8.tar.xz", DownloadState::Downloading),
        ("coreutils-9.8.tar.xz.sig", DownloadState::Queued),
        ("coreutils-9.9.tar.xz", DownloadState::Queued),
    ]
    .into_iter()
    .map(|(name, state)| {
        let mut c = download(name, DownloadKind::Http, state);
        c.progress.total_size = Some(6 * mib);
        c.progress.completed_size = 3 * mib;
        c.progress.download_speed = 2 * mib;
        c
    })
    .collect();
    rows.push(Row::Mirror(Box::new(MirrorRow {
        id: uuid::Uuid::from_u128(0x5eed),
        root_url: "https://ftp.gnu.org/gnu/coreutils/".into(),
        created_at: chrono::Utc::now(),
        status: RecursiveJobStatus {
            root_url: "https://ftp.gnu.org/gnu/coreutils/".into(),
            child_ids: children.iter().map(|c| c.id).collect(),
            state: RecursiveJobState::Running,
            progress: RecursiveJobProgress {
                total_children: 214,
                queued_children: 150,
                active_children: 4,
                completed_children: 60,
                completed_size: 310 * mib,
                total_size: Some(1100 * mib),
                ..Default::default()
            },
        },
        children,
    })));

    let mut d = download(
        "Big Buck Bunny (2008) 4K",
        DownloadKind::Magnet,
        DownloadState::Connecting,
    );
    d.progress.total_size = None;
    d.progress.completed_size = 0;
    d.progress.download_speed = 0;
    d.metadata.url = None;
    rows.push(Row::Download(Box::new(d)));

    let mut d = download(
        "archlinux-2026.11.01-x86_64.iso",
        DownloadKind::Http,
        DownloadState::Paused,
    );
    d.progress.total_size = Some(gib + 200 * mib);
    d.progress.completed_size = 400 * mib;
    d.progress.download_speed = 0;
    rows.push(Row::Download(Box::new(d)));

    let mut d = download(
        "fedora-workstation-43.iso",
        DownloadKind::Http,
        DownloadState::Queued,
    );
    d.progress.total_size = Some(2 * gib);
    d.progress.completed_size = 0;
    d.progress.download_speed = 0;
    rows.push(Row::Download(Box::new(d)));

    let mut d = download(
        "linux-6.18.tar.xz",
        DownloadKind::Http,
        DownloadState::Completed,
    );
    d.progress.total_size = Some(148 * mib);
    d.progress.completed_size = 148 * mib;
    d.progress.download_speed = 0;
    d.metadata.checksum = Some(ExpectedChecksum::sha256("ab".repeat(32)));
    rows.push(Row::Download(Box::new(d)));

    let mut d = download(
        "dataset-2026-11.parquet",
        DownloadKind::Http,
        DownloadState::Error {
            kind: "Network { kind: HttpStatus(403), message: \"x\", retryable: false }".into(),
            message: "HTTP error: 403 Forbidden".into(),
            retryable: false,
        },
    );
    d.progress.total_size = Some(900 * mib);
    d.progress.completed_size = 120 * mib;
    d.progress.download_speed = 0;
    rows.push(Row::Download(Box::new(d)));

    for (slot, view) in app.counts.iter_mut().zip(ViewMode::ALL) {
        *slot = rows.iter().filter(|row| view.includes(row)).count();
    }
    app.mirror_files = (60, 214);
    app.rows = rows;
    app.selected = 0;
    app.download_speed = 50 * mib;
    app.upload_speed = 900 * 1024;
    app.peak_download_speed = 61 * mib;
    app.peak_upload_speed = 2 * mib;
    app.speed_history.clear();
    for i in 0..SPEED_HISTORY as u64 {
        let wave = ((i as f64 / 7.0).sin() + 1.3) / 2.3;
        let burst = ((i as f64 / 23.0).cos() + 1.2) / 2.2;
        app.speed_history.push_back((
            (wave * burst * 61.0 * mib as f64) as u64,
            ((1.0 - wave * 0.6) * 2.0 * mib as f64) as u64,
        ));
    }
    for (level, message) in [
        (ActivityLevel::Info, "added ubuntu-26.04-desktop-amd64.iso"),
        (
            ActivityLevel::Info,
            "mirror https://ftp.gnu.org/gnu/coreutils/ \u{b7} 214 files queued",
        ),
        (ActivityLevel::Success, "complete linux-6.18.tar.xz"),
        (
            ActivityLevel::Warning,
            "paused archlinux-2026.11.01-x86_64.iso",
        ),
        (
            ActivityLevel::Error,
            "dataset-2026-11.parquet \u{b7} HTTP error: 403 Forbidden",
        ),
        (
            ActivityLevel::Success,
            "verify linux-6.18.tar.xz \u{b7} data ok \u{b7} sha256 matches",
        ),
    ] {
        app.push_activity(level, message.to_string());
    }
}

#[tokio::test]
async fn invalid_add_input_stays_in_tui_and_ctrl_c_always_quits() {
    let (mut app, _dir) = app().await;
    key(&mut app, KeyCode::Char('a')).await;
    type_text(&mut app, "this is not a URL").await;
    assert!(!key(&mut app, KeyCode::Enter).await);
    assert!(matches!(app.dialog, Some(DialogState::Error { .. })));
    assert!(press(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL).await);
    app.engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn control_chords_are_not_shortcuts() {
    let (mut app, _dir) = app().await;
    // Ctrl+A used to open the add dialog, Ctrl+Q used to quit.
    assert!(!press(&mut app, KeyCode::Char('a'), KeyModifiers::CONTROL).await);
    assert!(app.dialog.is_none());
    assert!(!press(&mut app, KeyCode::Char('q'), KeyModifiers::CONTROL).await);
    assert!(!press(&mut app, KeyCode::Char('q'), KeyModifiers::ALT).await);
    assert!(key(&mut app, KeyCode::Char('q')).await);
    app.engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn unicode_add_editing_paste_and_dialog_at_80x24() {
    let (mut app, _dir) = app().await;
    key(&mut app, KeyCode::Char('a')).await;
    type_text(&mut app, "雪éx").await;
    key(&mut app, KeyCode::Left).await;
    key(&mut app, KeyCode::Backspace).await;
    assert!(matches!(
        &app.dialog,
        Some(DialogState::AddUrl { input, cursor, .. }) if input == "雪x" && *cursor == 1
    ));
    // A paste is inserted at the cursor in one go, without control characters.
    app.handle_terminal_event(&Event::Paste("a\nb".into()))
        .await
        .unwrap();
    assert!(matches!(
        &app.dialog,
        Some(DialogState::AddUrl { input, cursor, .. }) if input == "雪abx" && *cursor == 3
    ));
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("Enter URL"), "{screen}");
    assert!(screen.contains("Cancel"), "{screen}");
    assert!(screen.contains("Save paused"), "{screen}");
    key(&mut app, KeyCode::Esc).await;
    key(&mut app, KeyCode::Char('?')).await;
    let help = render(&mut app, 80, 24);
    assert!(help.contains("verify and repair"), "{help}");
    assert!(help.contains("Press any key to close"), "{help}");
    key(&mut app, KeyCode::Char('x')).await;
    assert!(!app.show_help);
    app.engine.shutdown().await.unwrap();
}

#[test]
fn add_dialog_recognises_every_kind_of_input() {
    let kind = |text: &str, single| detect_input(text, single).0;
    assert_eq!(kind("", false), AddKind::Empty);
    assert_eq!(kind("https://example.com/file.iso", false), AddKind::Http);
    assert_eq!(kind("https://example.com/pub/", false), AddKind::Directory);
    assert_eq!(
        kind("https://example.com/pub/?C=M;O=D", false),
        AddKind::Directory,
        "a query string does not hide the trailing slash"
    );
    assert_eq!(
        kind("https://example.com/pub/", true),
        AddKind::Http,
        "Tab turns a directory URL back into one download"
    );
    assert_eq!(
        kind(
            "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567",
            false
        ),
        AddKind::Magnet
    );
    assert_eq!(kind("not a url at all", false), AddKind::Invalid);

    let dir = TempDir::new().unwrap();
    let meta = dir.path().join("files.meta4");
    std::fs::write(&meta, "<metalink/>").unwrap();
    assert_eq!(kind(&meta.display().to_string(), false), AddKind::Metalink);
    let torrent = dir.path().join("x.torrent");
    std::fs::write(&torrent, "d4:infod4:name1:xee").unwrap();
    assert_eq!(
        kind(&torrent.display().to_string(), false),
        AddKind::Torrent
    );
}

#[tokio::test]
async fn settings_save_custom_path_and_keep_draft_on_failure() {
    let (mut app, dir) = app().await;
    key(&mut app, KeyCode::Char('S')).await;
    if let Some(DialogState::Settings { draft, dirty, .. }) = &mut app.dialog {
        draft.tui.theme = "light".into();
        draft.general.file_conflict = crate::config::FileConflict::Rename;
        *dirty = true;
    } else {
        panic!("uppercase shortcut did not open settings");
    }
    key(&mut app, KeyCode::Esc).await;
    assert!(app.dialog.is_none());
    assert!(dir.path().join("custom.toml").exists());
    let saved = CliConfig::load(app.config_path.as_deref()).unwrap();
    assert_eq!(saved.tui.theme, "light");
    assert_eq!(
        saved.general.file_conflict,
        crate::config::FileConflict::Rename
    );
    assert_eq!(
        app.engine.file_conflict_policy(),
        gosh_dl::FileConflictPolicy::Rename,
        "the running engine follows the setting without a restart"
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

#[tokio::test]
async fn settings_are_edited_with_the_keyboard() {
    let (mut app, _dir) = app().await;
    key(&mut app, KeyCode::Char('S')).await;
    key(&mut app, KeyCode::Right).await; // Engine tab, "Max concurrent downloads"
    key(&mut app, KeyCode::Enter).await;
    assert!(matches!(
        &app.dialog,
        Some(DialogState::Settings { editing: Some(text), active_tab: 1, .. }) if text == "5"
    ));
    key(&mut app, KeyCode::Backspace).await;
    key(&mut app, KeyCode::Char('0')).await;
    key(&mut app, KeyCode::Enter).await;
    assert!(
        matches!(
            &app.dialog,
            Some(DialogState::Settings {
                editing: Some(_),
                dirty: false,
                ..
            })
        ),
        "an invalid value keeps the editor open"
    );
    assert!(app
        .toasts
        .last()
        .unwrap()
        .message
        .contains("Invalid setting"));
    key(&mut app, KeyCode::Backspace).await;
    key(&mut app, KeyCode::Char('7')).await;
    key(&mut app, KeyCode::Enter).await;
    assert!(matches!(
        &app.dialog,
        Some(DialogState::Settings { editing: None, dirty: true, draft, .. })
            if draft.engine.max_concurrent_downloads == 7
    ));
    // Global download limit: empty means unlimited.
    for _ in 0..3 {
        key(&mut app, KeyCode::Down).await;
    }
    key(&mut app, KeyCode::Enter).await;
    type_text(&mut app, "2M").await;
    key(&mut app, KeyCode::Enter).await;
    let screen = render(&mut app, 120, 40);
    assert!(screen.contains("2M"), "{screen}");
    assert!(screen.contains("engine.global_download_limit"), "{screen}");
    assert!(screen.contains("unsaved changes"), "{screen}");
    key(&mut app, KeyCode::Esc).await;
    assert!(app.dialog.is_none());
    assert_eq!(app.config.engine.max_concurrent_downloads, 7);
    assert_eq!(
        app.config.engine.global_download_limit,
        Some(2 * 1024 * 1024)
    );
    assert_eq!(app.toasts.last().unwrap().message, "Settings saved");
    app.engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn schedule_rules_are_shown_and_toggled() {
    let (mut app, _dir) = app_with(|config| {
        config.schedule.rules = vec![crate::config::ScheduleRuleConfig {
            start_hour: 0,
            end_hour: 23,
            days: "all".into(),
            download_limit: Some("1M".into()),
            upload_limit: None,
            enabled: true,
        }];
    })
    .await;
    let screen = render(&mut app, 140, 40);
    assert!(
        screen.contains("rule 1"),
        "the status line names the active rule\n{screen}"
    );

    key(&mut app, KeyCode::Char('S')).await;
    key(&mut app, KeyCode::Char('5')).await;
    let screen = render(&mut app, 120, 40);
    assert!(screen.contains("This week"), "{screen}");
    assert!(screen.contains("active now"), "{screen}");
    assert!(screen.contains("[x]"), "{screen}");

    key(&mut app, KeyCode::Enter).await;
    let screen = render(&mut app, 120, 40);
    assert!(screen.contains("[ ]"), "{screen}");
    assert!(!screen.contains("active now"), "{screen}");
    key(&mut app, KeyCode::Esc).await;

    let saved = CliConfig::load(app.config_path.as_deref()).unwrap();
    assert!(!saved.schedule.rules[0].enabled);
    assert!(saved.to_engine_config().schedule_rules.is_empty());
    let screen = render(&mut app, 140, 40);
    assert!(screen.contains("no schedule rule active"), "{screen}");
    app.engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn every_screen_renders_at_every_size() {
    let (mut app, _dir) = app().await;
    let sizes = [
        (1, 1),
        (20, 6),
        (59, 11),
        (60, 12),
        (79, 19),
        (80, 24),
        (100, 24),
        (120, 40),
        (200, 60),
    ];
    for populated in [false, true] {
        if populated {
            load_demo(&mut app);
        }
        for (width, height) in sizes {
            for selected in 0..app.rows.len().max(1) {
                app.selected = selected;
                render(&mut app, width, height);
            }
            app.selected = 0;
            for c in ['a', 'S', 'A', '?', 'c', 'd', 'C', '/'] {
                app.dialog = None;
                app.show_help = false;
                app.search = None;
                key(&mut app, KeyCode::Char(c)).await;
                render(&mut app, width, height);
            }
            app.search = None;
            for tab in 0..settings::TAB_NAMES.len() {
                app.dialog = Some(DialogState::Settings {
                    active_tab: tab,
                    selected_row: 10,
                    editing: (tab == 1).then(|| "a long value being typed".to_string()),
                    draft: Box::new(app.config.clone()),
                    dirty: true,
                });
                render(&mut app, width, height);
            }
            for outcome in [
                None,
                Some(VerifyOutcome::Failed("disk on fire".into())),
                Some(VerifyOutcome::Report(VerifyReport {
                    valid: false,
                    bad_pieces: 3,
                    detail: "3 of 2800 pieces failed".into(),
                })),
            ] {
                app.dialog = Some(DialogState::Verify {
                    id: DownloadId::new(),
                    name: "debian-13.2.0-amd64-netinst.iso".into(),
                    tag: "TORRENT".into(),
                    repair: false,
                    outcome,
                });
                render(&mut app, width, height);
            }
            app.dialog = Some(DialogState::Error {
                message: "a long error\nwith several lines ".repeat(12),
            });
            render(&mut app, width, height);
            app.dialog = Some(DialogState::BatchImport {
                phase: TuiApp::review_batch("https://a.example/x\nnonsense here\n".into()),
            });
            render(&mut app, width, height);
            app.dialog = None;
            app.push_toast(
                "a toast that is rather long, to check truncation".repeat(3),
                ToastLevel::Warning,
            );
            render(&mut app, width, height);
        }
    }

    app.toasts.clear();
    app.dialog = Some(DialogState::Settings {
        active_tab: 1,
        selected_row: 10,
        editing: None,
        draft: Box::new(app.config.clone()),
        dirty: false,
    });
    assert!(render(&mut app, 80, 24).contains("Accept invalid TLS certs"));
    app.engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn wide_and_compact_layouts_show_the_expected_panels() {
    let (mut app, _dir) = app().await;
    let empty = render(&mut app, 120, 36);
    assert!(
        empty.contains("Nothing in this view. Press a to add"),
        "{empty}"
    );

    load_demo(&mut app);
    let wide = render(&mut app, 120, 36);
    for needle in [
        "Downloads 8",
        "1 All 8",
        "2 Active 4",
        "3 Completed 2",
        "4 Mirrors 1",
        "Network",
        "Details HTTP\u{d7}8",
        "Activity",
        "TORRENT",
        "MAGNET",
        "MIRROR",
        "60/214 files",
        "1 mirror (60/214 files)",
        "validated by ETag",
        "1 fallback URL",
        "sha256",
        "HTTP 403",
        "queued \u{b7} normal priority",
    ] {
        assert!(wide.contains(needle), "missing {needle:?} in\n{wide}");
    }

    // The mirror row shows its own details, not a child's.
    app.selected = 2;
    let mirror = render(&mut app, 120, 36);
    assert!(mirror.contains("Details MIRROR"), "{mirror}");
    assert!(mirror.contains("coreutils-9.8.tar.xz"), "{mirror}");
    assert!(mirror.contains("60 done"), "{mirror}");

    // Torrent details name the swarm; hidden when the user turns them off.
    app.selected = 1;
    let torrent = render(&mut app, 120, 36);
    assert!(torrent.contains("23 connected"), "{torrent}");
    assert!(torrent.contains("2800"), "{torrent}");
    app.config.tui.show_peers = false;
    app.config.tui.show_speed_graph = false;
    let quiet = render(&mut app, 120, 36);
    assert!(!quiet.contains("23 connected"), "{quiet}");
    assert!(!quiet.contains("Network"), "{quiet}");
    assert!(quiet.contains("Priority"), "{quiet}");

    let compact = render(&mut app, 80, 24);
    assert!(compact.contains("1 All 8"), "{compact}");
    assert!(compact.contains("3 Done 2"), "{compact}");
    assert!(compact.contains("debian-13.2.0"), "{compact}");
    assert!(!compact.contains("Activity"), "{compact}");

    // Hiding the activity panel gives its lines back to the list.
    key(&mut app, KeyCode::Char('L')).await;
    assert!(!render(&mut app, 120, 36).contains("Activity"));
    app.engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn batch_review_is_portable_and_does_not_report_failed_imports_as_success() {
    let (mut app, dir) = app().await;
    let torrent = dir.path().join("broken.torrent");
    std::fs::write(&torrent, "not valid torrent metadata").unwrap();
    key(&mut app, KeyCode::Char('A')).await;
    // A pasted list keeps its lines; comments and blanks are dropped on review.
    app.handle_terminal_event(&Event::Paste(format!(
        "# my list\r\n{}\r\n\r\nnot a link\r\n",
        torrent.display()
    )))
    .await
    .unwrap();
    key(&mut app, KeyCode::F(2)).await;
    let Some(DialogState::BatchImport {
        phase: BatchPhase::Review { entries, .. },
    }) = &app.dialog
    else {
        panic!("F2 did not open the review");
    };
    assert_eq!(entries.len(), 2);
    assert!(entries[0].valid && entries[0].selected);
    assert!(!entries[1].valid && !entries[1].selected);
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("1/2 selected"), "{screen}");
    assert!(screen.contains("[torrent]"), "{screen}");

    key(&mut app, KeyCode::Enter).await;
    assert_eq!(app.toasts.last().unwrap().message, "Added 0/1 downloads");
    assert!(app.engine.list().is_empty());
    assert!(matches!(app.dialog, Some(DialogState::Error { .. })));
    app.engine.shutdown().await.unwrap();
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
    let (base, server) = serve(vec![("/test.bin", "test")]).await;
    assert!(
        app.add_input(&format!("{base}/test.bin"), false, false, true)
            .await
    );
    assert!(app.dialog.is_none());
    let id = app.engine.list()[0].id;
    tick_until(&mut app, "download", |app| {
        matches!(state_of(app, id), DownloadState::Completed)
    })
    .await;

    // Verify opens a dialog that shows the result when it arrives.
    key(&mut app, KeyCode::Char('v')).await;
    assert!(matches!(
        app.dialog,
        Some(DialogState::Verify {
            outcome: None,
            repair: false,
            ..
        })
    ));
    assert!(render(&mut app, 100, 30).contains("Checking the data on disk"));
    finish_check(&mut app).await;
    assert!(app.activity_log.back().unwrap().message.contains("data ok"));
    assert!(render(&mut app, 100, 30).contains("is intact"));
    key(&mut app, KeyCode::Esc).await;
    assert!(app.dialog.is_none());

    let path = app.config.general.download_dir.join("test.bin");
    tokio::fs::write(&path, b"bad").await.unwrap();
    key(&mut app, KeyCode::Char('v')).await;
    finish_check(&mut app).await;
    assert!(app
        .activity_log
        .back()
        .unwrap()
        .message
        .contains("NOT valid"));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("NOT valid"), "{screen}");
    assert!(screen.contains("Repair"), "{screen}");

    // V inside the result offers the repair, behind a confirmation.
    key(&mut app, KeyCode::Char('V')).await;
    assert!(matches!(
        app.dialog,
        Some(DialogState::ConfirmRepair { .. })
    ));
    key(&mut app, KeyCode::Char('y')).await;
    assert!(matches!(
        app.dialog,
        Some(DialogState::Verify { repair: true, .. })
    ));
    finish_check(&mut app).await;
    assert!(app
        .activity_log
        .back()
        .unwrap()
        .message
        .contains("repair queued"));
    key(&mut app, KeyCode::Esc).await;
    tick_until(&mut app, "repair", |app| {
        matches!(state_of(app, id), DownloadState::Completed)
    })
    .await;
    assert_eq!(tokio::fs::read(path).await.unwrap(), b"test");

    // Closing the dialog early turns the result into a toast.
    key(&mut app, KeyCode::Char('v')).await;
    key(&mut app, KeyCode::Esc).await;
    app.toasts.clear();
    finish_check(&mut app).await;
    assert!(app.toasts.last().unwrap().message.contains("data ok"));

    key(&mut app, KeyCode::Char('/')).await;
    type_text(&mut app, "test").await;
    key(&mut app, KeyCode::Enter).await;
    assert_eq!(app.selected_row().unwrap().id(), RowId::Download(id));
    type_text(&mut app, "/zzz").await;
    assert!(app.rows.is_empty(), "the filter hides what does not match");
    assert!(render(&mut app, 100, 30).contains("Nothing matches"));
    key(&mut app, KeyCode::Esc).await;
    assert_eq!(app.rows.len(), 1);

    key(&mut app, KeyCode::Char('d')).await;
    assert!(render(&mut app, 100, 30).contains("delete its files"));
    key(&mut app, KeyCode::Char('n')).await;
    assert!(app.engine.status(id).is_some());
    key(&mut app, KeyCode::Char('d')).await;
    key(&mut app, KeyCode::Char('y')).await;
    assert!(app.engine.status(id).is_none());
    assert!(app.rows.is_empty());
    app.engine.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn save_paused_adds_without_starting() {
    let (mut app, _dir) = app().await;
    let (base, server) = serve(vec![("/later.bin", "later")]).await;
    key(&mut app, KeyCode::Char('a')).await;
    type_text(&mut app, &format!("{base}/later.bin")).await;
    key(&mut app, KeyCode::F(3)).await;
    assert!(app.dialog.is_none(), "{:?}", app.dialog);
    let id = app.engine.list()[0].id;
    tokio::time::sleep(Duration::from_millis(100)).await;
    app.on_tick().await;
    assert!(matches!(state_of(&app, id), DownloadState::Paused));
    assert!(app
        .toasts
        .last()
        .unwrap()
        .message
        .starts_with("saved paused"));
    assert!(render(&mut app, 120, 36).contains("v verifies data on disk, r resumes"));

    key(&mut app, KeyCode::Char('r')).await;
    tick_until(&mut app, "resumed download", |app| {
        matches!(state_of(app, id), DownloadState::Completed)
    })
    .await;
    assert_eq!(
        std::fs::read(app.config.general.download_dir.join("later.bin")).unwrap(),
        b"later"
    );
    app.engine.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn directory_urls_become_one_mirror_row() {
    let (mut app, _dir) = app().await;
    let (base, server) = serve(vec![
        (
            "/pub/",
            "<html><a href=\"a.bin\">a.bin</a> <a href=\"b.bin\">b.bin</a></html>",
        ),
        ("/pub/a.bin", "aaaa"),
        ("/pub/b.bin", "bbbbbb"),
    ])
    .await;

    key(&mut app, KeyCode::Char('a')).await;
    type_text(&mut app, &format!("{base}/pub/")).await;
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("mirrored recursively"), "{screen}");
    assert!(screen.contains("Single file"), "{screen}");
    key(&mut app, KeyCode::Enter).await;
    // Discovery runs in the background; the interface stays usable.
    assert!(app.dialog.is_none());
    assert_eq!(app.pending_mirrors(), 1);

    tick_until(
        &mut app,
        "mirror to finish",
        |app| matches!(app.rows.first(), Some(row) if row.state() == RowState::Completed),
    )
    .await;
    assert_eq!(app.pending_mirrors(), 0);
    assert_eq!(app.rows.len(), 1, "the two files are folded into the job");
    assert!(matches!(&app.rows[0], Row::Mirror(m) if m.children.len() == 2));
    assert_eq!(app.counts, [1, 0, 1, 1]);
    assert_eq!(app.mirror_files, (2, 2));
    assert_eq!(app.engine.list().len(), 2);

    let screen = render(&mut app, 120, 36);
    assert!(screen.contains("MIRROR"), "{screen}");
    assert!(screen.contains("2 files"), "{screen}");
    assert!(screen.contains("a.bin"), "{screen}");

    // Verifying a whole mirror is not offered; cancelling removes the job.
    key(&mut app, KeyCode::Char('v')).await;
    assert!(app.dialog.is_none());
    key(&mut app, KeyCode::Char('c')).await;
    assert!(render(&mut app, 100, 30).contains("Remove this mirror?"));
    key(&mut app, KeyCode::Char('y')).await;
    assert!(app.rows.is_empty());
    assert!(app.engine.list().is_empty());
    assert!(app.engine.list_recursive_jobs().is_empty());
    let mut found = Vec::new();
    let mut stack = vec![app.config.general.download_dir.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            if entry.path().is_dir() {
                stack.push(entry.path());
            } else {
                found.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    found.sort();
    assert_eq!(found, ["a.bin", "b.bin"], "c keeps the files");
    app.engine.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn views_keep_the_selection_and_reordering_sticks() {
    let (mut app, _dir) = app().await;
    let (base, server) = serve(vec![("/a.bin", "a"), ("/b.bin", "b"), ("/c.bin", "c")]).await;
    for name in ["a", "b", "c"] {
        assert!(
            app.add_input(&format!("{base}/{name}.bin"), false, true, false)
                .await
        );
    }
    let names = |app: &TuiApp| app.rows.iter().map(Row::name).collect::<Vec<_>>();
    assert_eq!(names(&app), ["a.bin", "b.bin", "c.bin"]);

    key(&mut app, KeyCode::Home).await;
    key(&mut app, KeyCode::Char('J')).await;
    assert_eq!(names(&app), ["b.bin", "a.bin", "c.bin"]);
    assert_eq!(app.selected, 1, "the selection follows the moved row");
    app.on_tick().await;
    assert_eq!(
        names(&app),
        ["b.bin", "a.bin", "c.bin"],
        "a refresh keeps the order"
    );

    key(&mut app, KeyCode::Char('2')).await;
    assert!(
        app.rows.is_empty(),
        "nothing is active: all three are paused"
    );
    key(&mut app, KeyCode::Tab).await;
    assert_eq!(app.mode, ViewMode::Completed);
    key(&mut app, KeyCode::Char('1')).await;
    assert_eq!(app.rows.len(), 3);

    key(&mut app, KeyCode::Char('C')).await;
    assert!(matches!(app.dialog, Some(DialogState::ConfirmCancelAll)));
    key(&mut app, KeyCode::Enter).await;
    assert!(app.rows.is_empty());
    app.engine.shutdown().await.unwrap();
    server.abort();
}

// ---------------------------------------------------------------------
// README screenshots:
//   GOSH_SCREENSHOT_DIR=screenshots cargo test screenshot -- --ignored
// renders the demo screen to SVG.
// ---------------------------------------------------------------------

fn hex(color: Color, fallback: Color) -> String {
    match (color, fallback) {
        (Color::Rgb(r, g, b), _) | (_, Color::Rgb(r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "#000000".to_string(),
    }
}

fn svg(buffer: &Buffer, theme: &Theme) -> String {
    const W: f64 = 9.0;
    const H: f64 = 19.0;
    let (cols, rows) = (buffer.area.width, buffer.area.height);
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" font-family=\"ui-monospace, 'DejaVu Sans Mono', Menlo, Consolas, monospace\" font-size=\"15\">\n<rect width=\"100%\" height=\"100%\" rx=\"8\" fill=\"{}\"/>\n",
        f64::from(cols) * W + 16.0,
        f64::from(rows) * H + 16.0,
        hex(theme.bg, theme.bg)
    );
    for y in 0..rows {
        for x in 0..cols {
            let cell = &buffer[(x, y)];
            let style = cell.style();
            let mut fg = style.fg.unwrap_or(Color::Reset);
            let mut bg = style.bg.unwrap_or(Color::Reset);
            if style.add_modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let px = 8.0 + f64::from(x) * W;
            let py = 8.0 + f64::from(y) * H;
            let fg = hex(fg, theme.text);
            let opacity = if style.add_modifier.contains(Modifier::DIM) {
                " opacity=\"0.45\""
            } else {
                ""
            };
            if bg != Color::Reset && bg != theme.bg {
                out.push_str(&format!(
                    "<rect x=\"{px}\" y=\"{py}\" width=\"{W}\" height=\"{H}\" fill=\"{}\"{opacity}/>\n",
                    hex(bg, theme.bg)
                ));
            }
            let Some(ch) = cell.symbol().chars().next() else {
                continue;
            };
            // Block and line glyphs are drawn as shapes so they join up
            // whatever font the viewer has.
            if ('\u{2581}'..='\u{2588}').contains(&ch) {
                let h = H * f64::from(ch as u32 - 0x2580) / 8.0;
                out.push_str(&format!(
                    "<rect x=\"{px}\" y=\"{}\" width=\"{W}\" height=\"{h}\" fill=\"{fg}\"{opacity}/>\n",
                    py + H - h
                ));
            } else if ch == '\u{2501}' || ch == '\u{2500}' {
                let thickness = if ch == '\u{2501}' { 3.0 } else { 1.0 };
                out.push_str(&format!(
                    "<rect x=\"{px}\" y=\"{}\" width=\"{W}\" height=\"{thickness}\" fill=\"{fg}\"{opacity}/>\n",
                    py + (H - thickness) / 2.0
                ));
            } else if ch == '\u{2502}' {
                out.push_str(&format!(
                    "<rect x=\"{}\" y=\"{py}\" width=\"1\" height=\"{H}\" fill=\"{fg}\"{opacity}/>\n",
                    px + W / 2.0 - 0.5
                ));
            } else if ch != ' ' {
                let weight = if style.add_modifier.contains(Modifier::BOLD) {
                    " font-weight=\"bold\""
                } else {
                    ""
                };
                let decoration = if style.add_modifier.contains(Modifier::UNDERLINED) {
                    " text-decoration=\"underline\""
                } else {
                    ""
                };
                let escaped = match ch {
                    '<' => "&lt;".to_string(),
                    '>' => "&gt;".to_string(),
                    '&' => "&amp;".to_string(),
                    other => other.to_string(),
                };
                out.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"{fg}\"{weight}{decoration}{opacity}>{escaped}</text>\n",
                    px + W / 2.0,
                    py + 14.0
                ));
            }
        }
    }
    out.push_str("</svg>\n");
    out
}

#[tokio::test]
#[ignore = "writes the README screenshots; needs GOSH_SCREENSHOT_DIR"]
async fn screenshot() {
    let Some(dir) = std::env::var_os("GOSH_SCREENSHOT_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let (mut app, _tmp) = app_with(|config| {
        config.schedule.rules = vec![
            crate::config::ScheduleRuleConfig {
                start_hour: 9,
                end_hour: 17,
                days: "weekdays".into(),
                download_limit: Some("2M".into()),
                upload_limit: Some("256K".into()),
                enabled: true,
            },
            crate::config::ScheduleRuleConfig {
                start_hour: 0,
                end_hour: 23,
                days: "all".into(),
                download_limit: Some("60M".into()),
                upload_limit: Some("2M".into()),
                enabled: true,
            },
        ];
    })
    .await;
    // Show a home-relative folder instead of the temporary one.
    app.config.general.download_dir = dirs::home_dir().unwrap_or_default().join("Downloads");
    app.config.general.storage_backend = crate::config::StorageBackend::Sqlite;
    app.theme = Theme::mocha();
    app.activity_log.clear();
    load_demo(&mut app);
    // Home-relative paths, so the pictures do not show a temporary folder.
    let home = dirs::home_dir().unwrap_or_default();
    app.config_path = Some(home.join(".config/gosh-dl/config.toml"));
    for row in &mut app.rows {
        match row {
            Row::Download(d) => d.metadata.save_dir = home.join("Downloads"),
            Row::Mirror(m) => {
                for child in &mut m.children {
                    child.metadata.save_dir = home.join("Downloads/coreutils");
                }
            }
        }
    }
    let theme = Theme::mocha();

    let wide = draw(&mut app, 132, 38);
    std::fs::write(dir.join("tui.svg"), svg(&wide, &theme)).unwrap();
    let compact = draw(&mut app, 80, 24);
    std::fs::write(dir.join("tui-compact.svg"), svg(&compact, &theme)).unwrap();

    app.dialog = Some(DialogState::Settings {
        active_tab: settings::SCHEDULE_TAB,
        selected_row: 0,
        editing: None,
        draft: Box::new(app.config.clone()),
        dirty: false,
    });
    let schedule = draw(&mut app, 132, 38);
    std::fs::write(dir.join("tui-schedule.svg"), svg(&schedule, &theme)).unwrap();

    app.dialog = Some(DialogState::AddUrl {
        input: "https://ftp.gnu.org/gnu/coreutils/".into(),
        cursor: 34,
        single_file: false,
    });
    let add = draw(&mut app, 132, 38);
    std::fs::write(dir.join("tui-add.svg"), svg(&add, &theme)).unwrap();
    app.engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn demo_screens_draw() {
    // `GOSH_TUI_DUMP=1 cargo test demo_screens_draw -- --nocapture` prints
    // the demo screens as plain text; otherwise this only checks they draw.
    let (mut app, _dir) = app().await;
    load_demo(&mut app);
    let dump = std::env::var_os("GOSH_TUI_DUMP").is_some();
    for (width, height) in [(132, 38), (80, 24)] {
        let screen = render(&mut app, width, height);
        if dump {
            println!("{screen}");
        }
    }
    for c in ['?', 'a', 'S'] {
        app.dialog = None;
        app.show_help = false;
        key(&mut app, KeyCode::Char(c)).await;
        let screen = render(&mut app, 132, 38);
        if dump {
            println!("{screen}");
        }
    }
    app.engine.shutdown().await.unwrap();
}
