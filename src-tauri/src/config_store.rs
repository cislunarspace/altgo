//! Config store — persistent config with atomic patch-and-save.

use serde::{Deserialize, Deserializer};
use tokio::sync::Mutex;

use crate::config::Config;

fn apply_nested_opt_u16(target: &mut Option<u16>, patch: Option<Option<u16>>) {
    match patch {
        None => {}
        Some(None) => *target = None,
        Some(Some(v)) => *target = Some(v),
    }
}

/// Distinguishes JSON field-absent (no patch) from JSON `null` (clear the stored evdev code).
fn deserialize_opt_patch_u16<'de, D>(deserializer: D) -> Result<Option<Option<u16>>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(Some(None)),
        serde_json::Value::Number(n) => {
            let u = n
                .as_u64()
                .ok_or_else(|| serde::de::Error::custom("linux_evdev_code: expected number"))?;
            if u > u16::MAX as u64 {
                return Err(serde::de::Error::custom("linux_evdev_code out of range"));
            }
            Ok(Some(Some(u as u16)))
        }
        _ => Err(serde::de::Error::custom(
            "linux_evdev_code: expected null or number",
        )),
    }
}

/// Partial update applied to the in-memory config. All fields are optional;
/// absent fields are left unchanged.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPatch {
    pub key_name: Option<String>,
    /// `None` = field absent (no change); `Some(None)` = JSON `null` (clear);
    /// `Some(Some(v))` = set to v.
    #[serde(default, deserialize_with = "deserialize_opt_patch_u16")]
    pub linux_evdev_code: Option<Option<u16>>,
    pub language: Option<String>,
    pub engine: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub polish_level: Option<String>,
    pub polish_model: Option<String>,
    pub polish_api_key: Option<String>,
    pub polish_api_base_url: Option<String>,
    pub gui_language: Option<String>,
}

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

        if let Some(v) = patch.key_name {
            cfg.key_listener.key_name = v;
        }
        apply_nested_opt_u16(
            &mut cfg.key_listener.linux_evdev_code,
            patch.linux_evdev_code,
        );
        if let Some(v) = patch.language {
            cfg.transcriber.language = v;
        }
        if let Some(v) = patch.engine {
            cfg.transcriber.engine = v;
        }
        if let Some(v) = patch.model {
            cfg.transcriber.model = v;
        }
        if let Some(v) = patch.api_key {
            cfg.transcriber.api_key = v;
        }
        if let Some(v) = patch.api_base_url {
            cfg.transcriber.api_base_url = v;
        }
        if let Some(v) = patch.polish_level {
            cfg.polisher.level = v;
        }
        if let Some(v) = patch.polish_model {
            cfg.polisher.model = v;
        }
        if let Some(v) = patch.polish_api_key {
            cfg.polisher.api_key = v;
        }
        if let Some(v) = patch.polish_api_base_url {
            cfg.polisher.api_base_url = v;
        }
        if let Some(v) = patch.gui_language {
            cfg.gui.language = v;
        }

        cfg.validate().map_err(|e| e.to_string())?;
        cfg.save(&self.config_path)
            .map_err(|e| format!("save failed: {}", e))?;

        Ok(cfg.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigPatch;

    #[test]
    fn evdev_json_null_clears() {
        let j = r#"{"linuxEvdevCode":null}"#;
        let p: ConfigPatch = serde_json::from_str(j).unwrap();
        assert_eq!(p.linux_evdev_code, Some(None));
    }

    #[test]
    fn evdev_missing_field_means_no_patch() {
        let j = r#"{}"#;
        let p: ConfigPatch = serde_json::from_str(j).unwrap();
        assert!(p.linux_evdev_code.is_none());
    }

    #[test]
    fn evdev_number_sets() {
        let j = r#"{"linuxEvdevCode":100}"#;
        let p: ConfigPatch = serde_json::from_str(j).unwrap();
        assert_eq!(p.linux_evdev_code, Some(Some(100)));
    }
}
