//! Linux 按键监听器。
//!
//! 使用 `xinput test-xi2` 命令捕获全局键盘事件，依赖 XInput2 扩展，
//! 无需 root 权限即可在所有现代 X11 系统上工作。
//!
//! 通过 `xmodmap -pke` 解析按键名称到 keycode 的映射。

use super::KeyEvent;
use anyhow::{Context, Result};
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc;

/// Cache for xmodmap keycode mappings. Parsing xmodmap output is expensive,
/// so we cache the entire keycode table on first use.
static XMODMAP_CACHE: OnceLock<std::collections::HashMap<String, u8>> = OnceLock::new();

/// X11 按键监听器，使用 `xinput test-xi2` 捕获全局按键事件。
///
/// 无需 root 权限，依赖 XInput2 扩展。
pub struct X11Listener {
    key_name: String,
    running: Arc<AtomicBool>,
    child: Option<Child>,
}

impl X11Listener {
    /// 创建新的 X11 监听器，验证 `xinput` 是否可用。
    pub fn new(key_name: &str) -> Result<Self> {
        Command::new("xinput")
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("xinput not found — install xinput package")?;

        Ok(Self {
            key_name: key_name.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            child: None,
        })
    }

    /// 开始监听按键事件，返回事件通道。
    pub fn start(&mut self) -> Result<mpsc::UnboundedReceiver<KeyEvent>> {
        let keycode = self.resolve_keycode()?;

        tracing::info!("resolved key '{}' to keycode {}", self.key_name, keycode);

        let (tx, rx) = mpsc::unbounded_channel();
        let running = Arc::clone(&self.running);
        running.store(true, Ordering::SeqCst);

        let mut child = Command::new("xinput")
            .args(["test-xi2", "--root"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to start xinput test-xi2")?;

        let stdout = child.stdout.take().context("no stdout from xinput")?;

        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
            let mut event_type: Option<bool> = None; // true=press, false=release

            for line in reader.lines() {
                if !running.load(Ordering::SeqCst) {
                    tracing::info!("key listener thread stopped by user");
                    break;
                }

                let line = match line {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::warn!(error = %e, "xinput stdout read error, key listener thread exiting");
                        break;
                    }
                };

                let trimmed = line.trim();

                if trimmed.contains("KeyPress") && trimmed.starts_with("EVENT") {
                    event_type = Some(true);
                } else if trimmed.contains("KeyRelease") && trimmed.starts_with("EVENT") {
                    event_type = Some(false);
                } else if let Some(detail_str) = trimmed.strip_prefix("detail:") {
                    if let Some(pressed) = event_type.take() {
                        if let Ok(detail) = detail_str.trim().parse::<u8>() {
                            if detail == keycode && tx.send(KeyEvent { pressed }).is_err() {
                                tracing::warn!(
                                    "key event receiver dropped, key listener thread exiting"
                                );
                                break;
                            }
                        }
                    } else {
                        tracing::debug!(
                            line = %trimmed,
                            "detail line without preceding event type, skipping"
                        );
                    }
                } else if !trimmed.is_empty()
                    && !trimmed.starts_with("EVENT")
                    && !trimmed.contains(':')
                {
                    tracing::trace!(line = %trimmed, "unparsed xinput line");
                }
            }
            tracing::warn!("xinput stdout closed, key listener thread exiting");
        });

        self.child = Some(child);
        Ok(rx)
    }

    /// 停止监听。
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn resolve_keycode(&self) -> Result<u8> {
        let keycode_map = XMODMAP_CACHE.get_or_init(|| {
            let output = match Command::new("xmodmap")
                .arg("-pke")
                .output()
            {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!(error = %e, "xmodmap not found or failed, keycode resolution may fail");
                    return std::collections::HashMap::new();
                }
            };

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut map = std::collections::HashMap::new();

            // xmodmap -pke output format: keycode <N> = keysym ...
            // e.g., "keycode  64 = Alt_L Meta_L Alt_L Meta_L"
            for line in stdout.lines() {
                if let Some(keycode_str) = line.split_whitespace().nth(1) {
                    if let Ok(keycode) = keycode_str.parse::<u8>() {
                        // Extract all keysyms from the line (skip "keycode N =")
                        for keysym in line.split_whitespace().skip(3) {
                            // Skip "=" if present
                            let keysym = keysym.trim_end_matches('=');
                            if !keysym.is_empty() && !map.contains_key(keysym) {
                                map.insert(keysym.to_string(), keycode);
                            }
                        }
                    }
                }
            }
            map
        });

        keycode_map.get(&self.key_name).copied().ok_or_else(|| {
            anyhow::anyhow!(
                "keycode for '{}' not found in xmodmap output",
                self.key_name
            )
        })
    }
}

impl Drop for X11Listener {
    fn drop(&mut self) {
        self.stop();
    }
}
