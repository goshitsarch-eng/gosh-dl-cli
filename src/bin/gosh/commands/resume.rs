use anyhow::Result;
use gosh_dl::DownloadState;

use crate::app::App;
use crate::cli::{OutputFormat, ResumeArgs};
use crate::util::resolve_download_ids;

pub async fn execute(args: ResumeArgs, app: &App) -> Result<()> {
    // "all" delegates to the engine's batch API
    if args.ids.len() == 1 && args.ids[0].eq_ignore_ascii_case("all") {
        let result = app.engine().resume_all().await;
        let code = super::batch::report(&result, &super::batch::RESUME, OutputFormat::Table)?;
        if code != 0 {
            anyhow::bail!("Failed to resume {} download(s)", result.failed.len());
        }
        return Ok(());
    }

    let ids = resolve_download_ids(&args.ids, app.engine(), |d| {
        matches!(d.state, DownloadState::Paused)
    })?;

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
