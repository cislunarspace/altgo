//! Windows key listener stub.
//!
//! Low-level keyboard hook integration will live here. For now the pipeline uses
//! the platform alias and fails gracefully at runtime on Windows.

use super::KeyEvent;
use crate::config::KeyListenerConfig;
use anyhow::Result;
use tokio::sync::mpsc;

pub struct WindowsListener;

#[allow(dead_code)]
pub fn vk_from_key_name(_key_name: &str) -> Option<i32> {
    None
}

impl WindowsListener {
    pub fn new(_cfg: &KeyListenerConfig) -> Result<Self> {
        anyhow::bail!("Windows key listener is not implemented yet")
    }

    pub fn start(&mut self) -> Result<(mpsc::UnboundedReceiver<KeyEvent>, &'static str)> {
        anyhow::bail!("Windows key listener is not implemented yet")
    }
}
