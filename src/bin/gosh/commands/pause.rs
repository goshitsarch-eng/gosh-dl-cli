use anyhow::Result;
use gosh_dl::types::DownloadId;

use crate::app::App;
use crate::cli::PauseArgs;

pub async fn execute(args: PauseArgs, app: &App) -> Result<()> {
    let ids = resolve_ids(&args.ids, app)?;

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

fn resolve_ids(ids: &[String], app: &App) -> Result<Vec<DownloadId>> {
    if ids.len() == 1 && ids[0].to_lowercase() == "all" {
        // Pause all active downloads
        let active = app.engine().active();
        return Ok(active.into_iter().map(|d| d.id).collect());
    }

    ids.iter()
        .map(|s| parse_download_id(s))
        .collect()
}

fn parse_download_id(s: &str) -> Result<DownloadId> {
    // Try parsing as GID first (16 hex chars)
    if let Some(id) = DownloadId::from_gid(s) {
        return Ok(id);
    }

    // Try parsing as full UUID
    if let Ok(uuid) = uuid::Uuid::parse_str(s) {
        return Ok(DownloadId::from_uuid(uuid));
    }

    anyhow::bail!("Invalid download ID: {}", s)
}
