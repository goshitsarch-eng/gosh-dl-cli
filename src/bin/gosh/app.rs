use anyhow::Result;
use gosh_dl::DownloadEngine;
use gosh_dl::DownloadEvent;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config::CliConfig;

/// Application state coordinator
pub struct App {
    /// The download engine instance
    engine: Arc<DownloadEngine>,

    /// Application configuration
    pub config: CliConfig,
}

impl App {
    /// Create a new application instance with the given configuration
    pub async fn new(config: CliConfig) -> Result<Self> {
        // Ensure database directory exists
        if let Some(parent) = config.general.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let engine_config = config.to_engine_config();
        let engine = DownloadEngine::new(engine_config).await?;

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
