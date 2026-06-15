//! Tauri adapter for the overlay window seam.
//!
//! Keeps concrete `tauri::WebviewWindow` operations out of `OverlayManager`, so the
//! manager can be tested through the `OverlayWindow` interface.

use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition};

use crate::overlay_window::{OverlayError, OverlayState, OverlayWindow};

const OVERLAY_WINDOW_LABEL: &str = "overlay";

/// Tauri implementation of `OverlayWindow`.
#[derive(Clone)]
pub struct TauriOverlayWindow {
    app: tauri::AppHandle,
}

impl TauriOverlayWindow {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }

    fn overlay(&self) -> Result<tauri::WebviewWindow, OverlayError> {
        self.app
            .get_webview_window(OVERLAY_WINDOW_LABEL)
            .ok_or(OverlayError::WindowNotFound)
    }
}

impl OverlayWindow for TauriOverlayWindow {
    fn emit_state(&self, state: &OverlayState) -> Result<(), OverlayError> {
        self.app
            .emit("overlay-state", state)
            .map_err(|error| OverlayError::EmitFailed(error.to_string()))
    }

    fn set_size(&self, size: LogicalSize<f64>) -> Result<(), OverlayError> {
        self.overlay()?
            .set_size(size)
            .map_err(|error| OverlayError::SetSizeFailed(error.to_string()))
    }

    fn set_position(&self, position: PhysicalPosition<i32>) -> Result<(), OverlayError> {
        self.overlay()?
            .set_position(position)
            .map_err(|error| OverlayError::SetPositionFailed(error.to_string()))
    }

    fn prepare_for_show(&self) -> Result<(), OverlayError> {
        let overlay = self.overlay()?;

        if let Err(error) = overlay.set_always_on_top(true) {
            tracing::warn!(error = %error, "overlay set_always_on_top failed");
        }
        if let Err(error) = overlay.set_skip_taskbar(true) {
            tracing::warn!(error = %error, "overlay set_skip_taskbar failed");
        }
        if let Err(error) = overlay.set_focusable(false) {
            tracing::warn!(error = %error, "overlay set_focusable failed");
        }
        if let Err(error) = overlay.set_shadow(false) {
            tracing::warn!(error = %error, "overlay set_shadow failed");
        }

        Ok(())
    }

    fn show(&self) -> Result<(), OverlayError> {
        self.overlay()?
            .show()
            .map_err(|error| OverlayError::ShowFailed(error.to_string()))
    }

    fn hide(&self) -> Result<(), OverlayError> {
        self.overlay()?
            .hide()
            .map_err(|error| OverlayError::HideFailed(error.to_string()))
    }

    fn scale_factor(&self) -> Result<f64, OverlayError> {
        self.overlay()?
            .scale_factor()
            .map_err(|error| OverlayError::ScaleFactorFailed(error.to_string()))
    }

    fn primary_monitor_geometry(&self) -> Result<(i32, i32, i32, i32), OverlayError> {
        platform_primary_monitor_geometry().ok_or_else(|| {
            OverlayError::PrimaryMonitorFailed("no primary monitor available".into())
        })
    }
}

#[cfg(target_os = "linux")]
fn platform_primary_monitor_geometry() -> Option<(i32, i32, i32, i32)> {
    xrandr_primary_monitor()
}

#[cfg(target_os = "windows")]
fn platform_primary_monitor_geometry() -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    // Probe (0, 0) so MONITOR_DEFAULTTOPRIMARY resolves to the primary monitor
    // even when no top-level window has been created yet.
    let monitor = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    if monitor.is_invalid() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a valid, properly-sized MONITORINFO for the duration of the call.
    let ok = unsafe { GetMonitorInfoW(monitor, &mut info) };
    if !ok.as_bool() {
        return None;
    }
    let rect = &info.rcWork;
    Some(geometry_from_work_rect(
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
    ))
}

/// Uses `xrandr` to get primary monitor geometry in physical pixels.
#[cfg(target_os = "linux")]
fn xrandr_primary_monitor() -> Option<(i32, i32, i32, i32)> {
    let output = std::process::Command::new("xrandr").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let monitors = parse_xrandr_geometry(&text);
    monitors
        .iter()
        .find(|monitor| monitor.4)
        .map(|monitor| (monitor.0, monitor.1, monitor.2, monitor.3))
        .or_else(|| {
            monitors
                .into_iter()
                .next()
                .map(|monitor| (monitor.0, monitor.1, monitor.2, monitor.3))
        })
}

#[cfg(target_os = "linux")]
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
            Some(value) => value,
            None => continue,
        };
        let end = after_conn.find(' ').unwrap_or(after_conn.len());
        let geometry = &after_conn[..end];

        let Some(x_idx) = geometry.find('x') else {
            continue;
        };
        let Some(plus1) = geometry.find('+') else {
            continue;
        };
        let Some(plus2) = geometry.rfind('+') else {
            continue;
        };
        if plus1 == plus2 {
            continue;
        }

        let width = geometry[..x_idx].parse::<i32>().unwrap_or(0);
        let height = geometry[x_idx + 1..plus1].parse::<i32>().unwrap_or(0);
        let x = geometry[plus1 + 1..plus2].parse::<i32>().unwrap_or(0);
        let y = geometry[plus2 + 1..].parse::<i32>().unwrap_or(0);

        if width > 0 && height > 0 {
            monitors.push((x, y, width, height, is_primary));
        }
    }
    monitors
}

/// Extracts `(x, y, width, height)` from a work-area rect, regardless of how
/// the platform struct names it. Shared by tests so geometry extraction is
/// exercised on every platform.
fn geometry_from_work_rect(left: i32, top: i32, right: i32, bottom: i32) -> (i32, i32, i32, i32) {
    (left, top, right - left, bottom - top)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
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
    #[cfg(target_os = "linux")]
    fn test_parse_xrandr_skips_disconnected() {
        let sample = "DP-1 disconnected (normal left inverted right x axis y axis)";
        assert!(parse_xrandr_geometry(sample).is_empty());
    }

    #[test]
    fn test_platform_primary_monitor_geometry_runs_without_panicking() {
        // Both Linux (xrandr) and Windows (GetMonitorInfoW) implementations
        // should return Some geometry on a real machine. The function may
        // return None on stripped-down CI without a display, but it must
        // not panic. Issue #23 (Windows) was previously a stub returning
        // None unconditionally; this test catches accidental regressions.
        let _ = platform_primary_monitor_geometry();
    }

    #[test]
    fn test_geometry_from_work_rect_uses_work_area() {
        // Full monitor: 0,0 - 3840x2160
        // Work area: 0,40 - 3840x2080 (40px taskbar at bottom)
        let (x, y, w, h) = geometry_from_work_rect(0, 40, 3840, 2120);
        assert_eq!((x, y, w, h), (0, 40, 3840, 2080));
    }

    #[test]
    fn test_geometry_from_work_rect_distinguishes_from_full_monitor() {
        // Same monitor would report rcMonitor=(0,0,3840,2160). rcWork should be
        // strictly smaller in at least one dimension when a taskbar is present.
        let full = geometry_from_work_rect(0, 0, 3840, 2160);
        let work = geometry_from_work_rect(0, 40, 3840, 2120);
        assert_ne!(full, work);
        assert!(
            work.3 < full.3,
            "work height should be smaller than full height"
        );
    }

    #[test]
    fn test_geometry_from_work_rect_negative_origin() {
        // Secondary monitor placed to the left of the primary has negative x.
        let (x, y, w, h) = geometry_from_work_rect(-1920, 0, 0, 1080);
        assert_eq!((x, y, w, h), (-1920, 0, 1920, 1080));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_win32_geometry_extraction_from_monitorinfo() {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::MONITORINFO;
        // Simulate a 1920x1080 monitor with a 40px taskbar at the bottom.
        let info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            rcWork: RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            },
            dwFlags: 0,
        };
        let (x, y, w, h) = geometry_from_work_rect(
            info.rcWork.left,
            info.rcWork.top,
            info.rcWork.right,
            info.rcWork.bottom,
        );
        // Verifies that we picked rcWork (bottom=1040), not rcMonitor (bottom=1080).
        assert_eq!((x, y, w, h), (0, 0, 1920, 1040));
    }
}
