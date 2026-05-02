//! Tauri 管道事件接收器实现。
//!
//! 将管道事件转发为 Tauri 事件、剪贴板操作和浮窗管理。

use std::sync::Arc;
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition};

use crate::{
    config, history::HistoryStore, pipeline::PipelineOutput, pipeline_controller::PipelineStatus,
    pipeline_event_handler::PipelineEventHandler, pipeline_sink::PipelineSink,
};

const OVERLAY_RECORDING_W: f64 = 200.0;
const OVERLAY_RECORDING_H: f64 = 48.0;
const OVERLAY_PROCESSING_W: f64 = 268.0;
const OVERLAY_PROCESSING_H: f64 = 56.0;
const OVERLAY_RESULT_W: f64 = 520.0;
const OVERLAY_RESULT_H: f64 = 100.0;
const OVERLAY_BOTTOM_OFFSET: f64 = 80.0;

/// Get the primary (or first) monitor geometry from `xrandr` in X11
/// device coordinates. Returns (x, y, width, height).
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

fn position_overlay_linux(
    overlay: &tauri::WebviewWindow,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let _ = overlay.set_size(LogicalSize::new(width, height));

    let Some((mx, my, mw, mh)) = xrandr_primary_monitor() else {
        return Err("xrandr returned no primary monitor".into());
    };

    let scale = overlay.scale_factor().map_err(|e| e.to_string())?;
    let phys_w = (width * scale).round() as i32;
    let phys_h = (height * scale).round() as i32;
    let offset_phys = (OVERLAY_BOTTOM_OFFSET * scale).round() as i32;

    let x = mx + (mw - phys_w) / 2;
    let y = my + mh - phys_h - offset_phys;

    tracing::debug!(
        "linux overlay pos: primary=({},{},{},{}) pos=({},{}) scale={}",
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

fn position_overlay(overlay: &tauri::WebviewWindow, width: f64, height: f64) {
    if let Err(e) = position_overlay_linux(overlay, width, height) {
        tracing::warn!("overlay positioning failed: {}", e);
    }
}

fn emit_pipeline_status(
    app: &tauri::AppHandle,
    status: &Arc<std::sync::RwLock<PipelineStatus>>,
    value: PipelineStatus,
) {
    let _ = app.emit("pipeline-status", value.as_str());
    if let Ok(mut s) = status.write() {
        *s = value;
    }
}

/// Tauri 管道事件接收器 — 将管道事件转发为 Tauri 事件和系统操作。
pub struct TauriPipelineSink {
    app: tauri::AppHandle,
    pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
    event_handler: PipelineEventHandler,
}

impl TauriPipelineSink {
    pub fn new(
        app: tauri::AppHandle,
        pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
        cfg: Arc<config::Config>,
    ) -> Self {
        let event_handler = PipelineEventHandler::new(cfg.output.prefer_polished);
        Self {
            app,
            pipeline_status,
            event_handler,
        }
    }
}

impl PipelineSink for TauriPipelineSink {
    fn on_status_change(&self, status: &str) {
        let ps = match status {
            "recording" => PipelineStatus::Recording,
            "processing" => PipelineStatus::Processing,
            "done" => PipelineStatus::Done,
            _ => PipelineStatus::Idle,
        };
        emit_pipeline_status(&self.app, &self.pipeline_status, ps);
        if let Some(overlay) = self.app.get_webview_window("overlay") {
            match status {
                "recording" => {
                    position_overlay(&overlay, OVERLAY_RECORDING_W, OVERLAY_RECORDING_H);
                    let _ = overlay.show();
                }
                "processing" => {
                    let _ = overlay.hide();
                    position_overlay(&overlay, OVERLAY_PROCESSING_W, OVERLAY_PROCESSING_H);
                    let _ = overlay.show();
                }
                "idle" | "stopped" => {
                    let _ = overlay.hide();
                }
                _ => {}
            }
        }
    }

    fn on_error(&self, message: &str) {
        let _ = self.app.emit("pipeline-error", message);
    }

    fn on_transcription_result(&self, output: &PipelineOutput) {
        if output.raw_text.is_empty() {
            emit_pipeline_status(&self.app, &self.pipeline_status, PipelineStatus::Idle);
            return;
        }

        let app = self.app.clone();
        let status = self.pipeline_status.clone();
        let output_clone = output.clone();
        let event_handler = self.event_handler.clone();

        tauri::async_runtime::spawn(async move {
            let history_store = app.state::<HistoryStore>().inner().clone();
            let result = event_handler
                .handle_transcription(&output_clone, &history_store)
                .await;

            match result {
                Some(res) => {
                    if res.history_appended {
                        let _ = app.emit("history-updated", ());
                    }

                    emit_pipeline_status(&app, &status, PipelineStatus::Done);
                    if let Some(overlay) = app.get_webview_window("overlay") {
                        position_overlay(&overlay, OVERLAY_RESULT_W, OVERLAY_RESULT_H);
                        let _ = overlay.show();
                    }
                    let _ = app.emit("transcription-result", &res.clipboard_text);
                }
                None => {
                    emit_pipeline_status(&app, &status, PipelineStatus::Idle);
                }
            }
        });
    }

    fn on_progress(&self, phase: &str, fraction: Option<f32>) {
        let _ = self.app.emit(
            "transcription-progress",
            serde_json::json!({ "phase": phase, "fraction": fraction }),
        );
    }

    fn on_key_listener_backend(&self, backend: &str) {
        let _ = self.app.emit("key-listener-backend", backend);
    }
}
