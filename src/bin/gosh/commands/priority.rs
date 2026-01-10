use anyhow::Result;

use crate::app::App;
use crate::cli::PriorityArgs;
use crate::util::resolve_download_id;

pub async fn execute(args: PriorityArgs, app: &App) -> Result<()> {
    let id = resolve_download_id(&args.id, app.engine())?;
    let priority = args.priority.to_engine_priority();

    app.engine().set_priority(id, priority)?;

    println!(
        "Set priority of {} to {:?}",
        id.to_gid(),
        args.priority
    );

    Ok(())
}
