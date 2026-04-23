//! 按键监听器模块（Linux）。
//!
//! 使用 `xinput test-xi2`（XInput2 扩展）监听按键事件。

mod linux;

pub(crate) use linux::list_keyboard_devices;
#[allow(dead_code)]
pub type PlatformListener = linux::X11Listener;

/// 按键事件。
#[derive(Debug)]
pub struct KeyEvent {
    /// 是否为按下事件
    pub pressed: bool,
}

/// 将原始按键事件转发给状态机。
///
/// 此前实现曾错误地在松开时延迟发送 release（防抖），导致短按松开后 `release`
/// 晚于长按定时器，状态机误触发录音。现改为即时转发；若需抑制 IME 抖动，可在上层调参。
pub async fn debounce_task(
    mut key_events: tokio::sync::mpsc::UnboundedReceiver<KeyEvent>,
    key_tx: tokio::sync::mpsc::UnboundedSender<crate::state_machine::KeyEvent>,
    _debounce_window: std::time::Duration,
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
