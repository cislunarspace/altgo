//! altgo 核心库。
//!
//! 包含所有平台的语音转文字管道逻辑：
//! 按键监听 → 状态机 → 录音 → 语音识别 → 文本润色 → 输出

pub mod audio;
pub mod cmd;
pub mod config;
pub mod key_capture;
pub mod key_listener;
pub mod model;
pub mod output;
pub mod pipeline;
pub mod polisher;
pub mod recorder;
pub mod resource;
pub mod state_machine;
pub mod transcriber;
pub mod tray;

pub use pipeline::PipelineOutput;

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub struct AppState {
    pub config: Mutex<config::Config>,
    pub config_path: std::path::PathBuf,
    pub pipeline: Mutex<Option<PipelineHandle>>,
    pub pipeline_status: Arc<std::sync::RwLock<String>>,
}

pub struct PipelineHandle {
    pub stop_tx: tokio::sync::oneshot::Sender<()>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_path = config::Config::default_config_path();
            let cfg = config::Config::load(&config_path).expect("failed to load config");
            cfg.validate().expect("invalid config");

            let pipeline_status = Arc::new(std::sync::RwLock::new(String::from("idle")));
            let state = AppState {
                config: Mutex::new(cfg),
                config_path,
                pipeline: Mutex::new(None),
                pipeline_status: pipeline_status.clone(),
            };
            app.manage(state);

            tray::create_tray(app)?;

            #[cfg(target_os = "windows")]
            {
                std::thread::spawn(|| {
                    recorder::warmup_device();
                    tracing::info!("audio device warmup complete");
                });
            }

            cmd::spawn_pipeline_at_startup(app.handle().clone())?;

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
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // TODO: stop pipeline on exit
            }
        });
}
