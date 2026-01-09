use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gosh")]
#[command(author, version, about = "Fast download manager with HTTP and BitTorrent support")]
#[command(propagate_version = true)]
#[command(after_help = "Run 'gosh' without arguments to start the interactive TUI.\n\
    Or pass URLs directly: gosh https://example.com/file.zip")]
pub struct Cli {
    /// Config file path (default: ~/.config/gosh/config.toml)
    #[arg(short, long, global = true, env = "GOSH_CONFIG")]
    pub config: Option<PathBuf>,

    /// Increase logging verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Output format for commands
    #[arg(long, value_enum, default_value = "table", global = true)]
    pub output: OutputFormat,

    /// Output directory for direct downloads
    #[arg(short = 'd', long, global = true)]
    pub dir: Option<PathBuf>,

    /// Output filename (only for single direct downloads)
    #[arg(short = 'o', long)]
    pub out: Option<String>,

    /// Custom headers for direct downloads (format: "Name: Value")
    #[arg(short = 'H', long = "header", value_name = "HEADER")]
    pub headers: Vec<String>,

    /// User agent string for direct downloads
    #[arg(long)]
    pub user_agent: Option<String>,

    /// Referer URL for direct downloads
    #[arg(long)]
    pub referer: Option<String>,

    /// Cookies for direct downloads (format: "name=value")
    #[arg(long = "cookie")]
    pub cookies: Vec<String>,

    /// Expected checksum (format: "md5:xxx" or "sha256:xxx")
    #[arg(long)]
    pub checksum: Option<String>,

    /// Maximum connections per download
    #[arg(short = 'x', long)]
    pub max_connections: Option<usize>,

    /// Maximum download speed (bytes/sec, supports K/M/G suffixes)
    #[arg(long)]
    pub max_speed: Option<String>,

    /// Sequential download mode (for torrents)
    #[arg(long)]
    pub sequential: bool,

    /// Select specific files (for torrents, comma-separated indices)
    #[arg(long)]
    pub select_files: Option<String>,

    /// Seed ratio limit (for torrents)
    #[arg(long)]
    pub seed_ratio: Option<f64>,

    /// URLs to download directly (without entering TUI)
    #[arg(value_name = "URL")]
    pub urls: Vec<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a new download (HTTP URL, magnet link, or torrent file)
    Add(AddArgs),

    /// List all downloads
    List(ListArgs),

    /// Show detailed status of a download
    Status(StatusArgs),

    /// Pause one or more downloads
    Pause(PauseArgs),

    /// Resume one or more paused downloads
    Resume(ResumeArgs),

    /// Cancel and optionally delete one or more downloads
    Cancel(CancelArgs),

    /// Set download priority
    Priority(PriorityArgs),

    /// Show global download/upload statistics
    Stats,

    /// Parse and show torrent file information
    Info(InfoArgs),

    /// Manage configuration
    Config(ConfigArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// URL, magnet link, or torrent file path
    /// Can be specified multiple times, or use '-' to read from stdin
    #[arg(required_unless_present = "input_file")]
    pub urls: Vec<String>,

    /// Read URLs from file (one per line)
    #[arg(short = 'i', long)]
    pub input_file: Option<PathBuf>,

    /// Output directory
    #[arg(short = 'd', long)]
    pub dir: Option<PathBuf>,

    /// Output filename (only for single downloads)
    #[arg(short = 'o', long)]
    pub out: Option<String>,

    /// Download priority
    #[arg(short = 'p', long, value_enum, default_value = "normal")]
    pub priority: Priority,

    /// Wait for download to complete (show progress)
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Custom headers (format: "Name: Value")
    #[arg(short = 'H', long = "header", value_name = "HEADER")]
    pub headers: Vec<String>,

    /// User agent string
    #[arg(long)]
    pub user_agent: Option<String>,

    /// Referer URL
    #[arg(long)]
    pub referer: Option<String>,

    /// Cookies (format: "name=value")
    #[arg(long = "cookie")]
    pub cookies: Vec<String>,

    /// Expected checksum (format: "md5:xxx" or "sha256:xxx")
    #[arg(long)]
    pub checksum: Option<String>,

    /// Maximum connections per download
    #[arg(short = 'x', long)]
    pub max_connections: Option<usize>,

    /// Maximum download speed (bytes/sec, supports K/M/G suffixes)
    #[arg(long)]
    pub max_speed: Option<String>,

    /// Sequential download mode (for torrents - download in order)
    #[arg(long)]
    pub sequential: bool,

    /// Select specific files (for torrents, comma-separated indices starting from 0)
    #[arg(long)]
    pub select_files: Option<String>,

    /// Seed ratio limit (for torrents, e.g., 1.0 = upload same amount as downloaded)
    #[arg(long)]
    pub seed_ratio: Option<f64>,
}

#[derive(Args)]
pub struct ListArgs {
    /// Filter by state
    #[arg(short = 's', long, value_enum)]
    pub state: Option<StateFilter>,

    /// Show only download IDs (useful for scripting)
    #[arg(long)]
    pub ids_only: bool,
}

#[derive(Args)]
pub struct StatusArgs {
    /// Download ID (full UUID or short GID)
    pub id: String,

    /// Show peer information (for torrents)
    #[arg(long)]
    pub peers: bool,

    /// Show file list (for torrents)
    #[arg(long)]
    pub files: bool,
}

#[derive(Args)]
pub struct PauseArgs {
    /// Download IDs to pause (use 'all' to pause all active downloads)
    #[arg(required = true)]
    pub ids: Vec<String>,
}

#[derive(Args)]
pub struct ResumeArgs {
    /// Download IDs to resume (use 'all' to resume all paused downloads)
    #[arg(required = true)]
    pub ids: Vec<String>,
}

#[derive(Args)]
pub struct CancelArgs {
    /// Download IDs to cancel
    #[arg(required = true)]
    pub ids: Vec<String>,

    /// Also delete downloaded files
    #[arg(long)]
    pub delete: bool,

    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Args)]
pub struct PriorityArgs {
    /// Download ID
    pub id: String,

    /// New priority level
    #[arg(value_enum)]
    pub priority: Priority,
}

#[derive(Args)]
pub struct InfoArgs {
    /// Path to torrent file
    pub file: PathBuf,
}

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Show configuration file path
    Path,
    /// Set a configuration value
    Set {
        /// Configuration key (e.g., 'general.download_dir')
        key: String,
        /// New value
        value: String,
    },
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable table format
    Table,
    /// Compact JSON
    Json,
    /// Pretty-printed JSON
    JsonPretty,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateFilter {
    /// Active (downloading/seeding)
    Active,
    /// Waiting in queue
    Waiting,
    /// Paused
    Paused,
    /// Completed
    Completed,
    /// Failed with error
    Error,
}

impl Priority {
    pub fn to_engine_priority(self) -> gosh_dl::DownloadPriority {
        match self {
            Priority::Low => gosh_dl::DownloadPriority::Low,
            Priority::Normal => gosh_dl::DownloadPriority::Normal,
            Priority::High => gosh_dl::DownloadPriority::High,
            Priority::Critical => gosh_dl::DownloadPriority::Critical,
        }
    }
}
