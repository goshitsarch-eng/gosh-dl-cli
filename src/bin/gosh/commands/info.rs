use anyhow::{Context, Result};
use gosh_dl::torrent::Metainfo;
use serde::Serialize;
use std::path::PathBuf;

use crate::cli::{InfoArgs, OutputFormat};

#[derive(Serialize)]
struct TorrentInfo {
    name: String,
    info_hash: String,
    total_size: u64,
    total_size_human: String,
    piece_length: u64,
    piece_count: usize,
    private: bool,
    files: Vec<FileInfo>,
    announce: Option<String>,
    trackers: Vec<Vec<String>>,
    creation_date: Option<String>,
    created_by: Option<String>,
    comment: Option<String>,
    web_seeds: Vec<String>,
}

#[derive(Serialize)]
struct FileInfo {
    index: usize,
    path: PathBuf,
    size: u64,
    size_human: String,
}

pub async fn execute(args: InfoArgs, output: OutputFormat) -> Result<()> {
    let data = tokio::fs::read(&args.file)
        .await
        .with_context(|| format!("Failed to read torrent file: {}", args.file.display()))?;

    let metainfo = Metainfo::parse(&data)
        .with_context(|| format!("Failed to parse torrent file: {}", args.file.display()))?;

    let info = build_torrent_info(&metainfo);

    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&info)?);
        }
        OutputFormat::JsonPretty => {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        OutputFormat::Table => {
            print_torrent_info(&info);
        }
    }

    Ok(())
}

fn build_torrent_info(metainfo: &Metainfo) -> TorrentInfo {
    let files: Vec<FileInfo> = metainfo
        .info
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| FileInfo {
            index: i,
            path: f.path.clone(),
            size: f.length,
            size_human: format_size(f.length),
        })
        .collect();

    TorrentInfo {
        name: metainfo.info.name.clone(),
        info_hash: hex::encode(metainfo.info_hash),
        total_size: metainfo.info.total_size,
        total_size_human: format_size(metainfo.info.total_size),
        piece_length: metainfo.info.piece_length,
        piece_count: metainfo.info.pieces.len() / 20,
        private: metainfo.info.private,
        files,
        announce: metainfo.announce.clone(),
        trackers: metainfo.announce_list.clone(),
        creation_date: metainfo.creation_date.map(|d| {
            chrono::DateTime::from_timestamp(d, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| d.to_string())
        }),
        created_by: metainfo.created_by.clone(),
        comment: metainfo.comment.clone(),
        web_seeds: metainfo.url_list.clone(),
    }
}

fn print_torrent_info(info: &TorrentInfo) {
    println!("=== Torrent Information ===");
    println!("Name: {}", info.name);
    println!("Info Hash: {}", info.info_hash);
    println!(
        "Size: {} ({} bytes)",
        info.total_size_human, info.total_size
    );
    println!("Piece Size: {}", format_size(info.piece_length));
    println!("Pieces: {}", info.piece_count);
    if info.private {
        println!("Private: Yes");
    }
    println!();

    if let Some(ref announce) = info.announce {
        println!("=== Tracker ===");
        println!("Primary: {}", announce);
    }

    if !info.trackers.is_empty() {
        println!("Tracker Tiers:");
        for (tier, trackers) in info.trackers.iter().enumerate() {
            println!("  Tier {}:", tier + 1);
            for tracker in trackers {
                println!("    - {}", tracker);
            }
        }
        println!();
    }

    if !info.web_seeds.is_empty() {
        println!("=== Web Seeds ===");
        for seed in &info.web_seeds {
            println!("  - {}", seed);
        }
        println!();
    }

    println!("=== Files ({}) ===", info.files.len());
    for file in &info.files {
        println!(
            "  [{:3}] {:>10}  {}",
            file.index,
            file.size_human,
            file.path.display()
        );
    }
    println!();

    println!("=== Metadata ===");
    if let Some(ref date) = info.creation_date {
        println!("Created: {}", date);
    }
    if let Some(ref by) = info.created_by {
        println!("Created By: {}", by);
    }
    if let Some(ref comment) = info.comment {
        println!("Comment: {}", comment);
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
