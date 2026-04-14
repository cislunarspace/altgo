//! GUI module — eframe-based UI with system tray (Linux only for tray).
//!
//! Compiled when the `gui` feature is enabled.

pub mod app;
pub mod state;

#[cfg(target_os = "linux")]
pub mod tray;

use std::sync::Arc;

pub use state::SharedState;

/// Run the GUI event loop.
pub fn run_gui(state: Arc<SharedState>) -> anyhow::Result<()> {
    let state_clone = Arc::clone(&state);

    // Spawn the audio pipeline in a background thread with its own Tokio runtime.
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build Tokio runtime for GUI pipeline");
        rt.block_on(async {
            if let Err(e) = run_pipeline(state_clone).await {
                tracing::error!(error = %e, "GUI pipeline exited with error");
            }
        });
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("altgo — 语音转文字")
            .with_inner_size([480.0, 360.0])
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "altgo",
        options,
        Box::new(move |_cc| {
            #[cfg(target_os = "linux")]
            {
                let app_handle = _cc.platform.clone();
                let tray = tray::build_tray(&app_handle);
                state.set_app_handle(app_handle);
                state.set_tray(tray);
            }

            Ok(Box::new(app::AltgoApp::new(state)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;

    Ok(())
}

/// Run the full audio pipeline (key listener → recorder → transcriber → polisher → output).
/// Updates `SharedState` at each stage so the GUI can display progress.
async fn run_pipeline(state: Arc<SharedState>) -> anyhow::Result<()> {
    use std::str::FromStr;
    use tokio::sync::mpsc;

    // Load config from default path.
    let config_path = crate::config::Config::default_config_path();
    let cfg = Arc::new(crate::config::Config::load(&config_path)?);

    // Initialize key listener.
    let mut listener = crate::key_listener::PlatformListener::new(&cfg.key_listener.key_name)?;

    // Initialize recorder.
    let mut recorder =
        crate::recorder::PlatformRecorder::new(cfg.recorder.sample_rate, cfg.recorder.channels);

    // Initialize transcriber.
    let transcriber = match cfg.transcriber.engine.as_str() {
        "local" => PipelineTranscriber::Local(crate::transcriber::LocalWhisper::new(
            cfg.transcriber.model.clone(),
            cfg.transcriber.language.clone(),
        )),
        _ => PipelineTranscriber::Api(crate::transcriber::WhisperApi::new(
            cfg.transcriber.api_key.clone(),
            cfg.transcriber.api_base_url.clone(),
            cfg.transcriber.model.clone(),
            cfg.transcriber.language.clone(),
            cfg.transcriber.timeout(),
        )),
    };

    // Initialize polisher.
    let polish_level =
        crate::polisher::PolishLevel::from_str(&cfg.polisher.level).unwrap_or_else(|_| {
            tracing::warn!(
                level = %cfg.polisher.level,
                "invalid polish level in config, using 'medium'"
            );
            crate::polisher::PolishLevel::Medium
        });
    let formatter = crate::polisher::LLMFormatter::with_max_tokens(
        cfg.polisher.api_key.clone(),
        cfg.polisher.api_base_url.clone(),
        cfg.polisher.model.clone(),
        cfg.polisher.timeout(),
        cfg.polisher.max_tokens,
    );

    // Start key listener.
    let key_events = listener.start()?;

    // Debounce task.
    let (key_tx, key_rx) = mpsc::unbounded_channel();
    let debounce_window = cfg.key_listener.debounce_window();
    tokio::spawn(debounce_task(key_events, key_tx, debounce_window));

    // Start state machine.
    let sm = crate::state_machine::Machine::new(
        cfg.key_listener.long_press_threshold(),
        cfg.key_listener.double_click_interval(),
    );
    let mut commands = sm.run(key_rx);

    tracing::info!("GUI pipeline initialized — waiting for trigger key");

    while let Some(cmd) = commands.recv().await {
        match cmd {
            crate::state_machine::Command::StartRecord => {
                tracing::info!("recording started");
                state.set_recording(crate::gui::state::RecordingState::Recording);
                if cfg.output.enable_notify {
                    let _ = crate::output::notify_processing();
                }
                if let Err(e) = recorder.start() {
                    tracing::error!(error = %e, "failed to start recording");
                }
            }
            crate::state_machine::Command::StopRecord => {
                tracing::info!("recording stopped, processing...");
                state.set_recording(crate::gui::state::RecordingState::Processing);
                let wav_data = match recorder.stop() {
                    Ok(data) => data,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to stop recording");
                        state.set_recording(crate::gui::state::RecordingState::Idle);
                        continue;
                    }
                };

                let cfg = Arc::clone(&cfg);
                let formatter = formatter.clone();
                let transcriber = transcriber.clone();
                let state = Arc::clone(&state);

                tokio::spawn(async move {
                    if let Err(e) = process_audio(
                        &cfg,
                        &transcriber,
                        &formatter,
                        &wav_data,
                        polish_level,
                        &state,
                    )
                    .await
                    {
                        tracing::error!(error = %e, "audio processing failed");
                        state.set_recording(crate::gui::state::RecordingState::Idle);
                    }
                });
            }
        }
    }

    listener.stop();
    Ok(())
}

/// Voice processing pipeline: transcription → polishing → clipboard output.
async fn process_audio(
    cfg: &Arc<crate::config::Config>,
    transcriber: &PipelineTranscriber,
    formatter: &crate::polisher::LLMFormatter,
    wav_data: &[u8],
    polish_level: crate::polisher::PolishLevel,
    state: &Arc<SharedState>,
) -> anyhow::Result<()> {
    let result = match transcriber {
        PipelineTranscriber::Local(lw) => lw.transcribe(wav_data).await,
        PipelineTranscriber::Api(api) => api.transcribe(wav_data).await,
    };

    let result = result.map_err(|e| {
        tracing::error!(error = %e, "transcription failed");
        e
    })?;

    tracing::info!(text = %result.text, "transcribed");

    if result.text.is_empty() {
        tracing::warn!("empty transcription, skipping");
        state.set_recording(crate::gui::state::RecordingState::Idle);
        return Ok(());
    }

    let mut polish_failed = false;
    let polished = formatter
        .polish(&result.text, polish_level)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "polish failed, using raw text");
            polish_failed = true;
            result.text.clone()
        });

    if polish_failed && cfg.output.enable_notify {
        let _ = crate::output::notify(
            "altgo",
            "润色失败，已使用原始文本",
            cfg.output.notify_timeout_ms,
        );
    }

    tracing::info!(text = %polished, "polished");

    if let Err(e) = crate::output::write_clipboard(&polished).await {
        tracing::error!(error = %e, "clipboard write failed");
    }

    if cfg.output.enable_notify {
        let _ = crate::output::notify_result(&polished, cfg.output.notify_timeout_ms);
    }

    state.set_transcription(polished);
    tracing::info!("done — text copied to clipboard");
    Ok(())
}

/// Transcriber backend wrapper for the GUI pipeline.
#[derive(Clone)]
enum PipelineTranscriber {
    Api(crate::transcriber::WhisperApi),
    Local(crate::transcriber::LocalWhisper),
}

/// Debounce task — filters IME-induced key chatter on Windows.
async fn debounce_task(
    mut raw_events: tokio::sync::mpsc::UnboundedReceiver<crate::key_listener::KeyEvent>,
    key_tx: tokio::sync::mpsc::UnboundedSender<crate::state_machine::KeyEvent>,
    debounce_window: std::time::Duration,
) {
    let mut is_pressed = false;
    let mut pending_release: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;

    loop {
        tokio::select! {
            Some(evt) = raw_events.recv() => {
                if evt.pressed {
                    pending_release = None;
                    is_pressed = true;
                    if key_tx.send(crate::state_machine::KeyEvent { pressed: true }).is_err() {
                        break;
                    }
                } else if is_pressed && pending_release.is_none() {
                    pending_release = Some(Box::pin(tokio::time::sleep(debounce_window)));
                }
            },
            _ = async {
                if let Some(timer) = &mut pending_release {
                    timer.as_mut().await;
                } else {
                    std::future::pending::<()>().await;
                }
            }, if pending_release.is_some() => {
                pending_release = None;
                is_pressed = false;
                if key_tx.send(crate::state_machine::KeyEvent { pressed: false }).is_err() {
                    break;
                }
            },
            else => break,
        }
    }
}
