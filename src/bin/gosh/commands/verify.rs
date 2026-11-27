//! `gosh verify` and `gosh repair`: the engine's on-disk integrity checks.

use anyhow::Result;
use gosh_dl::VerifyReport;
use serde::Serialize;

use crate::app::App;
use crate::cli::{OutputFormat, VerifyArgs};
use crate::util::{exit_codes, resolve_download_id};

#[derive(Serialize)]
struct Report<'a> {
    id: String,
    name: &'a str,
    valid: bool,
    bad_pieces: usize,
    detail: &'a str,
    /// Set by `repair`: whether a new transfer was started for bad data.
    #[serde(skip_serializing_if = "Option::is_none")]
    repair_started: Option<bool>,
}

fn print_report(
    output: OutputFormat,
    id: gosh_dl::DownloadId,
    name: &str,
    report: &VerifyReport,
    repair_started: Option<bool>,
) -> Result<()> {
    let json = Report {
        id: id.to_gid(),
        name,
        valid: report.valid,
        bad_pieces: report.bad_pieces,
        detail: &report.detail,
        repair_started,
    };
    match output {
        OutputFormat::Json => println!("{}", serde_json::to_string(&json)?),
        OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&json)?),
        OutputFormat::Table => {
            println!("Download: {} ({})", name, id.to_gid());
            println!(
                "Result:   {}",
                if report.valid { "valid" } else { "NOT valid" }
            );
            if report.bad_pieces > 0 {
                println!("Bad:      {} piece(s)", report.bad_pieces);
            }
            println!("Detail:   {}", report.detail);
        }
    }
    Ok(())
}

/// Check the data on disk. Exit code 0 when valid, 1 when not.
pub async fn verify(args: VerifyArgs, app: &App, output: OutputFormat) -> Result<i32> {
    let id = resolve_download_id(&args.id, app.engine())?;
    let name = app
        .engine()
        .status(id)
        .map(|s| s.metadata.name)
        .unwrap_or_default();
    let report = app.engine().verify(id).await?;
    print_report(output, id, &name, &report, None)?;
    Ok(if report.valid {
        exit_codes::SUCCESS
    } else {
        exit_codes::PARTIAL_FAILURE
    })
}

/// Check the data and, when it is bad, fetch it again in the foreground.
///
/// The engine's `repair` only *starts* the new transfer; a command that
/// returned right away would shut the engine down before a single byte was
/// fetched, so this waits for the download like `gosh resume` does.
pub async fn repair(args: VerifyArgs, app: &App, output: OutputFormat) -> Result<i32> {
    let id = resolve_download_id(&args.id, app.engine())?;
    let name = app
        .engine()
        .status(id)
        .map(|s| s.metadata.name)
        .unwrap_or_default();
    // Subscribe first so a fast re-download cannot finish unnoticed.
    let events = app.subscribe();
    let report = app.engine().repair(id).await?;
    let started = !report.valid;
    print_report(output, id, &name, &report, Some(started))?;
    if !started {
        return Ok(exit_codes::SUCCESS);
    }
    if output == OutputFormat::Table {
        eprintln!("Repairing: downloading the missing or corrupt data again...");
    }
    super::add::wait_for_completion(app, &[id], events).await
}
