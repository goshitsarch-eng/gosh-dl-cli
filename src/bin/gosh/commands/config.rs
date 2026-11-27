use std::path::Path;

use anyhow::Result;

use crate::cli::{ConfigAction, ConfigArgs};
use crate::config::CliConfig;

pub async fn execute(
    args: ConfigArgs,
    config: &CliConfig,
    config_path: Option<&Path>,
) -> Result<()> {
    match args.action {
        ConfigAction::Show => show_config(config),
        ConfigAction::Path => show_path(config_path),
        ConfigAction::Get { key } => get_config_value(config, &key),
        ConfigAction::Set { key, value } => set_config_value(&key, &value, config_path),
    }
}

fn show_config(config: &CliConfig) -> Result<()> {
    let toml_str = toml::to_string_pretty(config)?;
    println!("{}", toml_str);
    Ok(())
}

fn show_path(config_path: Option<&Path>) -> Result<()> {
    let path = config_path
        .map(Path::to_path_buf)
        .unwrap_or_else(CliConfig::default_path);
    println!("{}", path.display());

    if path.exists() {
        println!("(file exists)");
    } else {
        println!("(file does not exist - using defaults)");
    }

    Ok(())
}

fn get_config_value(config: &CliConfig, key: &str) -> Result<()> {
    println!("{}", value_of(config, key)?);
    Ok(())
}

/// The current value of a configuration key, formatted the way
/// `gosh config set` accepts it back. Shared with the TUI settings dialog.
pub fn value_of(config: &CliConfig, key: &str) -> Result<String> {
    let parts: Vec<&str> = key.split('.').collect();

    let value = match parts.as_slice() {
        ["general", "download_dir"] => config.general.download_dir.display().to_string(),
        ["general", "database_path"] => config.general.database_path.display().to_string(),
        ["general", "storage_backend"] => format_storage_backend(config.general.storage_backend),
        ["general", "file_conflict"] => config.general.file_conflict.as_str().to_string(),
        ["general", "log_file"] => display_optional_path(config.general.log_file.as_ref()),
        ["general", "log_level"] => config.general.log_level.clone(),
        ["engine", "max_concurrent_downloads"] => {
            config.engine.max_concurrent_downloads.to_string()
        }
        ["engine", "max_connections_per_download"] => {
            config.engine.max_connections_per_download.to_string()
        }
        ["engine", "min_segment_size"] => config.engine.min_segment_size.to_string(),
        ["engine", "global_download_limit"] => config
            .engine
            .global_download_limit
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unlimited".to_string()),
        ["engine", "global_upload_limit"] => config
            .engine
            .global_upload_limit
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unlimited".to_string()),
        ["engine", "user_agent"] => config.engine.user_agent.clone(),
        ["engine", "enable_dht"] => config.engine.enable_dht.to_string(),
        ["engine", "enable_pex"] => config.engine.enable_pex.to_string(),
        ["engine", "enable_lpd"] => config.engine.enable_lpd.to_string(),
        ["engine", "max_peers"] => config.engine.max_peers.to_string(),
        ["engine", "seed_ratio"] => config.engine.seed_ratio.to_string(),
        ["engine", "proxy_url"] => display_optional_string(config.engine.proxy_url.as_ref()),
        ["engine", "connect_timeout"] => config.engine.connect_timeout.to_string(),
        ["engine", "read_timeout"] => config.engine.read_timeout.to_string(),
        ["engine", "max_retries"] => config.engine.max_retries.to_string(),
        ["engine", "accept_invalid_certs"] => config.engine.accept_invalid_certs.to_string(),
        ["tui", "refresh_rate_ms"] => config.tui.refresh_rate_ms.to_string(),
        ["tui", "theme"] => config.tui.theme.clone(),
        ["tui", "show_speed_graph"] => config.tui.show_speed_graph.to_string(),
        ["tui", "show_peers"] => config.tui.show_peers.to_string(),
        _ => anyhow::bail!("Unknown configuration key: {}", key),
    };

    Ok(value)
}

fn set_config_value(key: &str, value: &str, config_path: Option<&Path>) -> Result<()> {
    // Load current config from the same path the CLI used
    let mut config = CliConfig::load(config_path)?;
    apply_value(&mut config, key, value)?;

    // Validate before saving
    config.validate()?;

    // Save the updated config to the same path
    config.save(config_path)?;
    println!("Configuration saved: {} = {}", key, value);

    Ok(())
}

/// Parse `value` and store it under `key`. Does not validate the config as
/// a whole or save it. Shared with the TUI settings dialog.
pub fn apply_value(config: &mut CliConfig, key: &str, value: &str) -> Result<()> {
    let parts: Vec<&str> = key.split('.').collect();

    match parts.as_slice() {
        ["general", "download_dir"] => {
            config.general.download_dir = value.into();
        }
        ["general", "database_path"] => {
            config.general.database_path = value.into();
        }
        ["general", "storage_backend"] => {
            config.general.storage_backend = parse_storage_backend(value)?;
        }
        ["general", "file_conflict"] => {
            config.general.file_conflict = crate::config::FileConflict::parse(value)?;
        }
        ["general", "log_file"] => {
            config.general.log_file = parse_optional_path(value);
        }
        ["general", "log_level"] => {
            config.general.log_level = value.to_string();
        }
        ["engine", "max_concurrent_downloads"] => {
            config.engine.max_concurrent_downloads = value.parse()?;
        }
        ["engine", "max_connections_per_download"] => {
            config.engine.max_connections_per_download = value.parse()?;
        }
        ["engine", "min_segment_size"] => {
            config.engine.min_segment_size = parse_size(value)?;
        }
        ["engine", "global_download_limit"] => {
            config.engine.global_download_limit = if is_unlimited(value) {
                None
            } else {
                Some(parse_size(value)?)
            };
        }
        ["engine", "global_upload_limit"] => {
            config.engine.global_upload_limit = if is_unlimited(value) {
                None
            } else {
                Some(parse_size(value)?)
            };
        }
        ["engine", "user_agent"] => {
            config.engine.user_agent = value.to_string();
        }
        ["engine", "enable_dht"] => {
            config.engine.enable_dht = parse_bool(value)?;
        }
        ["engine", "enable_pex"] => {
            config.engine.enable_pex = parse_bool(value)?;
        }
        ["engine", "enable_lpd"] => {
            config.engine.enable_lpd = parse_bool(value)?;
        }
        ["engine", "max_peers"] => {
            config.engine.max_peers = value.parse()?;
        }
        ["engine", "seed_ratio"] => {
            config.engine.seed_ratio = value.parse()?;
        }
        ["engine", "proxy_url"] => {
            config.engine.proxy_url = parse_optional_string(value);
        }
        ["engine", "connect_timeout"] => {
            config.engine.connect_timeout = value.parse()?;
        }
        ["engine", "read_timeout"] => {
            config.engine.read_timeout = value.parse()?;
        }
        ["engine", "max_retries"] => {
            config.engine.max_retries = value.parse()?;
        }
        ["engine", "accept_invalid_certs"] => {
            config.engine.accept_invalid_certs = parse_bool(value)?;
        }
        ["tui", "refresh_rate_ms"] => {
            config.tui.refresh_rate_ms = value.parse()?;
        }
        ["tui", "theme"] => {
            config.tui.theme = value.to_string();
        }
        ["tui", "show_speed_graph"] => {
            config.tui.show_speed_graph = parse_bool(value)?;
        }
        ["tui", "show_peers"] => {
            config.tui.show_peers = parse_bool(value)?;
        }
        _ => anyhow::bail!("Unknown configuration key: {}", key),
    }

    Ok(())
}

fn is_unlimited(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "unlimited" | "none" | "unset"
    )
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Ok(true),
        "false" | "off" | "no" | "0" => Ok(false),
        other => anyhow::bail!("Expected true/false (or on/off), got '{}'", other),
    }
}

fn format_storage_backend(backend: crate::config::StorageBackend) -> String {
    use crate::config::StorageBackend;
    match backend {
        StorageBackend::Sqlite => "sqlite",
        StorageBackend::File => "file",
        StorageBackend::None => "none",
    }
    .to_string()
}

fn parse_storage_backend(value: &str) -> Result<crate::config::StorageBackend> {
    use crate::config::StorageBackend;
    match value.to_lowercase().as_str() {
        "sqlite" => Ok(StorageBackend::Sqlite),
        "file" => Ok(StorageBackend::File),
        "none" => Ok(StorageBackend::None),
        other => anyhow::bail!(
            "Invalid storage backend '{}'. Expected: sqlite, file, none",
            other
        ),
    }
}

fn display_optional_path(path: Option<&std::path::PathBuf>) -> String {
    path.map(|p| p.display().to_string())
        .unwrap_or_else(|| "unset".to_string())
}

fn display_optional_string(value: Option<&String>) -> String {
    value.cloned().unwrap_or_else(|| "unset".to_string())
}

fn parse_optional_path(value: &str) -> Option<std::path::PathBuf> {
    if value == "unset" || value.trim().is_empty() {
        None
    } else {
        Some(value.into())
    }
}

fn parse_optional_string(value: &str) -> Option<String> {
    if value == "unset" || value.trim().is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Sizes and speeds share one parser: the copy that used to live here
/// multiplied without an overflow check, so `config set
/// engine.global_download_limit 99999999999G` panicked in debug builds and
/// silently wrapped to a tiny limit in release builds.
fn parse_size(s: &str) -> Result<u64> {
    crate::util::parse_speed(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn get_optional_values_print_unset() {
        let config = CliConfig::default();

        assert_eq!(
            display_optional_path(config.general.log_file.as_ref()),
            "unset"
        );
        assert_eq!(
            display_optional_string(config.engine.proxy_url.as_ref()),
            "unset"
        );
    }

    #[test]
    fn set_and_get_new_scalar_keys_round_trip() {
        let tempdir = TempDir::new().unwrap();
        let config_path = tempdir.path().join("config.toml");

        set_config_value(
            "general.log_file",
            "/tmp/gosh.log",
            Some(config_path.as_path()),
        )
        .unwrap();
        set_config_value(
            "engine.proxy_url",
            "http://localhost:8080",
            Some(config_path.as_path()),
        )
        .unwrap();
        set_config_value("engine.connect_timeout", "15", Some(config_path.as_path())).unwrap();
        set_config_value("engine.read_timeout", "45", Some(config_path.as_path())).unwrap();
        set_config_value("engine.max_retries", "7", Some(config_path.as_path())).unwrap();
        set_config_value(
            "engine.accept_invalid_certs",
            "true",
            Some(config_path.as_path()),
        )
        .unwrap();

        let config = CliConfig::load(Some(config_path.as_path())).unwrap();
        assert_eq!(
            display_optional_path(config.general.log_file.as_ref()),
            "/tmp/gosh.log"
        );
        assert_eq!(
            display_optional_string(config.engine.proxy_url.as_ref()),
            "http://localhost:8080"
        );
        assert_eq!(config.engine.connect_timeout, 15);
        assert_eq!(config.engine.read_timeout, 45);
        assert_eq!(config.engine.max_retries, 7);
        assert!(config.engine.accept_invalid_certs);
    }

    #[test]
    fn set_unset_clears_optional_values() {
        let tempdir = TempDir::new().unwrap();
        let config_path = tempdir.path().join("config.toml");

        set_config_value(
            "general.log_file",
            "/tmp/gosh.log",
            Some(config_path.as_path()),
        )
        .unwrap();
        set_config_value("general.log_file", "unset", Some(config_path.as_path())).unwrap();
        set_config_value(
            "engine.proxy_url",
            "http://localhost:8080",
            Some(config_path.as_path()),
        )
        .unwrap();
        set_config_value("engine.proxy_url", "unset", Some(config_path.as_path())).unwrap();

        let config = CliConfig::load(Some(config_path.as_path())).unwrap();
        assert!(config.general.log_file.is_none());
        assert!(config.engine.proxy_url.is_none());
    }

    #[test]
    fn size_overflow_is_an_error_not_a_wrapped_value() {
        let tempdir = TempDir::new().unwrap();
        let config_path = tempdir.path().join("config.toml");
        let err = set_config_value(
            "engine.global_download_limit",
            "18446744073709551615G",
            Some(config_path.as_path()),
        )
        .unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
    }

    #[test]
    fn file_conflict_round_trips_and_rejects_typos() {
        let tempdir = TempDir::new().unwrap();
        let config_path = tempdir.path().join("config.toml");
        set_config_value(
            "general.file_conflict",
            "rename",
            Some(config_path.as_path()),
        )
        .unwrap();
        let config = CliConfig::load(Some(config_path.as_path())).unwrap();
        assert_eq!(
            config.general.file_conflict,
            crate::config::FileConflict::Rename
        );
        assert!(set_config_value(
            "general.file_conflict",
            "clobber",
            Some(config_path.as_path())
        )
        .is_err());
    }

    #[test]
    fn unknown_keys_still_error_cleanly() {
        let err = set_config_value("engine.unknown_key", "1", None).unwrap_err();
        assert!(err.to_string().contains("Unknown configuration key"));
    }
}
