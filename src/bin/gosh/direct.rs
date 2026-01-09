//! Direct download mode - aria2-style CLI usage
//!
//! Allows running `gosh URL [URL2 URL3...]` to download files directly
//! with progress bars, without entering the TUI.

use anyhow::{bail, Result};
use gosh_dl::types::{DownloadEvent, DownloadId, DownloadOptions, DownloadState};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::app::App;
use crate::config::CliConfig;
use crate::input::url_parser::{parse_input, ParsedInput};

/// Options for direct download mode
pub struct DirectOptions {
    pub urls: Vec<String>,
    pub dir: Option<PathBuf>,
    pub out: Option<String>,
    pub headers: Vec<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub cookies: Vec<String>,
    pub checksum: Option<String>,
    pub max_connections: Option<usize>,
    pub max_speed: Option<String>,
    pub sequential: bool,
    pub select_files: Option<String>,
    pub seed_ratio: Option<f64>,
}

/// Exit codes for direct download mode
pub mod exit_codes {
    pub const SUCCESS: i32 = 0;
    pub const PARTIAL_FAILURE: i32 = 1;
    pub const TOTAL_FAILURE: i32 = 2;
    pub const INTERRUPTED: i32 = 130;
}

/// State tracking for each download
struct DownloadInfo {
    name: String,
    progress_bar: ProgressBar,
    completed: bool,
    failed: bool,
}

/// Execute direct download mode for the given URLs
pub async fn execute(opts: DirectOptions, config: CliConfig) -> Result<()> {
    if opts.urls.is_empty() {
        bail!("No URLs provided");
    }

    // Validate: can't use -o with multiple downloads
    if opts.out.is_some() && opts.urls.len() > 1 {
        bail!("Cannot use -o/--out with multiple downloads");
    }

    // Parse all inputs first to fail fast on invalid URLs
    let inputs: Vec<ParsedInput> = opts
        .urls
        .iter()
        .map(|u| parse_input(u))
        .collect::<Result<_>>()?;

    // Initialize the download engine
    let app = App::new(config).await?;

    // Setup Ctrl+C handler
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_clone = interrupted.clone();
    let ctrl_c_task = tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        interrupted_clone.store(true, Ordering::SeqCst);
    });

    // Setup multi-progress bar
    let multi = MultiProgress::new();

    let bar_style = ProgressStyle::with_template(
        "{spinner:.green} {msg:<40} [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) ETA: {eta}",
    )?
    .progress_chars("=> ");

    let spinner_style =
        ProgressStyle::with_template("{spinner:.green} {msg:<40} {bytes} ({bytes_per_sec})")?;

    // Add downloads and create progress bars
    let mut downloads: HashMap<DownloadId, DownloadInfo> = HashMap::new();
    let mut failed_to_add = 0;

    for input in &inputs {
        let pb = multi.add(ProgressBar::new(0));
        pb.set_style(spinner_style.clone());
        pb.set_message(truncate_name(&input.display(), 40));
        pb.enable_steady_tick(Duration::from_millis(100));

        let options = build_options(&opts, input)?;

        let result = match input {
            ParsedInput::Http(url) => app.engine().add_http(url, options).await,
            ParsedInput::Magnet(uri) => app.engine().add_magnet(uri, options).await,
            ParsedInput::TorrentFile(path) => match tokio::fs::read(path).await {
                Ok(data) => app.engine().add_torrent(&data, options).await,
                Err(e) => Err(e.into()),
            },
        };

        match result {
            Ok(id) => {
                downloads.insert(
                    id,
                    DownloadInfo {
                        name: input.display(),
                        progress_bar: pb,
                        completed: false,
                        failed: false,
                    },
                );
            }
            Err(e) => {
                pb.abandon_with_message(format!("Failed: {}", truncate_name(&e.to_string(), 35)));
                failed_to_add += 1;
            }
        }
    }

    if downloads.is_empty() {
        app.shutdown().await?;
        eprintln!("All downloads failed to start");
        std::process::exit(exit_codes::TOTAL_FAILURE);
    }

    // Subscribe to events and monitor progress
    let mut events = app.subscribe();
    let download_ids: HashSet<DownloadId> = downloads.keys().copied().collect();

    loop {
        // Check if all downloads are done
        if downloads.values().all(|d| d.completed || d.failed) {
            break;
        }

        // Check for interrupt
        if interrupted.load(Ordering::SeqCst) {
            // Cancel all active downloads
            for id in &download_ids {
                let _ = app.engine().cancel(*id, false).await;
            }
            for info in downloads.values() {
                if !info.completed && !info.failed {
                    info.progress_bar.abandon_with_message("Interrupted");
                }
            }
            ctrl_c_task.abort();
            app.shutdown().await?;
            std::process::exit(exit_codes::INTERRUPTED);
        }

        // Process events with timeout
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                // Cancel all active downloads
                for id in &download_ids {
                    let _ = app.engine().cancel(*id, false).await;
                }
                for info in downloads.values() {
                    if !info.completed && !info.failed {
                        info.progress_bar.abandon_with_message("Interrupted");
                    }
                }
                ctrl_c_task.abort();
                app.shutdown().await?;
                std::process::exit(exit_codes::INTERRUPTED);
            }
            event = events.recv() => {
                match event {
                    Ok(DownloadEvent::Progress { id, progress }) if download_ids.contains(&id) => {
                        if let Some(info) = downloads.get_mut(&id) {
                            if let Some(total) = progress.total_size {
                                if info.progress_bar.length() != Some(total) {
                                    info.progress_bar.set_length(total);
                                    info.progress_bar.set_style(bar_style.clone());
                                }
                            }
                            info.progress_bar.set_position(progress.completed_size);
                        }
                    }
                    Ok(DownloadEvent::Completed { id }) if download_ids.contains(&id) => {
                        if let Some(info) = downloads.get_mut(&id) {
                            info.completed = true;
                            info.progress_bar
                                .finish_with_message(format!("{} - Done", truncate_name(&info.name, 33)));
                        }
                    }
                    Ok(DownloadEvent::Failed { id, error, .. }) if download_ids.contains(&id) => {
                        if let Some(info) = downloads.get_mut(&id) {
                            info.failed = true;
                            info.progress_bar
                                .abandon_with_message(format!("Failed: {}", truncate_name(&error, 32)));
                        }
                    }
                    Ok(DownloadEvent::StateChanged { id, new_state, .. })
                        if download_ids.contains(&id) =>
                    {
                        if let Some(info) = downloads.get_mut(&id) {
                            match new_state {
                                DownloadState::Connecting => {
                                    info.progress_bar.set_message(format!(
                                        "{} - Connecting...",
                                        truncate_name(&info.name, 25)
                                    ));
                                }
                                DownloadState::Downloading => {
                                    info.progress_bar
                                        .set_message(truncate_name(&info.name, 40));
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => break, // Channel closed
                    _ => continue,
                }
            }
        }
    }

    ctrl_c_task.abort();

    // Shutdown engine gracefully
    app.shutdown().await?;

    // Determine exit code
    let completed_count = downloads.values().filter(|d| d.completed).count();
    let failed_count = downloads.values().filter(|d| d.failed).count() + failed_to_add;
    let total = inputs.len();

    if failed_count == 0 {
        std::process::exit(exit_codes::SUCCESS);
    } else if completed_count > 0 {
        eprintln!(
            "\n{}/{} downloads completed, {} failed",
            completed_count, total, failed_count
        );
        std::process::exit(exit_codes::PARTIAL_FAILURE);
    } else {
        eprintln!("\nAll {} downloads failed", total);
        std::process::exit(exit_codes::TOTAL_FAILURE);
    }
}

/// Build download options from direct mode CLI options
fn build_options(opts: &DirectOptions, input: &ParsedInput) -> Result<DownloadOptions> {
    let mut options = DownloadOptions::default();

    if let Some(ref dir) = opts.dir {
        options.save_dir = Some(dir.clone());
    }

    if let Some(ref name) = opts.out {
        options.filename = Some(name.clone());
    }

    if let Some(ref ua) = opts.user_agent {
        options.user_agent = Some(ua.clone());
    }

    if let Some(ref referer) = opts.referer {
        options.referer = Some(referer.clone());
    }

    // Parse headers
    for header in &opts.headers {
        if let Some((name, value)) = header.split_once(':') {
            options
                .headers
                .push((name.trim().to_string(), value.trim().to_string()));
        }
    }

    // Parse cookies
    if !opts.cookies.is_empty() {
        options.cookies = Some(opts.cookies.clone());
    }

    // Parse checksum
    if let Some(ref checksum) = opts.checksum {
        options.checksum = Some(parse_checksum(checksum)?);
    }

    if let Some(max_conn) = opts.max_connections {
        options.max_connections = Some(max_conn);
    }

    if let Some(ref speed) = opts.max_speed {
        options.max_download_speed = Some(parse_speed(speed)?);
    }

    // Torrent-specific options
    if matches!(input, ParsedInput::Magnet(_) | ParsedInput::TorrentFile(_)) {
        if opts.sequential {
            options.sequential = Some(true);
        }

        if let Some(ref files) = opts.select_files {
            let indices: Vec<usize> = files
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if !indices.is_empty() {
                options.selected_files = Some(indices);
            }
        }

        if let Some(ratio) = opts.seed_ratio {
            options.seed_ratio = Some(ratio);
        }
    }

    Ok(options)
}

fn parse_checksum(s: &str) -> Result<gosh_dl::http::ExpectedChecksum> {
    if let Some(hash) = s.strip_prefix("md5:") {
        Ok(gosh_dl::http::ExpectedChecksum::md5(hash.to_string()))
    } else if let Some(hash) = s.strip_prefix("sha256:") {
        Ok(gosh_dl::http::ExpectedChecksum::sha256(hash.to_string()))
    } else {
        bail!("Invalid checksum format. Use 'md5:HASH' or 'sha256:HASH'")
    }
}

fn parse_speed(s: &str) -> Result<u64> {
    let s = s.trim().to_uppercase();

    if let Some(num) = s.strip_suffix('K') {
        Ok(num.parse::<u64>()? * 1024)
    } else if let Some(num) = s.strip_suffix('M') {
        Ok(num.parse::<u64>()? * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix('G') {
        Ok(num.parse::<u64>()? * 1024 * 1024 * 1024)
    } else {
        Ok(s.parse()?)
    }
}

fn truncate_name(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
