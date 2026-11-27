//! View model for the download list.
//!
//! The engine reports every download separately, including the hundreds of
//! children a recursive mirror creates. The TUI shows one row per *thing the
//! user started*: a download, or a mirror job that stands for all of its
//! files. Everything here is plain data derived from engine snapshots, so
//! the rendering code never has to ask the engine anything.

use chrono::{DateTime, Datelike, Local, Timelike, Utc};
use gosh_dl::{
    DownloadId, DownloadKind, DownloadState, DownloadStatus, RecursiveJobState, RecursiveJobStatus,
};

use crate::config::CliConfig;
use crate::format::{error_kind_label, format_duration, format_size, format_speed};

/// Stable identity of a list row across refreshes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowId {
    Download(DownloadId),
    Mirror(uuid::Uuid),
}

/// What a row is, for the coloured tag in front of its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Http,
    Torrent,
    Magnet,
    Mirror,
}

/// Display state shared by downloads and mirror jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Queued,
    Connecting,
    Downloading,
    Seeding,
    Paused,
    Completed,
    Error,
    /// Mirror only: finished, with some files failed or removed.
    Partial,
    /// Mirror only: every child was cancelled.
    Empty,
}

impl RowState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Connecting => "Connecting",
            Self::Downloading => "Downloading",
            Self::Seeding => "Seeding",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Error => "Error",
            Self::Partial => "Partial",
            Self::Empty => "Empty",
        }
    }

    /// Static icon; `None` for the states that show the animated spinner.
    pub fn icon(self) -> Option<&'static str> {
        match self {
            Self::Downloading | Self::Connecting => None,
            Self::Seeding => Some("\u{2191}"),
            Self::Paused => Some("\u{2016}"),
            Self::Queued => Some("\u{25f7}"),
            Self::Completed => Some("\u{2713}"),
            Self::Error => Some("\u{2717}"),
            Self::Partial => Some("!"),
            Self::Empty => Some("\u{2205}"),
        }
    }

    /// Transferring or trying to (the "Active" view).
    pub fn is_active(self) -> bool {
        matches!(self, Self::Connecting | Self::Downloading | Self::Seeding)
    }

    pub fn from_download(state: &DownloadState) -> Self {
        match state {
            DownloadState::Queued => Self::Queued,
            DownloadState::Connecting => Self::Connecting,
            DownloadState::Downloading => Self::Downloading,
            DownloadState::Seeding => Self::Seeding,
            DownloadState::Paused => Self::Paused,
            DownloadState::Completed => Self::Completed,
            DownloadState::Error { .. } => Self::Error,
        }
    }

    pub fn from_mirror(state: RecursiveJobState) -> Self {
        match state {
            RecursiveJobState::Empty => Self::Empty,
            RecursiveJobState::Queued => Self::Queued,
            RecursiveJobState::Running => Self::Downloading,
            RecursiveJobState::Paused => Self::Paused,
            RecursiveJobState::Completed => Self::Completed,
            RecursiveJobState::Failed => Self::Error,
            RecursiveJobState::Partial => Self::Partial,
        }
    }
}

/// A recursive mirror job together with its child downloads.
#[derive(Debug, Clone)]
pub struct MirrorRow {
    pub id: uuid::Uuid,
    pub root_url: String,
    pub created_at: DateTime<Utc>,
    pub status: RecursiveJobStatus,
    /// Children still known to the engine, in manifest order.
    pub children: Vec<DownloadStatus>,
}

impl MirrorRow {
    pub fn download_speed(&self) -> u64 {
        self.children
            .iter()
            .filter(|c| RowState::from_download(&c.state).is_active())
            .map(|c| c.progress.download_speed)
            .sum()
    }

    /// Sum of the children's sizes, when every child's size is known.
    pub fn total_size(&self) -> Option<u64> {
        self.status.progress.total_size
    }

    pub fn save_dir(&self) -> Option<&std::path::Path> {
        self.children.first().map(|c| c.metadata.save_dir.as_path())
    }

    /// The root URL without its scheme: `ftp.gnu.org/gnu/coreutils/`.
    pub fn display_name(&self) -> String {
        self.root_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string()
    }
}

/// One line item in the download list.
#[derive(Debug, Clone)]
pub enum Row {
    Download(Box<DownloadStatus>),
    Mirror(Box<MirrorRow>),
}

impl Row {
    pub fn id(&self) -> RowId {
        match self {
            Self::Download(d) => RowId::Download(d.id),
            Self::Mirror(m) => RowId::Mirror(m.id),
        }
    }

    pub fn kind(&self) -> RowKind {
        match self {
            Self::Download(d) => match d.kind {
                DownloadKind::Http => RowKind::Http,
                DownloadKind::Torrent => RowKind::Torrent,
                DownloadKind::Magnet => RowKind::Magnet,
            },
            Self::Mirror(_) => RowKind::Mirror,
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Download(d) => d.metadata.name.clone(),
            Self::Mirror(m) => m.display_name(),
        }
    }

    pub fn state(&self) -> RowState {
        match self {
            Self::Download(d) => RowState::from_download(&d.state),
            Self::Mirror(m) => RowState::from_mirror(m.status.state),
        }
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        match self {
            Self::Download(d) => d.created_at,
            Self::Mirror(m) => m.created_at,
        }
    }

    /// `HTTP×8`, `TORRENT`, `MAGNET`, `MIRROR`. An HTTP download shows its
    /// live connection count while it has one.
    pub fn tag(&self) -> String {
        match self {
            Self::Download(d) => match d.kind {
                DownloadKind::Http if d.progress.connections > 0 && self.state().is_active() => {
                    format!("HTTP\u{d7}{}", d.progress.connections)
                }
                DownloadKind::Http => "HTTP".to_string(),
                DownloadKind::Torrent => "TORRENT".to_string(),
                DownloadKind::Magnet => "MAGNET".to_string(),
            },
            Self::Mirror(_) => "MIRROR".to_string(),
        }
    }

    pub fn completed_size(&self) -> u64 {
        match self {
            Self::Download(d) => d.progress.completed_size,
            Self::Mirror(m) => m.status.progress.completed_size,
        }
    }

    pub fn total_size(&self) -> Option<u64> {
        match self {
            Self::Download(d) => d.progress.total_size,
            Self::Mirror(m) => m.total_size(),
        }
    }

    pub fn download_speed(&self) -> u64 {
        match self {
            Self::Download(d) => d.progress.download_speed,
            Self::Mirror(m) => m.download_speed(),
        }
    }

    pub fn upload_speed(&self) -> u64 {
        match self {
            Self::Download(d) => d.progress.upload_speed,
            Self::Mirror(_) => 0,
        }
    }

    /// Completion in percent. A mirror whose sizes are not all known yet is
    /// measured in files instead of bytes.
    pub fn percent(&self) -> f64 {
        match self {
            Self::Download(d) => {
                if matches!(d.state, DownloadState::Completed) {
                    100.0
                } else {
                    d.progress.percentage().clamp(0.0, 100.0)
                }
            }
            Self::Mirror(m) => {
                let p = &m.status.progress;
                match m.total_size() {
                    Some(total) if total > 0 => {
                        (p.completed_size as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
                    }
                    _ if p.total_children > 0 => {
                        p.completed_children as f64 / p.total_children as f64 * 100.0
                    }
                    _ => 0.0,
                }
            }
        }
    }

    pub fn eta_seconds(&self) -> Option<u64> {
        match self {
            Self::Download(d) => d.progress.eta_seconds,
            Self::Mirror(m) => {
                let speed = m.download_speed();
                let remaining = m
                    .total_size()?
                    .saturating_sub(m.status.progress.completed_size);
                (speed > 0).then(|| remaining / speed)
            }
        }
    }

    pub fn priority(&self) -> String {
        match self {
            Self::Download(d) => d.priority.to_string(),
            Self::Mirror(m) => m
                .children
                .first()
                .map(|c| c.priority.to_string())
                .unwrap_or_else(|| "normal".to_string()),
        }
    }

    fn files_prefix(&self) -> String {
        match self {
            Self::Mirror(m) => format!(
                "{}/{} files  ",
                m.status.progress.completed_children, m.status.progress.total_children
            ),
            Self::Download(_) => String::new(),
        }
    }

    /// The right-hand summary on a row's second line (wide layout).
    pub fn meta(&self) -> String {
        let pct = self.percent();
        match self.state() {
            // A mirror row leads with its file count; its ETA is in the
            // details panel, where there is room for both.
            RowState::Downloading if matches!(self, Self::Mirror(_)) => format!(
                "{}{:.1}%  {}/s",
                self.files_prefix(),
                pct,
                format_speed(self.download_speed()),
            ),
            RowState::Downloading => {
                let eta = self
                    .eta_seconds()
                    .map(format_duration)
                    .unwrap_or_else(|| "\u{2014}".to_string());
                format!(
                    "{:.1}%  {}/s  ETA {}",
                    pct,
                    format_speed(self.download_speed()),
                    eta
                )
            }
            RowState::Connecting => "connecting\u{2026}".to_string(),
            RowState::Seeding => {
                format!("seeding  \u{2191} {}/s", format_speed(self.upload_speed()))
            }
            RowState::Paused => format!("{}{:.1}%  paused", self.files_prefix(), pct),
            RowState::Queued => format!("queued \u{b7} {} priority", self.priority()),
            RowState::Completed => match self {
                Self::Download(d) => {
                    let size =
                        format_size(d.progress.total_size.unwrap_or(d.progress.completed_size));
                    match &d.metadata.checksum {
                        Some(sum) => format!(
                            "{}  {} \u{2713}",
                            size,
                            sum.algorithm.to_string().to_lowercase()
                        ),
                        None => size,
                    }
                }
                Self::Mirror(m) => format!(
                    "{} files  {}",
                    m.status.progress.total_children,
                    format_size(m.status.progress.completed_size)
                ),
            },
            RowState::Error => match self {
                Self::Download(d) => {
                    let label = match &d.state {
                        DownloadState::Error { kind, .. } => error_kind_label(kind),
                        _ => "failed".to_string(),
                    };
                    format!("{:.1}%  {} \u{b7} r retries", pct, label)
                }
                Self::Mirror(m) => format!("{} files failed", m.status.progress.failed_children),
            },
            RowState::Partial => match self {
                Self::Mirror(m) => {
                    let p = &m.status.progress;
                    format!(
                        "{}/{} files  {} failed",
                        p.completed_children,
                        p.total_children,
                        p.failed_children + p.missing_children
                    )
                }
                Self::Download(_) => String::new(),
            },
            RowState::Empty => "no files left".to_string(),
        }
    }

    /// The same summary squeezed into about 17 columns (compact layout).
    pub fn meta_short(&self) -> String {
        let pct = self.percent();
        match self.state() {
            RowState::Downloading => {
                format!("{:.0}% {}/s", pct, format_speed(self.download_speed()))
            }
            RowState::Connecting => "connecting\u{2026}".to_string(),
            RowState::Seeding => format!("\u{2191} {}/s", format_speed(self.upload_speed())),
            RowState::Paused => format!("{:.0}% paused", pct),
            RowState::Queued => format!("queued \u{b7} {}", self.priority()),
            RowState::Completed => format!(
                "{} \u{2713}",
                format_size(self.total_size().unwrap_or(self.completed_size()))
            ),
            RowState::Error => "failed".to_string(),
            RowState::Partial => "partial".to_string(),
            RowState::Empty => "empty".to_string(),
        }
    }

    /// Text searched by the `/` filter.
    pub fn search_text(&self) -> (String, String, String) {
        match self {
            Self::Download(d) => (
                d.metadata.name.to_lowercase(),
                d.metadata
                    .url
                    .clone()
                    .or_else(|| d.metadata.magnet_uri.clone())
                    .unwrap_or_default()
                    .to_lowercase(),
                self.state().label().to_lowercase(),
            ),
            Self::Mirror(m) => (
                m.display_name().to_lowercase(),
                m.root_url.to_lowercase(),
                self.state().label().to_lowercase(),
            ),
        }
    }
}

/// Which rows a tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    All,
    Active,
    Completed,
    Mirrors,
}

impl ViewMode {
    pub const ALL: [ViewMode; 4] = [Self::All, Self::Active, Self::Completed, Self::Mirrors];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Active => "Active",
            Self::Completed => "Completed",
            Self::Mirrors => "Mirrors",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Completed => "Done",
            other => other.label(),
        }
    }

    pub fn includes(self, row: &Row) -> bool {
        match self {
            Self::All => true,
            Self::Active => row.state().is_active(),
            Self::Completed => matches!(row.state(), RowState::Completed | RowState::Seeding),
            Self::Mirrors => matches!(row, Row::Mirror(_)),
        }
    }
}

/// Group engine snapshots into list rows: mirror jobs first absorb their
/// children, every remaining download is a row of its own.
pub fn build_rows(
    downloads: Vec<DownloadStatus>,
    jobs: Vec<(gosh_dl::TrackedRecursiveJob, RecursiveJobStatus)>,
) -> Vec<Row> {
    let mut by_id: std::collections::HashMap<DownloadId, DownloadStatus> =
        downloads.into_iter().map(|d| (d.id, d)).collect();
    let mut rows = Vec::new();
    for (job, status) in jobs {
        let children: Vec<DownloadStatus> = job
            .child_ids
            .iter()
            .filter_map(|id| by_id.remove(id))
            .collect();
        rows.push(Row::Mirror(Box::new(MirrorRow {
            id: job.id,
            root_url: job.root_url,
            created_at: job.created_at,
            status,
            children,
        })));
    }
    rows.extend(by_id.into_values().map(|d| Row::Download(Box::new(d))));
    rows.sort_by_key(|r| r.created_at());
    rows
}

/// The schedule rule in force right now, as `(index, rule)`.
pub fn active_schedule_rule(
    config: &CliConfig,
) -> Option<(usize, &crate::config::ScheduleRuleConfig)> {
    let now = Local::now();
    active_schedule_rule_at(config, now.hour() as u8, now.weekday())
}

/// First enabled rule matching the given local hour and weekday: the same
/// "first match wins" order the engine applies.
pub fn active_schedule_rule_at(
    config: &CliConfig,
    hour: u8,
    weekday: chrono::Weekday,
) -> Option<(usize, &crate::config::ScheduleRuleConfig)> {
    config
        .schedule
        .rules
        .iter()
        .enumerate()
        .find(|(_, rule)| rule.enabled && rule.matches(hour, weekday))
}

/// `~/Downloads` instead of `/home/alice/Downloads`.
pub fn tilde_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            return if rest.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display())
            };
        }
    }
    path.display().to_string()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use gosh_dl::{DownloadMetadata, DownloadProgress, RecursiveJobProgress};

    pub(crate) fn download(name: &str, kind: DownloadKind, state: DownloadState) -> DownloadStatus {
        DownloadStatus {
            id: DownloadId::new(),
            kind,
            state,
            priority: gosh_dl::DownloadPriority::Normal,
            progress: DownloadProgress {
                total_size: Some(1000),
                completed_size: 250,
                download_speed: 100,
                upload_speed: 0,
                connections: 4,
                seeders: 0,
                peers: 0,
                eta_seconds: Some(8),
            },
            metadata: DownloadMetadata {
                name: name.to_string(),
                url: Some(format!("https://example.com/{name}")),
                magnet_uri: None,
                info_hash: None,
                save_dir: std::path::PathBuf::from("/tmp"),
                filename: Some(name.to_string()),
                user_agent: None,
                referer: None,
                headers: Vec::new(),
                cookies: Vec::new(),
                checksum: None,
                mirrors: Vec::new(),
                etag: None,
                last_modified: None,
            },
            torrent_info: None,
            peers: None,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    #[test]
    fn mirror_children_are_folded_into_one_row() {
        let a = download("a.bin", DownloadKind::Http, DownloadState::Downloading);
        let b = download("b.bin", DownloadKind::Http, DownloadState::Completed);
        let solo = download("solo.iso", DownloadKind::Http, DownloadState::Paused);
        let job = gosh_dl::TrackedRecursiveJob {
            id: uuid::Uuid::new_v4(),
            root_url: "https://example.com/pub/".to_string(),
            child_ids: vec![a.id, b.id],
            created_at: Utc::now(),
        };
        let status = RecursiveJobStatus {
            root_url: job.root_url.clone(),
            child_ids: job.child_ids.clone(),
            state: RecursiveJobState::Running,
            progress: RecursiveJobProgress {
                total_children: 2,
                active_children: 1,
                completed_children: 1,
                completed_size: 1250,
                total_size: Some(2000),
                ..Default::default()
            },
        };

        let rows = build_rows(vec![a, b, solo.clone()], vec![(job, status)]);
        assert_eq!(rows.len(), 2, "two children collapse into the mirror row");

        let mirror = rows.iter().find(|r| matches!(r, Row::Mirror(_))).unwrap();
        assert_eq!(mirror.tag(), "MIRROR");
        assert_eq!(mirror.name(), "example.com/pub/");
        assert_eq!(mirror.state(), RowState::Downloading);
        assert!(
            mirror.meta().starts_with("1/2 files  62.5%"),
            "{}",
            mirror.meta()
        );
        assert_eq!(mirror.download_speed(), 100, "only active children count");

        let single = rows
            .iter()
            .find(|r| r.id() == RowId::Download(solo.id))
            .unwrap();
        assert_eq!(single.state(), RowState::Paused);
        assert!(ViewMode::Mirrors.includes(mirror));
        assert!(!ViewMode::Mirrors.includes(single));
        assert!(ViewMode::Active.includes(mirror));
        assert!(!ViewMode::Active.includes(single));
    }

    #[test]
    fn row_summaries_follow_the_state() {
        let mut d = download("file.iso", DownloadKind::Http, DownloadState::Downloading);
        let row = Row::Download(Box::new(d.clone()));
        assert_eq!(row.tag(), "HTTP\u{d7}4");
        assert_eq!(row.meta(), "25.0%  100 B/s  ETA 0:08");
        assert_eq!(row.meta_short(), "25% 100 B/s");

        d.state = DownloadState::Paused;
        let row = Row::Download(Box::new(d.clone()));
        assert_eq!(row.tag(), "HTTP", "no live connection count when idle");
        assert_eq!(row.meta(), "25.0%  paused");

        d.state = DownloadState::Error {
            kind: "Network { kind: HttpStatus(403), message: \"x\", retryable: false }".into(),
            message: "HTTP error: 403".into(),
            retryable: false,
        };
        let row = Row::Download(Box::new(d.clone()));
        assert_eq!(row.meta(), "25.0%  HTTP 403 \u{b7} r retries");

        d.state = DownloadState::Completed;
        d.metadata.checksum = Some(gosh_dl::http::ExpectedChecksum::sha256("ab".repeat(32)));
        let row = Row::Download(Box::new(d));
        assert_eq!(row.percent(), 100.0);
        assert!(row.meta().ends_with("sha256 \u{2713}"), "{}", row.meta());
    }

    #[test]
    fn schedule_rule_lookup_skips_disabled_rules() {
        let mut config = CliConfig::default();
        let rule = |start, end, enabled| crate::config::ScheduleRuleConfig {
            start_hour: start,
            end_hour: end,
            days: "all".to_string(),
            download_limit: Some("1M".to_string()),
            upload_limit: None,
            enabled,
        };
        config.schedule.rules = vec![rule(0, 23, false), rule(9, 17, true)];
        let monday = chrono::Weekday::Mon;
        assert_eq!(
            active_schedule_rule_at(&config, 10, monday).map(|(i, _)| i),
            Some(1),
            "the disabled catch-all must not win"
        );
        assert!(active_schedule_rule_at(&config, 20, monday).is_none());
        // Disabled rules are not handed to the engine either.
        assert_eq!(config.to_engine_config().schedule_rules.len(), 1);
    }
}
