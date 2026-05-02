//! Tauri commands — 前端通过 IPC 调用的函数。

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    config,
    config_store::{ConfigPatch, ConfigStore},
    history,
    history::HistoryStore,
    output,
    pipeline_controller::{PipelineController, PipelineStatus},
    polisher,
    tauri_sink::TauriPipelineSink,
};

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
