//! 转写文本历史（持久化 JSON，不含录音）。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

static HISTORY_IO_LOCK: Mutex<()> = Mutex::new(());

/// 与 `altgo.toml` 同目录：`~/.config/altgo/history.json`
pub fn default_history_path() -> PathBuf {
    crate::config::Config::default_config_path()
        .parent()
        .expect("config path has parent")
        .join("history.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub created_at_ms: u64,
    /// Whisper 原始转写（快捷润色以此为输入）
    pub raw_text: String,
    /// 当前展示文本（润色后或与 raw 相同）
    pub text: String,
}

#[derive(Serialize, Deserialize, Default)]
struct HistoryFile {
    entries: Vec<HistoryEntry>,
}

fn load_raw(path: &Path) -> Result<HistoryFile> {
    if !path.exists() {
        return Ok(HistoryFile::default());
    }
    let s = fs::read_to_string(path).context("read history file")?;
    if s.trim().is_empty() {
        return Ok(HistoryFile::default());
    }
    serde_json::from_str(&s).context("parse history json")
}

fn save_raw(path: &Path, data: &HistoryFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create history directory")?;
    }
    let s = serde_json::to_string_pretty(data).context("serialize history")?;
    fs::write(path, s).context("write history file")
}

/// 追加一条记录（最新在前）。
pub fn append_entry(path: &Path, raw_text: String, text: String) -> Result<HistoryEntry> {
    let _g = HISTORY_IO_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("history lock poisoned"))?;
    let mut file = load_raw(path)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let entry = HistoryEntry {
        id: Uuid::new_v4().to_string(),
        created_at_ms: now,
        raw_text,
        text,
    };
    file.entries.insert(0, entry.clone());
    save_raw(path, &file)?;
    Ok(entry)
}

pub fn list_entries(path: &Path) -> Result<Vec<HistoryEntry>> {
    let _g = HISTORY_IO_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("history lock poisoned"))?;
    let file = load_raw(path)?;
    Ok(file.entries)
}

pub fn delete_entries(path: &Path, ids: &[String]) -> Result<usize> {
    let _g = HISTORY_IO_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("history lock poisoned"))?;
    let mut file = load_raw(path)?;
    let before = file.entries.len();
    let id_set: HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
    file.entries.retain(|e| !id_set.contains(e.id.as_str()));
    save_raw(path, &file)?;
    Ok(before - file.entries.len())
}

pub fn clear_all(path: &Path) -> Result<()> {
    let _g = HISTORY_IO_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("history lock poisoned"))?;
    save_raw(path, &HistoryFile::default())
}

pub fn update_entry_text(path: &Path, id: &str, new_text: String) -> Result<HistoryEntry> {
    let _g = HISTORY_IO_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("history lock poisoned"))?;
    let mut file = load_raw(path)?;
    for e in &mut file.entries {
        if e.id == id {
            e.text = new_text;
            let out = e.clone();
            save_raw(path, &file)?;
            return Ok(out);
        }
    }
    anyhow::bail!("history entry not found: {}", id)
}

pub fn get_entry(path: &Path, id: &str) -> Result<Option<HistoryEntry>> {
    let _g = HISTORY_IO_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("history lock poisoned"))?;
    let file = load_raw(path)?;
    Ok(file.entries.iter().find(|e| e.id == id).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn append_and_list() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let e1 = append_entry(&path, "raw one".into(), "one".into()).unwrap();
        let e2 = append_entry(&path, "raw two".into(), "two".into()).unwrap();
        let list = list_entries(&path).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, e2.id);
        assert_eq!(list[1].id, e1.id);
    }

    #[test]
    fn delete_and_clear() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let e = append_entry(&path, "r".into(), "t".into()).unwrap();
        delete_entries(&path, &[e.id.clone()]).unwrap();
        assert!(list_entries(&path).unwrap().is_empty());
        append_entry(&path, "a".into(), "b".into()).unwrap();
        clear_all(&path).unwrap();
        assert!(list_entries(&path).unwrap().is_empty());
    }

    #[test]
    fn update_text() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let e = append_entry(&path, "raw".into(), "old".into()).unwrap();
        let updated = update_entry_text(&path, &e.id, "new".into()).unwrap();
        assert_eq!(updated.text, "new");
        assert_eq!(updated.raw_text, "raw");
    }
}
