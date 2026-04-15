use tauri::{Manager, RunEvent};
use tokio::sync::Mutex;

mod cmd;
mod tray;

pub struct AppState {
    config: Mutex<altgo::config::Config>,
    config_path: std::path::PathBuf,
    pipeline: Mutex<Option<PipelineHandle>>,
}

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

            let cfg_arc = std::sync::Arc::new(cfg.clone());

            let state = AppState {
                config: Mutex::new(cfg),
                config_path,
                pipeline: Mutex::new(None),
            };
            app.manage(state);

            tray::create_tray(app)?;

            let app_handle = app.handle().clone();
            let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();

            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build tokio runtime");
                rt.block_on(cmd::run_pipeline(app_handle, cfg_arc, stop_rx));
            });

            {
                let state = app.state::<AppState>();
                *state.pipeline.blocking_lock() = Some(PipelineHandle { stop_tx });
            }

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
