//! Linux 按键监听器。
//!
//! - **Wayland 会话**：优先 `evtest` 读 `/dev/input/event*`（XWayland 上 `xinput test-xi2` 常能启动但收不到全局键盘）。
//! - **传统 X11**：优先 `xinput test-xi2`（XInput2），失败再 `evtest`。
//!
//! 通过 `xmodmap -pke` 解析按键名称到 keycode 的映射（xinput 路径）。

use super::KeyEvent;
use crate::config::KeyListenerConfig;
use anyhow::{Context, Result};
use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc;

/// Cache for xmodmap keycode mappings. Parsing xmodmap output is expensive,
/// so we cache the entire keycode table on first use.
static XMODMAP_CACHE: OnceLock<std::collections::HashMap<String, u8>> = OnceLock::new();

/// evdev keycode for Alt keys（evtest 回退；与 `linux/input-event-codes.h` 一致）
const EVDEV_KEY_ALT: u16 = 56; // KEY_LEFTALT
const EVDEV_KEY_ALT_R: u16 = 100; // KEY_RIGHTALT

/// X11 keycode for Alt_L (from xmodmap output).
#[allow(dead_code)]
const X11_KEYCODE_ALT: u8 = 64;

/// X11 按键监听器，使用 `xinput test-xi2` 捕获全局按键事件。
///
/// 无需 root 权限，依赖 XInput2 扩展。
pub struct X11Listener {
    key_name: String,
    linux_evdev_code: Option<u16>,
    running: Arc<AtomicBool>,
    child: Option<Child>,
}

/// 枚举可用于 `evtest` 回退的键盘设备（crate 内共享，供按键捕获使用）。
pub(crate) fn list_keyboard_devices() -> Result<Vec<PathBuf>> {
    let by_id_path = PathBuf::from("/dev/input/by-id");
    let mut devices = Vec::new();

    if by_id_path.exists() {
        for entry in std::fs::read_dir(&by_id_path)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with("-kbd") {
                let path = entry.path();
                if let Ok(real_path) = std::fs::canonicalize(&path) {
                    devices.push(real_path);
                }
            }
        }
    }

    if devices.is_empty() {
        for entry in std::fs::read_dir("/dev/input")? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("event") {
                let path = entry.path();
                // Avoid shell interpolation — pass args directly to evtest.
                let output = Command::new("evtest")
                    .arg("--info")
                    .arg(&path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output();

                if let Ok(output) = output {
                    let info = String::from_utf8_lossy(&output.stdout);
                    if info.contains("EV_KEY") && info.contains("KEY") {
                        devices.push(path);
                    }
                }
            }
        }
    }

    Ok(devices)
}

impl X11Listener {
    /// 创建新的 X11 监听器，验证 `xinput` 是否可用。
    pub fn new(cfg: &KeyListenerConfig) -> Result<Self> {
        Command::new("xinput")
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("xinput not found — install xinput package")?;

        Ok(Self {
            key_name: cfg.key_name.clone(),
            linux_evdev_code: cfg.linux_evdev_code,
            running: Arc::new(AtomicBool::new(false)),
            child: None,
        })
    }

    /// 开始监听按键事件，返回事件通道与后端标识（`"xinput"` / `"evtest"`）。
    pub fn start(&mut self) -> Result<(mpsc::UnboundedReceiver<KeyEvent>, &'static str)> {
        // evtest：有捕获码则只认该码；否则按 keysym 映射到 evdev（左/右 Alt 严格区分）。
        let allowed_evdev: std::sync::Arc<[u16]> = if let Some(c) = self.linux_evdev_code {
            std::sync::Arc::from([c])
        } else {
            let code = match self.key_name.as_str() {
                "Alt_L" => EVDEV_KEY_ALT,
                "Alt_R" | "ISO_Level3_Shift" | "AltGr" => EVDEV_KEY_ALT_R,
                _ => {
                    tracing::warn!(
                        "key_name '{}' not mapped for evtest fallback; defaulting to KEY_RIGHTALT",
                        self.key_name
                    );
                    EVDEV_KEY_ALT_R
                }
            };
            std::sync::Arc::from([code])
        };
        let display_set = std::env::var("DISPLAY")
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        let wayland_hint = std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE")
                .map(|v| v == "wayland")
                .unwrap_or(false);

        // On Wayland, DISPLAY still points at XWayland; `xinput test-xi2 --root` usually starts
        // but does not receive global keyboard events — looks like "no errors, no keys". Prefer evdev.
        if wayland_hint {
            tracing::info!(
                "Wayland session: trying evtest first (xinput on XWayland typically misses keyboard)"
            );
            match self.try_start_evtest(allowed_evdev.clone()) {
                Ok(rx) => return Ok((rx, "evtest")),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "evtest failed on Wayland, will try xinput (may still not receive keys)"
                    );
                }
            }
        }

        // Classic X11 or Wayland evtest fallback: xinput when we have a real X11 keymap.
        if display_set {
            tracing::info!(
                session = wayland_hint,
                "DISPLAY is set; trying xinput test-xi2"
            );
            match self.resolve_keycode() {
                Ok(keycode) => {
                    tracing::info!(
                        "resolved key '{}' to X11 keycode {}",
                        self.key_name,
                        keycode
                    );
                    let (tx, rx) = mpsc::unbounded_channel();
                    let running = Arc::clone(&self.running);
                    running.store(true, Ordering::SeqCst);
                    match self.try_start_xinput(keycode, tx, running) {
                        Ok(child) => {
                            self.child = Some(child);
                            return Ok((rx, "xinput"));
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "xinput failed to start, falling back to evtest"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "xmodmap/keycode resolution failed (no working X?), falling back to evtest"
                    );
                }
            }
        } else if !wayland_hint {
            tracing::info!("DISPLAY is unset; using evtest on /dev/input");
        }

        if wayland_hint {
            tracing::info!("Wayland: evtest requires read access to /dev/input/event* (e.g. user in group input)");
        }

        tracing::info!("starting evtest for key events");
        let rx = self.try_start_evtest(allowed_evdev)?;
        Ok((rx, "evtest"))
    }

    fn try_start_evtest(
        &mut self,
        allowed_evdev: std::sync::Arc<[u16]>,
    ) -> Result<mpsc::UnboundedReceiver<KeyEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        if let Err(e) = self.start_evtest_fallback(allowed_evdev, tx, running) {
            self.running.store(false, Ordering::SeqCst);
            return Err(e);
        }
        Ok(rx)
    }

    /// 尝试启动 xinput test-xi2。
    /// 如果检测到 XWayland (BadAccess)，返回错误触发 fallback。
    fn try_start_xinput(
        &mut self,
        keycode: u8,
        tx: tokio::sync::mpsc::UnboundedSender<KeyEvent>,
        running: Arc<AtomicBool>,
    ) -> Result<Child> {
        let mut child = Command::new("xinput")
            .args(["test-xi2", "--root"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to start xinput test-xi2")?;

        // Check stderr for XWayland warning
        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            std::thread::spawn(move || {
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines().take(10).flatten() {
                    if line.contains("Xwayland") || line.contains("BadAccess") {
                        tracing::warn!("detected XWayland, xinput test-xi2 will not work");
                    }
                }
            });
        }

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

        Ok(child)
    }

    /// 启动 evtest fallback 监听器。
    /// evtest 需要读取 /dev/input/event* 设备，需要用户属于 input 组。
    fn start_evtest_fallback(
        &mut self,
        allowed_evdev: std::sync::Arc<[u16]>,
        tx: tokio::sync::mpsc::UnboundedSender<KeyEvent>,
        running: Arc<AtomicBool>,
    ) -> Result<()> {
        // Find keyboard devices
        let keyboard_devices = list_keyboard_devices()?;
        if keyboard_devices.is_empty() {
            return Err(anyhow::anyhow!(
                "no keyboard devices found for evtest fallback"
            ));
        }

        tracing::info!(
            "using evtest fallback with {} keyboard devices",
            keyboard_devices.len()
        );

        for device in keyboard_devices {
            let running = Arc::clone(&running);
            let tx = tx.clone();
            let device_path = device.clone();
            let allowed = std::sync::Arc::clone(&allowed_evdev);

            std::thread::spawn(move || {
                // evtest prints device info and all EV_* lines to stdout (not stderr).
                // Reading stderr caused Wayland fallback to never see key events.
                let mut child = match Command::new("evtest")
                    .arg(&device_path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(error = %e, device = %device_path.display(), "failed to spawn evtest");
                        return;
                    }
                };

                let stdout = child.stdout.take().expect("evtest stdout captured");
                let reader = std::io::BufReader::new(stdout);

                for line in reader.lines() {
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }

                    let line = match line {
                        Ok(l) => l,
                        Err(_) => continue,
                    };

                    // Parse evtest output: "Event: time ..., type 1 (EV_KEY), code 100 (KEY_RIGHTALT), value 1"
                    if line.contains("EV_KEY") {
                        let Some(code_tail) = line.split("code ").nth(1) else {
                            continue;
                        };
                        let Some(code_str) = code_tail.split_whitespace().next() else {
                            continue;
                        };
                        let Ok(code) = code_str.parse::<u16>() else {
                            continue;
                        };
                        // 仅匹配允许的 evdev 码（通常为单个；捕获模式为按下的物理码）。
                        let key_matches = allowed.contains(&code);
                        if key_matches {
                            let Some(value_tail) = line.split("value ").nth(1) else {
                                continue;
                            };
                            let value_raw = value_tail
                                .trim()
                                .split(|c: char| c.is_whitespace() || c == ',')
                                .next()
                                .unwrap_or("");
                            let Ok(value) = value_raw.parse::<i32>() else {
                                continue;
                            };
                            // evdev: 0 = release, 1 = press, 2 = autorepeat (key still held).
                            // Treating repeat as release breaks hold-to-record.
                            let pressed = match value {
                                0 => false,
                                1 => true,
                                2 => continue,
                                _ => continue,
                            };
                            if tx.send(KeyEvent { pressed }).is_err() {
                                tracing::warn!("evtest: receiver dropped, exiting");
                                break;
                            }
                        }
                    }
                }
            });
        }

        Ok(())
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
            let output = match Command::new("xmodmap").arg("-pke").output() {
                Ok(o) => o,
                Err(e) => {
                    tracing::error!(error = %e, "xmodmap not found or failed to run");
                    return std::collections::HashMap::new();
                }
            };

            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim().is_empty() {
                tracing::error!("xmodmap returned empty output");
                return std::collections::HashMap::new();
            }

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
            if map.is_empty() {
                tracing::error!("xmodmap output contained no parseable keycode mappings");
            }
            map
        });

        if keycode_map.is_empty() {
            return Err(anyhow::anyhow!(
                "xmodmap failed to produce keycode mappings — is xmodmap installed and DISPLAY set?"
            ));
        }

        let candidates: &[&str] = match self.key_name.as_str() {
            "Alt_L" => &["Alt_L"],
            // 布局上可能只出现 Alt_R、ISO_Level3_Shift 或 AltGr 之一，任一对上即可。
            "Alt_R" | "ISO_Level3_Shift" | "AltGr" => &["Alt_R", "ISO_Level3_Shift", "AltGr"],
            _ => {
                let name = self.key_name.as_str();
                // 单元素切片：自定义 keysym 名
                return keycode_map.get(name).copied().ok_or_else(|| {
                    anyhow::anyhow!(
                        "keycode for '{}' not found in xmodmap output",
                        self.key_name
                    )
                });
            }
        };

        for name in candidates {
            if let Some(&k) = keycode_map.get(*name) {
                tracing::info!(
                    keysym = %name,
                    keycode = k,
                    "resolved xmodmap keycode for trigger key"
                );
                return Ok(k);
            }
        }

        Err(anyhow::anyhow!(
            "keycode for trigger '{}' not found in xmodmap (tried: {:?})",
            self.key_name,
            candidates
        ))
    }
}

impl Drop for X11Listener {
    fn drop(&mut self) {
        self.stop();
    }
}
