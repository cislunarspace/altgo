//! Tauri commands — 前端通过 IPC 调用的函数。

use std::str::FromStr;
use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, State};

use crate::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Idle,
    Recording,
    Processing,
    Done,
}

impl RecordingStatus {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub key_name: String,
    pub language: String,
    pub engine: String,
    pub model: String,
    pub api_base_url: String,
    pub polish_level: String,
    pub polish_model: String,
    pub polish_api_base_url: String,
    pub gui_language: String,
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<ConfigResponse, String> {
    let cfg = state.config.lock().await;
    Ok(ConfigResponse {
        key_name: cfg.key_listener.key_name.clone(),
        language: cfg.transcriber.language.clone(),
        engine: cfg.transcriber.engine.clone(),
        model: cfg.transcriber.model.clone(),
        api_base_url: cfg.transcriber.api_base_url.clone(),
        polish_level: cfg.polisher.level.clone(),
        polish_model: cfg.polisher.model.clone(),
        polish_api_base_url: cfg.polisher.api_base_url.clone(),
        gui_language: cfg.gui.language.clone(),
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConfigRequest {
    pub key_name: Option<String>,
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

#[tauri::command]
pub async fn save_config(
    state: State<'_, AppState>,
    req: SaveConfigRequest,
) -> Result<(), String> {
    let mut cfg = state.config.lock().await;

    if let Some(v) = req.key_name {
        cfg.key_listener.key_name = v;
    }
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

    cfg.save(&state.config_path)
        .map_err(|e| format!("save failed: {}", e))?;

    Ok(())
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

    let cfg = state.config.lock().await.clone();
    let cfg = Arc::new(cfg);

    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();

    let app_clone = app.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        rt.block_on(run_pipeline(app_clone, cfg, stop_rx));
    });

    *pipeline = Some(crate::PipelineHandle { stop_tx });
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
    let pipeline = state.pipeline.lock().await;
    if pipeline.is_some() {
        Ok(RecordingStatus::Idle)
    } else {
        Ok(RecordingStatus::Idle)
    }
}

async fn run_pipeline(
    app: tauri::AppHandle,
    cfg: Arc<altgo::config::Config>,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
) {
    let mut listener = match altgo::key_listener::PlatformListener::new(&cfg.key_listener.key_name)
    {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = %e, "failed to create key listener");
            let _ = app.emit("pipeline-error", format!("key listener: {}", e));
            return;
        }
    };
    #[cfg(target_os = "windows")]
    listener.set_poll_interval_ms(cfg.key_listener.poll_interval_ms);

    let mut recorder =
        altgo::recorder::PlatformRecorder::new(cfg.recorder.sample_rate, cfg.recorder.channels);

    let transcriber: altgo::transcriber::Transcriber = match cfg.transcriber.engine.as_str() {
        "local" => altgo::transcriber::Transcriber::Local(
            altgo::transcriber::LocalWhisper::new(
                cfg.transcriber.model.clone(),
                cfg.transcriber.language.clone(),
                cfg.transcriber.whisper_path.clone(),
            ),
        ),
        _ => match altgo::transcriber::WhisperApi::new(
            cfg.transcriber.api_key.clone(),
            cfg.transcriber.api_base_url.clone(),
            cfg.transcriber.model.clone(),
            cfg.transcriber.language.clone(),
            cfg.transcriber.temperature,
            cfg.transcriber.prompt.clone(),
            cfg.transcriber.timeout(),
        ) {
            Ok(api) => altgo::transcriber::Transcriber::Api(api),
            Err(e) => {
                tracing::error!(error = %e, "failed to create transcriber");
                let _ = app.emit("pipeline-error", format!("transcriber: {}", e));
                return;
            }
        },
    };

    let polish_level =
        altgo::polisher::PolishLevel::from_str(&cfg.polisher.level).unwrap_or_else(|_| {
            tracing::warn!("invalid polish level, using medium");
            altgo::polisher::PolishLevel::Medium
        });
    let polisher_protocol =
        altgo::polisher::ApiProtocol::from_str(&cfg.polisher.protocol).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "invalid polisher protocol, defaulting to openai");
            altgo::polisher::ApiProtocol::OpenAi
        });
    let formatter = match altgo::polisher::LLMFormatter::with_config(
        cfg.polisher.api_key.clone(),
        cfg.polisher.api_base_url.clone(),
        cfg.polisher.model.clone(),
        cfg.polisher.timeout(),
        cfg.polisher.max_tokens,
        polisher_protocol,
        cfg.polisher.temperature,
        cfg.transcriber.language.clone(),
        cfg.polisher.system_prompt.clone(),
    ) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "failed to create polisher");
            let _ = app.emit("pipeline-error", format!("polisher: {}", e));
            return;
        }
    };

    let key_events = match listener.start() {
        Ok(rx) => rx,
        Err(e) => {
            tracing::error!(error = %e, "failed to start key listener");
            let _ = app.emit("pipeline-error", format!("key listener start: {}", e));
            return;
        }
    };

    let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();
    let debounce_window = cfg.key_listener.debounce_window();
    tokio::spawn(altgo::key_listener::debounce_task(
        key_events,
        key_tx,
        debounce_window,
    ));

    let sm = altgo::state_machine::Machine::new(
        cfg.key_listener.long_press_threshold(),
        cfg.key_listener.double_click_interval(),
        cfg.key_listener.min_press_duration(),
    );
    let mut commands = sm.run(key_rx);

    let _ = app.emit("pipeline-status", "idle");

    loop {
        tokio::select! {
            cmd = commands.recv() => {
                match cmd {
                    Some(altgo::state_machine::Command::StartRecord) => {
                        tracing::info!("recording started");
                        let _ = app.emit("pipeline-status", "recording");
                        let _ = altgo::output::show_recording_window();
                        if let Err(e) = recorder.start() {
                            tracing::error!(error = %e, "failed to start recording");
                        }
                    }
                    Some(altgo::state_machine::Command::StopRecord) => {
                        tracing::info!("recording stopped, processing...");
                        let _ = app.emit("pipeline-status", "processing");
                        let _ = altgo::output::close_recording_window();
                        let wav_data = match recorder.stop() {
                            Ok(data) => data,
                            Err(e) => {
                                tracing::error!(error = %e, "failed to stop recording");
                                let _ = app.emit("pipeline-status", "idle");
                                continue;
                            }
                        };

                        let cfg = Arc::clone(&cfg);
                        let formatter = formatter.clone();
                        let transcriber = transcriber.clone();
                        let app = app.clone();

                        tokio::spawn(async move {
                            match altgo::pipeline::process_audio_core(
                                &transcriber,
                                &formatter,
                                &wav_data,
                                polish_level,
                            )
                            .await
                            {
                                Ok(output) => {
                                    if !output.text.is_empty() {
                                        let _ = altgo::output::output_text(
                                            &output.raw_text,
                                            &output.text,
                                            output.polish_failed,
                                            cfg.output.inject_at_cursor,
                                            cfg.output.prefer_polished,
                                            cfg.output.notify_timeout_ms,
                                        )
                                        .await;

                                        let _ = app.emit("transcription-result", &output.text);
                                    }
                                    let _ = app.emit("pipeline-status", "done");
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "audio processing failed");
                                    let _ = app.emit("pipeline-error", format!("processing: {}", e));
                                    let _ = app.emit("pipeline-status", "idle");
                                }
                            }
                        });
                    }
                    None => break,
                }
            }
            _ = &mut stop_rx => {
                tracing::info!("pipeline stop requested");
                break;
            }
        }
    }

    listener.stop();
    let _ = app.emit("pipeline-status", "stopped");
    tracing::info!("pipeline stopped");
}
