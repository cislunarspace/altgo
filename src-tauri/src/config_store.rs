//! Config store — persistent config with atomic patch-and-save.
//!
//! Thin persistence wrapper around `Config`. Patch logic lives in `config.rs`
//! (`ConfigPatch::apply_to_config`), keeping config definition and mutation co-located.

use tokio::sync::Mutex;

use crate::config::{Config, ConfigPatch};

/// Holds the live config and its backing file path.
/// All mutations go through `apply_patch`, which validates and persists atomically.
pub struct ConfigStore {
    pub config: Mutex<Config>,
    config_path: std::path::PathBuf,
}

impl ConfigStore {
    pub fn load(config_path: std::path::PathBuf) -> Self {
        let cfg = Config::load(&config_path).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to load config, using defaults");
            Config::default()
        });
        if let Err(e) = cfg.validate() {
            tracing::warn!(error = %e, "config validation failed");
        }
        Self {
            config: Mutex::new(cfg),
            config_path,
        }
    }

    pub async fn snapshot(&self) -> Config {
        self.config.lock().await.clone()
    }

    pub fn snapshot_blocking(&self) -> Config {
        self.config.blocking_lock().clone()
    }

    /// Apply a partial update, validate, persist to disk, and return the new config.
    pub async fn apply_patch(&self, patch: ConfigPatch) -> Result<Config, String> {
        let mut cfg = self.config.lock().await;

        patch.apply_to_config(&mut cfg);

        cfg.validate().map_err(|e| e.to_string())?;
        cfg.save(&self.config_path)
            .map_err(|e| format!("save failed: {}", e))?;

        Ok(cfg.clone())
    }
}
