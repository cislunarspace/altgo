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

/// 持续监听激活键的 trait seam。
///
/// 由平台 adapter 实现（`X11Listener`、`WindowsListener`）。
/// pipeline 以 `Box<dyn KeyListener>` 消费，便于注入测试 fake。
pub trait KeyListener: Send {
    /// 开始监听，返回事件通道与后端标识（如 `"xinput"` / `"wh_keyboard_ll"`）。
    fn start(
        &mut self,
    ) -> anyhow::Result<(tokio::sync::mpsc::UnboundedReceiver<KeyEvent>, &'static str)>;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用 fake listener，验证 trait 可以被 boxing 并启动。
    struct FakeListener {
        backend: &'static str,
    }

    impl KeyListener for FakeListener {
        fn start(
            &mut self,
        ) -> anyhow::Result<(tokio::sync::mpsc::UnboundedReceiver<KeyEvent>, &'static str)>
        {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let _ = tx.send(KeyEvent { pressed: true });
            let _ = tx.send(KeyEvent { pressed: false });
            Ok((rx, self.backend))
        }
    }

    #[test]
    fn boxed_key_listener_starts_and_returns_events() {
        let mut listener: Box<dyn KeyListener> = Box::new(FakeListener {
            backend: "fake-test",
        });
        let (mut rx, name) = listener.start().unwrap();
        assert_eq!(name, "fake-test");

        let first = rx.try_recv().unwrap();
        assert!(first.pressed);
        let second = rx.try_recv().unwrap();
        assert!(!second.pressed);
    }

    #[tokio::test]
    async fn debounce_task_forwards_pressed_and_released_events() {
        let (in_tx, in_rx) = tokio::sync::mpsc::unbounded_channel();
        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();

        in_tx.send(KeyEvent { pressed: true }).unwrap();
        in_tx.send(KeyEvent { pressed: false }).unwrap();
        drop(in_tx);

        let handle = tokio::spawn(debounce_task(in_rx, out_tx));

        let first = out_rx.recv().await.unwrap();
        assert!(first.pressed);
        let second = out_rx.recv().await.unwrap();
        assert!(!second.pressed);
        assert!(out_rx.recv().await.is_none());

        handle.await.unwrap();
    }

    #[test]
    fn key_event_debug_format() {
        let evt = KeyEvent { pressed: true };
        let dbg = format!("{:?}", evt);
        assert!(dbg.contains("KeyEvent"));
        assert!(dbg.contains("true"));
    }
}
