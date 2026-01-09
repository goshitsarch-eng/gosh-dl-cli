use anyhow::Result;
use gosh_dl::types::DownloadId;

use crate::app::App;
use crate::cli::PriorityArgs;

pub async fn execute(args: PriorityArgs, app: &App) -> Result<()> {
    let id = parse_download_id(&args.id)?;
    let priority = args.priority.to_engine_priority();

    app.engine().set_priority(id, priority)?;

    println!(
        "Set priority of {} to {:?}",
        id.to_gid(),
        args.priority
    );

    Ok(())
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
