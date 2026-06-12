//! 按键监听器模块（Linux）。
//!
//! 使用 `xinput test-xi2`（XInput2 扩展）监听按键事件。

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{list_keyboard_devices, X11Listener};
#[cfg(target_os = "windows")]
pub use windows::{vk_from_key_name, WindowsListener};

#[cfg(target_os = "linux")]
pub type PlatformListener = X11Listener;
#[cfg(target_os = "windows")]
pub type PlatformListener = WindowsListener;

/// KeyListener configuration subset.
#[derive(Debug, Clone)]
pub struct KeyListenerConfig {
    pub key_name: String,
    pub linux_evdev_code: Option<u16>,
    pub windows_vk: Option<i32>,
    pub long_press_threshold: std::time::Duration,
    pub double_click_interval: std::time::Duration,
    pub debounce_window: std::time::Duration,
    pub poll_interval_ms: u64,
    pub min_press_duration: std::time::Duration,
}

impl From<&crate::config::Config> for KeyListenerConfig {
    fn from(cfg: &crate::config::Config) -> Self {
        Self {
            key_name: cfg.key_listener.key_name.clone(),
            linux_evdev_code: cfg.key_listener.linux_evdev_code,
            windows_vk: cfg.key_listener.windows_vk,
            long_press_threshold: cfg.key_listener.long_press_threshold(),
            double_click_interval: cfg.key_listener.double_click_interval(),
            debounce_window: cfg.key_listener.debounce_window(),
            poll_interval_ms: cfg.key_listener.poll_interval_ms,
            min_press_duration: cfg.key_listener.min_press_duration(),
        }
    }
}

/// 按键事件。
#[derive(Debug)]
pub struct KeyEvent {
    /// 是否为按下事件
    pub pressed: bool,
}

/// 将原始按键事件转发给状态机（无防抖，即时转发）。
///
/// 即时转发避免了短按松开后 `release` 晚于长按定时器导致状态机误触发录音的问题。
pub async fn debounce_task(
    mut key_events: tokio::sync::mpsc::UnboundedReceiver<KeyEvent>,
    key_tx: tokio::sync::mpsc::UnboundedSender<crate::state_machine::KeyEvent>,
) {
    while let Some(evt) = key_events.recv().await {
        if key_tx
            .send(crate::state_machine::KeyEvent {
                pressed: evt.pressed,
            })
            .is_err()
        {
            break;
        }
    }
}
