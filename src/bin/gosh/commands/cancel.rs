use anyhow::Result;
use std::io::{self, Write};

use crate::app::App;
use crate::cli::CancelArgs;
use crate::util::resolve_download_ids;

pub async fn execute(args: CancelArgs, app: &App) -> Result<()> {
    // For "all", cancel all downloads
    let ids = resolve_download_ids(&args.ids, app.engine(), |_| true)?;

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
