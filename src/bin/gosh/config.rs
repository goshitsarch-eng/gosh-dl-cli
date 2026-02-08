use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CliConfig {
    pub general: GeneralConfig,
    pub engine: EngineSettings,
    pub tui: TuiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Default download directory
    pub download_dir: PathBuf,

    /// Database path for persistence
    pub database_path: PathBuf,

    /// Log file path (None = stderr only)
    pub log_file: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSettings {
    /// Maximum concurrent downloads
    pub max_concurrent_downloads: usize,

    /// Maximum connections per download
    pub max_connections_per_download: usize,

    /// Minimum segment size in bytes
    pub min_segment_size: u64,

    /// Global download speed limit (bytes/sec, None = unlimited)
    pub global_download_limit: Option<u64>,

    /// Global upload speed limit (bytes/sec, None = unlimited)
    pub global_upload_limit: Option<u64>,

    /// User agent string
    pub user_agent: String,

    /// Enable DHT for BitTorrent
    pub enable_dht: bool,

    /// Enable Peer Exchange for BitTorrent
    pub enable_pex: bool,

    /// Enable Local Peer Discovery for BitTorrent
    pub enable_lpd: bool,

    /// Maximum peers per torrent
    pub max_peers: usize,

    /// Default seed ratio (upload/download)
    pub seed_ratio: f64,

    /// HTTP proxy URL (http://, https://, or socks5://)
    pub proxy_url: Option<String>,

    /// Connect timeout in seconds
    pub connect_timeout: u64,

    /// Read timeout in seconds
    pub read_timeout: u64,

    /// Maximum retries for failed downloads
    pub max_retries: usize,

    /// Accept invalid TLS certificates (insecure)
    pub accept_invalid_certs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    /// Refresh rate in milliseconds
    pub refresh_rate_ms: u64,

    /// Color theme (dark, light)
    pub theme: String,

    /// Show speed graph
    pub show_speed_graph: bool,

    /// Show peer list for torrents
    pub show_peers: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        let download_dir = dirs::download_dir().unwrap_or_else(|| PathBuf::from("."));

        let data_dir = directories::ProjectDirs::from("com", "gosh", "gosh-dl")
            .map(|dirs| dirs.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            download_dir,
            database_path: data_dir.join("gosh.db"),
            log_file: None,
            log_level: "info".to_string(),
        }
    }
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: 5,
            max_connections_per_download: 8,
            min_segment_size: 1024 * 1024, // 1 MiB
            global_download_limit: None,
            global_upload_limit: None,
            user_agent: format!("gosh-dl/{}", env!("CARGO_PKG_VERSION")),
            enable_dht: true,
            enable_pex: true,
            enable_lpd: true,
            max_peers: 55,
            seed_ratio: 1.0,
            proxy_url: None,
            connect_timeout: 30,
            read_timeout: 60,
            max_retries: 3,
            accept_invalid_certs: false,
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            refresh_rate_ms: 250,
            theme: "dark".to_string(),
            show_speed_graph: true,
            show_peers: true,
        }
    }
}

impl CliConfig {
    /// Load configuration from file or use defaults
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = path.map(PathBuf::from).unwrap_or_else(Self::default_path);

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path).with_context(|| {
                format!("Failed to read config file: {}", config_path.display())
            })?;

            toml::from_str(&contents)
                .with_context(|| format!("Failed to parse config file: {}", config_path.display()))
        } else {
            Ok(Self::default())
        }
    }

    /// Get the default configuration file path
    pub fn default_path() -> PathBuf {
        directories::ProjectDirs::from("com", "gosh", "gosh-dl")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .unwrap_or_else(|| PathBuf::from("gosh-config.toml"))
    }

    /// Convert to engine configuration
    pub fn to_engine_config(&self) -> gosh_dl::config::EngineConfig {
        gosh_dl::config::EngineConfig {
            download_dir: self.general.download_dir.clone(),
            max_concurrent_downloads: self.engine.max_concurrent_downloads,
            max_connections_per_download: self.engine.max_connections_per_download,
            min_segment_size: self.engine.min_segment_size,
            global_download_limit: self.engine.global_download_limit,
            global_upload_limit: self.engine.global_upload_limit,
            schedule_rules: Vec::new(),
            user_agent: self.engine.user_agent.clone(),
            enable_dht: self.engine.enable_dht,
            enable_pex: self.engine.enable_pex,
            enable_lpd: self.engine.enable_lpd,
            max_peers: self.engine.max_peers,
            seed_ratio: self.engine.seed_ratio,
            database_path: Some(self.general.database_path.clone()),
            http: gosh_dl::config::HttpConfig {
                connect_timeout: self.engine.connect_timeout,
                read_timeout: self.engine.read_timeout,
                max_redirects: 10,
                max_retries: self.engine.max_retries,
                retry_delay_ms: 1000,
                max_retry_delay_ms: 30000,
                accept_invalid_certs: self.engine.accept_invalid_certs,
                proxy_url: self.engine.proxy_url.clone(),
            },
            torrent: gosh_dl::config::TorrentConfig::default(),
        }
    }

    /// Save configuration to file
    pub fn save(&self, path: Option<&Path>) -> Result<()> {
        let config_path = path.map(PathBuf::from).unwrap_or_else(Self::default_path);

        // Create parent directories if they don't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let contents = toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        std::fs::write(&config_path, contents)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
    }
}
