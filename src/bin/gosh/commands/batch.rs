//! Batch operations: pause-all, resume-all, cancel-all
//!
//! Thin wrappers over the gosh-dl 0.6.2 batch engine APIs, reporting
//! per-download outcomes from `BatchResult`.

use anyhow::Result;
use gosh_dl::BatchResult;
use serde::Serialize;
use std::io::{self, Write};

use crate::app::App;
use crate::cli::{CancelAllArgs, OutputFormat};
use crate::util::exit_codes;

/// Past-tense and infinitive forms for a batch verb, used in output.
pub struct BatchVerb {
    pub past: &'static str,
    pub infinitive: &'static str,
}

pub const PAUSE: BatchVerb = BatchVerb {
    past: "Paused",
    infinitive: "pause",
};
pub const RESUME: BatchVerb = BatchVerb {
    past: "Resumed",
    infinitive: "resume",
};
pub const CANCEL: BatchVerb = BatchVerb {
    past: "Cancelled",
    infinitive: "cancel",
};

#[derive(Serialize)]
struct BatchReport {
    succeeded: Vec<String>,
    skipped: Vec<String>,
    failed: Vec<FailedEntry>,
}

#[derive(Serialize)]
struct FailedEntry {
    id: String,
    error: String,
}

pub async fn pause_all(app: &App, output: OutputFormat) -> Result<i32> {
    let result = app.engine().pause_all().await;
    report(&result, &PAUSE, output)
}

pub async fn resume_all(app: &App, output: OutputFormat) -> Result<i32> {
    let events = app.subscribe();
    let result = app.engine().resume_all().await;
    let start_code = report(&result, &RESUME, output)?;
    let code = super::add::wait_for_completion(app, &result.succeeded, events).await?;
    Ok(if code == 0 { start_code } else { code })
}

pub async fn cancel_all(args: CancelAllArgs, app: &App, output: OutputFormat) -> Result<i32> {
    let total = app.engine().list().len();
    if total == 0 {
        return report(&BatchResult::default(), &CANCEL, output);
    }

    // Confirm unless --yes is specified
    if !args.yes {
        let action = if args.delete_files {
            "cancel and DELETE FILES for"
        } else {
            "cancel"
        };
        print!(
            "Are you sure you want to {} ALL {} download(s)? [y/N] ",
            action, total
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(exit_codes::SUCCESS);
        }
    }

    let result = app.engine().cancel_all(args.delete_files).await;
    report(&result, &CANCEL, output)
}

/// Print a `BatchResult` and map it to an exit code.
pub fn report(result: &BatchResult, verb: &BatchVerb, output: OutputFormat) -> Result<i32> {
    match output {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let report = BatchReport {
                succeeded: result.succeeded.iter().map(|id| id.to_gid()).collect(),
                skipped: result.skipped.iter().map(|id| id.to_gid()).collect(),
                failed: result
                    .failed
                    .iter()
                    .map(|(id, e)| FailedEntry {
                        id: id.to_gid(),
                        error: e.to_string(),
                    })
                    .collect(),
            };
            if output == OutputFormat::JsonPretty {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", serde_json::to_string(&report)?);
            }
        }
        OutputFormat::Table => {
            if result.succeeded.is_empty() && result.skipped.is_empty() && result.failed.is_empty()
            {
                println!("No downloads to {}", verb.infinitive);
                return Ok(exit_codes::SUCCESS);
            }
            for id in &result.succeeded {
                println!("{}: {}", verb.past, id.to_gid());
            }
            for id in &result.skipped {
                println!("Skipped: {}", id.to_gid());
            }
            for (id, e) in &result.failed {
                eprintln!("Failed to {} {}: {}", verb.infinitive, id.to_gid(), e);
            }
            println!(
                "{} {} download(s), {} skipped, {} failed",
                verb.past,
                result.succeeded.len(),
                result.skipped.len(),
                result.failed.len()
            );
        }
    }

    if result.failed.is_empty() {
        Ok(exit_codes::SUCCESS)
    } else {
        Ok(exit_codes::PARTIAL_FAILURE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::exit_codes;
    use gosh_dl::DownloadId;

    #[test]
    fn report_empty_result_is_success() {
        let result = BatchResult::default();
        let code = report(&result, &PAUSE, OutputFormat::Table).unwrap();
        assert_eq!(code, exit_codes::SUCCESS);
    }

    #[test]
    fn report_success_only_is_success() {
        let result = BatchResult {
            succeeded: vec![DownloadId::new()],
            ..Default::default()
        };
        let code = report(&result, &RESUME, OutputFormat::Table).unwrap();
        assert_eq!(code, exit_codes::SUCCESS);
    }

    #[test]
    fn report_json_serializes() {
        let result = BatchResult {
            succeeded: vec![DownloadId::new()],
            skipped: vec![DownloadId::new()],
            ..Default::default()
        };
        let code = report(&result, &CANCEL, OutputFormat::Json).unwrap();
        assert_eq!(code, exit_codes::SUCCESS);
    }
}
