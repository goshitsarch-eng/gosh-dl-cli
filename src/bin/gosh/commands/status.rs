use anyhow::Result;
use gosh_dl::types::{DownloadState, DownloadStatus};
use std::time::Duration;

use crate::app::App;
use crate::cli::{OutputFormat, StatusArgs};
use crate::util::resolve_download_id;

pub async fn execute(args: StatusArgs, app: &App, output: OutputFormat) -> Result<()> {
    let id = resolve_download_id(&args.id, app.engine())?;

    let status = app
        .engine()
        .status(id)
        .ok_or_else(|| anyhow::anyhow!("Download not found: {}", args.id))?;

    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&status)?);
        }
        OutputFormat::JsonPretty => {
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        OutputFormat::Table => {
            print_detailed_status(&status, args.peers, args.files);
        }
    }

    Ok(())
}

fn print_detailed_status(status: &DownloadStatus, show_peers: bool, show_files: bool) {
    println!("Download: {}", status.id.to_gid());
    println!("Name: {}", status.metadata.name);
    println!("Type: {:?}", status.kind);
    println!("State: {}", format_state(&status.state));
    println!("Priority: {:?}", status.priority);
    println!();

    // Progress section
    println!("=== Progress ===");
    let total = status
        .progress
        .total_size
        .map(format_size)
        .unwrap_or_else(|| "Unknown".to_string());
    let completed = format_size(status.progress.completed_size);
    let percentage = status.progress.percentage();

    println!("  Total Size: {}", total);
    println!("  Downloaded: {} ({:.1}%)", completed, percentage);
    println!(
        "  Download Speed: {}/s",
        format_size(status.progress.download_speed)
    );
    if status.progress.upload_speed > 0 {
        println!(
            "  Upload Speed: {}/s",
            format_size(status.progress.upload_speed)
        );
    }

    if let Some(eta) = status.progress.eta_seconds {
        println!("  ETA: {}", format_duration(eta));
    }

    println!("  Connections: {}", status.progress.connections);
    println!();

    // Location section
    println!("=== Location ===");
    println!("  Save Directory: {}", status.metadata.save_dir.display());
    if let Some(ref filename) = status.metadata.filename {
        println!("  Filename: {}", filename);
    }
    if let Some(ref url) = status.metadata.url {
        println!("  URL: {}", url);
    }
    if let Some(ref magnet) = status.metadata.magnet_uri {
        println!("  Magnet: {}", truncate(magnet, 60));
    }
    if let Some(ref info_hash) = status.metadata.info_hash {
        println!("  Info Hash: {}", info_hash);
    }
    println!();

    // Torrent-specific info
    if let Some(ref torrent_info) = status.torrent_info {
        println!("=== Torrent Info ===");
        println!("  Pieces: {}", torrent_info.pieces_count);
        println!("  Piece Size: {}", format_size(torrent_info.piece_length));
        println!("  Files: {}", torrent_info.files.len());
        println!("  Seeders: {}", status.progress.seeders);
        println!("  Peers: {}", status.progress.peers);
        if torrent_info.private {
            println!("  Private: Yes");
        }
        println!();

        // File list
        if show_files && !torrent_info.files.is_empty() {
            println!("=== Files ===");
            for file in &torrent_info.files {
                let progress = if file.size > 0 {
                    file.completed as f64 / file.size as f64 * 100.0
                } else {
                    100.0
                };
                let selected = if file.selected { "*" } else { " " };
                println!(
                    "  [{}] {:3} {:>10} {:5.1}% {}",
                    selected,
                    file.index,
                    format_size(file.size),
                    progress,
                    file.path.display()
                );
            }
            println!();
        }
    }

    // Peer list
    if show_peers {
        if let Some(ref peers) = status.peers {
            if !peers.is_empty() {
                println!("=== Peers ({}) ===", peers.len());
                for peer in peers.iter().take(20) {
                    let client = peer.client.as_deref().unwrap_or("Unknown");
                    println!(
                        "  {}:{} - {} - {}/s down, {}/s up - {:.1}%",
                        peer.ip,
                        peer.port,
                        client,
                        format_size(peer.download_speed),
                        format_size(peer.upload_speed),
                        peer.progress * 100.0
                    );
                }
                if peers.len() > 20 {
                    println!("  ... and {} more", peers.len() - 20);
                }
                println!();
            }
        }
    }

    // Timestamps
    println!("=== Timestamps ===");
    println!(
        "  Created: {}",
        status.created_at.format("%Y-%m-%d %H:%M:%S")
    );
    if let Some(completed_at) = status.completed_at {
        println!("  Completed: {}", completed_at.format("%Y-%m-%d %H:%M:%S"));
    }
}

fn format_state(state: &DownloadState) -> String {
    match state {
        DownloadState::Queued => "Queued".to_string(),
        DownloadState::Connecting => "Connecting".to_string(),
        DownloadState::Downloading => "Downloading".to_string(),
        DownloadState::Seeding => "Seeding".to_string(),
        DownloadState::Paused => "Paused".to_string(),
        DownloadState::Completed => "Completed".to_string(),
        DownloadState::Error { kind, message, .. } => {
            format!("Error ({}): {}", kind, message)
        }
    }
}

fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        "0 B".to_string()
    } else if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_duration(seconds: u64) -> String {
    let duration = Duration::from_secs(seconds);
    humantime::format_duration(duration).to_string()
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
