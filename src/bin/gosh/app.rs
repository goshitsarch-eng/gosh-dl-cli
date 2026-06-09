use anyhow::Result;
use gosh_dl::DownloadEvent;
use gosh_dl::{DownloadEngine, FileStorage};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config::{CliConfig, StorageBackend};

/// Application state coordinator
pub struct App {
    /// The download engine instance
    engine: Arc<DownloadEngine>,

    /// Application configuration
    pub config: CliConfig,
}

/// Build the download engine honoring the configured storage backend
pub async fn create_engine(config: &CliConfig) -> Result<Arc<DownloadEngine>> {
    // Ensure database directory exists
    if let Some(parent) = config.general.database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let engine_config = config.to_engine_config();
    let engine = match config.general.storage_backend {
        // to_engine_config sets database_path only for sqlite
        StorageBackend::Sqlite | StorageBackend::None => DownloadEngine::new(engine_config).await?,
        StorageBackend::File => {
            let state_dir = config
                .general
                .database_path
                .parent()
                .map(|p| p.join("state"))
                .unwrap_or_else(|| std::path::PathBuf::from("state"));
            let storage = FileStorage::new(state_dir).await?;
            DownloadEngine::with_storage(engine_config, Arc::new(storage)).await?
        }
    };
    Ok(engine)
}

impl App {
    /// Create a new application instance with the given configuration
    pub async fn new(config: CliConfig) -> Result<Self> {
        let engine = create_engine(&config).await?;
        Ok(Self { engine, config })
    }

    /// Get a reference to the download engine
    pub fn engine(&self) -> &Arc<DownloadEngine> {
        &self.engine
    }

    /// Subscribe to engine events
    pub fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.engine.subscribe()
    }

    /// Shutdown the engine gracefully
    pub async fn shutdown(&self) -> Result<()> {
        self.engine.shutdown().await?;
        Ok(())
    }
}
