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
    // Initialize logging for GUI mode (CLI path does its own init in main.rs).
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init()
        .ok(); // ok() to ignore double-init if CLI path already called init()

    // Load config once and share between GUI and pipeline.
    let config_path = crate::config::Config::default_config_path();
    let cfg = Arc::new(crate::config::Config::load(&config_path)?);
    cfg.as_ref().validate()?;
    let lang = i18n::Lang::from_code(&cfg.gui.language);

    let state_clone = Arc::clone(&state);
    let cfg_clone = Arc::clone(&cfg);

    // Spawn the audio pipeline in a background thread with its own Tokio runtime.
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!(error = %e, "failed to build Tokio runtime for GUI pipeline");
                return;
            }
        };
        rt.block_on(async {
            if let Err(e) = run_pipeline(state_clone, cfg_clone).await {
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
        ..Default::default()
    };

    eframe::run_native(
        "altgo",
        options,
        Box::new(move |cc| {
            install_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(app::AltgoApp::new(
                state,
                config_path,
                (*cfg).clone(),
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;

    Ok(())
}

/// Run the full audio pipeline (key listener → recorder → transcriber → polisher → output).
/// Updates `SharedState` at each stage so the GUI can display progress.
async fn run_pipeline(
    state: Arc<SharedState>,
    cfg: Arc<crate::config::Config>,
) -> anyhow::Result<()> {
    use std::str::FromStr;
    use tokio::sync::mpsc;

    // Initialize key listener.
    let mut listener = crate::key_listener::PlatformListener::new(&cfg.key_listener.key_name)?;
    #[cfg(target_os = "windows")]
    listener.set_poll_interval_ms(cfg.key_listener.poll_interval_ms);

    // Initialize recorder.
    let mut recorder =
        crate::recorder::PlatformRecorder::new(cfg.recorder.sample_rate, cfg.recorder.channels);

    // Initialize transcriber.
    let transcriber: crate::transcriber::Transcriber = match cfg.transcriber.engine.as_str() {
        "local" => crate::transcriber::Transcriber::Local(crate::transcriber::LocalWhisper::new(
            cfg.transcriber.model.clone(),
            cfg.transcriber.language.clone(),
        )),
        _ => crate::transcriber::Transcriber::Api(crate::transcriber::WhisperApi::new(
            cfg.transcriber.api_key.clone(),
            cfg.transcriber.api_base_url.clone(),
            cfg.transcriber.model.clone(),
            cfg.transcriber.language.clone(),
            cfg.transcriber.timeout(),
        )?),
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
    )?;

    // Start key listener.
    let key_events = listener.start()?;

    // Debounce task.
    let (key_tx, key_rx) = mpsc::unbounded_channel();
    let debounce_window = cfg.key_listener.debounce_window();
    tokio::spawn(crate::key_listener::debounce_task(
        key_events,
        key_tx,
        debounce_window,
    ));

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
                    let lang = i18n::Lang::from_code(&cfg.gui.language);
                    let _ = crate::output::notify_processing(i18n::t("notify.processing", lang));
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
    transcriber: &crate::transcriber::Transcriber,
    formatter: &crate::polisher::LLMFormatter,
    wav_data: &[u8],
    polish_level: crate::polisher::PolishLevel,
    state: &Arc<SharedState>,
) -> anyhow::Result<()> {
    let output =
        crate::pipeline::process_audio_core(transcriber, formatter, wav_data, polish_level)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "transcription failed");
                e
            })?;

    if output.text.is_empty() {
        state.set_recording(crate::gui::state::RecordingState::Idle);
        return Ok(());
    }

    if output.polish_failed && cfg.output.enable_notify {
        let lang = i18n::Lang::from_code(&cfg.gui.language);
        let _ = crate::output::notify(
            "altgo",
            i18n::t("notify.polish_failed", lang),
            cfg.output.notify_timeout_ms,
        );
    }

    if let Err(e) = crate::output::write_clipboard(&output.text).await {
        tracing::error!(error = %e, "clipboard write failed");
    }

    if cfg.output.enable_notify {
        let _ = crate::output::notify_result(&output.text, cfg.output.notify_timeout_ms);
    }

    state.set_transcription(output.text);
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
            // Prepend CJK font so it takes priority over default Latin fonts.
            // This ensures CJK codepoints are rendered by the system font.
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk-system-font".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "cjk-system-font".to_owned());
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
            // NotoSansSC-VF.ttf is a single .ttf (not .ttc collection), so ab_glyph
            // parses it correctly without font-index issues.
            "C:/Windows/Fonts/NotoSansSC-VF.ttf",
            "C:/Windows/Fonts/msyh.ttc", // Microsoft YaHei (default on Win10+)
            "C:/Windows/Fonts/msyhboot.ttc", // YaHei boot variant
            "C:/Windows/Fonts/simsun.ttc", // SimSun (legacy)
            "C:/Windows/Fonts/simhei.ttf", // SimHei (legacy)
            "C:/Windows/Fonts/simsun.ttf", // SimSun TTF variant
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "/System/Library/Fonts/PingFang.ttc", // PingFang SC (default on macOS 10.11+)
            "/System/Library/Fonts/STHeiti Light.ttc", // STHeiti (older macOS)
            "/System/Library/Fonts/Hiragino Sans GB.ttc", // Hiragino (older macOS)
            "/Library/Fonts/Arial Unicode.ttf",   // Arial Unicode MS
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
