use anyhow::Result;
use gosh_dl::DownloadStatus;

use crate::app::App;
use crate::cli::{ListArgs, OutputFormat, StateFilter};
use crate::format::format_speed;
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
        for line in format_summary(
            downloads.len(),
            stats.num_active,
            stats.num_waiting,
            stats.num_stopped,
            stats.download_speed,
            stats.upload_speed,
        ) {
            println!("{line}");
        }
    }

    Ok(())
}

fn filter_paused(downloads: &[DownloadStatus]) -> Vec<DownloadStatus> {
    downloads
        .iter()
        .filter(|d| matches!(d.state, gosh_dl::DownloadState::Paused))
        .cloned()
        .collect()
}

fn filter_completed(downloads: &[DownloadStatus]) -> Vec<DownloadStatus> {
    downloads
        .iter()
        .filter(|d| matches!(d.state, gosh_dl::DownloadState::Completed))
        .cloned()
        .collect()
}

fn filter_errors(downloads: &[DownloadStatus]) -> Vec<DownloadStatus> {
    downloads
        .iter()
        .filter(|d| matches!(d.state, gosh_dl::DownloadState::Error { .. }))
        .cloned()
        .collect()
}

fn format_summary(
    shown_count: usize,
    active_count: usize,
    waiting_count: usize,
    stopped_count: usize,
    download_speed: u64,
    upload_speed: u64,
) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        format!("Showing {} download(s)", shown_count),
        format!(
            "Global totals: {} active, {} waiting, {} stopped",
            active_count, waiting_count, stopped_count
        ),
    ];

    if download_speed > 0 || upload_speed > 0 {
        lines.push(format!(
            "Speed: {}/s down, {}/s up",
            format_speed(download_speed),
            format_speed(upload_speed)
        ));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_wording_separates_filtered_and_global_counts() {
        let summary = format_summary(2, 5, 1, 9, 0, 0);

        assert!(summary.iter().any(|line| line == "Showing 2 download(s)"));
        assert!(summary
            .iter()
            .any(|line| line == "Global totals: 5 active, 1 waiting, 9 stopped"));
    }
}
