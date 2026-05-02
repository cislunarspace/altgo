//! Tauri commands — 前端通过 IPC 调用的函数。

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State};

use crate::{config, history, output, pipeline, pipeline_sink::PipelineSink, polisher, AppState};

const OVERLAY_RECORDING_W: f64 = 200.0;
const OVERLAY_RECORDING_H: f64 = 48.0;
const OVERLAY_PROCESSING_W: f64 = 268.0;
const OVERLAY_PROCESSING_H: f64 = 56.0;
const OVERLAY_RESULT_W: f64 = 520.0;
const OVERLAY_RESULT_H: f64 = 100.0;
const OVERLAY_BOTTOM_OFFSET: f64 = 80.0;

fn apply_nested_opt_u16(target: &mut Option<u16>, patch: Option<Option<u16>>) {
    match patch {
        None => {}
        Some(None) => *target = None,
        Some(Some(v)) => *target = Some(v),
    }
}

/// 区分 JSON 中「字段缺失」与「`null`」：前者不修改配置，后者清除已保存的 evdev 键码。
fn deserialize_opt_patch_u16<'de, D>(deserializer: D) -> Result<Option<Option<u16>>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(Some(None)),
        serde_json::Value::Number(n) => {
            let u = n
                .as_u64()
                .ok_or_else(|| serde::de::Error::custom("linux_evdev_code: expected number"))?;
            if u > u16::MAX as u64 {
                return Err(serde::de::Error::custom("linux_evdev_code out of range"));
            }
            Ok(Some(Some(u as u16)))
        }
        _ => Err(serde::de::Error::custom(
            "linux_evdev_code: expected null or number",
        )),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Idle,
    Recording,
    Processing,
    Done,
}

/// Root-window mouse coordinates from `xdotool` (physical pixels).
#[allow(dead_code)]
fn mouse_position_physical() -> Option<(i32, i32)> {
    let mouse_out = std::process::Command::new("xdotool")
        .args(["getmouselocation", "--shell"])
        .output()
        .ok()?;

    if !mouse_out.status.success() {
        return None;
    }

    let mouse_str = String::from_utf8_lossy(&mouse_out.stdout);
    let mut mouse_x: i32 = 0;
    let mut mouse_y: i32 = 0;

    for line in mouse_str.lines() {
        if let Some(val) = line.strip_prefix("X=") {
            mouse_x = val.parse().ok()?;
        } else if let Some(val) = line.strip_prefix("Y=") {
            mouse_y = val.parse().ok()?;
        }
    }

    tracing::debug!("mouse position (physical): ({}, {})", mouse_x, mouse_y);
    Some((mouse_x, mouse_y))
}

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
        // Match lines like:
        //   DP-1 connected primary 1920x1080+0+0 ...
        //   HDMI-1 connected 1920x1080+1920+0 ...
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
        // Find the geometry string "WxH+X+Y"
        let end = after_conn.find(' ').unwrap_or(after_conn.len());
        let geo = &after_conn[..end];

        // Parse WxH+X+Y
        let Some(x_idx) = geo.find('x') else { continue };
        let Some(plus1) = geo.find('+') else { continue };
        let Some(plus2) = geo.rfind('+') else {
            continue;
        };
        if plus1 == plus2 {
            continue; // only one '+'
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

/// Position overlay on the **primary** monitor only.
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
    pipeline_status: Arc<std::sync::RwLock<String>>,
    cfg: Arc<config::Config>,
}

impl TauriPipelineSink {
    fn new(
        app: tauri::AppHandle,
        pipeline_status: Arc<std::sync::RwLock<String>>,
        cfg: Arc<config::Config>,
    ) -> Self {
        Self {
            app,
            pipeline_status,
            cfg,
        }
    }
}

/// 发送管道状态事件并更新共享状态。
fn emit_pipeline_status(
    app: &tauri::AppHandle,
    status: &Arc<std::sync::RwLock<String>>,
    value: &str,
) {
    let _ = app.emit("pipeline-status", value);
    if let Ok(mut s) = status.write() {
        *s = value.to_string();
    }
}

impl PipelineSink for TauriPipelineSink {
    fn on_status_change(&self, status: &str) {
        emit_pipeline_status(&self.app, &self.pipeline_status, status);
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
            emit_pipeline_status(&self.app, &self.pipeline_status, "idle");
            return;
        }

        let prefer_polished = self.cfg.output.prefer_polished;
        let app = self.app.clone();
        let status = self.pipeline_status.clone();
        let raw_text = output.raw_text.clone();
        let polish_failed = output.polish_failed;
        let text = output.text.clone();

        tauri::async_runtime::spawn(async move {
            let text_to_use = if prefer_polished && !polish_failed && !text.trim().is_empty() {
                text.clone()
            } else {
                raw_text.clone()
            };

            let display_text = text_to_use.clone();

            // 写入剪贴板
            let _ = output::write_clipboard(&display_text).await;

            // 写入历史
            let hist_path = app.state::<AppState>().history_path.clone();
            match tokio::task::spawn_blocking(move || {
                history::append_entry(&hist_path, raw_text, display_text)
            })
            .await
            {
                Ok(Ok(_)) => {
                    let _ = app.emit("history-updated", ());
                }
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "failed to append transcription history");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "history append task failed");
                }
            }

            // 更新状态和显示 overlay
            emit_pipeline_status(&app, &status, "done");
            if let Some(overlay) = app.get_webview_window("overlay") {
                position_overlay(&overlay, OVERLAY_RESULT_W, OVERLAY_RESULT_H);
                let _ = overlay.show();
            }
            let _ = app.emit("transcription-result", &text_to_use);
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

/// 在专用线程上启动管道，返回停止句柄。
fn spawn_pipeline_thread(
    app: &AppHandle,
    cfg: Arc<config::Config>,
    pipeline_status: Arc<std::sync::RwLock<String>>,
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
pub async fn get_config(state: State<'_, AppState>) -> Result<ConfigResponse, String> {
    let cfg = state.config.lock().await;
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConfigRequest {
    pub key_name: Option<String>,
    /// `None` = 请求中未带此字段（不修改）；`Some(None)` = JSON `null`（清除）；`Some(Some)` = 设置键码。
    #[serde(default, deserialize_with = "deserialize_opt_patch_u16")]
    pub linux_evdev_code: Option<Option<u16>>,
    pub language: Option<String>,
    pub engine: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub polish_level: Option<String>,
    pub polish_model: Option<String>,
    pub polish_api_key: Option<String>,
    pub polish_api_base_url: Option<String>,
    pub gui_language: Option<String>,
}

/// Start the main voice pipeline thread (used at app startup).
pub fn spawn_pipeline_at_startup(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let cfg = Arc::new(state.config.blocking_lock().clone());
    cfg.validate().map_err(|e| e.to_string())?;
    let handle = spawn_pipeline_thread(&app, cfg, state.pipeline_status.clone());
    *state.pipeline.blocking_lock() = Some(handle);
    Ok(())
}

/// Stop the current pipeline and start a new one using the latest in-memory config (after save).
async fn restart_pipeline(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut p = state.pipeline.lock().await;
        if let Some(h) = p.take() {
            let _ = h.stop_tx.send(());
        }
    }
    tokio::time::sleep(Duration::from_millis(320)).await;
    let cfg = Arc::new(state.config.lock().await.clone());
    cfg.validate().map_err(|e| e.to_string())?;
    let handle = spawn_pipeline_thread(&app, cfg, state.pipeline_status.clone());
    *state.pipeline.lock().await = Some(handle);
    Ok(())
}

#[tauri::command]
pub async fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    req: SaveConfigRequest,
) -> Result<(), String> {
    let mut cfg = state.config.lock().await;

    if let Some(v) = req.key_name {
        cfg.key_listener.key_name = v;
    }
    apply_nested_opt_u16(&mut cfg.key_listener.linux_evdev_code, req.linux_evdev_code);
    if let Some(v) = req.language {
        cfg.transcriber.language = v;
    }
    if let Some(v) = req.engine {
        cfg.transcriber.engine = v;
    }
    if let Some(v) = req.model {
        cfg.transcriber.model = v;
    }
    if let Some(v) = req.api_key {
        cfg.transcriber.api_key = v;
    }
    if let Some(v) = req.api_base_url {
        cfg.transcriber.api_base_url = v;
    }
    if let Some(v) = req.polish_level {
        cfg.polisher.level = v;
    }
    if let Some(v) = req.polish_model {
        cfg.polisher.model = v;
    }
    if let Some(v) = req.polish_api_key {
        cfg.polisher.api_key = v;
    }
    if let Some(v) = req.polish_api_base_url {
        cfg.polisher.api_base_url = v;
    }
    if let Some(v) = req.gui_language {
        cfg.gui.language = v;
    }

    cfg.validate().map_err(|e| e.to_string())?;
    cfg.save(&state.config_path)
        .map_err(|e| format!("save failed: {}", e))?;
    drop(cfg);

    restart_pipeline(app).await
}

#[tauri::command]
pub async fn capture_activation_key(
    state: State<'_, AppState>,
) -> Result<crate::key_capture::CaptureActivationResponse, String> {
    {
        let mut p = state.pipeline.lock().await;
        if let Some(h) = p.take() {
            let _ = h.stop_tx.send(());
        }
    }
    tokio::time::sleep(Duration::from_millis(400)).await;

    match tokio::task::spawn_blocking(crate::key_capture::capture_activation_key_blocking).await {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn start_pipeline(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut pipeline = state.pipeline.lock().await;
    if pipeline.is_some() {
        return Err("pipeline already running".into());
    }

    let cfg = Arc::new(state.config.lock().await.clone());
    let handle = spawn_pipeline_thread(&app, cfg, state.pipeline_status.clone());
    *pipeline = Some(handle);
    Ok(())
}

#[tauri::command]
pub async fn stop_pipeline(state: State<'_, AppState>) -> Result<(), String> {
    let mut pipeline = state.pipeline.lock().await;
    if let Some(handle) = pipeline.take() {
        let _ = handle.stop_tx.send(());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<RecordingStatus, String> {
    let status = state
        .pipeline_status
        .read()
        .unwrap_or_else(|e| e.into_inner());
    match status.as_str() {
        "recording" => Ok(RecordingStatus::Recording),
        "processing" => Ok(RecordingStatus::Processing),
        "done" => Ok(RecordingStatus::Done),
        _ => Ok(RecordingStatus::Idle),
    }
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

/// 在后台下载模型并立即返回，避免 GUI 长时间占用 IPC（大模型可达数 GB）。
/// 进度见 `model-download-progress`，结束见 `model-download-finished`（`success` / `error` / `path`）。
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
    state: State<'_, AppState>,
) -> Result<Vec<history::HistoryEntry>, String> {
    history::list_entries(&state.history_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_history_entries(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<usize, String> {
    history::delete_entries(&state.history_path, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    history::clear_all(&state.history_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn polish_history_entry(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<history::HistoryEntry, String> {
    let history_path = state.history_path.clone();
    let cfg = state.config.lock().await.clone();

    let entry = tokio::task::spawn_blocking({
        let p = history_path.clone();
        let id = id.clone();
        move || history::get_entry(&p, &id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let entry = entry.ok_or_else(|| "history entry not found".to_string())?;

    let polish_level = polisher::PolishLevel::effective(&cfg.polisher.level);
    let formatter = polisher::LLMFormatter::try_from(&cfg).map_err(|e| e.to_string())?;

    let source = entry.raw_text.clone();
    let polished = formatter
        .polish(&source, polish_level)
        .await
        .map_err(|e| e.to_string())?;

    let history_path_write = history_path;
    let out = tokio::task::spawn_blocking(move || {
        history::update_entry_text(&history_path_write, &id, polished)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let _ = app.emit("history-updated", ());
    Ok(out)
}

#[cfg(test)]
mod save_config_request_tests {
    use super::SaveConfigRequest;

    #[test]
    fn linux_evdev_json_null_clears_stored_code() {
        let j = r#"{"linuxEvdevCode":null}"#;
        let req: SaveConfigRequest = serde_json::from_str(j).unwrap();
        assert_eq!(req.linux_evdev_code, Some(None));
    }

    #[test]
    fn linux_evdev_missing_field_means_no_patch() {
        let j = r#"{}"#;
        let req: SaveConfigRequest = serde_json::from_str(j).unwrap();
        assert!(req.linux_evdev_code.is_none());
    }

    #[test]
    fn linux_evdev_number_sets() {
        let j = r#"{"linuxEvdevCode":100}"#;
        let req: SaveConfigRequest = serde_json::from_str(j).unwrap();
        assert_eq!(req.linux_evdev_code, Some(Some(100)));
    }
}
