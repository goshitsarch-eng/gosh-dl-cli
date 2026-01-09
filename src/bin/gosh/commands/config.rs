use anyhow::Result;

use crate::cli::{ConfigAction, ConfigArgs};
use crate::config::CliConfig;

pub async fn execute(args: ConfigArgs, config: &CliConfig) -> Result<()> {
    match args.action {
        ConfigAction::Show => show_config(config),
        ConfigAction::Path => show_path(),
        ConfigAction::Get { key } => get_config_value(config, &key),
        ConfigAction::Set { key, value } => set_config_value(&key, &value),
    }
}

fn show_config(config: &CliConfig) -> Result<()> {
    let toml_str = toml::to_string_pretty(config)?;
    println!("{}", toml_str);
    Ok(())
}

fn show_path() -> Result<()> {
    let path = CliConfig::default_path();
    println!("{}", path.display());

    if path.exists() {
        println!("(file exists)");
    } else {
        println!("(file does not exist - using defaults)");
    }

    Ok(())
}

fn get_config_value(config: &CliConfig, key: &str) -> Result<()> {
    let parts: Vec<&str> = key.split('.').collect();

    let value = match parts.as_slice() {
        ["general", "download_dir"] => config.general.download_dir.display().to_string(),
        ["general", "database_path"] => config.general.database_path.display().to_string(),
        ["general", "log_level"] => config.general.log_level.clone(),
        ["engine", "max_concurrent_downloads"] => {
            config.engine.max_concurrent_downloads.to_string()
        }
        ["engine", "max_connections_per_download"] => {
            config.engine.max_connections_per_download.to_string()
        }
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
        ["tui", "refresh_rate_ms"] => config.tui.refresh_rate_ms.to_string(),
        ["tui", "theme"] => config.tui.theme.clone(),
        ["tui", "show_speed_graph"] => config.tui.show_speed_graph.to_string(),
        ["tui", "show_peers"] => config.tui.show_peers.to_string(),
        _ => anyhow::bail!("Unknown configuration key: {}", key),
    };

    println!("{}", value);
    Ok(())
}

fn set_config_value(key: &str, value: &str) -> Result<()> {
    // Load current config or create default
    let mut config = CliConfig::load(None)?;

    let parts: Vec<&str> = key.split('.').collect();

    match parts.as_slice() {
        ["general", "download_dir"] => {
            config.general.download_dir = value.into();
        }
        ["general", "database_path"] => {
            config.general.database_path = value.into();
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
        ["engine", "global_download_limit"] => {
            config.engine.global_download_limit = if value == "unlimited" || value == "0" {
                None
            } else {
                Some(parse_size(value)?)
            };
        }
        ["engine", "global_upload_limit"] => {
            config.engine.global_upload_limit = if value == "unlimited" || value == "0" {
                None
            } else {
                Some(parse_size(value)?)
            };
        }
        ["engine", "user_agent"] => {
            config.engine.user_agent = value.to_string();
        }
        ["engine", "enable_dht"] => {
            config.engine.enable_dht = value.parse()?;
        }
        ["engine", "enable_pex"] => {
            config.engine.enable_pex = value.parse()?;
        }
        ["engine", "enable_lpd"] => {
            config.engine.enable_lpd = value.parse()?;
        }
        ["engine", "max_peers"] => {
            config.engine.max_peers = value.parse()?;
        }
        ["engine", "seed_ratio"] => {
            config.engine.seed_ratio = value.parse()?;
        }
        ["tui", "refresh_rate_ms"] => {
            config.tui.refresh_rate_ms = value.parse()?;
        }
        ["tui", "theme"] => {
            config.tui.theme = value.to_string();
        }
        ["tui", "show_speed_graph"] => {
            config.tui.show_speed_graph = value.parse()?;
        }
        ["tui", "show_peers"] => {
            config.tui.show_peers = value.parse()?;
        }
        _ => anyhow::bail!("Unknown configuration key: {}", key),
    }

    // Save the updated config
    config.save(None)?;
    println!("Configuration saved: {} = {}", key, value);

    Ok(())
}

fn parse_size(s: &str) -> Result<u64> {
    let s = s.trim().to_uppercase();

    if let Some(num) = s.strip_suffix('K') {
        Ok(num.trim().parse::<u64>()? * 1024)
    } else if let Some(num) = s.strip_suffix('M') {
        Ok(num.trim().parse::<u64>()? * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix('G') {
        Ok(num.trim().parse::<u64>()? * 1024 * 1024 * 1024)
    } else {
        Ok(s.parse()?)
    }
}
