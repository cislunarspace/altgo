//! Tauri commands — 前端通过 IPC 调用的函数。

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{
    config::ConfigPatch,
    config_store::ConfigStore,
    history,
    history::HistoryStore,
    output,
    overlay::manager::{OverlayManager, OverlayState},
    overlay::tauri::TauriOverlayWindow,
    pipeline_controller::{PipelineController, PipelineStatus},
    polisher, voice_pipeline,
};

async fn restart_pipeline(
    app: AppHandle,
    controller: &PipelineController,
    config_store: &ConfigStore,
) -> Result<(), String> {
    let cfg = Arc::new(config_store.snapshot().await);
    cfg.validate().map_err(|e| e.to_string())?;
    let status_arc = controller.status_arc();
    controller.stop().await;
    controller
        .start_with(|| crate::spawn_pipeline_thread(&app, cfg, status_arc))
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
    let cfg = config_store.snapshot().await;
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
        .start_with(|| crate::spawn_pipeline_thread(&app, cfg, status_arc))
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
pub async fn copy_text(
    output_state: State<'_, Arc<dyn output::Output>>,
    text: String,
) -> Result<(), String> {
    let out = output_state.inner().clone_box();
    tokio::task::spawn_blocking(move || out.write_clipboard(&text))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    OverlayManager::new(TauriOverlayWindow::new(app)).set_state(OverlayState::hidden());
    Ok(())
}

#[tauri::command]
pub async fn list_models() -> Result<Vec<crate::model::ModelEntry>, String> {
    Ok(crate::model::list_all_with_status())
}

#[tauri::command]
pub async fn download_model(app: tauri::AppHandle, name: String) -> Result<(), String> {
    crate::model::validate_name(&name).map_err(|e| e.to_string())?;

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
    crate::model::delete(&name).map_err(|e| e.to_string())
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
    let formatter =
        polisher::LLMFormatter::from_config_with_sources(&cfg).map_err(|e| e.to_string())?;
    let polish_level = polisher::PolishLevel::effective(&cfg.polisher.level);

    let updated = voice_pipeline::dispatch_history_polish(
        history_store.inner(),
        &id,
        &formatter,
        polish_level,
    )
    .await?;

    let _ = app.emit("history-updated", ());
    Ok(updated)
}
