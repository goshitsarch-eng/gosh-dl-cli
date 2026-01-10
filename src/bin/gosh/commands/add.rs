use anyhow::{bail, Context, Result};
use gosh_dl::types::{DownloadEvent, DownloadId, DownloadOptions};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::time::Duration;

use crate::app::App;
use crate::cli::{AddArgs, OutputFormat};
use crate::input::url_parser::{parse_input, ParsedInput};
use crate::output::table::print_add_results;

#[derive(Serialize)]
pub struct AddResult {
    pub id: String,
    pub input: String,
    pub kind: String,
}

pub async fn execute(args: AddArgs, app: &App, output: OutputFormat) -> Result<()> {
    // Collect all URLs from various sources
    let mut urls = args.urls.clone();

    // Check for stdin input (indicated by '-' in urls)
    if urls.iter().any(|u| u == "-") {
        urls.retain(|u| u != "-");
        urls.extend(read_urls_from_stdin()?);
    }

    // Read from input file if specified
    if let Some(ref file) = args.input_file {
        urls.extend(read_urls_from_file(file)?);
    }

    if urls.is_empty() {
        bail!("No URLs provided. Use positional arguments, -i <file>, or pipe to stdin with '-'");
    }

    // Validate single filename for multiple downloads
    if args.out.is_some() && urls.len() > 1 {
        bail!("Cannot use -o/--out with multiple downloads. Remove -o or add one URL at a time.");
    }

    // Parse and categorize inputs
    let inputs: Vec<ParsedInput> = urls.iter().map(|u| parse_input(u)).collect::<Result<_>>()?;

    // Add each download
    let mut results = Vec::new();
    for input in inputs {
        let options = build_options(&args, &input)?;

        let id = match &input {
            ParsedInput::Http(url) => app.engine().add_http(url, options).await?,
            ParsedInput::Magnet(uri) => app.engine().add_magnet(uri, options).await?,
            ParsedInput::TorrentFile(path) => {
                let data = tokio::fs::read(path)
                    .await
                    .with_context(|| format!("Failed to read torrent file: {}", path.display()))?;
                app.engine().add_torrent(&data, options).await?
            }
        };

        results.push(AddResult {
            id: id.to_gid(),
            input: input.display(),
            kind: input.kind().to_string(),
        });
    }

    // If --wait, monitor until completion
    if args.wait {
        wait_for_completion(app, &results).await?;
    }

    // Output results
    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&results)?);
        }
        OutputFormat::JsonPretty => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        OutputFormat::Table => {
            print_add_results(&results);
        }
    }

    Ok(())
}

fn read_urls_from_stdin() -> Result<Vec<String>> {
    let stdin = io::stdin();
    let urls: Vec<String> = stdin
        .lock()
        .lines()
        .filter_map(|line| {
            line.ok().and_then(|l| {
                let trimmed = l.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
        })
        .collect();
    Ok(urls)
}

fn read_urls_from_file(path: &PathBuf) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read input file: {}", path.display()))?;

    let urls: Vec<String> = content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect();

    Ok(urls)
}

fn build_options(args: &AddArgs, input: &ParsedInput) -> Result<DownloadOptions> {
    let mut options = DownloadOptions {
        priority: args.priority.to_engine_priority(),
        ..Default::default()
    };

    if let Some(ref dir) = args.dir {
        options.save_dir = Some(dir.clone());
    }

    if let Some(ref name) = args.out {
        options.filename = Some(name.clone());
    }

    if let Some(ref ua) = args.user_agent {
        options.user_agent = Some(ua.clone());
    }

    if let Some(ref referer) = args.referer {
        options.referer = Some(referer.clone());
    }

    // Parse headers
    for header in &args.headers {
        if let Some((name, value)) = header.split_once(':') {
            options
                .headers
                .push((name.trim().to_string(), value.trim().to_string()));
        }
    }

    // Parse cookies
    if !args.cookies.is_empty() {
        options.cookies = Some(args.cookies.clone());
    }

    // Parse checksum
    if let Some(ref checksum) = args.checksum {
        options.checksum = Some(parse_checksum(checksum)?);
    }

    if let Some(max_conn) = args.max_connections {
        options.max_connections = Some(max_conn);
    }

    if let Some(ref speed) = args.max_speed {
        options.max_download_speed = Some(parse_speed(speed)?);
    }

    // Torrent-specific options
    if matches!(input, ParsedInput::Magnet(_) | ParsedInput::TorrentFile(_)) {
        if args.sequential {
            options.sequential = Some(true);
        }

        if let Some(ref files) = args.select_files {
            let indices: Vec<usize> = files
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if !indices.is_empty() {
                options.selected_files = Some(indices);
            }
        }

        if let Some(ratio) = args.seed_ratio {
            options.seed_ratio = Some(ratio);
        }
    }

    Ok(options)
}

fn parse_checksum(s: &str) -> Result<gosh_dl::http::ExpectedChecksum> {
    if let Some(hash) = s.strip_prefix("md5:") {
        Ok(gosh_dl::http::ExpectedChecksum::md5(hash.to_string()))
    } else if let Some(hash) = s.strip_prefix("sha256:") {
        Ok(gosh_dl::http::ExpectedChecksum::sha256(hash.to_string()))
    } else {
        bail!("Invalid checksum format. Use 'md5:HASH' or 'sha256:HASH'")
    }
}

fn parse_speed(s: &str) -> Result<u64> {
    let s = s.trim().to_uppercase();

    if let Some(num) = s.strip_suffix('K') {
        Ok(num.parse::<u64>()? * 1024)
    } else if let Some(num) = s.strip_suffix('M') {
        Ok(num.parse::<u64>()? * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix('G') {
        Ok(num.parse::<u64>()? * 1024 * 1024 * 1024)
    } else {
        Ok(s.parse()?)
    }
}

async fn wait_for_completion(app: &App, results: &[AddResult]) -> Result<()> {
    let ids: HashSet<DownloadId> = results
        .iter()
        .filter_map(|r| DownloadId::from_gid(&r.id))
        .collect();

    if ids.is_empty() {
        return Ok(());
    }

    let mut remaining = ids.clone();
    let mut events = app.subscribe();

    // Setup progress bars
    let multi = MultiProgress::new();
    let style = ProgressStyle::with_template(
        "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}",
    )?
    .progress_chars("=> ");

    let bars: HashMap<DownloadId, ProgressBar> = ids
        .iter()
        .map(|id| {
            let pb = multi.add(ProgressBar::new(0));
            pb.set_style(style.clone());
            pb.enable_steady_tick(Duration::from_millis(100));
            (*id, pb)
        })
        .collect();

    // Set initial messages
    for result in results {
        if let Some(id) = DownloadId::from_gid(&result.id) {
            if let Some(pb) = bars.get(&id) {
                pb.set_message(truncate_string(&result.input, 30));
            }
        }
    }

    while !remaining.is_empty() {
        match events.recv().await {
            Ok(DownloadEvent::Progress { id, progress }) if ids.contains(&id) => {
                if let Some(pb) = bars.get(&id) {
                    if let Some(total) = progress.total_size {
                        pb.set_length(total);
                    }
                    pb.set_position(progress.completed_size);
                }
            }
            Ok(DownloadEvent::Completed { id }) if ids.contains(&id) => {
                if let Some(pb) = bars.get(&id) {
                    pb.finish_with_message("Done");
                }
                remaining.remove(&id);
            }
            Ok(DownloadEvent::Failed { id, error, .. }) if ids.contains(&id) => {
                if let Some(pb) = bars.get(&id) {
                    pb.abandon_with_message(format!("Failed: {}", truncate_string(&error, 40)));
                }
                remaining.remove(&id);
            }
            Ok(DownloadEvent::Paused { id }) if ids.contains(&id) => {
                if let Some(pb) = bars.get(&id) {
                    pb.set_message("Paused");
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            _ => continue,
        }
    }

    Ok(())
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
