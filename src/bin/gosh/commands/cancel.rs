use anyhow::Result;
use gosh_dl::types::DownloadId;
use std::io::{self, Write};

use crate::app::App;
use crate::cli::CancelArgs;

pub async fn execute(args: CancelArgs, app: &App) -> Result<()> {
    let ids = resolve_ids(&args.ids, app)?;

    if ids.is_empty() {
        println!("No downloads to cancel");
        return Ok(());
    }

    // Confirm unless --yes is specified
    if !args.yes {
        let action = if args.delete {
            "cancel and DELETE FILES for"
        } else {
            "cancel"
        };

        print!(
            "Are you sure you want to {} {} download(s)? [y/N] ",
            action,
            ids.len()
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    let mut success_count = 0;
    let mut error_count = 0;

    for id in ids {
        match app.engine().cancel(id, args.delete).await {
            Ok(_) => {
                if args.delete {
                    println!("Cancelled and deleted: {}", id.to_gid());
                } else {
                    println!("Cancelled: {}", id.to_gid());
                }
                success_count += 1;
            }
            Err(e) => {
                eprintln!("Failed to cancel {}: {}", id.to_gid(), e);
                error_count += 1;
            }
        }
    }

    if success_count > 0 {
        println!("Successfully cancelled {} download(s)", success_count);
    }

    if error_count > 0 {
        anyhow::bail!("Failed to cancel {} download(s)", error_count);
    }

    Ok(())
}

fn resolve_ids(ids: &[String], app: &App) -> Result<Vec<DownloadId>> {
    if ids.len() == 1 && ids[0].to_lowercase() == "all" {
        // Cancel all downloads
        let all = app.engine().list();
        return Ok(all.into_iter().map(|d| d.id).collect());
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
