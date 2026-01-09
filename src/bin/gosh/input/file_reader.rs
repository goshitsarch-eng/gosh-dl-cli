use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Read URLs from a file, one per line
/// Lines starting with # are treated as comments
/// Empty lines are ignored
pub fn read_urls_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
    let path = path.as_ref();
    let file = File::open(path)
        .with_context(|| format!("Failed to open input file: {}", path.display()))?;

    let reader = BufReader::new(file);
    let urls: Vec<String> = reader
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

/// Read URLs from stdin
pub fn read_urls_from_stdin() -> Result<Vec<String>> {
    use std::io;

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
