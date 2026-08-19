//! Config store — persistent config with atomic patch-and-save.
//!
//! Thin persistence wrapper around `Config`. Patch logic lives in `config.rs`
//! (`ConfigPatch::apply_to_config`), keeping config definition and mutation co-located.

use tokio::sync::{Mutex, MutexGuard};

use crate::config::{Config, ConfigPatch};

/// Holds the live config and its backing file path.
/// All mutations go through `apply_patch`, which validates and persists atomically.
pub struct ConfigStore {
    pub(crate) config: Mutex<Config>,
    config_path: std::path::PathBuf,
}

/// 已校验且已持久化的配置更新。
///
/// 持有锁直到调用方完成依赖本次更新的后续操作，防止其他配置更新插入。
pub struct ConfigUpdate<'a> {
    config: Config,
    _guard: MutexGuard<'a, Config>,
}

impl ConfigUpdate<'_> {
    pub fn config(&self) -> &Config {
        &self.config
    }
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

    /// Apply a partial update, validate, persist, and keep the config lock held.
    ///
    /// The returned update lets a caller finish a dependent operation, such as restarting
    /// the voice pipeline, before another patch can replace the active configuration.
    /// If validation or persistence fails, the in-memory config is rolled back before
    /// returning the error, keeping memory and disk consistent.
    pub async fn apply_patch_for_update(
        &self,
        patch: ConfigPatch,
    ) -> Result<ConfigUpdate<'_>, String> {
        let mut cfg = self.config.lock().await;
        let original = cfg.clone();

        patch.apply_to_config(&mut cfg);

        if let Err(e) = cfg.validate() {
            *cfg = original;
            return Err(e.to_string());
        }

        if let Err(e) = cfg.save(&self.config_path) {
            *cfg = original;
            return Err(format!("save failed: {}", e));
        }
        let updated = cfg.clone();

        Ok(ConfigUpdate {
            config: updated,
            _guard: cfg,
        })
    }

    /// Apply a partial update, validate, persist to disk, and return the new config.
    ///
    /// If validation fails, the in-memory config is rolled back to its pre-patch state
    /// before returning the error, keeping memory and disk consistent.
    pub async fn apply_patch(&self, patch: ConfigPatch) -> Result<Config, String> {
        Ok(self.apply_patch_for_update(patch).await?.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (ConfigStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("altgo.toml");
        (ConfigStore::load(path), dir)
    }

    #[tokio::test]
    async fn load_creates_default_config_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("altgo.toml");
        let store = ConfigStore::load(path);
        let cfg = store.snapshot().await;
        assert_eq!(cfg.key_listener.key_name, "Alt_R");
    }

    #[tokio::test]
    async fn snapshot_returns_current_config() {
        let (store, _dir) = temp_store();
        let cfg = store.snapshot().await;
        assert_eq!(cfg.transcriber.model, "");
        assert_eq!(cfg.polisher.level, "none");
    }

    #[tokio::test]
    async fn apply_patch_updates_config_and_persists() {
        let (store, dir) = temp_store();
        let patch: ConfigPatch =
            serde_json::from_str(r#"{"keyName":"space","language":"en"}"#).unwrap();
        let result = store.apply_patch(patch).await.unwrap();
        assert_eq!(result.key_listener.key_name, "space");
        assert_eq!(result.transcriber.language, "en");

        // Verify persistence: reload from disk
        let path = dir.path().join("altgo.toml");
        let reloaded = Config::load(&path).unwrap();
        assert_eq!(reloaded.key_listener.key_name, "space");
        assert_eq!(reloaded.transcriber.language, "en");
    }

    #[tokio::test]
    async fn apply_patch_save_failure_rolls_back_memory() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf());
        let patch: ConfigPatch = serde_json::from_str(r#"{"language":"en"}"#).unwrap();

        let result = store.apply_patch(patch).await;

        assert!(result.is_err());
        assert_eq!(store.snapshot().await.transcriber.language, "zh");
    }

    #[tokio::test]
    async fn apply_patch_preserves_unset_fields() {
        let (store, _dir) = temp_store();
        let patch: ConfigPatch = serde_json::from_str(r#"{"language":"en"}"#).unwrap();
        let result = store.apply_patch(patch).await.unwrap();
        // Unchanged fields keep defaults
        assert_eq!(result.key_listener.key_name, "Alt_R");
        assert_eq!(result.transcriber.model, "");
        assert_eq!(result.transcriber.language, "en");
    }

    #[tokio::test]
    async fn apply_patch_rejects_invalid_config() {
        let (store, _dir) = temp_store();
        // Enabling polishing without an API key should fail validation.
        let patch: ConfigPatch =
            serde_json::from_str(r#"{"polishLevel":"light","polishApiKey":""}"#).unwrap();
        let result = store.apply_patch(patch).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("api_key"));
    }

    #[tokio::test]
    async fn apply_patch_invalid_config_rolls_back_memory() {
        let (store, _dir) = temp_store();
        // Set up a valid config with polishing enabled.
        let patch: ConfigPatch = serde_json::from_str(
            r#"{"polishLevel":"light","polishApiKey":"polish-key","polishApiBaseUrl":"https://api.deepseek.com","polishModel":"deepseek-chat"}"#,
        )
        .unwrap();
        store.apply_patch(patch).await.unwrap();
        let original = store.snapshot().await;
        assert_eq!(original.transcriber.model, "");
        assert_eq!(original.polisher.level, "light");
        assert_eq!(original.polisher.api_key, "polish-key");

        // Now apply an invalid patch that would partially mutate config.
        let invalid_patch: ConfigPatch =
            serde_json::from_str(r#"{"polishLevel":"medium","polishApiKey":""}"#).unwrap();
        let result = store.apply_patch(invalid_patch).await;
        assert!(result.is_err());

        // Memory must be rolled back to the pre-patch state.
        let after = store.snapshot().await;
        assert_eq!(after.transcriber.model, "");
        assert_eq!(after.polisher.level, "light");
        assert_eq!(after.polisher.api_key, "polish-key");
    }

    #[tokio::test]
    async fn apply_patch_rollback_then_valid_patch_still_works() {
        let (store, _dir) = temp_store();
        let patch: ConfigPatch = serde_json::from_str(
            r#"{"polishLevel":"light","polishApiKey":"polish-key","polishApiBaseUrl":"https://api.deepseek.com","polishModel":"deepseek-chat"}"#,
        )
        .unwrap();
        store.apply_patch(patch).await.unwrap();

        let invalid_patch: ConfigPatch =
            serde_json::from_str(r#"{"polishLevel":"medium","polishApiKey":""}"#).unwrap();
        assert!(store.apply_patch(invalid_patch).await.is_err());

        let valid_patch: ConfigPatch =
            serde_json::from_str(r#"{"polishApiKey":"next-key","model":"sense-voice"}"#).unwrap();
        let result = store.apply_patch(valid_patch).await.unwrap();
        assert_eq!(result.polisher.api_key, "next-key");
        assert_eq!(result.transcriber.model, "sense-voice");

        let snapshot = store.snapshot().await;
        assert_eq!(snapshot.polisher.api_key, "next-key");
        assert_eq!(snapshot.transcriber.model, "sense-voice");
    }

    #[tokio::test]
    async fn apply_patch_linux_evdev_null_clears() {
        let (store, _dir) = temp_store();
        // First set linux_evdev_code
        let patch: ConfigPatch = serde_json::from_str(r#"{"linuxEvdevCode":56}"#).unwrap();
        let result = store.apply_patch(patch).await.unwrap();
        assert_eq!(result.key_listener.linux_evdev_code, Some(56));

        // Then clear it
        let patch: ConfigPatch = serde_json::from_str(r#"{"linuxEvdevCode":null}"#).unwrap();
        let result = store.apply_patch(patch).await.unwrap();
        assert!(result.key_listener.linux_evdev_code.is_none());
    }
}
