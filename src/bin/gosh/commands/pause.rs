use anyhow::Result;
use gosh_dl::DownloadState;

use crate::app::App;
use crate::cli::{OutputFormat, PauseArgs};
use crate::util::resolve_download_ids;

pub async fn execute(args: PauseArgs, app: &App) -> Result<()> {
    // "all" delegates to the engine's batch API, which also holds queued
    // downloads so they don't get promoted into freed slots
    if args.ids.len() == 1 && args.ids[0].eq_ignore_ascii_case("all") {
        let result = app.engine().pause_all().await;
        let code = super::batch::report(&result, &super::batch::PAUSE, OutputFormat::Table)?;
        if code != 0 {
            anyhow::bail!("Failed to pause {} download(s)", result.failed.len());
        }
        return Ok(());
    }

    let ids = resolve_download_ids(&args.ids, app.engine(), |d| {
        matches!(
            d.state,
            DownloadState::Downloading | DownloadState::Seeding | DownloadState::Connecting
        )
    })?;

    let mut success_count = 0;
    let mut error_count = 0;

    for id in ids {
        match app.engine().pause(id).await {
            Ok(_) => {
                println!("Paused: {}", id.to_gid());
                success_count += 1;
            }
            Err(e) => {
                eprintln!("Failed to pause {}: {}", id.to_gid(), e);
                error_count += 1;
            }
        }
    }

    if success_count > 0 {
        println!("Successfully paused {} download(s)", success_count);
    }

    if error_count > 0 {
        anyhow::bail!("Failed to pause {} download(s)", error_count);
    }

    Ok(())
}
