//! 短时捕获用户按下的物理键，用于设置激活录音键（Linux evdev / Windows VK）。

use serde::Serialize;

#[cfg(target_os = "linux")]
use std::io::BufRead;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
#[cfg(target_os = "linux")]
use std::sync::mpsc;
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::time::{Duration, Instant};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureActivationResponse {
    pub key_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux_evdev_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_vk: Option<i32>,
}

#[cfg(target_os = "linux")]
fn parse_ev_key_line(line: &str) -> Option<(u16, i32)> {
    if !line.contains("EV_KEY") {
        return None;
    }
    let code_tail = line.split("code ").nth(1)?;
    let code_str = code_tail.split_whitespace().next()?;
    let code = code_str.parse::<u16>().ok()?;
    let value_tail = line.split("value ").nth(1)?;
    let value_raw = value_tail
        .trim()
        .split(|c: char| c.is_whitespace() || c == ',')
        .next()?;
    let value = value_raw.parse::<i32>().ok()?;
    Some((code, value))
}

/// 将常见 evdev 码映射为 `xmodmap -pke` 中可能出现的 keysym 名称；未知则 `evdev_<code>`。
#[cfg(target_os = "linux")]
pub fn evdev_code_to_keysym_name(code: u16) -> String {
    match code {
        1 => "Escape".to_string(),
        2 => "1".to_string(),
        3 => "2".to_string(),
        4 => "3".to_string(),
        5 => "4".to_string(),
        6 => "5".to_string(),
        7 => "6".to_string(),
        8 => "7".to_string(),
        9 => "8".to_string(),
        10 => "9".to_string(),
        11 => "0".to_string(),
        12 => "minus".to_string(),
        13 => "equal".to_string(),
        14 => "BackSpace".to_string(),
        15 => "Tab".to_string(),
        16 => "q".to_string(),
        17 => "w".to_string(),
        18 => "e".to_string(),
        19 => "r".to_string(),
        20 => "t".to_string(),
        21 => "y".to_string(),
        22 => "u".to_string(),
        23 => "i".to_string(),
        24 => "o".to_string(),
        25 => "p".to_string(),
        26 => "bracketleft".to_string(),
        27 => "bracketright".to_string(),
        28 => "Return".to_string(),
        29 => "Control_L".to_string(),
        30 => "a".to_string(),
        31 => "s".to_string(),
        32 => "d".to_string(),
        33 => "f".to_string(),
        34 => "g".to_string(),
        35 => "h".to_string(),
        36 => "j".to_string(),
        37 => "k".to_string(),
        38 => "l".to_string(),
        39 => "semicolon".to_string(),
        40 => "apostrophe".to_string(),
        41 => "grave".to_string(),
        42 => "Shift_L".to_string(),
        43 => "backslash".to_string(),
        44 => "z".to_string(),
        45 => "x".to_string(),
        46 => "c".to_string(),
        47 => "v".to_string(),
        48 => "b".to_string(),
        49 => "n".to_string(),
        50 => "m".to_string(),
        51 => "comma".to_string(),
        52 => "period".to_string(),
        53 => "slash".to_string(),
        54 => "Shift_R".to_string(),
        55 => "KP_Multiply".to_string(),
        56 => "Alt_L".to_string(),
        57 => "space".to_string(),
        58 => "Caps_Lock".to_string(),
        59 => "F1".to_string(),
        60 => "F2".to_string(),
        61 => "F3".to_string(),
        62 => "F4".to_string(),
        63 => "F5".to_string(),
        64 => "F6".to_string(),
        65 => "F7".to_string(),
        66 => "F8".to_string(),
        67 => "F9".to_string(),
        68 => "F10".to_string(),
        69 => "Num_Lock".to_string(),
        70 => "Scroll_Lock".to_string(),
        87 => "F11".to_string(),
        88 => "F12".to_string(),
        96 => "KP_Enter".to_string(),
        97 => "Control_R".to_string(),
        98 => "KP_Divide".to_string(),
        99 => "Print".to_string(),
        100 => "ISO_Level3_Shift".to_string(),
        102 => "Home".to_string(),
        103 => "Up".to_string(),
        104 => "Prior".to_string(),
        105 => "Left".to_string(),
        106 => "Right".to_string(),
        107 => "End".to_string(),
        108 => "Down".to_string(),
        109 => "Next".to_string(),
        110 => "Insert".to_string(),
        111 => "Delete".to_string(),
        113 => "Mute".to_string(),
        114 => "VolumeDown".to_string(),
        115 => "VolumeUp".to_string(),
        125 => "Super_L".to_string(),
        126 => "Super_R".to_string(),
        _ => format!("evdev_{code}"),
    }
}

#[cfg(target_os = "linux")]
fn capture_evdev_press(timeout: Duration) -> Result<u16, String> {
    let devices = crate::key_listener::list_keyboard_devices()
        .map_err(|e| format!("keyboard devices: {}", e))?;
    if devices.is_empty() {
        return Err("未找到键盘设备（/dev/input）".into());
    }

    let (tx, rx) = mpsc::sync_channel::<u16>(1);
    let deadline = Instant::now() + timeout;

    for device in devices {
        let tx = tx.clone();
        let path: PathBuf = device;
        thread::spawn(move || {
            let mut child = match Command::new("evtest")
                .arg(&path)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => return,
            };
            let stdout = match child.stdout.take() {
                Some(s) => s,
                None => return,
            };
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let Some((code, value)) = parse_ev_key_line(&line) else {
                    continue;
                };
                if value == 1 && tx.try_send(code).is_ok() {
                    let _ = child.kill();
                    return;
                }
            }
        });
    }

    drop(tx);
    let remaining = deadline.saturating_duration_since(Instant::now());
    rx.recv_timeout(remaining).map_err(|_| {
        "超时：未检测到按键（请确认对 /dev/input 有读权限，如在 input 组）".to_string()
    })
}

#[cfg(target_os = "linux")]
pub fn capture_activation_key_blocking() -> Result<CaptureActivationResponse, String> {
    let code = capture_evdev_press(Duration::from_secs(12))?;
    let key_name = evdev_code_to_keysym_name(code);
    Ok(CaptureActivationResponse {
        key_name,
        linux_evdev_code: Some(code),
        windows_vk: None,
    })
}

#[cfg(target_os = "windows")]
fn vk_to_display_name(vk: i32) -> String {
    match vk {
        0xA5 => "ISO_Level3_Shift".to_string(),
        0xA4 => "Alt_L".to_string(),
        0x5B => "Super_L".to_string(),
        0x5C => "Super_R".to_string(),
        0xA3 => "Control_R".to_string(),
        0xA1 => "Shift_R".to_string(),
        0xA0 => "Shift_L".to_string(),
        0xA2 => "Control_L".to_string(),
        0x20 => "space".to_string(),
        0x08 => "Back".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Return".to_string(),
        0x1B => "Escape".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        0x21 => "Prior".to_string(),
        0x22 => "Next".to_string(),
        0x23 => "End".to_string(),
        0x24 => "Home".to_string(),
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),
        0x30..=0x39 => format!("{}", vk - 0x30),
        0x41..=0x5A => {
            let c = (vk - 0x41 + b'a') as char;
            c.to_string()
        }
        0x70..=0x7B => format!("F{}", vk - 0x70 + 1),
        _ => format!("VK_0x{vk:02X}"),
    }
}

#[cfg(target_os = "windows")]
pub fn capture_activation_key_blocking() -> Result<CaptureActivationResponse, String> {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(vKey: i32) -> i16;
    }
    fn down(vk: i32) -> bool {
        unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
    }

    let timeout = Duration::from_secs(12);
    let start = Instant::now();
    let mut prev = vec![false; 256];
    for vk in 0..256i32 {
        if vk <= 0x06 {
            continue;
        }
        prev[vk as usize] = down(vk);
    }

    while start.elapsed() < timeout {
        for vk in 0..256i32 {
            if vk <= 0x06 {
                continue;
            }
            let now = down(vk);
            if now && !prev[vk as usize] {
                let name = vk_to_display_name(vk);
                return Ok(CaptureActivationResponse {
                    key_name: name,
                    linux_evdev_code: None,
                    windows_vk: Some(vk),
                });
            }
            prev[vk as usize] = now;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    Err("超时：未检测到按键".into())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn capture_activation_key_blocking() -> Result<CaptureActivationResponse, String> {
    Err("不支持的平台".into())
}
