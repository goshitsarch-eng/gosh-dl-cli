use gosh_dl::DownloadStatus;

use crate::commands::add::AddResult;
use crate::format::{format_duration, format_speed, format_state};
use crate::util::truncate_str;

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
            .map(format_duration)
            .unwrap_or_else(|| "--".to_string());
        let state = format_state(&dl.state);
        let name = truncate_str(&dl.metadata.name, 35);

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

    println!("{:<16} {:<10} Input", "ID", "Type");
    println!("{}", "─".repeat(70));

    for result in results {
        println!(
            "{:<16} {:<10} {}",
            result.id,
            result.kind,
            truncate_str(&result.input, 50)
        );
    }

    println!();
    println!("Added {} download(s)", results.len());
}
