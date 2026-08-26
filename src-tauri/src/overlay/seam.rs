//! 浮窗窗口接缝。
//!
//! 定义 `OverlayWindow` 接口——`OverlayManager` 与窗口系统交互的唯一通道。平台相关的
//! Tauri 调用收进真正的 seam 后面，manager 的行为即可用 fake adapter 测试。
//! Overlay window seam.
//!
//! Defines the `OverlayWindow` interface — the only way `OverlayManager` talks to the
//! window system. This keeps platform-specific Tauri calls behind a real seam and makes
//! the manager's behaviour testable with fake adapters.

use tauri::{LogicalSize, PhysicalPosition};
use thiserror::Error;

/// 悬浮窗状态管理 seam。
///
/// 由 `OverlayManager` 实现，`TauriPipelineSink` 以 `Box<dyn OverlaySink>` 的形式消费，
/// sink 因此不必依赖具体的 manager 或窗口类型。
/// Seam for overlay state management.
///
/// Implemented by `OverlayManager`; consumed by `TauriPipelineSink` as
/// `Box<dyn OverlaySink>` so the sink does not depend on the concrete
/// manager or window types.
pub trait OverlaySink: Send + Sync {
    fn set_state(&self, state: OverlayState);
}

/// 驱动悬浮窗过程中可能出现的错误。
/// Errors that can occur while driving an overlay window.
#[derive(Debug, Error)]
pub enum OverlayError {
    #[error("overlay window not found")]
    WindowNotFound,

    #[error("failed to emit overlay state: {0}")]
    EmitFailed(String),

    #[error("failed to set size: {0}")]
    SetSizeFailed(String),

    #[error("failed to set position: {0}")]
    SetPositionFailed(String),

    #[error("failed to show window: {0}")]
    ShowFailed(String),

    #[error("failed to hide window: {0}")]
    HideFailed(String),

    #[error("failed to read scale factor: {0}")]
    ScaleFactorFailed(String),

    #[error("failed to query primary monitor: {0}")]
    PrimaryMonitorFailed(String),

    #[error("failed to prepare window for show: {0}")]
    PrepareForShowFailed(String),
}

/// Overlay 阶段 —— Rust 内部用枚举流通，仅在序列化给前端时转字符串。
///
/// 序列化为 `"recording"` / `"processing"` / `"done"` / `"hidden"`，
/// 与前端 `overlay-state` 事件协议保持一致。
///
/// Overlay 相位 —— 在 Rust 内部以枚举流通，仅在序列化给前端时转成字符串。
///
/// Serializes as `"recording"` / `"processing"` / `"done"` / `"hidden"`, kept in step with the
/// frontend `overlay-state` event protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayPhase {
    Recording,
    Processing,
    Done,
    Hidden,
}

impl OverlayPhase {
    /// 与前端 `overlay-state` 协议一致的小写名称（与 serde 序列化相同）。
    /// Lowercase name aligned with the frontend `overlay-state` protocol (same as serde serialization).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Processing => "processing",
            Self::Done => "done",
            Self::Hidden => "hidden",
        }
    }
}

/// 从 Rust 发往前端浮窗的视觉状态。
/// Visual state emitted from Rust to the frontend overlay.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    /// 当前相位（序列化为前端协议字符串）。
    /// Current phase (serialized as the frontend protocol string).
    /// Current phase（序列化为前端协议字符串）。
    pub phase: OverlayPhase,
}

impl OverlayState {
    pub fn recording() -> Self {
        Self {
            phase: OverlayPhase::Recording,
        }
    }

    pub fn processing() -> Self {
        Self {
            phase: OverlayPhase::Processing,
        }
    }

    pub fn done() -> Self {
        Self {
            phase: OverlayPhase::Done,
        }
    }

    pub fn hidden() -> Self {
        Self {
            phase: OverlayPhase::Hidden,
        }
    }
}

/// `OverlayManager` 所需的窗口系统操作。
///
/// 所有方法都是同步的：底层 Tauri 窗口 API 本身同步，manager 也不需要异步控制流。
/// Window-system operations required by `OverlayManager`.
///
/// All methods are synchronous because the underlying Tauri window APIs are
/// synchronous, and the manager does not need async control flow.
pub trait OverlayWindow: Send + Sync + Clone {
    /// 经 Tauri 事件（或等价通道）把视觉状态发给前端。
    /// Emit the visual state to the frontend via Tauri event (or equivalent).
    fn emit_state(&self, state: &OverlayState) -> Result<(), OverlayError>;

    /// 把浮窗调整为指定的逻辑尺寸。
    /// Resize the overlay window to the requested logical size.
    fn set_size(&self, size: LogicalSize<f64>) -> Result<(), OverlayError>;

    /// 把浮窗移动到指定的物理位置。
    /// Move the overlay window to the requested physical position.
    fn set_position(&self, position: PhysicalPosition<i32>) -> Result<(), OverlayError>;

    /// 在浮窗显示前准备原生窗口标志。
    /// Prepare native window flags before the overlay is shown.
    fn prepare_for_show(&self) -> Result<(), OverlayError>;

    /// 显示浮窗。
    /// Show the overlay window.
    fn show(&self) -> Result<(), OverlayError>;

    /// 隐藏浮窗。
    /// Hide the overlay window.
    fn hide(&self) -> Result<(), OverlayError>;

    /// 返回浮窗所在显示器的缩放系数。
    /// Return the monitor scale factor for the overlay window.
    fn scale_factor(&self) -> Result<f64, OverlayError>;

    /// 以 `(x, y, width, height)` 物理像素形式返回主显示器几何信息。
    /// Return the primary monitor geometry as `(x, y, width, height)` physical pixels.
    fn primary_monitor_geometry(&self) -> Result<(i32, i32, i32, i32), OverlayError>;
}

/// 悬浮窗在屏幕上的预设位置。
/// Preset on-screen position of the overlay window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlayPosition {
    /// 底部居中（默认）。
    /// Bottom center (default).
    #[default]
    BottomCenter,
    /// 顶部居中。
    /// Top center.
    TopCenter,
}

impl OverlayPosition {
    /// 从配置字符串解析；未知值回退到底部居中，保证旧配置与手误值可用。
    /// Parses a value from the config string; unknown values fall back to bottom center so
    /// legacy configs and typos keep working.
    pub fn effective(value: &str) -> Self {
        match value {
            "top_center" => Self::TopCenter,
            _ => Self::BottomCenter,
        }
    }
}

#[cfg(test)]
mod position_tests {
    use super::OverlayPosition;

    #[test]
    fn effective_parses_known_values() {
        assert_eq!(
            OverlayPosition::effective("bottom_center"),
            OverlayPosition::BottomCenter
        );
        assert_eq!(
            OverlayPosition::effective("top_center"),
            OverlayPosition::TopCenter
        );
    }

    #[test]
    fn effective_falls_back_to_bottom_center_for_unknown_values() {
        assert_eq!(
            OverlayPosition::effective(""),
            OverlayPosition::BottomCenter
        );
        assert_eq!(
            OverlayPosition::effective("left"),
            OverlayPosition::BottomCenter
        );
    }
}
