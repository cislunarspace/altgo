use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub key_listener: KeyListenerConfig,
    pub recorder: RecorderConfig,
    pub transcriber: TranscriberConfig,
    pub polisher: PolisherConfig,
    pub output: OutputConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct KeyListenerConfig {
    pub key_name: String,
    pub long_press_threshold_ms: u64,
    pub double_click_interval_ms: u64,
}

impl KeyListenerConfig {
    #[allow(dead_code)]
    pub fn long_press_threshold(&self) -> Duration {
        Duration::from_millis(self.long_press_threshold_ms)
    }

    #[allow(dead_code)]
    pub fn double_click_interval(&self) -> Duration {
        Duration::from_millis(self.double_click_interval_ms)
    }
}

impl Default for KeyListenerConfig {
    fn default() -> Self {
        Self {
            key_name: "ISO_Level3_Shift".to_string(),
            long_press_threshold_ms: 300,
            double_click_interval_ms: 300,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct RecorderConfig {
    pub sample_rate: u32,
    pub channels: u32,
    pub bit_depth: u32,
    pub buffer_size_ms: u64,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            bit_depth: 16,
            buffer_size_ms: 100,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct TranscriberConfig {
    pub engine: String,
    pub api_key: String,
    pub api_base_url: String,
    pub model: String,
    pub language: String,
    pub timeout_seconds: u64,
}

impl TranscriberConfig {
    #[allow(dead_code)]
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
}

impl Default for TranscriberConfig {
    fn default() -> Self {
        Self {
            engine: "local".to_string(),
            api_key: String::new(),
            api_base_url: "https://api.openai.com".to_string(),
            model: String::new(),
            language: "zh".to_string(),
            timeout_seconds: 30,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct PolisherConfig {
    pub engine: String,
    pub api_key: String,
    pub api_base_url: String,
    pub model: String,
    pub level: String,
    pub timeout_seconds: u64,
}

impl PolisherConfig {
    #[allow(dead_code)]
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
}

impl Default for PolisherConfig {
    fn default() -> Self {
        Self {
            engine: "openai".to_string(),
            api_key: String::new(),
            api_base_url: "https://api.openai.com".to_string(),
            model: "gpt-3.5-turbo".to_string(),
            level: "medium".to_string(),
            timeout_seconds: 60,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct OutputConfig {
    pub enable_notify: bool,
    pub notify_timeout_ms: u64,
    pub clipboard_tool: String,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            enable_notify: true,
            notify_timeout_ms: 3000,
            clipboard_tool: String::new(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let mut cfg = if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("read {}", path.display()))?;
            toml::from_str(&content).with_context(|| format!("parse {}", path.display()))?
        } else {
            Config::default()
        };

        // Environment variable overrides for API keys.
        if let Ok(key) = std::env::var("ALTGO_TRANSCRIBER_API_KEY") {
            cfg.transcriber.api_key = key;
        }
        if let Ok(key) = std::env::var("ALTGO_POLISHER_API_KEY") {
            cfg.polisher.api_key = key;
        }

        Ok(cfg)
    }

    pub fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("altgo")
            .join("altgo.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.key_listener.key_name, "ISO_Level3_Shift");
        assert_eq!(cfg.key_listener.long_press_threshold_ms, 300);
        assert_eq!(cfg.recorder.sample_rate, 16000);
        assert_eq!(cfg.transcriber.engine, "local");
        assert_eq!(cfg.polisher.level, "medium");
        assert!(cfg.output.enable_notify);
        assert_eq!(cfg.logging.level, "info");
    }

    #[test]
    fn test_load_nonexistent_file() {
        let cfg = Config::load(Path::new("/nonexistent/altgo.toml")).unwrap();
        assert_eq!(cfg.recorder.sample_rate, 16000);
    }

    #[test]
    fn test_load_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("altgo.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"
[key_listener]
key_name = "Alt_R"

[recorder]
sample_rate = 48000

[transcriber]
engine = "api"
language = "en"

[polisher]
level = "heavy"

[output]
enable_notify = false

[logging]
level = "debug"
"#
        )
        .unwrap();

        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.key_listener.key_name, "Alt_R");
        assert_eq!(cfg.recorder.sample_rate, 48000);
        assert_eq!(cfg.transcriber.engine, "api");
        assert_eq!(cfg.transcriber.language, "en");
        assert_eq!(cfg.polisher.level, "heavy");
        assert!(!cfg.output.enable_notify);
        assert_eq!(cfg.logging.level, "debug");
    }

    #[test]
    fn test_load_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("altgo.toml");
        std::fs::write(&path, "this is not valid [[[").unwrap();
        assert!(Config::load(&path).is_err());
    }

    #[test]
    fn test_env_override() {
        // SAFETY: test race is acceptable; env mutation in single-threaded test runner.
        std::env::set_var("ALTGO_TRANSCRIBER_API_KEY", "test-trans-key");
        std::env::set_var("ALTGO_POLISHER_API_KEY", "test-polish-key");

        let cfg = Config::load(Path::new("/nonexistent.toml")).unwrap();
        assert_eq!(cfg.transcriber.api_key, "test-trans-key");
        assert_eq!(cfg.polisher.api_key, "test-polish-key");

        std::env::remove_var("ALTGO_TRANSCRIBER_API_KEY");
        std::env::remove_var("ALTGO_POLISHER_API_KEY");
    }

    #[test]
    fn test_timeout_helpers() {
        let cfg = Config::default();
        assert_eq!(cfg.transcriber.timeout(), Duration::from_secs(30));
        assert_eq!(cfg.polisher.timeout(), Duration::from_secs(60));
        assert_eq!(
            cfg.key_listener.long_press_threshold(),
            Duration::from_millis(300)
        );
        assert_eq!(
            cfg.key_listener.double_click_interval(),
            Duration::from_millis(300)
        );
    }

    #[test]
    fn test_default_config_path() {
        let path = Config::default_config_path();
        assert!(path.to_string_lossy().contains("altgo"));
        assert!(path.to_string_lossy().ends_with("altgo.toml"));
    }
}
