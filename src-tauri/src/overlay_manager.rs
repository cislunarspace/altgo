//! 悬浮窗管理模块。
//!
//! 把窗口物理属性（尺寸、位置、显隐）从 `TauriPipelineSink` 中分离出来，
//! 提供单一接口 `set_state`：Rust 只描述意图，由本模块一次性完成所有窗口操作。
//!
//! 与前端的分工：
//! - 本模块负责**窗口物理层**：emit `overlay-state` 事件、resize、reposition、show/hide
//! - 前端负责**视觉层**：CSS transition / animation 处理 entry / exit / crossfade

use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition};

/// 悬浮窗视觉状态 —— 一次性发给前端，驱动所有 CSS 过渡。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    /// 当前阶段：`"recording"` / `"processing"` / `"done"` / `"hidden"`
    pub phase: String,
}

impl OverlayState {
    pub fn recording() -> Self {
        Self {
            phase: "recording".into(),
        }
    }
    pub fn processing() -> Self {
        Self {
            phase: "processing".into(),
        }
    }
    pub fn done() -> Self {
        Self {
            phase: "done".into(),
        }
    }
    pub fn hidden() -> Self {
        Self {
            phase: "hidden".into(),
        }
    }
}

/// 各阶段对应的悬浮窗逻辑尺寸（CSS pixels）。
const SIZE_RECORDING: (f64, f64) = (200.0, 48.0);
const SIZE_PROCESSING: (f64, f64) = (268.0, 56.0);
const SIZE_DONE: (f64, f64) = (520.0, 100.0);

/// 距屏幕底部的偏移（CSS pixels）。
const BOTTOM_OFFSET: f64 = 80.0;

/// 悬浮窗管理器 —— 负责窗口物理层的一切操作。
#[derive(Clone)]
pub struct OverlayManager {
    app: tauri::AppHandle,
}

impl OverlayManager {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }

    /// 设置悬浮窗状态。
    ///
    /// 这是一个**原子意图**：调用方只需描述「现在应该显示什么阶段」，
    /// 本方法内部一次性完成 emit → resize → reposition → show/hide，
    /// 不再拆成三次独立 IPC。
    pub fn set_state(&self, state: OverlayState) {
        // 1. 先发事件给前端，让 CSS 立即开始过渡
        let _ = self.app.emit("overlay-state", &state);

        // 2. 同步调整窗口物理属性
        if let Some(overlay) = self.app.get_webview_window("overlay") {
            match state.phase.as_str() {
                "hidden" => {
                    let _ = overlay.hide();
                }
                phase => {
                    let (w, h) = dimensions_for_phase(phase);
                    if let Err(e) = overlay.set_size(LogicalSize::new(w, h)) {
                        tracing::warn!(error = %e, "overlay set_size failed");
                    }
                    if let Err(e) = position_overlay(&overlay, w, h) {
                        tracing::warn!(error = %e, "overlay positioning failed");
                    }
                    if let Err(e) = overlay.show() {
                        tracing::warn!(error = %e, "overlay show failed");
                    }
                }
            }
        }
    }
}

fn dimensions_for_phase(phase: &str) -> (f64, f64) {
    match phase {
        "recording" => SIZE_RECORDING,
        "processing" => SIZE_PROCESSING,
        "done" => SIZE_DONE,
        _ => SIZE_RECORDING,
    }
}

/// 使用 `xrandr` 获取主显示器几何信息（物理坐标）。
fn xrandr_primary_monitor() -> Option<(i32, i32, i32, i32)> {
    let output = std::process::Command::new("xrandr").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let monitors = parse_xrandr_geometry(&text);
    monitors
        .iter()
        .find(|m| m.4)
        .map(|m| (m.0, m.1, m.2, m.3))
        .or_else(|| monitors.into_iter().next().map(|m| (m.0, m.1, m.2, m.3)))
}

fn parse_xrandr_geometry(output: &str) -> Vec<(i32, i32, i32, i32, bool)> {
    let mut monitors = Vec::new();
    for line in output.lines() {
        if !line.contains(" connected ") {
            continue;
        }
        let is_primary = line.contains(" connected primary ");
        let after_conn = if is_primary {
            line.split(" connected primary ").nth(1)
        } else {
            line.split(" connected ").nth(1)
        };
        let after_conn = match after_conn {
            Some(s) => s,
            None => continue,
        };
        let end = after_conn.find(' ').unwrap_or(after_conn.len());
        let geo = &after_conn[..end];

        let Some(x_idx) = geo.find('x') else { continue };
        let Some(plus1) = geo.find('+') else { continue };
        let Some(plus2) = geo.rfind('+') else {
            continue;
        };
        if plus1 == plus2 {
            continue;
        }

        let w = geo[..x_idx].parse::<i32>().unwrap_or(0);
        let h = geo[x_idx + 1..plus1].parse::<i32>().unwrap_or(0);
        let x = geo[plus1 + 1..plus2].parse::<i32>().unwrap_or(0);
        let y = geo[plus2 + 1..].parse::<i32>().unwrap_or(0);

        if w > 0 && h > 0 {
            monitors.push((x, y, w, h, is_primary));
        }
    }
    monitors
}

fn position_overlay(overlay: &tauri::WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let Some((mx, my, mw, mh)) = xrandr_primary_monitor() else {
        return Err("xrandr returned no primary monitor".into());
    };

    let scale = overlay.scale_factor().map_err(|e| e.to_string())?;
    let phys_w = (width * scale).round() as i32;
    let phys_h = (height * scale).round() as i32;
    let offset_phys = (BOTTOM_OFFSET * scale).round() as i32;

    let x = mx + (mw - phys_w) / 2;
    let y = my + mh - phys_h - offset_phys;

    tracing::debug!(
        "overlay pos: primary=({},{},{},{}) pos=({},{}) scale={}",
        mx,
        my,
        mw,
        mh,
        x,
        y,
        scale
    );

    overlay
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimensions_for_phase() {
        assert_eq!(dimensions_for_phase("recording"), SIZE_RECORDING);
        assert_eq!(dimensions_for_phase("processing"), SIZE_PROCESSING);
        assert_eq!(dimensions_for_phase("done"), SIZE_DONE);
        assert_eq!(dimensions_for_phase("unknown"), SIZE_RECORDING);
    }

    #[test]
    fn test_parse_xrandr_geometry() {
        let sample = r#"
DP-1 connected primary 3840x2160+0+0 (normal left inverted right x axis y axis) 597mm x 336mm
DP-2 connected 1920x1080+3840+0 (normal left inverted right x axis y axis) 527mm x 296mm
"#;
        let parsed = parse_xrandr_geometry(sample);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], (0, 0, 3840, 2160, true));
        assert_eq!(parsed[1], (3840, 0, 1920, 1080, false));
    }

    #[test]
    fn test_parse_xrandr_skips_disconnected() {
        let sample = "DP-1 disconnected (normal left inverted right x axis y axis)";
        assert!(parse_xrandr_geometry(sample).is_empty());
    }
}
