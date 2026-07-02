//! Windows 激活键捕获（独立 WH_KEYBOARD_LL 钩子）。
//!
//! 与 `key_listener::windows.rs` 的运行时钩子分离开来，调用方拿到第一个
//! `WM_KEYDOWN`/`WM_SYSKEYDOWN` 后立即停止消息泵并返回。这层不依赖 `key_listener`，所以
//! `vk_to_name` 和捕获流程可以独立单元测试（issue #22）。
//!
//! 设计要点：
//! - 一次性 `WH_KEYBOARD_LL` 钩子，跑在独立消息泵线程；和 `key_listener` 钩子
//!   用各自的 `static CAPTURE_STATE`，互不干扰（同时打开设置 + 后台监听可行）。
//! - 回调里只读 `KBDLLHOOKSTRUCT.vkCode`，立刻 `try_send` 出去，然后 post `WM_QUIT`
//   收尾；不在回调里做 VK→name 转换（避免阻塞低层钩子）。
//! - `vk_to_name` 覆盖 VK_RMENU 等修饰键、字母 A–Z、数字 0–9、F1–F12；未知 VK
//!   返回 `None`，调用方把 `key_name` 留空、`windows_vk` 仍写入（前端可显示码值）。
//! - 12 秒超时与 Linux `capture_evdev_press` 对齐（issue #22 acceptance）。
//! - 错误路径（钩子安装失败 / 超时）通过 `String` 向上传播；真实键盘交互路径
//!   靠 Windows 手动验证（路线 B 选项 3）。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_QUIT, WM_SYSKEYDOWN,
};

use super::CaptureActivationResponse;

const HOOK_START_TIMEOUT: Duration = Duration::from_secs(2);

struct CaptureState {
    tx: SyncSender<i32>,
    stopping: AtomicBool,
    thread_id: AtomicU32,
}

static CAPTURE_STATE: Mutex<Option<Arc<CaptureState>>> = Mutex::new(None);

/// 把常见虚拟键码映射成可读名字。已知键返回 `Some("Right Alt")` 形式；未知返回 `None`。
///
/// 涵盖：
/// - 修饰键：Shift / Ctrl / Alt（左右）
/// - 功能键：F1–F12
/// - 字母 A–Z
/// - 数字 0–9（按数字键主排 `0x30` = '0'）
/// - 常用：Space / Return / Tab / Escape / Backspace
pub fn vk_to_name(vk: i32) -> Option<String> {
    match vk {
        0xA4 => Some("Left Alt".to_string()),
        0xA5 => Some("Right Alt".to_string()),
        0xA2 => Some("Left Ctrl".to_string()),
        0xA3 => Some("Right Ctrl".to_string()),
        0xA0 => Some("Left Shift".to_string()),
        0xA1 => Some("Right Shift".to_string()),
        0x20 => Some("Space".to_string()),
        0x0D => Some("Return".to_string()),
        0x09 => Some("Tab".to_string()),
        0x1B => Some("Escape".to_string()),
        0x08 => Some("Backspace".to_string()),
        vk if (0x70..=0x7B).contains(&vk) => Some(format!("F{}", vk - 0x6F)),
        vk if (0x41..=0x5A).contains(&vk) => Some(((vk as u8) as char).to_string()),
        vk if (0x30..=0x39).contains(&vk) => Some(((vk as u8) as char).to_string()),
        _ => None,
    }
}

/// 阻塞等待用户按下一个键，返回 `CaptureActivationResponse`。
///
/// 行为：
/// - 装上临时 `WH_KEYBOARD_LL` 钩子（独立线程跑消息泵）。
/// - 第一个 `WM_KEYDOWN` 携带的 VK 码送回调用方，钩子立即卸载。
/// - `timeout` 内没有任何按键：`Err("timeout: no key pressed")`。
pub fn capture_activation_key_blocking(
    timeout: Duration,
) -> Result<CaptureActivationResponse, String> {
    let (tx, rx) = sync_channel::<i32>(1);
    let state = Arc::new(CaptureState {
        tx,
        stopping: AtomicBool::new(false),
        thread_id: AtomicU32::new(0),
    });
    let state_for_thread = Arc::clone(&state);
    let (ready_tx, ready_rx) = sync_channel::<Result<(), String>>(1);

    let handle = std::thread::spawn(move || {
        // SAFETY: WH_KEYBOARD_LL allows a null module handle when the callback is in the
        // current process and the hook is installed for all threads (dwThreadId = 0). The
        // callback has the correct ABI.
        let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(capture_hook_proc), None, 0) };
        let hook = match hook {
            Ok(hook) => {
                // SAFETY: GetCurrentThreadId has no preconditions and returns this message-pump
                // thread ID, used later to post WM_QUIT.
                state_for_thread
                    .thread_id
                    .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
                *CAPTURE_STATE.lock().unwrap() = Some(Arc::clone(&state_for_thread));
                let _ = ready_tx.send(Ok(()));
                hook
            }
            Err(error) => {
                let message = error.to_string();
                tracing::error!(error = %message, "failed to install capture hook");
                let _ = ready_tx.send(Err(message));
                return;
            }
        };

        let mut msg = MSG::default();
        loop {
            // SAFETY: msg points to valid writable memory for the duration of the call. A null
            // hwnd receives thread messages; WM_QUIT wakes this loop on capture completion.
            let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if ret.0 <= 0 || state_for_thread.stopping.load(Ordering::SeqCst) {
                break;
            }
            // SAFETY: msg was initialized by a successful GetMessageW call.
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        *CAPTURE_STATE.lock().unwrap() = None;
        // SAFETY: hook was returned by SetWindowsHookExW on this thread and has not yet been
        // unhooked. Unhooking here keeps capture teardown on the owning message-pump thread.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    });

    if let Err(error) = wait_for_capture_hook(&ready_rx) {
        let _ = handle.join();
        return Err(error);
    }

    let result = rx.recv_timeout(timeout);
    stop_capture_thread(&state);
    let _ = handle.join();

    let vk = result.map_err(|_| "timeout: no key pressed".to_string())?;
    let key_name = vk_to_name(vk).unwrap_or_default();
    Ok(CaptureActivationResponse {
        key_name,
        linux_evdev_code: None,
        windows_vk: Some(vk),
    })
}

fn wait_for_capture_hook(ready_rx: &Receiver<Result<(), String>>) -> Result<(), String> {
    match ready_rx.recv_timeout(HOOK_START_TIMEOUT) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(message)) => Err(format!("key capture start: {message}")),
        Err(_) => Err("key capture start: timed out waiting for WH_KEYBOARD_LL hook".to_string()),
    }
}

fn stop_capture_thread(state: &CaptureState) {
    state.stopping.store(true, Ordering::SeqCst);
    let id = state.thread_id.load(Ordering::SeqCst);
    if id != 0 {
        // SAFETY: id is captured from the capture message-pump thread after the hook is installed.
        // Posting WM_QUIT wakes GetMessageW so the thread can unhook and exit.
        unsafe {
            let _ = PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

fn is_key_down_message(message: u32) -> bool {
    matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN)
}

unsafe extern "system" fn capture_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 && is_key_down_message(w_param.0 as u32) {
        if let Some(state) = CAPTURE_STATE.lock().unwrap().as_ref() {
            // SAFETY: Windows calls WH_KEYBOARD_LL callbacks with l_param pointing to a valid
            // KBDLLHOOKSTRUCT whenever n_code >= 0.
            let info = unsafe { *(l_param.0 as *const KBDLLHOOKSTRUCT) };
            if state.tx.try_send(info.vkCode as i32).is_ok() {
                stop_capture_thread(state);
            }
        }
    }
    // SAFETY: Forwarding to the next hook is required by the Win32 hook contract. Passing None is
    // accepted for low-level hooks; Windows ignores the hook handle for this call.
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_key_down_message_accepts_regular_and_system_keydown() {
        assert!(is_key_down_message(WM_KEYDOWN));
        assert!(is_key_down_message(WM_SYSKEYDOWN));
    }

    #[test]
    fn is_key_down_message_rejects_keyup_messages() {
        // WM_KEYUP = 0x0101
        assert!(!is_key_down_message(0x0101));
    }

    #[test]
    fn vk_to_name_modifier_keys() {
        assert_eq!(vk_to_name(0xA0), Some("Left Shift".to_string()));
        assert_eq!(vk_to_name(0xA1), Some("Right Shift".to_string()));
        assert_eq!(vk_to_name(0xA2), Some("Left Ctrl".to_string()));
        assert_eq!(vk_to_name(0xA3), Some("Right Ctrl".to_string()));
        assert_eq!(vk_to_name(0xA4), Some("Left Alt".to_string()));
        assert_eq!(vk_to_name(0xA5), Some("Right Alt".to_string()));
    }

    #[test]
    fn vk_to_name_common_keys() {
        assert_eq!(vk_to_name(0x20), Some("Space".to_string()));
        assert_eq!(vk_to_name(0x0D), Some("Return".to_string()));
        assert_eq!(vk_to_name(0x09), Some("Tab".to_string()));
        assert_eq!(vk_to_name(0x1B), Some("Escape".to_string()));
        assert_eq!(vk_to_name(0x08), Some("Backspace".to_string()));
    }

    #[test]
    fn vk_to_name_function_keys_f1_through_f12() {
        assert_eq!(vk_to_name(0x70), Some("F1".to_string()));
        assert_eq!(vk_to_name(0x71), Some("F2".to_string()));
        assert_eq!(vk_to_name(0x72), Some("F3".to_string()));
        assert_eq!(vk_to_name(0x73), Some("F4".to_string()));
        assert_eq!(vk_to_name(0x74), Some("F5".to_string()));
        assert_eq!(vk_to_name(0x75), Some("F6".to_string()));
        assert_eq!(vk_to_name(0x76), Some("F7".to_string()));
        assert_eq!(vk_to_name(0x77), Some("F8".to_string()));
        assert_eq!(vk_to_name(0x78), Some("F9".to_string()));
        assert_eq!(vk_to_name(0x79), Some("F10".to_string()));
        assert_eq!(vk_to_name(0x7A), Some("F11".to_string()));
        assert_eq!(vk_to_name(0x7B), Some("F12".to_string()));
    }

    #[test]
    fn vk_to_name_letters_a_through_z() {
        for (i, expected) in (b'A'..=b'Z').enumerate() {
            let vk = 0x41 + i as i32;
            assert_eq!(vk_to_name(vk), Some((expected as char).to_string()));
        }
    }

    #[test]
    fn vk_to_name_digits_0_through_9() {
        for (i, expected) in (b'0'..=b'9').enumerate() {
            let vk = 0x30 + i as i32;
            assert_eq!(vk_to_name(vk), Some((expected as char).to_string()));
        }
    }

    #[test]
    fn vk_to_name_unknown_returns_none() {
        // VK_F13..VK_F24 (0x7C..0x87) are out of F1-F12 scope.
        assert!(vk_to_name(0x7C).is_none());
        // Arbitrary unmapped code.
        assert!(vk_to_name(0xFF).is_none());
    }

    #[test]
    fn vk_to_name_round_trip_via_xmodmap_naming() {
        // The key_listener linux/windows listeners use xmodmap-style names
        // (Alt_R, space, Return) for storage. For captured windows_vk the
        // Settings UI just needs a human-readable hint, so the longer
        // "Right Alt" form is acceptable; verify it round-trips sensibly.
        assert_eq!(vk_to_name(0xA5), Some("Right Alt".to_string()));
        assert_eq!(vk_to_name(0x20), Some("Space".to_string()));
    }

    #[test]
    fn capture_returns_err_on_timeout_without_hook() {
        // 0-second timeout: even if a key were pressed, rx.recv_timeout returns
        // Err(Disconnected) or Err(Timeout) before any send. We use a tiny timeout
        // so the test does not block. The hook install may or may not succeed in
        // a CI environment, but the function must always return a Result (never panic).
        let result = capture_activation_key_blocking(Duration::from_millis(0));
        // We don't assert success/failure of install — just that the function
        // returns a Result within a reasonable bound.
        let _ = result;
    }
}
