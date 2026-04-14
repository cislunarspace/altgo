//! GUI module — eframe-based UI.
//!
//! Compiled when the `gui` feature is enabled.

pub mod app;
pub mod i18n;
pub mod state;

use std::sync::Arc;

pub use state::SharedState;

/// Run the GUI event loop.
pub fn run_gui(state: Arc<SharedState>) -> anyhow::Result<()> {
    let state_clone = Arc::clone(&state);

    // Load config early for language and font setup.
    let config_path = crate::config::Config::default_config_path();
    let cfg = crate::config::Config::load(&config_path).unwrap_or_default();
    let lang = i18n::Lang::from_code(&cfg.gui.language);

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

    let window_title = i18n::t("window.title", lang);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(window_title)
            .with_inner_size([480.0, 360.0])
            .with_resizable(false),
        // Keep app alive when window is closed - we handle close ourselves for tray behavior
        ..Default::default()
    };

    eframe::run_native(
        "altgo",
        options,
        Box::new(move |cc| {
            install_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(app::AltgoApp::new(state, config_path, cfg)))
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
        let lang = i18n::Lang::from_code(&cfg.gui.language);
        let _ = crate::output::notify(
            "altgo",
            i18n::t("notify.polish_failed", lang),
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

/// Install CJK-capable fonts from system paths.
///
/// Tries multiple font paths per platform so that Chinese (and other CJK)
/// characters render correctly in egui. Falls back gracefully with a warning
/// if no suitable font is found.
fn install_cjk_fonts(ctx: &egui::Context) {
    let font_bytes = load_cjk_system_font();
    match font_bytes {
        Some((bytes, name)) => {
            tracing::info!(font = %name, "installing CJK font");
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "cjk-system-font".to_owned(),
                egui::FontData::from_owned(bytes),
            );
            // Append CJK font as fallback for Proportional and Monospace families.
            // Latin/emoji fonts are tried first; CJK covers the rest.
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("cjk-system-font".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("cjk-system-font".to_owned());
            ctx.set_fonts(fonts);
        }
        None => {
            tracing::warn!(
                "no CJK system font found — Chinese/Japanese/Korean text may not render"
            );
        }
    }
}

/// Try to load a CJK-capable system font, returning `(bytes, font_name)` on success.
fn load_cjk_system_font() -> Option<(Vec<u8>, String)> {
    let candidates: Vec<&str> = platform_cjk_fonts();
    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            tracing::debug!(path, "found CJK font");
            return Some((bytes, (*path).to_string()));
        }
    }
    None
}

/// Return candidate CJK font paths for the current platform.
fn platform_cjk_fonts() -> Vec<&'static str> {
    if cfg!(target_os = "windows") {
        vec![
            "C:/Windows/Fonts/msyh.ttc",       // Microsoft YaHei (default on Win10+)
            "C:/Windows/Fonts/msyhboot.ttc",    // YaHei boot variant
            "C:/Windows/Fonts/simsun.ttc",       // SimSun (legacy)
            "C:/Windows/Fonts/simhei.ttf",       // SimHei (legacy)
            "C:/Windows/Fonts/simsun.ttf",       // SimSun TTF variant
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "/System/Library/Fonts/PingFang.ttc",          // PingFang SC (default on macOS 10.11+)
            "/System/Library/Fonts/STHeiti Light.ttc",     // STHeiti (older macOS)
            "/System/Library/Fonts/Hiragino Sans GB.ttc",  // Hiragino (older macOS)
            "/Library/Fonts/Arial Unicode.ttf",             // Arial Unicode MS
        ]
    } else {
        // Linux — many distributions, many possible paths.
        vec![
            // Noto Sans CJK (most common on modern distros)
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            // Noto Sans SC (Simplified Chinese subset, smaller)
            "/usr/share/fonts/opentype/noto/NotoSansSC-Regular.otf",
            "/usr/share/fonts/truetype/noto/NotoSansSC-Regular.ttf",
            // Droid Sans Fallback (older distros)
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            // WenQuanYi (common on Chinese-focused distros)
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/wqy-zenhei/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            // Source Han Sans (Adobe/Google)
            "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
            "/usr/share/fonts/SourceHanSans/SourceHanSansSC-Regular.otf",
        ]
    }
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
