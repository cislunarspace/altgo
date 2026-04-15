//! altgo Tauri 后端。
//!
//! 注册 Tauri commands、系统托盘，连接前端和核心管道。

use tauri::{Manager, RunEvent};
use tokio::sync::Mutex;

mod cmd;
mod tray;

/// 应用状态 — 在 Tauri 管理器中共享。
pub struct AppState {
    config: Mutex<altgo::config::Config>,
    config_path: std::path::PathBuf,
    pipeline: Mutex<Option<PipelineHandle>>,
}

/// 后台管道的句柄，用于停止。
struct PipelineHandle {
    stop_tx: tokio::sync::oneshot::Sender<()>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_path = altgo::config::Config::default_config_path();
            let cfg = altgo::config::Config::load(&config_path)
                .expect("failed to load config");
            cfg.validate().expect("invalid config");

            let state = AppState {
                config: Mutex::new(cfg),
                config_path,
                pipeline: Mutex::new(None),
            };
            app.manage(state);

            tray::create_tray(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd::get_config,
            cmd::save_config,
            cmd::start_pipeline,
            cmd::stop_pipeline,
            cmd::get_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let RunEvent::ExitRequested { .. } = event {
                // TODO: stop pipeline on exit
            }
        });
}
