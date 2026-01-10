use anyhow::{bail, Result};
use gosh_dl::types::DownloadId;
use gosh_dl::DownloadEngine;

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
    filter: impl Fn(&gosh_dl::types::DownloadStatus) -> bool,
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
