use gosh_dl::types::{DownloadState, DownloadStatus};
use std::time::Duration;

use crate::commands::add::AddResult;

pub fn print_download_table(downloads: &[DownloadStatus]) {
    if downloads.is_empty() {
        println!("No downloads");
        return;
    }

    // Header
    println!(
        "{:<16} {:<35} {:>8} {:>12} {:>10} {:<12}",
        "ID", "Name", "Progress", "Speed", "ETA", "State"
    );
    println!("{}", "─".repeat(95));

    // Rows
    for dl in downloads {
        let progress = dl.progress.percentage();
        let speed = format_speed(dl.progress.download_speed);
        let eta = dl
            .progress
            .eta_seconds
            .map(|s| format_duration(s))
            .unwrap_or_else(|| "--".to_string());
        let state = format_state(&dl.state);
        let name = truncate(&dl.metadata.name, 35);

        println!(
            "{:<16} {:<35} {:>7.1}% {:>10}/s {:>10} {:<12}",
            dl.id.to_gid(),
            name,
            progress,
            speed,
            eta,
            state
        );
    }
}

pub fn print_add_results(results: &[AddResult]) {
    if results.is_empty() {
        return;
    }

    println!("{:<16} {:<10} {}", "ID", "Type", "Input");
    println!("{}", "─".repeat(70));

    for result in results {
        println!(
            "{:<16} {:<10} {}",
            result.id,
            result.kind,
            truncate(&result.input, 50)
        );
    }

    println!();
    println!("Added {} download(s)", results.len());
}

fn format_state(state: &DownloadState) -> String {
    match state {
        DownloadState::Queued => "Queued".to_string(),
        DownloadState::Connecting => "Connecting".to_string(),
        DownloadState::Downloading => "Downloading".to_string(),
        DownloadState::Seeding => "Seeding".to_string(),
        DownloadState::Paused => "Paused".to_string(),
        DownloadState::Completed => "Completed".to_string(),
        DownloadState::Error { kind, .. } => format!("Error: {}", truncate(kind, 10)),
    }
}

fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec == 0 {
        "0 B".to_string()
    } else if bytes_per_sec < 1024 {
        format!("{} B", bytes_per_sec)
    } else if bytes_per_sec < 1024 * 1024 {
        format!("{:.1} KB", bytes_per_sec as f64 / 1024.0)
    } else if bytes_per_sec < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes_per_sec as f64 / (1024.0 * 1024.0))
    } else {
        format!(
            "{:.2} GB",
            bytes_per_sec as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    }
}

fn format_duration(seconds: u64) -> String {
    if seconds == 0 {
        return "--".to_string();
    }

    let duration = Duration::from_secs(seconds);
    let hours = duration.as_secs() / 3600;
    let minutes = (duration.as_secs() % 3600) / 60;
    let secs = duration.as_secs() % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{}:{:02}", minutes, secs)
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
