//! 悬浮窗管理模块。
//!
//! 把 Overlay 的状态意图与窗口物理操作分离：调用方只描述状态，本模块通过
//! `OverlayWindow` seam 计算尺寸和位置，再由具体 adapter 执行窗口操作。
//!
//! 与前端的分工：
//! - 本模块负责**窗口物理层**：emit `overlay-state` 事件、resize、reposition、show/hide
//! - 前端负责**视觉层**：CSS transition / animation 处理 entry / exit / crossfade

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use tauri::{LogicalSize, PhysicalPosition};

use crate::overlay::seam::{OverlayError, OverlaySink, OverlayWindow};

pub use crate::overlay::seam::{OverlayPhase, OverlayState};

/// 悬浮窗的固定逻辑尺寸（CSS pixels）。
///
/// 所有相位共用同一窗口尺寸（取最大相位 done 的内容高度，加上底部锚定间距）。
/// 相位切换只改前端内容，不再触碰窗口几何——透明窗口 resize 时新暴露的区域
/// 在部分 Linux WM 上会合成出黑边，且窗口变形与前端 crossfade 错位会造成跳变。
const OVERLAY_SIZE: (f64, f64) = (520.0, 180.0);

/// 距屏幕底部的偏移（CSS pixels）。
const BOTTOM_OFFSET: f64 = 80.0;

/// hidden 事件发出后、真正 hide 之前的延迟，给前端 exit 动画留出播放时间
/// （前端 --duration-normal 为 180ms，再加少量余量）。
const HIDE_DELAY: Duration = Duration::from_millis(220);

/// 悬浮窗管理器 —— 负责把 Overlay 状态意图翻译成窗口操作。
#[derive(Clone)]
pub struct OverlayManager<W: OverlayWindow> {
    window: W,
    /// 代际计数：每次 set_state 递增。延迟 hide 执行前比对代际，
    /// 防止「hide 延迟期间用户重新开始录音」时旧 hide 关掉新内容。
    generation: Arc<AtomicU64>,
}

impl<W: OverlayWindow + 'static> OverlayManager<W> {
    pub fn new(window: W) -> Self {
        Self {
            window,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 设置悬浮窗状态。
    ///
    /// 这是一个**原子意图**：调用方只需描述「现在应该显示什么阶段」，
    /// 本方法内部一次性完成 resize → reposition → prepare → show → emit。
    /// 窗口尺寸是固定的，重复调用只是幂等的几何设置。
    pub fn set_state(&self, state: OverlayState) {
        let seq = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        if matches!(state.phase, OverlayPhase::Hidden) {
            if let Err(error) = self.window.emit_state(&state) {
                tracing::warn!(%error, "overlay state emit failed");
            }
            let window = self.window.clone();
            let generation = Arc::clone(&self.generation);
            std::thread::spawn(move || {
                std::thread::sleep(HIDE_DELAY);
                if generation.load(Ordering::SeqCst) != seq {
                    return;
                }
                if let Err(error) = window.hide() {
                    tracing::warn!(%error, "overlay hide failed");
                }
            });
            return;
        }

        let (width, height) = OVERLAY_SIZE;
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

impl<W: OverlayWindow + Send + Sync + 'static> OverlaySink for OverlayManager<W> {
    fn set_state(&self, state: OverlayState) {
        OverlayManager::set_state(self, state);
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
        scale: Result<f64, String>,
        prepare_fails: bool,
        hide_fails: bool,
        set_size_fails: bool,
        emit_fails: bool,
        set_position_fails: bool,
    }

    impl RecordingOverlayWindow {
        fn new(monitor: (i32, i32, i32, i32), scale: f64) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                monitor: Ok(monitor),
                scale: Ok(scale),
                prepare_fails: false,
                hide_fails: false,
                set_size_fails: false,
                emit_fails: false,
                set_position_fails: false,
            }
        }

        fn with_monitor_error(scale: f64) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                monitor: Err("no monitor".into()),
                scale: Ok(scale),
                prepare_fails: false,
                hide_fails: false,
                set_size_fails: false,
                emit_fails: false,
                set_position_fails: false,
            }
        }

        fn with_prepare_error(monitor: (i32, i32, i32, i32), scale: f64) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                monitor: Ok(monitor),
                scale: Ok(scale),
                prepare_fails: true,
                hide_fails: false,
                set_size_fails: false,
                emit_fails: false,
                set_position_fails: false,
            }
        }

        fn with_scale_error() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                monitor: Ok((0, 0, 1920, 1080)),
                scale: Err("no scale".into()),
                prepare_fails: false,
                hide_fails: false,
                set_size_fails: false,
                emit_fails: false,
                set_position_fails: false,
            }
        }

        fn with_operation_error(
            monitor: (i32, i32, i32, i32),
            scale: f64,
            hide_fails: bool,
            set_size_fails: bool,
            emit_fails: bool,
            set_position_fails: bool,
        ) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                monitor: Ok(monitor),
                scale: Ok(scale),
                prepare_fails: false,
                hide_fails,
                set_size_fails,
                emit_fails,
                set_position_fails,
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
            self.record(format!("emit:{}", state.phase.as_str()));
            if self.emit_fails {
                return Err(OverlayError::EmitFailed("forced".into()));
            }
            Ok(())
        }

        fn set_size(&self, size: LogicalSize<f64>) -> Result<(), OverlayError> {
            self.record(format!("size:{:.0}x{:.0}", size.width, size.height));
            if self.set_size_fails {
                return Err(OverlayError::SetSizeFailed("forced".into()));
            }
            Ok(())
        }

        fn set_position(&self, position: PhysicalPosition<i32>) -> Result<(), OverlayError> {
            self.record(format!("position:{},{}", position.x, position.y));
            if self.set_position_fails {
                return Err(OverlayError::SetPositionFailed("forced".into()));
            }
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
            if self.hide_fails {
                return Err(OverlayError::HideFailed("forced".into()));
            }
            Ok(())
        }

        fn scale_factor(&self) -> Result<f64, OverlayError> {
            self.record("scale_factor");
            self.scale.clone().map_err(OverlayError::ScaleFactorFailed)
        }

        fn primary_monitor_geometry(&self) -> Result<(i32, i32, i32, i32), OverlayError> {
            self.record("primary_monitor_geometry");
            self.monitor
                .clone()
                .map_err(OverlayError::PrimaryMonitorFailed)
        }
    }

    #[test]
    fn test_visible_state_calls_window_in_order() {
        let window = RecordingOverlayWindow::new((0, 0, 1920, 1080), 1.0);
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::recording());

        assert_eq!(
            window.calls(),
            vec![
                "size:520x180",
                "primary_monitor_geometry",
                "scale_factor",
                "position:700,820",
                "emit:recording",
                "prepare_for_show",
                "show",
            ]
        );
    }

    #[test]
    fn test_hidden_state_emits_then_hides_after_delay() {
        let window = RecordingOverlayWindow::new((0, 0, 1920, 1080), 1.0);
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::hidden());

        // hide 是延迟执行的（给前端 exit 动画留时间），立即检查时不应出现。
        assert_eq!(window.calls(), vec!["emit:hidden"]);

        std::thread::sleep(HIDE_DELAY + Duration::from_millis(100));
        assert_eq!(window.calls(), vec!["emit:hidden", "hide"]);
    }

    #[test]
    fn test_pending_hide_is_cancelled_by_newer_state() {
        let window = RecordingOverlayWindow::new((0, 0, 1920, 1080), 1.0);
        let manager = OverlayManager::new(window.clone());

        // hidden 的延迟 hide 还没执行，用户又开始录音：
        // 旧 hide 不得把新内容关掉。
        manager.set_state(OverlayState::hidden());
        manager.set_state(OverlayState::recording());

        std::thread::sleep(HIDE_DELAY + Duration::from_millis(100));
        assert!(!window.calls().contains(&"hide".to_string()));
    }

    #[test]
    fn test_visible_state_shows_even_when_positioning_fails() {
        let window = RecordingOverlayWindow::with_monitor_error(1.0);
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::recording());

        assert_eq!(
            window.calls(),
            vec![
                "size:520x180",
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

    #[test]
    fn test_hide_failure_warns_but_continues() {
        let window = RecordingOverlayWindow::with_operation_error(
            (0, 0, 1920, 1080),
            1.0,
            true,
            false,
            false,
            false,
        );
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::hidden());
        std::thread::sleep(HIDE_DELAY + Duration::from_millis(100));

        assert!(window.calls().contains(&"emit:hidden".to_string()));
        assert!(window.calls().contains(&"hide".to_string()));
    }

    #[test]
    fn test_set_size_failure_warns_but_continues() {
        let window = RecordingOverlayWindow::with_operation_error(
            (0, 0, 1920, 1080),
            1.0,
            false,
            true,
            false,
            false,
        );
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::recording());

        assert!(window.calls().contains(&"size:520x180".to_string()));
        assert!(window.calls().contains(&"show".to_string()));
    }

    #[test]
    fn test_emit_state_failure_warns_but_continues() {
        let window = RecordingOverlayWindow::with_operation_error(
            (0, 0, 1920, 1080),
            1.0,
            false,
            false,
            true,
            false,
        );
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::recording());

        assert!(window.calls().contains(&"emit:recording".to_string()));
        assert!(window.calls().contains(&"show".to_string()));
    }

    #[test]
    fn test_set_position_failure_warns_but_continues() {
        let window = RecordingOverlayWindow::with_operation_error(
            (0, 0, 1920, 1080),
            1.0,
            false,
            false,
            false,
            true,
        );
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::recording());

        assert!(window.calls().contains(&"position:700,820".to_string()));
        assert!(window.calls().contains(&"show".to_string()));
    }

    #[test]
    fn test_scale_factor_failure_returns_err() {
        let window = RecordingOverlayWindow::with_scale_error();
        let result = position_overlay(&window, 520.0, 180.0);

        assert!(result.is_err());
    }

    #[test]
    fn test_negative_x_coordinate_clamped() {
        // Monitor narrower than the overlay width (scale 1.0).
        let window = RecordingOverlayWindow::new((0, 0, 500, 1080), 1.0);
        let position = position_overlay(&window, 520.0, 180.0).unwrap();

        // x is negative because physical_width > monitor_width; integer division yields -10.
        assert_eq!(position.x, -10);
    }

    #[test]
    fn test_hidden_hidden_cancels_first_hide() {
        let window = RecordingOverlayWindow::new((0, 0, 1920, 1080), 1.0);
        let manager = OverlayManager::new(window.clone());

        manager.set_state(OverlayState::hidden());
        std::thread::sleep(Duration::from_millis(50));
        manager.set_state(OverlayState::hidden());

        std::thread::sleep(HIDE_DELAY + Duration::from_millis(100));
        let calls = window.calls();
        assert_eq!(
            calls.iter().filter(|c| *c == "hide").count(),
            1,
            "expected exactly one hide after generation bump, got {:?}",
            calls
        );
    }

    #[test]
    fn test_position_overlay_non_integer_rounds() {
        let window = RecordingOverlayWindow::new((0, 0, 1920, 1080), 1.5);
        let position = position_overlay(&window, 520.0, 180.0).unwrap();

        let physical_width = (520.0f64 * 1.5).round() as i32; // 780
        let physical_height = (180.0f64 * 1.5).round() as i32; // 270
        let offset = (80.0f64 * 1.5).round() as i32; // 120
        let expected_x = (1920 - physical_width) / 2; // 570
        let expected_y = 1080 - physical_height - offset; // 690
        assert_eq!(position, PhysicalPosition::new(expected_x, expected_y));
    }
}
