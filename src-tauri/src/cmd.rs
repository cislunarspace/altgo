//! Tauri commands — 前端通过 IPC 调用的函数。

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State};

use crate::{
    config,
    config_store::{ConfigPatch, ConfigStore},
    history,
    history::HistoryStore,
    output, pipeline,
    pipeline_controller::{PipelineController, PipelineStatus},
    pipeline_sink::PipelineSink,
    polisher,
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

    // Prefer the explicitly-marked primary monitor.
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

/// Tauri 管道事件接收器 — 将管道事件转发为 Tauri 事件和系统操作。
struct TauriPipelineSink {
    app: tauri::AppHandle,
    pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
    event_handler: crate::pipeline_event_handler::PipelineEventHandler,
}

impl TauriPipelineSink {
    fn new(
        app: tauri::AppHandle,
        pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
        cfg: Arc<config::Config>,
    ) -> Self {
        let event_handler =
            crate::pipeline_event_handler::PipelineEventHandler::new(cfg.output.prefer_polished);
        Self {
            app,
            pipeline_status,
            event_handler,
        }
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

    fn on_transcription_result(&self, output: &pipeline::PipelineOutput) {
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

/// Spawn a new pipeline OS thread. Returns the stop handle.
/// Exposed as `pub(crate)` so `lib.rs` and `PipelineController` callers can use it.
pub(crate) fn spawn_pipeline_thread(
    app: &AppHandle,
    cfg: Arc<config::Config>,
    pipeline_status: Arc<std::sync::RwLock<PipelineStatus>>,
) -> crate::PipelineHandle {
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let app_handle = app.clone();
    let cfg_clone = cfg.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        let sink = TauriPipelineSink::new(app_handle, pipeline_status, cfg_clone);
        rt.block_on(crate::pipeline_orchestrator::run(cfg, stop_rx, sink));
    });
    crate::PipelineHandle { stop_tx }
}

async fn restart_pipeline(
    app: AppHandle,
    controller: &PipelineController,
    config_store: &ConfigStore,
) -> Result<(), String> {
    let cfg = Arc::new(config_store.snapshot().await);
    cfg.validate().map_err(|e| e.to_string())?;
    let status_arc = controller.status_arc();
    controller
        .restart_with(|| spawn_pipeline_thread(&app, cfg, status_arc))
        .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub key_name: String,
    pub linux_evdev_code: Option<u16>,
    pub language: String,
    pub engine: String,
    pub model: String,
    pub api_base_url: String,
    pub polish_level: String,
    pub polish_model: String,
    pub polish_api_base_url: String,
    pub gui_language: String,
    pub has_transcriber_api_key: bool,
    pub has_polisher_api_key: bool,
}

#[tauri::command]
pub async fn get_config(config_store: State<'_, ConfigStore>) -> Result<ConfigResponse, String> {
    let cfg = config_store.config.lock().await;
    Ok(ConfigResponse {
        key_name: cfg.key_listener.key_name.clone(),
        linux_evdev_code: cfg.key_listener.linux_evdev_code,
        language: cfg.transcriber.language.clone(),
        engine: cfg.transcriber.engine.clone(),
        model: cfg.transcriber.model.clone(),
        api_base_url: cfg.transcriber.api_base_url.clone(),
        polish_level: cfg.polisher.level.clone(),
        polish_model: cfg.polisher.model.clone(),
        polish_api_base_url: cfg.polisher.api_base_url.clone(),
        gui_language: cfg.gui.language.clone(),
        has_transcriber_api_key: !cfg.transcriber.api_key.trim().is_empty(),
        has_polisher_api_key: !cfg.polisher.api_key.trim().is_empty(),
    })
}

#[tauri::command]
pub async fn save_config(
    app: AppHandle,
    config_store: State<'_, ConfigStore>,
    controller: State<'_, PipelineController>,
    patch: ConfigPatch,
) -> Result<(), String> {
    config_store.apply_patch(patch).await?;
    restart_pipeline(app, &controller, &config_store).await
}

#[tauri::command]
pub async fn capture_activation_key(
    controller: State<'_, PipelineController>,
) -> Result<crate::key_capture::CaptureActivationResponse, String> {
    controller.stop().await;
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    match tokio::task::spawn_blocking(crate::key_capture::capture_activation_key_blocking).await {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn start_pipeline(
    app: tauri::AppHandle,
    config_store: State<'_, ConfigStore>,
    controller: State<'_, PipelineController>,
) -> Result<(), String> {
    let cfg = Arc::new(config_store.snapshot().await);
    let status_arc = controller.status_arc();
    controller
        .start_with(|| spawn_pipeline_thread(&app, cfg, status_arc))
        .await
}

#[tauri::command]
pub async fn stop_pipeline(controller: State<'_, PipelineController>) -> Result<(), String> {
    controller.stop().await;
    Ok(())
}

#[tauri::command]
pub async fn get_status(
    controller: State<'_, PipelineController>,
) -> Result<PipelineStatus, String> {
    Ok(controller.current_status())
}

#[tauri::command]
pub async fn copy_text(text: String) -> Result<(), String> {
    output::write_clipboard(&text)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub name: String,
    pub filename: String,
    pub size_bytes: u64,
    pub description: String,
    pub downloaded: bool,
}

#[tauri::command]
pub async fn list_models() -> Result<Vec<ModelEntry>, String> {
    let downloaded = crate::model::list_downloaded();
    let entries = crate::model::models_info()
        .iter()
        .map(|m| ModelEntry {
            name: m.name.to_string(),
            filename: m.filename.to_string(),
            size_bytes: m.size_bytes,
            description: m.description.to_string(),
            downloaded: downloaded.iter().any(|d| d == m.name),
        })
        .collect();
    Ok(entries)
}

#[tauri::command]
pub async fn download_model(app: tauri::AppHandle, name: String) -> Result<(), String> {
    if crate::model::models_info()
        .iter()
        .all(|m| m.name != name.as_str())
    {
        return Err(format!("unknown model: {}", name));
    }

    if let Some(info) = crate::model::models_info()
        .iter()
        .find(|m| m.name == name.as_str())
    {
        let _ = app.emit(
            "model-download-progress",
            serde_json::json!({
                "name": name,
                "downloaded": 0_u64,
                "total": info.size_bytes,
            }),
        );
    }

    let app_task = app.clone();
    let name_task = name.clone();
    tauri::async_runtime::spawn(async move {
        let result = crate::model::download_with_progress(&name_task, {
            let app = app_task.clone();
            let name_clone = name_task.clone();
            move |downloaded, total| {
                let _ = app.emit(
                    "model-download-progress",
                    serde_json::json!({
                        "name": name_clone,
                        "downloaded": downloaded,
                        "total": total,
                    }),
                );
            }
        })
        .await;

        match result {
            Ok(path) => {
                let _ = app_task.emit(
                    "model-download-finished",
                    serde_json::json!({
                        "name": name_task,
                        "success": true,
                        "path": path.to_string_lossy(),
                    }),
                );
            }
            Err(e) => {
                let _ = app_task.emit(
                    "model-download-finished",
                    serde_json::json!({
                        "name": name_task,
                        "success": false,
                        "error": e.to_string(),
                    }),
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn delete_model(name: String) -> Result<(), String> {
    let info = crate::model::models_info()
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| format!("unknown model: {}", name))?;
    let path = crate::model::models_dir().join(info.filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn resolve_model(model: String) -> Result<Option<String>, String> {
    Ok(crate::model::resolve_model_path(&model).map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn list_history(
    history_store: State<'_, HistoryStore>,
) -> Result<Vec<history::HistoryEntry>, String> {
    let store = history_store.inner().clone();
    tokio::task::spawn_blocking(move || store.list())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_history_entries(
    history_store: State<'_, HistoryStore>,
    ids: Vec<String>,
) -> Result<usize, String> {
    let store = history_store.inner().clone();
    tokio::task::spawn_blocking(move || store.delete(&ids))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(history_store: State<'_, HistoryStore>) -> Result<(), String> {
    let store = history_store.inner().clone();
    tokio::task::spawn_blocking(move || store.clear())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn polish_history_entry(
    app: AppHandle,
    config_store: State<'_, ConfigStore>,
    history_store: State<'_, HistoryStore>,
    id: String,
) -> Result<history::HistoryEntry, String> {
    let cfg = config_store.snapshot().await;
    let store = history_store.inner().clone();
    let store2 = store.clone();

    let entry = tokio::task::spawn_blocking({
        let id = id.clone();
        move || store.get(&id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let entry = entry.ok_or_else(|| "history entry not found".to_string())?;

    let polish_level = polisher::PolishLevel::effective(&cfg.polisher.level);
    let formatter = polisher::LLMFormatter::try_from(&cfg).map_err(|e| e.to_string())?;

    let polished = formatter
        .polish(&entry.raw_text, polish_level)
        .await
        .map_err(|e| e.to_string())?;

    let out = tokio::task::spawn_blocking(move || store2.update_text(&id, polished))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    let _ = app.emit("history-updated", ());
    Ok(out)
}
