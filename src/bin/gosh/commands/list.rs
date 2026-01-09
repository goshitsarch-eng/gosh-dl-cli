use anyhow::Result;
use gosh_dl::types::DownloadStatus;

use crate::app::App;
use crate::cli::{ListArgs, OutputFormat, StateFilter};
use crate::output::table::print_download_table;

pub async fn execute(args: ListArgs, app: &App, output: OutputFormat) -> Result<()> {
    let downloads = match args.state {
        Some(StateFilter::Active) => app.engine().active(),
        Some(StateFilter::Waiting) => app.engine().waiting(),
        Some(StateFilter::Paused) => filter_paused(&app.engine().list()),
        Some(StateFilter::Completed) => filter_completed(&app.engine().stopped()),
        Some(StateFilter::Error) => filter_errors(&app.engine().stopped()),
        None => app.engine().list(),
    };

    if args.ids_only {
        for dl in &downloads {
            println!("{}", dl.id.to_gid());
        }
        return Ok(());
    }

    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&downloads)?);
        }
        OutputFormat::JsonPretty => {
            println!("{}", serde_json::to_string_pretty(&downloads)?);
        }
        OutputFormat::Table => {
            print_download_table(&downloads);
        }
    }

    // Print summary
    if matches!(output, OutputFormat::Table) {
        let stats = app.engine().global_stats();
        println!();
        println!(
            "Total: {} downloads ({} active, {} waiting, {} stopped)",
            downloads.len(),
            stats.num_active,
            stats.num_waiting,
            stats.num_stopped
        );
        if stats.download_speed > 0 || stats.upload_speed > 0 {
            println!(
                "Speed: {} down, {} up",
                format_speed(stats.download_speed),
                format_speed(stats.upload_speed)
            );
        }
    }

    Ok(())
}

fn filter_paused(downloads: &[DownloadStatus]) -> Vec<DownloadStatus> {
    downloads
        .iter()
        .filter(|d| matches!(d.state, gosh_dl::types::DownloadState::Paused))
        .cloned()
        .collect()
}

fn filter_completed(downloads: &[DownloadStatus]) -> Vec<DownloadStatus> {
    downloads
        .iter()
        .filter(|d| matches!(d.state, gosh_dl::types::DownloadState::Completed))
        .cloned()
        .collect()
}

fn filter_errors(downloads: &[DownloadStatus]) -> Vec<DownloadStatus> {
    downloads
        .iter()
        .filter(|d| matches!(d.state, gosh_dl::types::DownloadState::Error { .. }))
        .cloned()
        .collect()
}

fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec == 0 {
        "0 B/s".to_string()
    } else if bytes_per_sec < 1024 {
        format!("{} B/s", bytes_per_sec)
    } else if bytes_per_sec < 1024 * 1024 {
        format!("{:.1} KB/s", bytes_per_sec as f64 / 1024.0)
    } else if bytes_per_sec < 1024 * 1024 * 1024 {
        format!("{:.2} MB/s", bytes_per_sec as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB/s", bytes_per_sec as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
