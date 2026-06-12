//! 悬浮窗管理模块。
//!
//! 把 Overlay 的状态意图与窗口物理操作分离：调用方只描述状态，本模块通过
//! `OverlayWindow` seam 计算尺寸和位置，再由具体 adapter 执行窗口操作。
//!
//! 与前端的分工：
//! - 本模块负责**窗口物理层**：emit `overlay-state` 事件、resize、reposition、show/hide
//! - 前端负责**视觉层**：CSS transition / animation 处理 entry / exit / crossfade

use tauri::{LogicalSize, PhysicalPosition};

use crate::overlay_window::{OverlayError, OverlayWindow};

pub use crate::overlay_window::OverlayState;

/// 各阶段对应的悬浮窗逻辑尺寸（CSS pixels）。
const SIZE_RECORDING: (f64, f64) = (200.0, 48.0);
const SIZE_PROCESSING: (f64, f64) = (268.0, 56.0);
const SIZE_DONE: (f64, f64) = (520.0, 100.0);

/// 距屏幕底部的偏移（CSS pixels）。
const BOTTOM_OFFSET: f64 = 80.0;

/// 悬浮窗管理器 —— 负责把 Overlay 状态意图翻译成窗口操作。
#[derive(Clone)]
pub struct OverlayManager<W: OverlayWindow> {
    window: W,
}

impl<W: OverlayWindow> OverlayManager<W> {
    pub fn new(window: W) -> Self {
        Self { window }
    }

    /// 设置悬浮窗状态。
    ///
    /// 这是一个**原子意图**：调用方只需描述「现在应该显示什么阶段」，
    /// 本方法内部一次性完成 resize → reposition → prepare → show → emit。
    pub fn set_state(&self, state: OverlayState) {
        if state.phase == "hidden" {
            if let Err(error) = self.window.emit_state(&state) {
                tracing::warn!(%error, "overlay state emit failed");
            }
            if let Err(error) = self.window.hide() {
                tracing::warn!(%error, "overlay hide failed");
            }
            return;
        }

        let (width, height) = dimensions_for_phase(&state.phase);
        if let Err(error) = self.window.set_size(LogicalSize::new(width, height)) {
            tracing::warn!(%error, "overlay set_size failed");
        }

        match position_overlay(&self.window, width, height) {
            Ok(position) => {
                if let Err(error) = self.window.set_position(position) {
                    tracing::warn!(%error, "overlay set_position failed");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "overlay positioning failed");
            }
        }

        if let Err(error) = self.window.emit_state(&state) {
            tracing::warn!(%error, "overlay state emit failed");
        }

        if let Err(error) = self.window.prepare_for_show() {
            tracing::warn!(%error, "overlay prepare_for_show failed");
        }

        if let Err(error) = self.window.show() {
            tracing::warn!(%error, "overlay show failed");
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

fn position_overlay<W: OverlayWindow>(
    window: &W,
    width: f64,
    height: f64,
) -> Result<PhysicalPosition<i32>, OverlayError> {
    let (monitor_x, monitor_y, monitor_width, monitor_height) =
        window.primary_monitor_geometry()?;
    let scale = window.scale_factor()?;
    let physical_width = (width * scale).round() as i32;
    let physical_height = (height * scale).round() as i32;
    let offset_physical = (BOTTOM_OFFSET * scale).round() as i32;

    let x = monitor_x + (monitor_width - physical_width) / 2;
    let y = monitor_y + monitor_height - physical_height - offset_physical;

    tracing::debug!(
        "overlay pos: primary=({},{},{},{}) pos=({},{}) scale={}",
        monitor_x,
        monitor_y,
        monitor_width,
        monitor_height,
        x,
        y,
        scale
    );

    Ok(PhysicalPosition::new(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct RecordingOverlayWindow {
        calls: Arc<Mutex<Vec<String>>>,
        monitor: Result<(i32, i32, i32, i32), String>,
        scale: f64,
        prepare_fails: bool,
    }

    impl RecordingOverlayWindow {
        fn new(monitor: (i32, i32, i32, i32), scale: f64) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                monitor: Ok(monitor),
                scale,
                prepare_fails: false,
            }
        }

        fn with_monitor_error(scale: f64) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                monitor: Err("no monitor".into()),
                scale,
                prepare_fails: false,
            }
        }

        fn with_prepare_error(monitor: (i32, i32, i32, i32), scale: f64) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                monitor: Ok(monitor),
                scale,
                prepare_fails: true,
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn record(&self, call: impl Into<String>) {
            self.calls.lock().unwrap().push(call.into());
        }
    }

    impl OverlayWindow for RecordingOverlayWindow {
        fn emit_state(&self, state: &OverlayState) -> Result<(), OverlayError> {
            self.record(format!("emit:{}", state.phase));
            Ok(())
        }

        fn set_size(&self, size: LogicalSize<f64>) -> Result<(), OverlayError> {
            self.record(format!("size:{:.0}x{:.0}", size.width, size.height));
            Ok(())
        }

        fn set_position(&self, position: PhysicalPosition<i32>) -> Result<(), OverlayError> {
            self.record(format!("position:{},{}", position.x, position.y));
            Ok(())
        }

        fn prepare_for_show(&self) -> Result<(), OverlayError> {
            self.record("prepare_for_show");
            if self.prepare_fails {
                return Err(OverlayError::PrepareForShowFailed("forced".into()));
            }
            Ok(())
        }

        fn show(&self) -> Result<(), OverlayError> {
            self.record("show");
            Ok(())
        }

        fn hide(&self) -> Result<(), OverlayError> {
            self.record("hide");
            Ok(())
        }

        fn scale_factor(&self) -> Result<f64, OverlayError> {
            self.record("scale_factor");
            Ok(self.scale)
        }

        fn primary_monitor_geometry(&self) -> Result<(i32, i32, i32, i32), OverlayError> {
            self.record("primary_monitor_geometry");
            self.monitor
                .clone()
                .map_err(OverlayError::PrimaryMonitorFailed)
        }
    }

    #[test]
    fn test_dimensions_for_phase() {
        assert_eq!(dimensions_for_phase("recording"), SIZE_RECORDING);
        assert_eq!(dimensions_for_phase("processing"), SIZE_PROCESSING);
        assert_eq!(dimensions_for_phase("done"), SIZE_DONE);
        assert_eq!(dimensions_for_phase("unknown"), SIZE_RECORDING);
    }

    #[test]
    fn test_visible_state_calls_window_in_order() {
        let window = RecordingOverlayWindow::new((0, 0, 1920, 1080), 1.0);
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::recording());

        assert_eq!(
            window.calls(),
            vec![
                "size:200x48",
                "primary_monitor_geometry",
                "scale_factor",
                "position:860,952",
                "emit:recording",
                "prepare_for_show",
                "show",
            ]
        );
    }

    #[test]
    fn test_hidden_state_emits_then_hides_without_geometry() {
        let window = RecordingOverlayWindow::new((0, 0, 1920, 1080), 1.0);
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::hidden());

        assert_eq!(window.calls(), vec!["emit:hidden", "hide"]);
    }

    #[test]
    fn test_visible_state_shows_even_when_positioning_fails() {
        let window = RecordingOverlayWindow::with_monitor_error(1.0);
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::recording());

        assert_eq!(
            window.calls(),
            vec![
                "size:200x48",
                "primary_monitor_geometry",
                "emit:recording",
                "prepare_for_show",
                "show",
            ]
        );
    }

    #[test]
    fn test_visible_state_shows_and_emits_when_prepare_fails() {
        let window = RecordingOverlayWindow::with_prepare_error((0, 0, 1920, 1080), 1.0);
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::recording());

        let calls = window.calls();
        assert!(calls.contains(&"prepare_for_show".to_string()));
        assert!(calls.contains(&"show".to_string()));
        assert!(calls.contains(&"emit:recording".to_string()));
        let show_idx = calls.iter().position(|c| c == "show").unwrap();
        let emit_idx = calls.iter().position(|c| c == "emit:recording").unwrap();
        assert!(emit_idx < show_idx);
    }

    #[test]
    fn test_position_overlay_applies_scale_factor() {
        let window = RecordingOverlayWindow::new((100, 50, 3840, 2160), 2.0);
        let position = position_overlay(&window, 200.0, 48.0).unwrap();

        assert_eq!(position, PhysicalPosition::new(1820, 1954));
    }
}
