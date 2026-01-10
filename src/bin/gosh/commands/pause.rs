use anyhow::Result;
use gosh_dl::types::DownloadState;

use crate::app::App;
use crate::cli::PauseArgs;
use crate::util::resolve_download_ids;

pub async fn execute(args: PauseArgs, app: &App) -> Result<()> {
    // For "all", pause only active downloads (downloading/seeding)
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
