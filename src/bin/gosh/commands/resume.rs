use anyhow::Result;
use gosh_dl::{BatchResult, DownloadState};

use crate::app::App;
use crate::cli::{OutputFormat, ResumeArgs};
use crate::util::resolve_download_ids;

pub async fn execute(args: ResumeArgs, app: &App, output: OutputFormat) -> Result<i32> {
    if args.ids.len() == 1 && args.ids[0].eq_ignore_ascii_case("all") {
        return super::batch::resume_all(app, output).await;
    }
    let ids = resolve_download_ids(&args.ids, app.engine(), |d| {
        matches!(d.state, DownloadState::Paused)
    })?;
    let events = app.subscribe();
    let mut result = BatchResult::default();
    for id in ids {
        match app.engine().resume(id).await {
            Ok(()) => result.succeeded.push(id),
            Err(error) => result.failed.push((id, error)),
        }
    }
    let start_code = super::batch::report(&result, &super::batch::RESUME, output)?;
    let code = super::add::wait_for_completion(app, &result.succeeded, events).await?;
    Ok(if code == 0 { start_code } else { code })
}
