//! Overlay window seam.
//!
//! Defines the `OverlayWindow` interface — the only way `OverlayManager` talks to the
//! window system. This keeps platform-specific Tauri calls behind a real seam and makes
//! the manager's behaviour testable with fake adapters.

use tauri::{LogicalSize, PhysicalPosition};
use thiserror::Error;

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

/// Visual state emitted from Rust to the frontend overlay.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    /// Current phase: `"recording"` / `"processing"` / `"done"` / `"hidden"`
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

/// Window-system operations required by `OverlayManager`.
///
/// All methods are synchronous because the underlying Tauri window APIs are
/// synchronous, and the manager does not need async control flow.
pub trait OverlayWindow: Send + Sync + Clone {
    /// Emit the visual state to the frontend via Tauri event (or equivalent).
    fn emit_state(&self, state: &OverlayState) -> Result<(), OverlayError>;

    /// Resize the overlay window to the requested logical size.
    fn set_size(&self, size: LogicalSize<f64>) -> Result<(), OverlayError>;

    /// Move the overlay window to the requested physical position.
    fn set_position(&self, position: PhysicalPosition<i32>) -> Result<(), OverlayError>;

    /// Prepare native window flags before the overlay is shown.
    fn prepare_for_show(&self) -> Result<(), OverlayError>;

    /// Show the overlay window.
    fn show(&self) -> Result<(), OverlayError>;

    /// Hide the overlay window.
    fn hide(&self) -> Result<(), OverlayError>;

    /// Return the monitor scale factor for the overlay window.
    fn scale_factor(&self) -> Result<f64, OverlayError>;

    /// Return the primary monitor geometry as `(x, y, width, height)` physical pixels.
    fn primary_monitor_geometry(&self) -> Result<(i32, i32, i32, i32), OverlayError>;
}

/// A no-op / null `OverlayWindow` useful in tests and headless contexts.
#[derive(Clone, Default)]
pub struct NoopOverlayWindow;

impl OverlayWindow for NoopOverlayWindow {
    fn emit_state(&self, _state: &OverlayState) -> Result<(), OverlayError> {
        Ok(())
    }

    fn set_size(&self, _size: LogicalSize<f64>) -> Result<(), OverlayError> {
        Ok(())
    }

    fn set_position(&self, _position: PhysicalPosition<i32>) -> Result<(), OverlayError> {
        Ok(())
    }

    fn prepare_for_show(&self) -> Result<(), OverlayError> {
        Ok(())
    }

    fn show(&self) -> Result<(), OverlayError> {
        Ok(())
    }

    fn hide(&self) -> Result<(), OverlayError> {
        Ok(())
    }

    fn scale_factor(&self) -> Result<f64, OverlayError> {
        Ok(1.0)
    }

    fn primary_monitor_geometry(&self) -> Result<(i32, i32, i32, i32), OverlayError> {
        Ok((0, 0, 1920, 1080))
    }
}
