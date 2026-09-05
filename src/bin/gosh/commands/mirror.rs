//! Recursive HTTP directory mirroring (wget -r style).
//!
//! `gosh mirror <URL>` discovers files under a directory-listing page and
//! downloads them through the engine's recursive job APIs. Job management
//! lives under `gosh mirror list|status|cancel|remove`.

use anyhow::{bail, Result};
use gosh_dl::{
    DownloadEvent, DownloadId, DownloadOptions, RecursiveJob, RecursiveJobState,
    RecursiveJobStatus, RecursiveOptions,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;

use crate::app::App;
use crate::cli::{MirrorAction, MirrorArgs, OutputFormat};
use crate::format::{format_size, print_error, print_warning};
use crate::util::{
    exit_codes, parse_speed, resolve_mirror_job_id, truncate_str, validate_max_connections,
};

/// Maximum number of per-file progress bars shown at once.
const MAX_FILE_BARS: usize = 8;

pub async fn execute(args: MirrorArgs, app: &App, output: OutputFormat) -> Result<i32> {
    match args.action {
        Some(MirrorAction::List) => list_jobs(app, output),
        Some(MirrorAction::Status { ref id }) => job_status(app, id, output),
        Some(MirrorAction::Cancel {
            ref id,
            delete_files,
            yes,
        }) => cancel_job(app, id, delete_files, yes, false).await,
        Some(MirrorAction::Remove {
            ref id,
            delete_files,
            yes,
        }) => cancel_job(app, id, delete_files, yes, true).await,
        None => {
            let Some(url) = args.url.clone() else {
                bail!("A URL is required. Usage: gosh mirror <URL> (see gosh mirror --help)");
            };
            run_mirror(app, &url, &args, output).await
        }
    }
}

fn build_recursive_options(args: &MirrorArgs) -> RecursiveOptions {
    let defaults = RecursiveOptions::default();
    RecursiveOptions {
        max_depth: args.depth.unwrap_or(defaults.max_depth),
        same_host_only: !args.span_hosts,
        allowed_prefix: args.prefix.clone(),
        include_patterns: args.include.clone(),
        exclude_patterns: args.exclude.clone(),
        preserve_paths: !args.flatten,
        overwrite_existing: args.overwrite,
        fail_fast: args.fail_fast,
        max_discovery_concurrency: args
            .discovery_concurrency
            .unwrap_or(defaults.max_discovery_concurrency),
    }
}

fn build_download_options(args: &MirrorArgs) -> Result<DownloadOptions> {
    let mut options = DownloadOptions::default();

    if let Some(ref dir) = args.dir {
        options.save_dir = Some(dir.clone());
    }

    if let Some(ref ua) = args.user_agent {
        options.user_agent = Some(ua.clone());
    }

    if let Some(ref referer) = args.referer {
        options.referer = Some(referer.clone());
    }

    for header in &args.headers {
        if let Some((name, value)) = header.split_once(':') {
            options
                .headers
                .push((name.trim().to_string(), value.trim().to_string()));
        } else {
            bail!("Invalid header format '{}'. Expected 'Name: Value'", header);
        }
    }

    if !args.cookies.is_empty() {
        options.cookies = Some(args.cookies.clone());
    }

    if let Some(max_conn) = validate_max_connections(args.max_connections)? {
        options.max_connections = Some(max_conn);
    }

    if let Some(ref speed) = args.max_speed {
        options.max_download_speed = Some(parse_speed(speed)?);
    }

    Ok(options)
}

/// A recursive job is finished once no child can still make progress.
/// `Partial` alone is not terminal: it also describes a mix of completed
/// and still-running children.
fn is_done(status: &RecursiveJobStatus) -> bool {
    let p = &status.progress;
    p.queued_children == 0 && p.active_children == 0 && p.paused_children == 0
}

fn final_exit_code(status: &RecursiveJobStatus) -> i32 {
    match status.state {
        RecursiveJobState::Completed => exit_codes::SUCCESS,
        RecursiveJobState::Failed => {
            print_error(&format!(
                "All {} file(s) failed",
                status.progress.total_children
            ));
            exit_codes::TOTAL_FAILURE
        }
        _ => {
            print_warning(&format!(
                "{}/{} file(s) completed, {} failed",
                status.progress.completed_children,
                status.progress.total_children,
                status.progress.failed_children + status.progress.missing_children
            ));
            exit_codes::PARTIAL_FAILURE
        }
    }
}

async fn run_mirror(app: &App, url: &str, args: &MirrorArgs, output: OutputFormat) -> Result<i32> {
    if args.detach && app.config.general.storage_backend == crate::config::StorageBackend::None {
        bail!("Enqueueing a mirror requires persistent storage; omit --enqueue/--detach to run in the foreground");
    }
    let options = build_download_options(args)?;
    let recursive = build_recursive_options(args);

    // Dry run: discover and list, never download
    if args.dry_run {
        eprintln!("Discovering files under {url} ...");
        let manifest = app
            .engine()
            .discover_http_recursive(url, &options, &recursive)
            .await?;
        return print_manifest(&manifest, output);
    }

    // Subscribe before adding so no event can be missed
    let mut job_events = app.engine().subscribe_recursive_jobs();
    let mut dl_events = app.subscribe();

    eprintln!("Discovering files under {url} ...");
    let job = app
        .engine()
        .add_http_recursive(url, options, recursive)
        .await?;

    if job.child_ids.is_empty() {
        bail!("No downloadable files discovered under {url}");
    }

    // Find the tracked job record (newest first) to identify our job in events
    let tracked_id = app
        .engine()
        .list_recursive_jobs()
        .into_iter()
        .find(|j| j.root_url == job.root_url && j.child_ids == job.child_ids)
        .map(|j| j.id);

    if args.detach {
        super::add::pause_downloads(app, &job.child_ids).await?;
        eprintln!("Saved mirror paused; no background process is running. Resume in the TUI or run: gosh resume {}",
            job.child_ids.iter().map(|id| id.to_gid()).collect::<Vec<_>>().join(" "));
        print_job_result(app, &job, tracked_id, output)?;
        return Ok(exit_codes::SUCCESS);
    }

    if output == OutputFormat::Table {
        println!("Mirroring {} file(s)", job.child_ids.len());
    }
    let code =
        run_mirror_foreground(app, &job, tracked_id, &mut job_events, &mut dl_events).await?;
    print_job_result(app, &job, tracked_id, output)?;
    Ok(code)
}

fn print_job_result(
    app: &App,
    job: &RecursiveJob,
    id: Option<uuid::Uuid>,
    output: OutputFormat,
) -> Result<()> {
    let status = app.engine().recursive_job_status(job);
    let result = serde_json::json!({"id": id, "root_url": job.root_url, "child_ids": job.child_ids, "status": status});
    match output {
        OutputFormat::Json => println!("{}", serde_json::to_string(&result)?),
        OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&result)?),
        OutputFormat::Table => {
            if let Some(id) = id {
                println!(
                    "Mirror job {}: {}",
                    &id.simple().to_string()[..8],
                    format_job_state(status.state)
                );
            }
        }
    }
    Ok(())
}

fn print_manifest(manifest: &gosh_dl::RecursiveManifest, output: OutputFormat) -> Result<i32> {
    match output {
        OutputFormat::Json => println!("{}", serde_json::to_string(manifest)?),
        OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(manifest)?),
        OutputFormat::Table => {
            if manifest.entries.is_empty() {
                println!(
                    "No downloadable files discovered under {}",
                    manifest.root_url
                );
                return Ok(exit_codes::SUCCESS);
            }
            println!("{:<10} PATH", "SIZE");
            let mut known_total: u64 = 0;
            let mut unknown = 0usize;
            for entry in &manifest.entries {
                let size = match entry.size_hint {
                    Some(s) => {
                        known_total += s;
                        format_size(s)
                    }
                    None => {
                        unknown += 1;
                        "?".to_string()
                    }
                };
                println!("{:<10} {}", size, entry.relative_path.display());
            }
            print!(
                "\n{} file(s), {} total",
                manifest.entries.len(),
                format_size(known_total)
            );
            if unknown > 0 {
                print!(" (+{unknown} of unknown size)");
            }
            println!();
        }
    }
    Ok(exit_codes::SUCCESS)
}

async fn run_mirror_foreground(
    app: &App,
    job: &RecursiveJob,
    tracked_id: Option<uuid::Uuid>,
    job_events: &mut tokio::sync::broadcast::Receiver<gosh_dl::RecursiveJobEvent>,
    dl_events: &mut tokio::sync::broadcast::Receiver<DownloadEvent>,
) -> Result<i32> {
    let child_ids: HashSet<DownloadId> = job.child_ids.iter().copied().collect();

    let multi = MultiProgress::new();
    let total_bar = multi.add(ProgressBar::new(child_ids.len() as u64));
    total_bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} total [{bar:30.cyan/blue}] {pos}/{len} files {msg}",
        )?
        .progress_chars("=> "),
    );
    total_bar.enable_steady_tick(Duration::from_millis(100));

    let file_style = ProgressStyle::with_template(
        "  {msg:<32} [{bar:20.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})",
    )?
    .progress_chars("=> ");

    let mut file_bars: HashMap<DownloadId, ProgressBar> = HashMap::new();
    let mut poll = tokio::time::interval(Duration::from_secs(2));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let update_total = |bar: &ProgressBar, status: &RecursiveJobStatus| {
        bar.set_position(status.progress.completed_children as u64);
        let mut msg = format!("({} active", status.progress.active_children);
        if status.progress.failed_children > 0 {
            msg += &format!(", {} failed", status.progress.failed_children);
        }
        msg += ")";
        bar.set_message(msg);
    };

    let final_status = loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                total_bar.abandon_with_message("Interrupted".to_string());
                for pb in file_bars.values() {
                    pb.abandon();
                }
                super::add::pause_downloads(app, &job.child_ids).await?;
                return Ok(exit_codes::INTERRUPTED);
            }
            ev = job_events.recv() => match ev {
                Ok(gosh_dl::RecursiveJobEvent::Updated { job: j, status })
                    if tracked_id == Some(j.id)
                        || (tracked_id.is_none() && j.root_url == job.root_url) =>
                {
                    update_total(&total_bar, &status);
                    if is_done(&status) {
                        break status;
                    }
                }
                Err(RecvError::Lagged(_)) => {
                    // poll tick below reconciles
                }
                Err(RecvError::Closed) => break app.engine().recursive_job_status(job),
                _ => {}
            },
            ev = dl_events.recv() => match ev {
                Ok(DownloadEvent::Progress { id, progress }) if child_ids.contains(&id) => {
                    if let Some(pb) = file_bars.get(&id) {
                        if let Some(total) = progress.total_size {
                            pb.set_length(total);
                        }
                        pb.set_position(progress.completed_size);
                    } else if file_bars.len() < MAX_FILE_BARS {
                        let pb = multi.add(ProgressBar::new(progress.total_size.unwrap_or(0)));
                        pb.set_style(file_style.clone());
                        let name = app
                            .engine()
                            .status(id)
                            .map(|s| s.metadata.name)
                            .unwrap_or_else(|| id.to_gid());
                        pb.set_message(truncate_str(&name, 32));
                        pb.set_position(progress.completed_size);
                        file_bars.insert(id, pb);
                    }
                }
                Ok(DownloadEvent::Completed { id }) if child_ids.contains(&id) => {
                    if let Some(pb) = file_bars.remove(&id) {
                        pb.finish_and_clear();
                        multi.remove(&pb);
                    }
                }
                Ok(DownloadEvent::Failed { id, error, .. }) if child_ids.contains(&id) => {
                    if let Some(pb) = file_bars.remove(&id) {
                        pb.abandon_with_message(format!(
                            "Failed: {}",
                            truncate_str(&error, 40)
                        ));
                    }
                }
                Ok(DownloadEvent::Removed { id }) if child_ids.contains(&id) => {
                    if let Some(pb) = file_bars.remove(&id) {
                        pb.finish_and_clear();
                        multi.remove(&pb);
                    }
                }
                Err(RecvError::Closed) => break app.engine().recursive_job_status(job),
                _ => {}
            },
            _ = poll.tick() => {
                // Race-proof fallback: derive status synchronously
                let status = app.engine().recursive_job_status(job);
                update_total(&total_bar, &status);
                if is_done(&status) {
                    break status;
                }
            }
        }
    };

    for pb in file_bars.values() {
        pb.finish_and_clear();
    }
    total_bar.finish();

    Ok(final_exit_code(&final_status))
}

fn list_jobs(app: &App, output: OutputFormat) -> Result<i32> {
    let jobs = app.engine().list_recursive_jobs();

    match output {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let entries: Vec<_> = jobs
                .iter()
                .map(|j| {
                    let status = app.engine().recursive_job_status(&j.as_job());
                    serde_json::json!({
                        "id": j.id,
                        "root_url": j.root_url,
                        "created_at": j.created_at,
                        "status": status,
                    })
                })
                .collect();
            if output == OutputFormat::JsonPretty {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                println!("{}", serde_json::to_string(&entries)?);
            }
        }
        OutputFormat::Table => {
            if jobs.is_empty() {
                println!("No mirror jobs");
                return Ok(exit_codes::SUCCESS);
            }
            println!(
                "{:<10} {:<11} {:>9} {:>10}  {:<20} URL",
                "ID", "STATE", "FILES", "SIZE", "CREATED"
            );
            for j in &jobs {
                let status = app.engine().recursive_job_status(&j.as_job());
                println!(
                    "{:<10} {:<11} {:>4}/{:<4} {:>10}  {:<20} {}",
                    &j.id.simple().to_string()[..8],
                    format_job_state(status.state),
                    status.progress.completed_children,
                    status.progress.total_children,
                    format_size(status.progress.completed_size),
                    j.created_at.format("%Y-%m-%d %H:%M:%S"),
                    truncate_str(&j.root_url, 50),
                );
            }
        }
    }
    Ok(exit_codes::SUCCESS)
}

fn job_status(app: &App, id_str: &str, output: OutputFormat) -> Result<i32> {
    let id = resolve_mirror_job_id(id_str, app.engine())?;
    let job = app
        .engine()
        .recursive_job(id)
        .ok_or_else(|| anyhow::anyhow!("Mirror job not found: {}", id_str))?;
    let status = app.engine().recursive_job_status(&job.as_job());

    match output {
        OutputFormat::Json => println!("{}", serde_json::to_string(&status)?),
        OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&status)?),
        OutputFormat::Table => {
            let p = &status.progress;
            println!("Mirror job:  {}", job.id);
            println!("URL:         {}", job.root_url);
            println!(
                "Created:     {}",
                job.created_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!("State:       {}", format_job_state(status.state));
            println!(
                "Files:       {}/{} completed ({} active, {} queued, {} paused, {} failed{})",
                p.completed_children,
                p.total_children,
                p.active_children,
                p.queued_children,
                p.paused_children,
                p.failed_children,
                if p.missing_children > 0 {
                    format!(", {} missing", p.missing_children)
                } else {
                    String::new()
                }
            );
            match p.total_size {
                Some(total) => println!(
                    "Size:        {} / {}",
                    format_size(p.completed_size),
                    format_size(total)
                ),
                None => println!("Size:        {}", format_size(p.completed_size)),
            }
        }
    }
    Ok(exit_codes::SUCCESS)
}

async fn cancel_job(
    app: &App,
    id_str: &str,
    delete_files: bool,
    yes: bool,
    remove: bool,
) -> Result<i32> {
    let id = resolve_mirror_job_id(id_str, app.engine())?;
    let job = app
        .engine()
        .recursive_job(id)
        .ok_or_else(|| anyhow::anyhow!("Mirror job not found: {}", id_str))?;

    if !yes {
        let verb = if remove { "remove" } else { "cancel" };
        let action = if delete_files {
            format!("{verb} and DELETE FILES for")
        } else {
            verb.to_string()
        };
        print!(
            "Are you sure you want to {} mirror job {} ({} file(s) from {})? [y/N] ",
            action,
            &id.simple().to_string()[..8],
            job.child_ids.len(),
            truncate_str(&job.root_url, 50)
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(exit_codes::SUCCESS);
        }
    }

    if remove {
        app.engine().remove_recursive_job(id, delete_files).await?;
        println!("Removed mirror job {}", &id.simple().to_string()[..8]);
    } else {
        app.engine().cancel_recursive_job(id, delete_files).await?;
        println!("Cancelled mirror job {}", &id.simple().to_string()[..8]);
    }
    Ok(exit_codes::SUCCESS)
}

fn format_job_state(state: RecursiveJobState) -> &'static str {
    match state {
        RecursiveJobState::Empty => "Empty",
        RecursiveJobState::Queued => "Queued",
        RecursiveJobState::Running => "Running",
        RecursiveJobState::Paused => "Paused",
        RecursiveJobState::Completed => "Completed",
        RecursiveJobState::Failed => "Failed",
        RecursiveJobState::Partial => "Partial",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::MirrorArgs;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: MirrorArgs,
    }

    fn parse(extra: &[&str]) -> MirrorArgs {
        let mut argv = vec!["test", "https://example.com/dir/"];
        argv.extend_from_slice(extra);
        TestCli::parse_from(argv).args
    }

    #[test]
    fn recursive_options_defaults() {
        let opts = build_recursive_options(&parse(&[]));
        let defaults = RecursiveOptions::default();
        assert_eq!(opts.max_depth, defaults.max_depth);
        assert!(opts.same_host_only);
        assert!(opts.preserve_paths);
        assert!(!opts.overwrite_existing);
        assert!(!opts.fail_fast);
        assert_eq!(
            opts.max_discovery_concurrency,
            defaults.max_discovery_concurrency
        );
    }

    #[test]
    fn recursive_options_flag_inversions() {
        let opts = build_recursive_options(&parse(&["--span-hosts", "--flatten"]));
        assert!(!opts.same_host_only);
        assert!(!opts.preserve_paths);
    }

    #[test]
    fn recursive_options_patterns_and_depth() {
        let opts = build_recursive_options(&parse(&[
            "--depth",
            "3",
            "--include",
            "*.iso",
            "--include",
            "*.sig",
            "--exclude",
            "*.tmp",
            "--discovery-concurrency",
            "8",
        ]));
        assert_eq!(opts.max_depth, 3);
        assert_eq!(opts.include_patterns, vec!["*.iso", "*.sig"]);
        assert_eq!(opts.exclude_patterns, vec!["*.tmp"]);
        assert_eq!(opts.max_discovery_concurrency, 8);
    }

    #[test]
    fn is_done_requires_no_pending_children() {
        let mut status = RecursiveJobStatus {
            root_url: "https://example.com/".into(),
            child_ids: vec![],
            state: RecursiveJobState::Partial,
            progress: Default::default(),
        };
        status.progress.total_children = 3;
        status.progress.completed_children = 2;
        status.progress.active_children = 1;
        assert!(!is_done(&status));

        status.progress.active_children = 0;
        status.progress.failed_children = 1;
        status.progress.completed_children = 2;
        assert!(is_done(&status));
    }
}
