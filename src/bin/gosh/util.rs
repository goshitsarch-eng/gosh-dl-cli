use anyhow::{bail, Result};
use gosh_dl::{DownloadEngine, DownloadId};

/// Parse a download ID string, supporting both full UUIDs and short GIDs.
///
/// The function tries the following in order:
/// 1. Parse as full UUID (e.g., "df5c1a3e-62bc-4678-8428-1b4fee2e24e2")
/// 2. Search engine's download list for a download with matching GID prefix
pub fn resolve_download_id(s: &str, engine: &DownloadEngine) -> Result<DownloadId> {
    // Try parsing as full UUID first
    if let Ok(uuid) = uuid::Uuid::parse_str(s) {
        return Ok(DownloadId::from_uuid(uuid));
    }

    // Try matching by GID prefix (search through downloads)
    let normalized = s.to_lowercase();
    let downloads = engine.list();

    let matches: Vec<_> = downloads
        .iter()
        .filter(|d| d.id.to_gid().to_lowercase().starts_with(&normalized))
        .collect();

    match matches.len() {
        0 => bail!("Download not found: {}", s),
        1 => Ok(matches[0].id),
        _ => {
            // Multiple matches - show them
            let ids: Vec<_> = matches.iter().map(|d| d.id.to_gid()).collect();
            bail!(
                "Ambiguous ID '{}' matches multiple downloads: {}. Please use a longer prefix or full UUID.",
                s,
                ids.join(", ")
            )
        }
    }
}

/// Resolve multiple download IDs, with support for "all" keyword.
pub fn resolve_download_ids(
    ids: &[String],
    engine: &DownloadEngine,
    filter: impl Fn(&gosh_dl::DownloadStatus) -> bool,
) -> Result<Vec<DownloadId>> {
    if ids.len() == 1 && ids[0].to_lowercase() == "all" {
        let all = engine.list();
        return Ok(all
            .into_iter()
            .filter(|d| filter(d))
            .map(|d| d.id)
            .collect());
    }

    ids.iter().map(|s| resolve_download_id(s, engine)).collect()
}

/// Truncate a string to `max_len` bytes, appending "..." if truncated.
/// Safe for UTF-8: always cuts on a char boundary.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_len.saturating_sub(3))
        .last()
        .unwrap_or(0);
    format!("{}...", &s[..end])
}

/// Validate an output filename, rejecting path traversal attempts.
pub fn sanitize_filename(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Output filename cannot be empty");
    }
    let path = std::path::Path::new(name);
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                bail!("Output filename must not contain '..'")
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                bail!("Output filename must be relative, not absolute")
            }
            _ => {}
        }
    }
    Ok(name.to_string())
}

/// Parse a speed string with optional K/M/G suffix into bytes per second.
pub fn parse_speed(s: &str) -> Result<u64> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_ascii() {
        assert_eq!(truncate_str("hello world", 11), "hello world");
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_emoji() {
        // "Hello 🌍!" is 11 bytes (🌍 = 4 bytes)
        assert_eq!(truncate_str("Hello 🌍!", 11), "Hello 🌍!");
        assert_eq!(truncate_str("Hello 🌍!", 8), "Hello...");
        // 🌍 is 4 bytes at offset 6; including it + "..." = 13 bytes > 10
        assert_eq!(truncate_str("Hello 🌍!", 10), "Hello ...");
        // "Hi 🌍 bye" = 11 bytes; emoji preserved when room allows
        assert_eq!(truncate_str("Hi 🌍 bye", 11), "Hi 🌍 bye");
        assert_eq!(truncate_str("Hi 🌍 bye", 10), "Hi 🌍...");
    }

    #[test]
    fn truncate_cjk() {
        // Each CJK char is 3 bytes
        assert_eq!(truncate_str("你好世界", 12), "你好世界");
        assert_eq!(truncate_str("你好世界", 9), "你好...");
    }

    #[test]
    fn truncate_short() {
        assert_eq!(truncate_str("abc", 3), "abc");
        assert_eq!(truncate_str("abc", 2), "...");
    }

    #[test]
    fn sanitize_valid() {
        assert_eq!(sanitize_filename("file.zip").unwrap(), "file.zip");
        assert_eq!(sanitize_filename("sub/file.zip").unwrap(), "sub/file.zip");
    }

    #[test]
    fn sanitize_rejects_traversal() {
        assert!(sanitize_filename("../etc/passwd").is_err());
        assert!(sanitize_filename("foo/../../bar").is_err());
    }

    #[test]
    fn sanitize_rejects_absolute() {
        assert!(sanitize_filename("/etc/passwd").is_err());
    }

    #[test]
    fn sanitize_rejects_empty() {
        assert!(sanitize_filename("").is_err());
        assert!(sanitize_filename("   ").is_err());
    }

    #[test]
    fn test_parse_speed_bytes() {
        assert_eq!(parse_speed("1000").unwrap(), 1000);
    }

    #[test]
    fn test_parse_speed_k() {
        assert_eq!(parse_speed("1K").unwrap(), 1024);
        assert_eq!(parse_speed("2k").unwrap(), 2048);
    }

    #[test]
    fn test_parse_speed_m() {
        assert_eq!(parse_speed("1M").unwrap(), 1024 * 1024);
        assert_eq!(parse_speed("5m").unwrap(), 5 * 1024 * 1024);
    }

    #[test]
    fn test_parse_speed_g() {
        assert_eq!(parse_speed("1G").unwrap(), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_parse_speed_invalid() {
        assert!(parse_speed("abc").is_err());
    }
}
