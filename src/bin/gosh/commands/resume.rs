use anyhow::Result;
use gosh_dl::types::{DownloadId, DownloadState};

use crate::app::App;
use crate::cli::ResumeArgs;

pub async fn execute(args: ResumeArgs, app: &App) -> Result<()> {
    let ids = resolve_ids(&args.ids, app)?;

    let mut success_count = 0;
    let mut error_count = 0;

    for id in ids {
        match app.engine().resume(id).await {
            Ok(_) => {
                println!("Resumed: {}", id.to_gid());
                success_count += 1;
            }
            Err(e) => {
                eprintln!("Failed to resume {}: {}", id.to_gid(), e);
                error_count += 1;
            }
        }
    }

    if success_count > 0 {
        println!("Successfully resumed {} download(s)", success_count);
    }

    if error_count > 0 {
        anyhow::bail!("Failed to resume {} download(s)", error_count);
    }

    Ok(())
}

fn resolve_ids(ids: &[String], app: &App) -> Result<Vec<DownloadId>> {
    if ids.len() == 1 && ids[0].to_lowercase() == "all" {
        // Resume all paused downloads
        let all = app.engine().list();
        return Ok(all
            .into_iter()
            .filter(|d| matches!(d.state, DownloadState::Paused))
            .map(|d| d.id)
            .collect());
    }

    ids.iter()
        .map(|s| parse_download_id(s))
        .collect()
}

fn parse_download_id(s: &str) -> Result<DownloadId> {
    if let Some(id) = DownloadId::from_gid(s) {
        return Ok(id);
    }

    if let Ok(uuid) = uuid::Uuid::parse_str(s) {
        return Ok(DownloadId::from_uuid(uuid));
    }

    anyhow::bail!("Invalid download ID: {}", s)
}
