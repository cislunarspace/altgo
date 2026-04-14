//! 按键监听器模块（跨平台）。
//!
//! 通过 `#[cfg(target_os)]` 条件编译为每个平台导出统一的类型别名 `PlatformListener`，
//! 实现静态分派，无需 trait 对象。
//!
//! - Linux：`xinput test-xi2`（XInput2 扩展）
//! - macOS：通过内联 Swift 脚本使用 CGEvent tap（需要辅助功能权限）
//! - Windows：PowerShell + `GetAsyncKeyState` 轮询

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub type PlatformListener = linux::X11Listener;

#[cfg(target_os = "macos")]
pub type PlatformListener = macos::MacOSListener;

#[cfg(target_os = "windows")]
pub type PlatformListener = windows::WindowsListener;

/// 按键事件。
#[derive(Debug)]
pub struct KeyEvent {
    /// 是否为按下事件
    pub pressed: bool,
}

/// 防抖任务：过滤 IME 引起的按键抖动，将稳定的按键事件转发给状态机。
pub(crate) async fn debounce_task(
    mut raw_events: tokio::sync::mpsc::UnboundedReceiver<KeyEvent>,
    key_tx: tokio::sync::mpsc::UnboundedSender<crate::state_machine::KeyEvent>,
    debounce_window: std::time::Duration,
) {
    let mut is_pressed = false;
    let mut pending_release: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;

    loop {
        tokio::select! {
            evt = raw_events.recv() => {
                match evt {
                    Some(evt) if evt.pressed => {
                        // Press cancels any pending release.
                        pending_release = None;
                        is_pressed = true;
                        if key_tx
                            .send(crate::state_machine::KeyEvent { pressed: true })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Some(_) => {
                        // Release — if no debounce is running, send immediately.
                        // If debounce is running, it will fire and send the release.
                        if is_pressed && pending_release.is_none() {
                            pending_release =
                                Some(Box::pin(tokio::time::sleep(debounce_window)));
                        }
                    }
                    None => break,
                }
            }
            // Debounce timer fired — forward the release to the state machine.
            _ = async {
                if let Some(timer) = &mut pending_release {
                    timer.as_mut().await;
                } else {
                    std::future::pending::<()>().await;
                }
            }, if pending_release.is_some() => {
                pending_release = None;
                is_pressed = false;
                if key_tx
                    .send(crate::state_machine::KeyEvent { pressed: false })
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}
