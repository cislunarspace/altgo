//! Prompt template management with hot-reload support.
//!
//! Loads prompt templates from `resources/prompts/`:
//! - `base.txt`: shared instruction + Chinese writing guidance
//! - `{level}-suffix.txt`: level-specific instruction
//!
//! Runtime composition: `base.txt` + `{level}-suffix.txt` → complete system prompt.
//! Filesystem watcher reloads templates when files change (500ms debounce).

use crate::polisher::PolishLevel;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

/// Errors that can occur during prompt loading or validation.
#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("Prompt file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Prompt file is empty: {0}")]
    EmptyFile(PathBuf),

    #[error("Failed to read prompt file {path}: {source}")]
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to start filesystem watcher: {0}")]
    WatcherError(#[from] notify::Error),
}

/// Manages prompt templates with hot-reload support.
#[derive(Clone)]
pub struct PromptStore {
    prompts_dir: PathBuf,
    cache: Arc<Mutex<PromptCache>>,
}

struct PromptCache {
    base: String,
    suffixes: HashMap<PolishLevel, String>,
}

impl PromptStore {
    /// Creates a new PromptStore for the given prompts directory.
    ///
    /// Does not load prompts immediately—call `load()` or `ensure_loaded()`.
    pub fn new(prompts_dir: PathBuf) -> Self {
        Self {
            prompts_dir,
            cache: Arc::new(Mutex::new(PromptCache {
                base: String::new(),
                suffixes: HashMap::new(),
            })),
        }
    }

    /// Loads all prompt templates from disk.
    ///
    /// Returns error if any file is missing or empty.
    pub fn load(&self) -> Result<(), PromptError> {
        let base = self.load_file("base.txt")?;
        let light = self.load_file("light-suffix.txt")?;
        let medium = self.load_file("medium-suffix.txt")?;
        let heavy = self.load_file("heavy-suffix.txt")?;

        let mut cache = self.cache.lock().unwrap();
        cache.base = base;
        cache.suffixes.insert(PolishLevel::Light, light);
        cache.suffixes.insert(PolishLevel::Medium, medium);
        cache.suffixes.insert(PolishLevel::Heavy, heavy);

        Ok(())
    }

    /// Loads prompts if cache is empty, otherwise returns cached prompts.
    pub fn ensure_loaded(&self) -> Result<(), PromptError> {
        let cache = self.cache.lock().unwrap();
        if cache.base.is_empty() {
            drop(cache);
            self.load()
        } else {
            Ok(())
        }
    }

    /// Composes the complete system prompt for the given polish level.
    ///
    /// Returns error if prompts haven't been loaded yet.
    pub fn get_system_prompt(&self, level: PolishLevel) -> Result<String, PromptError> {
        if matches!(level, PolishLevel::None) {
            return Ok(String::new());
        }

        let cache = self.cache.lock().unwrap();
        if cache.base.is_empty() {
            return Err(PromptError::FileNotFound(self.prompts_dir.join("base.txt")));
        }

        let suffix = cache.suffixes.get(&level).ok_or_else(|| {
            PromptError::FileNotFound(
                self.prompts_dir
                    .join(format!("{:?}-suffix.txt", level).to_lowercase()),
            )
        })?;

        Ok(format!("{}\n\n{}", cache.base.trim(), suffix.trim()))
    }

    /// Starts a filesystem watcher that reloads prompts when files change.
    ///
    /// Debounces changes with 500ms delay. Returns a handle that stops watching when dropped.
    pub fn start_watcher(self) -> Result<WatcherHandle, PromptError> {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if event.kind.is_modify() || event.kind.is_create() {
                        let _ = tx.send(());
                    }
                }
            },
            Config::default(),
        )?;

        watcher.watch(&self.prompts_dir, RecursiveMode::NonRecursive)?;

        let store = self.clone();
        let handle = tokio::spawn(async move {
            let mut pending_reload = false;

            loop {
                tokio::select! {
                    Some(()) = rx.recv() => {
                        pending_reload = true;
                    }
                    _ = sleep(Duration::from_millis(500)), if pending_reload => {
                        pending_reload = false;
                        if let Err(e) = store.load() {
                            eprintln!("Failed to reload prompts: {}", e);
                        } else {
                            println!("Prompts reloaded successfully");
                        }
                    }
                }
            }
        });

        Ok(WatcherHandle {
            _watcher: watcher,
            _task: handle,
        })
    }

    fn load_file(&self, filename: &str) -> Result<String, PromptError> {
        let path = self.prompts_dir.join(filename);

        if !path.exists() {
            return Err(PromptError::FileNotFound(path));
        }

        let content = fs::read_to_string(&path).map_err(|e| PromptError::ReadError {
            path: path.clone(),
            source: e,
        })?;

        if content.trim().is_empty() {
            return Err(PromptError::EmptyFile(path));
        }

        Ok(content)
    }
}

/// Handle that keeps the filesystem watcher alive.
///
/// Watcher stops when this handle is dropped.
pub struct WatcherHandle {
    _watcher: RecommendedWatcher,
    _task: tokio::task::JoinHandle<()>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn create_test_prompts(dir: &Path) {
        fs::write(dir.join("base.txt"), "Base prompt").unwrap();
        fs::write(dir.join("light-suffix.txt"), "Light suffix").unwrap();
        fs::write(dir.join("medium-suffix.txt"), "Medium suffix").unwrap();
        fs::write(dir.join("heavy-suffix.txt"), "Heavy suffix").unwrap();
    }

    #[test]
    fn test_load_success() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompts(temp_dir.path());

        let store = PromptStore::new(temp_dir.path().to_path_buf());
        assert!(store.load().is_ok());
    }

    #[test]
    fn test_load_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let store = PromptStore::new(temp_dir.path().to_path_buf());

        let result = store.load();
        assert!(matches!(result, Err(PromptError::FileNotFound(_))));
    }

    #[test]
    fn test_load_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("base.txt"), "").unwrap();

        let store = PromptStore::new(temp_dir.path().to_path_buf());
        let result = store.load();
        assert!(matches!(result, Err(PromptError::EmptyFile(_))));
    }

    #[test]
    fn test_get_system_prompt_composition() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompts(temp_dir.path());

        let store = PromptStore::new(temp_dir.path().to_path_buf());
        store.load().unwrap();

        let prompt = store.get_system_prompt(PolishLevel::Light).unwrap();
        assert!(prompt.contains("Base prompt"));
        assert!(prompt.contains("Light suffix"));
    }

    #[test]
    fn test_get_system_prompt_none_level() {
        let temp_dir = TempDir::new().unwrap();
        let store = PromptStore::new(temp_dir.path().to_path_buf());

        let prompt = store.get_system_prompt(PolishLevel::None).unwrap();
        assert_eq!(prompt, "");
    }

    #[test]
    fn test_ensure_loaded_lazy() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompts(temp_dir.path());

        let store = PromptStore::new(temp_dir.path().to_path_buf());

        // First call loads
        assert!(store.ensure_loaded().is_ok());

        // Second call uses cache
        assert!(store.ensure_loaded().is_ok());
    }
}
