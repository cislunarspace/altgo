//! Windows key listener.
//!
//! Uses a `WH_KEYBOARD_LL` low-level keyboard hook running in a dedicated
//! message-pump thread. The hook callback filters by the configured virtual-key
//! code and forwards `KeyEvent`s to the async pipeline through a tokio channel.

use super::{KeyEvent, KeyListener};
use crate::config::KeyListenerConfig;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_KEYUP, WM_QUIT,
};

const BACKEND_NAME: &str = "wh_keyboard_ll";
const HOOK_START_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

struct HookState {
    target_vk: i32,
    tx: mpsc::UnboundedSender<KeyEvent>,
    stopping: AtomicBool,
}

impl HookState {
    fn send(&self, pressed: bool) {
        let _ = self.tx.send(KeyEvent { pressed });
    }

    fn should_forward(&self, vk: i32) -> bool {
        vk == self.target_vk
    }
}

static HOOK_STATE: Mutex<Option<Arc<HookState>>> = Mutex::new(None);

pub fn vk_from_key_name(key_name: &str) -> Option<i32> {
    match key_name {
        "Alt_L" => Some(0xA4),     // VK_LMENU
        "Alt_R" => Some(0xA5),     // VK_RMENU
        "Control_L" => Some(0xA2), // VK_LCONTROL
        "Control_R" => Some(0xA3), // VK_RCONTROL
        "Shift_L" => Some(0xA0),   // VK_LSHIFT
        "Shift_R" => Some(0xA1),   // VK_RSHIFT
        "space" => Some(0x20),     // VK_SPACE
        "Return" => Some(0x0D),    // VK_RETURN
        "Tab" => Some(0x09),       // VK_TAB
        "Escape" => Some(0x1B),    // VK_ESCAPE
        _ => vk_from_function_key(key_name),
    }
}

fn vk_from_function_key(key_name: &str) -> Option<i32> {
    if let Some(rest) = key_name.strip_prefix('F') {
        if let Ok(n) = rest.parse::<i32>() {
            if (1..=12).contains(&n) {
                return Some(0x6F + n); // VK_F1 == 0x70
            }
        }
    }
    None
}

#[derive(Debug)]
pub struct WindowsListener {
    target_vk: i32,
    hook_thread: Option<std::thread::JoinHandle<()>>,
    thread_id: Arc<AtomicU32>,
}

impl WindowsListener {
    pub fn new(cfg: &KeyListenerConfig) -> Result<Self> {
        let target_vk = cfg
            .windows_vk
            .or_else(|| vk_from_key_name(&cfg.key_name))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "unsupported activation key '{}' on Windows; capture a key or choose from supported names",
                    cfg.key_name
                )
            })?;

        Ok(Self {
            target_vk,
            hook_thread: None,
            thread_id: Arc::new(AtomicU32::new(0)),
        })
    }

    pub fn start(&mut self) -> Result<(mpsc::UnboundedReceiver<KeyEvent>, &'static str)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
        let target_vk = self.target_vk;
        let thread_id = Arc::clone(&self.thread_id);

        let handle = std::thread::spawn(move || {
            let state = Arc::new(HookState {
                target_vk,
                tx,
                stopping: AtomicBool::new(false),
            });

            // SAFETY: WH_KEYBOARD_LL allows a null module handle when the callback is in the
            // current process and the hook is installed for all threads (dwThreadId = 0). The
            // callback has 'static lifetime and follows the required system ABI.
            let hook = unsafe {
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0)
            };
            let hook = match hook {
                Ok(hook) => {
                    *HOOK_STATE.lock().unwrap() = Some(Arc::clone(&state));
                    // SAFETY: GetCurrentThreadId has no preconditions and returns the ID of this
                    // hook/message-pump thread, which Drop later uses with PostThreadMessageW.
                    thread_id.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
                    let _ = ready_tx.send(Ok(()));
                    hook
                }
                Err(error) => {
                    let message = error.to_string();
                    tracing::error!(error = %message, "failed to install WH_KEYBOARD_LL hook");
                    let _ = ready_tx.send(Err(message));
                    return;
                }
            };

            let mut msg = MSG::default();
            loop {
                // SAFETY: msg points to valid writable memory for the duration of the call. A
                // null hwnd receives thread messages; WM_QUIT wakes this loop during Drop.
                let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
                if ret.0 <= 0 || state.stopping.load(Ordering::SeqCst) {
                    break;
                }
                // SAFETY: msg was initialized by a successful GetMessageW call.
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            *HOOK_STATE.lock().unwrap() = None;
            // SAFETY: hook was returned by SetWindowsHookExW on this thread and has not yet been
            // unhooked. Unhooking here keeps hook teardown on the owning message-pump thread.
            let _ = unsafe { UnhookWindowsHookEx(hook) };
        });

        self.hook_thread = Some(handle);
        match ready_rx.recv_timeout(HOOK_START_TIMEOUT) {
            Ok(Ok(())) => Ok((rx, BACKEND_NAME)),
            Ok(Err(message)) => {
                self.join_hook_thread();
                anyhow::bail!("key listener start: {message}")
            }
            Err(_) => {
                self.stop_hook_thread();
                anyhow::bail!("key listener start: timed out waiting for WH_KEYBOARD_LL hook")
            }
        }
    }

    fn stop_hook_thread(&mut self) {
        if let Some(state) = HOOK_STATE.lock().unwrap().as_ref() {
            state.stopping.store(true, Ordering::SeqCst);
        }

        let id = self.thread_id.load(Ordering::SeqCst);
        if id != 0 {
            // SAFETY: id is captured from the hook message-pump thread after it has installed its
            // message queue. Posting WM_QUIT is the documented way to wake GetMessageW.
            unsafe {
                let _ = PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }

        self.join_hook_thread();
    }

    fn join_hook_thread(&mut self) {
        if let Some(handle) = self.hook_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WindowsListener {
    fn drop(&mut self) {
        self.stop_hook_thread();
    }
}

impl KeyListener for WindowsListener {
    fn start(&mut self) -> Result<(mpsc::UnboundedReceiver<KeyEvent>, &'static str)> {
        self.start()
    }
}

unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        if let Some(state) = HOOK_STATE.lock().unwrap().as_ref() {
            // SAFETY: Windows calls WH_KEYBOARD_LL callbacks with l_param pointing to a valid
            // KBDLLHOOKSTRUCT whenever n_code >= 0.
            let info = unsafe { *(l_param.0 as *const KBDLLHOOKSTRUCT) };
            let vk = info.vkCode as i32;

            if state.should_forward(vk) {
                match w_param.0 as u32 {
                    WM_KEYDOWN => state.send(true),
                    WM_KEYUP => state.send(false),
                    _ => {}
                }
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

    fn test_config() -> crate::config::KeyListenerConfig {
        crate::config::KeyListenerConfig {
            key_name: "Alt_R".to_string(),
            linux_evdev_code: None,
            windows_vk: None,
            long_press_threshold_ms: 200,
            double_click_interval_ms: 300,
            debounce_window_ms: 100,
            poll_interval_ms: 30,
            min_press_duration_ms: 100,
        }
    }

    #[test]
    fn constructs_listener_from_key_name_fallback() {
        let listener = WindowsListener::new(&test_config());
        assert!(listener.is_ok());
    }

    #[test]
    fn constructs_listener_from_windows_vk() {
        let mut cfg = test_config();
        cfg.key_name = "space".to_string();
        cfg.windows_vk = Some(0x20);
        let listener = WindowsListener::new(&cfg);
        assert!(listener.is_ok());
    }

    #[test]
    fn rejects_unknown_key_name() {
        let mut cfg = test_config();
        cfg.key_name = "Caps_Lock".to_string();
        let result = WindowsListener::new(&cfg);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Caps_Lock"),
            "error should mention key name: {}",
            err
        );
    }

    #[test]
    fn start_returns_receiver_and_backend_name() {
        let mut cfg = test_config();
        cfg.windows_vk = Some(0x20);
        let mut listener = WindowsListener::new(&cfg).unwrap();
        let (mut rx, backend) = listener.start().unwrap();
        assert_eq!(backend, "wh_keyboard_ll");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn vk_from_key_name_resolves_supported_names() {
        // The listener takes xmodmap-style names; verify each supported name
        // resolves to a non-zero VK. (The reverse VK→name mapping for
        // capture is in `crate::key_capture::windows::vk_to_name`.)
        let names = [
            "Alt_L",
            "Alt_R",
            "Control_L",
            "Control_R",
            "Shift_L",
            "Shift_R",
            "space",
            "Return",
            "Tab",
            "Escape",
            "F1",
            "F2",
            "F3",
            "F4",
            "F5",
            "F6",
            "F7",
            "F8",
            "F9",
            "F10",
            "F11",
            "F12",
        ];
        for name in names {
            let vk = vk_from_key_name(name).expect(name);
            assert!(vk > 0, "expected non-zero VK for {name}, got {vk}");
        }
    }
}
