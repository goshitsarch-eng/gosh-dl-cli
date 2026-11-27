//! Table of the settings shown in the TUI settings dialog.
//!
//! Each row names a configuration key; values are read and written through
//! the same functions `gosh config get/set` uses, so the dialog and the
//! command line can never disagree about what a key accepts.

use anyhow::Result;

use crate::commands::config::{apply_value, value_of};
use crate::config::CliConfig;

pub const TAB_NAMES: [&str; 5] = ["General", "Engine", "BitTorrent", "Interface", "Schedule"];

/// Index of the Schedule tab, which shows rules instead of key/value rows.
pub const SCHEDULE_TAB: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Editor {
    /// Free text, edited in place
    Text,
    /// ON / OFF, toggled with Enter or Space
    Bool,
    /// One of a fixed set, cycled with Enter or Space
    Choice(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy)]
pub struct Setting {
    pub key: &'static str,
    pub label: &'static str,
    pub editor: Editor,
    /// Takes effect only after restarting gosh
    pub restart: bool,
}

const fn row(key: &'static str, label: &'static str, editor: Editor, restart: bool) -> Setting {
    Setting {
        key,
        label,
        editor,
        restart,
    }
}

const GENERAL: &[Setting] = &[
    row(
        "general.download_dir",
        "Download directory",
        Editor::Text,
        false,
    ),
    row(
        "general.file_conflict",
        "When a file name is taken",
        Editor::Choice(&["overwrite", "rename", "fail"]),
        false,
    ),
    row(
        "general.storage_backend",
        "Storage backend",
        Editor::Choice(&["sqlite", "file", "none"]),
        true,
    ),
    row("general.database_path", "Database path", Editor::Text, true),
    row(
        "general.log_level",
        "Log level",
        Editor::Choice(&["info", "debug", "trace", "warn", "error", "off"]),
        true,
    ),
    row("general.log_file", "Log file", Editor::Text, true),
];

const ENGINE: &[Setting] = &[
    row(
        "engine.max_concurrent_downloads",
        "Max concurrent downloads",
        Editor::Text,
        false,
    ),
    row(
        "engine.max_connections_per_download",
        "Connections per download",
        Editor::Text,
        false,
    ),
    row(
        "engine.min_segment_size",
        "Min segment size",
        Editor::Text,
        false,
    ),
    row(
        "engine.global_download_limit",
        "Global download limit",
        Editor::Text,
        false,
    ),
    row(
        "engine.global_upload_limit",
        "Global upload limit",
        Editor::Text,
        false,
    ),
    row("engine.max_retries", "Max retries", Editor::Text, true),
    row(
        "engine.connect_timeout",
        "Connect timeout (s)",
        Editor::Text,
        true,
    ),
    row(
        "engine.read_timeout",
        "Read timeout (s)",
        Editor::Text,
        true,
    ),
    row("engine.user_agent", "User agent", Editor::Text, true),
    row("engine.proxy_url", "Proxy", Editor::Text, true),
    row(
        "engine.accept_invalid_certs",
        "Accept invalid TLS certs",
        Editor::Bool,
        true,
    ),
];

const BITTORRENT: &[Setting] = &[
    row("engine.enable_dht", "DHT", Editor::Bool, false),
    row(
        "engine.enable_pex",
        "Peer exchange (PEX)",
        Editor::Bool,
        false,
    ),
    row(
        "engine.enable_lpd",
        "Local peer discovery (LPD)",
        Editor::Bool,
        false,
    ),
    row(
        "engine.max_peers",
        "Max peers per torrent",
        Editor::Text,
        false,
    ),
    row(
        "engine.seed_ratio",
        "Seed ratio (0 = unlimited)",
        Editor::Text,
        false,
    ),
];

const INTERFACE: &[Setting] = &[
    row(
        "tui.theme",
        "Theme",
        Editor::Choice(&["dark", "light", "macchiato"]),
        false,
    ),
    row(
        "tui.refresh_rate_ms",
        "Refresh rate (ms)",
        Editor::Text,
        false,
    ),
    row("tui.show_speed_graph", "Speed graph", Editor::Bool, false),
    row("tui.show_peers", "Swarm details", Editor::Bool, false),
];

/// Key/value rows of a tab (empty for the Schedule tab).
pub fn rows(tab: usize) -> &'static [Setting] {
    match tab {
        0 => GENERAL,
        1 => ENGINE,
        2 => BITTORRENT,
        3 => INTERFACE,
        _ => &[],
    }
}

/// How many selectable rows a tab has for the given draft config.
pub fn row_count(tab: usize, draft: &CliConfig) -> usize {
    if tab == SCHEDULE_TAB {
        draft.schedule.rules.len()
    } else {
        rows(tab).len()
    }
}

/// Keys holding a byte count, shown as `2M` instead of `2097152`.
const SIZE_KEYS: [&str; 3] = [
    "engine.min_segment_size",
    "engine.global_download_limit",
    "engine.global_upload_limit",
];

/// A byte count in the shortest form the size parser reads back exactly.
fn compact_size(bytes: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(1 << 30, "G"), (1 << 20, "M"), (1 << 10, "K")];
    for (unit, suffix) in UNITS {
        if bytes >= unit && bytes.is_multiple_of(unit) {
            return format!("{}{suffix}", bytes / unit);
        }
    }
    bytes.to_string()
}

fn readable(setting: &Setting, raw: String) -> String {
    if SIZE_KEYS.contains(&setting.key) {
        if let Ok(bytes) = raw.parse::<u64>() {
            return compact_size(bytes);
        }
    }
    raw
}

/// Whether the value is a file system path (shown from its end when cut).
pub fn is_path(setting: &Setting) -> bool {
    setting.key.ends_with("_dir")
        || setting.key.ends_with("_path")
        || setting.key.ends_with("_file")
}

/// The value as shown in the dialog.
pub fn display_value(draft: &CliConfig, setting: &Setting) -> String {
    let raw = value_of(draft, setting.key).unwrap_or_default();
    match setting.editor {
        Editor::Bool => if raw == "true" { "ON" } else { "OFF" }.to_string(),
        _ if raw == "unset" => "\u{2014}".to_string(),
        _ => readable(setting, raw),
    }
}

/// The text an in-place edit starts from (empty for unset values).
pub fn edit_value(draft: &CliConfig, setting: &Setting) -> String {
    let raw = value_of(draft, setting.key).unwrap_or_default();
    if raw == "unset" || raw == "unlimited" {
        String::new()
    } else {
        readable(setting, raw)
    }
}

/// Store an edited value. The draft is only replaced when the new value
/// parses *and* the resulting configuration is valid, so a typo can never
/// leave a half-applied draft behind.
pub fn apply(draft: &mut CliConfig, setting: &Setting, value: &str) -> Result<()> {
    let mut candidate = draft.clone();
    apply_value(&mut candidate, setting.key, value)?;
    candidate.validate()?;
    *draft = candidate;
    Ok(())
}

/// Toggle a boolean or step a choice to its next option.
pub fn activate(draft: &mut CliConfig, setting: &Setting) -> Result<()> {
    let current = value_of(draft, setting.key).unwrap_or_default();
    let next = match setting.editor {
        Editor::Bool => if current == "true" { "false" } else { "true" }.to_string(),
        Editor::Choice(options) => {
            let index = options.iter().position(|o| *o == current).unwrap_or(0);
            options[(index + 1) % options.len()].to_string()
        }
        Editor::Text => return Ok(()),
    };
    apply(draft, setting, &next)
}

/// Whether any setting that needs a restart differs between two configs.
pub fn restart_needed(before: &CliConfig, after: &CliConfig) -> bool {
    (0..SCHEDULE_TAB)
        .flat_map(rows)
        .filter(|s| s.restart)
        .any(|s| value_of(before, s.key).ok() != value_of(after, s.key).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_maps_to_a_real_config_key() {
        let mut config = CliConfig::default();
        for tab in 0..SCHEDULE_TAB {
            for setting in rows(tab) {
                let value = value_of(&config, setting.key)
                    .unwrap_or_else(|e| panic!("{} is not readable: {e}", setting.key));
                // Whatever is displayed must be accepted back unchanged.
                apply_value(&mut config, setting.key, &value)
                    .unwrap_or_else(|e| panic!("{} rejects its own value: {e}", setting.key));
                if let Editor::Choice(options) = setting.editor {
                    assert!(
                        options.contains(&value.as_str()),
                        "{}: default {value:?} is not among the choices",
                        setting.key
                    );
                }
            }
        }
    }

    #[test]
    fn invalid_edits_leave_the_draft_untouched() {
        let mut draft = CliConfig::default();
        draft.engine.global_download_limit = Some(1024);
        let limit = &ENGINE[3];
        assert_eq!(limit.key, "engine.global_download_limit");
        assert!(apply(&mut draft, limit, "typo").is_err());
        assert_eq!(draft.engine.global_download_limit, Some(1024));

        // Parses, but fails whole-config validation.
        let concurrent = &ENGINE[0];
        assert!(apply(&mut draft, concurrent, "0").is_err());
        assert_eq!(draft.engine.max_concurrent_downloads, 5);

        // An empty limit means unlimited.
        apply(&mut draft, limit, "").unwrap();
        assert_eq!(draft.engine.global_download_limit, None);
        apply(&mut draft, limit, "2M").unwrap();
        assert_eq!(draft.engine.global_download_limit, Some(2 * 1024 * 1024));
    }

    #[test]
    fn sizes_are_shown_in_a_form_that_reads_back_exactly() {
        for (bytes, text) in [
            (2 * 1024 * 1024, "2M"),
            (512 * 1024, "512K"),
            (3 << 30, "3G"),
            (1500, "1500"),
            (1536 * 1024, "1536K"),
        ] {
            assert_eq!(compact_size(bytes), text);
            assert_eq!(crate::util::parse_speed(text).unwrap(), bytes);
        }
        let mut draft = CliConfig::default();
        let limit = &ENGINE[3];
        assert_eq!(display_value(&draft, limit), "unlimited");
        assert_eq!(edit_value(&draft, limit), "");
        draft.engine.global_download_limit = Some(2 * 1024 * 1024);
        assert_eq!(display_value(&draft, limit), "2M");
        assert_eq!(edit_value(&draft, limit), "2M");
        assert!(is_path(&GENERAL[0]) && !is_path(&GENERAL[1]));
    }

    #[test]
    fn choices_cycle_and_booleans_toggle() {
        let mut draft = CliConfig::default();
        let conflict = &GENERAL[1];
        assert_eq!(display_value(&draft, conflict), "overwrite");
        activate(&mut draft, conflict).unwrap();
        assert_eq!(
            draft.general.file_conflict,
            crate::config::FileConflict::Rename
        );
        activate(&mut draft, conflict).unwrap();
        activate(&mut draft, conflict).unwrap();
        assert_eq!(
            draft.general.file_conflict,
            crate::config::FileConflict::Overwrite
        );

        let dht = &BITTORRENT[0];
        assert_eq!(display_value(&draft, dht), "ON");
        activate(&mut draft, dht).unwrap();
        assert!(!draft.engine.enable_dht);
    }

    #[test]
    fn restart_is_flagged_only_for_restart_settings() {
        let before = CliConfig::default();
        let mut after = before.clone();
        after.tui.theme = "light".into();
        after.engine.max_peers = 10;
        assert!(!restart_needed(&before, &after));
        after.engine.proxy_url = Some("socks5://127.0.0.1:1080".into());
        assert!(restart_needed(&before, &after));
    }
}
