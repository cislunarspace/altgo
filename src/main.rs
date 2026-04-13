mod audio;
mod config;
mod key_listener;
mod output;
mod polisher;
mod recorder;
mod state_machine;
mod transcriber;

use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;

#[derive(Parser)]
#[command(name = "altgo", about = "无需打字，言出法随 — Linux 语音转文字工具")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<String>,

    /// Print version
    #[arg(short = 'V', long)]
    version: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("altgo {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let config_path = cli
        .config
        .map(std::path::PathBuf::from)
        .unwrap_or_else(config::Config::default_config_path);

    let cfg = Arc::new(config::Config::load(&config_path)?);

    // Initialize logging.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cfg.logging.level));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    tracing::info!("altgo starting");

    // Initialize key listener.
    let mut listener = key_listener::X11Listener::new(&cfg.key_listener.key_name)?;

    // Initialize recorder.
    let mut recorder =
        recorder::PulseRecorder::new(cfg.recorder.sample_rate, cfg.recorder.channels);

    // Initialize transcriber.
    match cfg.transcriber.engine.as_str() {
        "local" => {
            tracing::info!(
                model = %cfg.transcriber.model,
                "using local whisper for transcription"
            );
        }
        _ => {
            tracing::info!("using Whisper API for transcription");
        }
    }

    // Initialize polisher.
    let polish_level = polisher::PolishLevel::from_str(&cfg.polisher.level)
        .unwrap_or(polisher::PolishLevel::Medium);
    let formatter = polisher::LLMFormatter::new(
        cfg.polisher.api_key.clone(),
        cfg.polisher.api_base_url.clone(),
        cfg.polisher.model.clone(),
        cfg.polisher.timeout(),
    );

    // Start key listener.
    let key_events = listener.start()?;

    // Convert key events for the state machine.
    let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();
    let key_tx = Arc::new(key_tx);
    tokio::spawn(async move {
        let key_tx = key_tx;
        let mut key_events = key_events;
        while let Some(evt) = key_events.recv().await {
            if key_tx
                .send(state_machine::KeyEvent {
                    pressed: evt.pressed,
                })
                .is_err()
            {
                break;
            }
        }
    });

    // Start state machine.
    let sm = state_machine::Machine::new(
        cfg.key_listener.long_press_threshold(),
        cfg.key_listener.double_click_interval(),
    );
    let mut commands = sm.run(key_rx);

    tracing::info!("altgo initialized — waiting for right Alt key");

    // Main event loop.
    while let Some(cmd) = commands.recv().await {
        match cmd {
            state_machine::Command::StartRecord => {
                tracing::info!("recording started");
                if cfg.output.enable_notify {
                    let _ = output::notify_processing();
                }
                if let Err(e) = recorder.start() {
                    tracing::error!(error = %e, "failed to start recording");
                }
            }
            state_machine::Command::StopRecord => {
                tracing::info!("recording stopped, processing...");
                let wav_data = match recorder.stop() {
                    Ok(data) => data,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to stop recording");
                        continue;
                    }
                };

                // Spawn processing pipeline.
                let cfg = Arc::clone(&cfg);
                let formatter = formatter.clone();

                tokio::spawn(async move {
                    if let Err(e) = process_audio(&cfg, &formatter, &wav_data, polish_level).await {
                        tracing::error!(error = %e, "audio processing failed");
                    }
                });
            }
        }
    }

    listener.stop();
    tracing::info!("altgo stopped");
    Ok(())
}

/// ASR → Polish → Output pipeline.
async fn process_audio(
    cfg: &Arc<config::Config>,
    formatter: &polisher::LLMFormatter,
    wav_data: &[u8],
    polish_level: polisher::PolishLevel,
) -> anyhow::Result<()> {
    // Step 1: Transcribe.
    let result = match cfg.transcriber.engine.as_str() {
        "local" => {
            let local = transcriber::LocalWhisper::new(
                cfg.transcriber.model.clone(),
                cfg.transcriber.language.clone(),
            );
            local.transcribe(wav_data).await
        }
        _ => {
            let api = transcriber::WhisperApi::new(
                cfg.transcriber.api_key.clone(),
                cfg.transcriber.api_base_url.clone(),
                cfg.transcriber.model.clone(),
                cfg.transcriber.language.clone(),
                cfg.transcriber.timeout(),
            );
            api.transcribe(wav_data).await
        }
    };

    let result = result.map_err(|e| {
        tracing::error!(error = %e, "transcription failed");
        e
    })?;

    tracing::info!(text = %result.text, "transcribed");

    if result.text.is_empty() {
        tracing::warn!("empty transcription, skipping");
        return Ok(());
    }

    // Step 2: Polish.
    let polished = formatter
        .polish(&result.text, polish_level)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "polish failed, using raw text");
            result.text.clone()
        });

    tracing::info!(text = %polished, "polished");

    // Step 3: Output.
    if let Err(e) = output::write_clipboard(&polished) {
        tracing::error!(error = %e, "clipboard write failed");
    }

    if cfg.output.enable_notify {
        let _ = output::notify_result(&polished, cfg.output.notify_timeout_ms);
    }

    tracing::info!("done — text copied to clipboard");
    Ok(())
}
