//! altgo 核心库。
//!
//! 包含所有平台的语音转文字管道逻辑：
//! 按键监听 → 状态机 → 录音 → 语音识别 → 文本润色 → 输出

pub mod audio;
pub mod cmd;
pub mod config;
pub mod config_store;
pub mod error;
pub mod history;
pub mod key_capture;
pub mod key_listener;
pub mod model;
pub mod output;
pub mod overlay;
pub mod pipeline_controller;
pub mod polisher;
pub mod prompt_store;
pub mod recorder;
pub mod resource;
pub mod state_machine;
pub mod tauri_sink;
pub mod transcriber;
pub mod tray;
pub mod voice_pipeline;
pub mod whisper_server;

use std::sync::Arc;
use tauri::Manager;

pub struct PipelineHandle {
    pub stop_tx: tokio::sync::oneshot::Sender<()>,
    pub thread_handle: std::thread::JoinHandle<()>,
}

/// Build the Tauri pipeline sink and spawn the voice pipeline on a dedicated
/// OS thread. Returns a stop handle the caller can use to terminate the loop.
///
/// Centralised here so `cmd.rs` only exposes `#[tauri::command]` functions
/// and never has to thread the sink/lifecycle construction through IPC.
pub(crate) fn spawn_pipeline_thread(
    app: &tauri::AppHandle,
    cfg: Arc<config::Config>,
    pipeline_status: Arc<std::sync::RwLock<crate::pipeline_controller::PipelineStatus>>,
) -> PipelineHandle {
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let app_handle = app.clone();
    let cfg_clone = cfg.clone();

    let overlay: Arc<dyn overlay::seam::OverlaySink> =
        Arc::new(overlay::manager::OverlayManager::new(
            overlay::tauri::TauriOverlayWindow::new(app_handle.clone()),
        ));

    let thread_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        let output: Arc<dyn output::Output> = app_handle
            .state::<Arc<dyn output::Output>>()
            .inner()
            .clone();
        let dispatch: Arc<dyn voice_pipeline::TranscriptionDispatch> =
            Arc::new(voice_pipeline::TranscriptionDispatcherImpl {
                output,
                history_store: app_handle
                    .state::<crate::history::HistoryStore>()
                    .inner()
                    .clone(),
            });
        let sink = tauri_sink::TauriPipelineSink::new(
            app_handle.clone(),
            pipeline_status,
            cfg_clone,
            dispatch,
            overlay,
        );
        rt.block_on(voice_pipeline::run(cfg, stop_rx, sink));
    });
    PipelineHandle {
        stop_tx,
        thread_handle,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_path = config::Config::default_config_path();
            let history_path = config_path
                .parent()
                .map(|p| p.join("history.json"))
                .unwrap_or_else(|| config_path.with_extension("history.json"));

            let config_store = config_store::ConfigStore::load(config_path);
            let history_store = history::HistoryStore::new(history_path);
            let pipeline_controller = pipeline_controller::PipelineController::new();

            let cfg = Arc::new(config_store.snapshot_blocking());
            cfg.validate().map_err(|e| e.to_string())?;

            app.manage(config_store);
            app.manage(history_store);
            app.manage(pipeline_controller);
            app.manage(Arc::new(output::PlatformOutput::new()) as Arc<dyn output::Output>);

            tray::create_tray(app)?;

            // Intercept close requests on the main window so the app stays in the tray.
            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(win) = app_handle.get_webview_window("main") {
                            let _ = win.hide();
                        }
                    }
                });
            }

            let controller = app.state::<pipeline_controller::PipelineController>();
            let status_arc = controller.status_arc();
            controller
                .start_with_blocking(|| spawn_pipeline_thread(app.handle(), cfg, status_arc))?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd::get_config,
            cmd::save_config,
            cmd::start_pipeline,
            cmd::stop_pipeline,
            cmd::get_status,
            cmd::copy_text,
            cmd::hide_overlay,
            cmd::list_models,
            cmd::download_model,
            cmd::delete_model,
            cmd::resolve_model,
            cmd::capture_activation_key,
            cmd::list_history,
            cmd::delete_history_entries,
            cmd::clear_history,
            cmd::polish_history_entry,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                app_handle
                    .state::<pipeline_controller::PipelineController>()
                    .stop_blocking();
            }
        });
}
