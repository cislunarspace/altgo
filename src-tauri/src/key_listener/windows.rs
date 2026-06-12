//! Windows key listener stub.
//!
//! Low-level keyboard hook integration will live here. For now the pipeline uses
//! the platform alias and fails gracefully at runtime on Windows.

use super::KeyEvent;
use crate::config::KeyListenerConfig;
use anyhow::Result;
use tokio::sync::mpsc;

const BACKEND_NAME: &str = "wh_keyboard_ll";

#[allow(dead_code)]
pub fn key_name_from_vk(vk: i32) -> Option<String> {
    match vk {
        0xA4 => Some("Alt_L".to_string()),
        0xA5 => Some("Alt_R".to_string()),
        0xA2 => Some("Control_L".to_string()),
        0xA3 => Some("Control_R".to_string()),
        0xA0 => Some("Shift_L".to_string()),
        0xA1 => Some("Shift_R".to_string()),
        0x20 => Some("space".to_string()),
        0x0D => Some("Return".to_string()),
        0x09 => Some("Tab".to_string()),
        0x1B => Some("Escape".to_string()),
        vk if (0x70..=0x7B).contains(&vk) => Some(format!("F{}", vk - 0x6F)),
        _ => None,
    }
}

#[derive(Debug)]
pub struct WindowsListener {
    #[allow(dead_code)]
    target_vk: i32,
}

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

        Ok(Self { target_vk })
    }

    pub fn start(&mut self) -> Result<(mpsc::UnboundedReceiver<KeyEvent>, &'static str)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let _ = tx;
        Ok((rx, BACKEND_NAME))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_listener_from_key_name_fallback() {
        let cfg = crate::config::KeyListenerConfig {
            key_name: "Alt_R".to_string(),
            linux_evdev_code: None,
            windows_vk: None,
            long_press_threshold_ms: 200,
            double_click_interval_ms: 300,
            debounce_window_ms: 100,
            poll_interval_ms: 30,
            min_press_duration_ms: 100,
        };
        let listener = WindowsListener::new(&cfg);
        assert!(listener.is_ok());
    }

    #[test]
    fn rejects_unknown_key_name() {
        let cfg = crate::config::KeyListenerConfig {
            key_name: "Caps_Lock".to_string(),
            linux_evdev_code: None,
            windows_vk: None,
            long_press_threshold_ms: 200,
            double_click_interval_ms: 300,
            debounce_window_ms: 100,
            poll_interval_ms: 30,
            min_press_duration_ms: 100,
        };
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
        let cfg = crate::config::KeyListenerConfig {
            key_name: "space".to_string(),
            linux_evdev_code: None,
            windows_vk: Some(0x20),
            long_press_threshold_ms: 200,
            double_click_interval_ms: 300,
            debounce_window_ms: 100,
            poll_interval_ms: 30,
            min_press_duration_ms: 100,
        };
        let mut listener = WindowsListener::new(&cfg).unwrap();
        let (mut rx, backend) = listener.start().unwrap();
        assert_eq!(backend, "wh_keyboard_ll");
        // No events yet; just confirm the channel is alive.
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn vk_mapping_round_trips_for_supported_keys() {
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
            assert_eq!(
                key_name_from_vk(vk),
                Some(name.to_string()),
                "round-trip failed for {}",
                name
            );
        }
    }
}
