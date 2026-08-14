//! 短时捕获用户按下的物理键，用于设置激活录音键（Linux evdev）。

mod linux;

pub use linux::LinuxKeyCapture;

pub type PlatformKeyCapture = LinuxKeyCapture;

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureActivationResponse {
    pub key_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux_evdev_code: Option<u16>,
}

/// 平台激活键捕获的 trait seam。
///
/// 由 Linux adapter 实现；`capture()` 为同步阻塞方法，调用方负责在独立线程执行。
pub trait KeyCapture: Send {
    fn capture(&mut self) -> Result<CaptureActivationResponse, String>;
}

/// 将常见 evdev 码映射为 `xmodmap -pke` 中可能出现的 keysym 名称；未知则 `evdev_<code>`。
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
        _ => format!("evdev_{code}"),
    }
}
