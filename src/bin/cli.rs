//! altgo CLI 入口。
//!
//! 纯命令行模式运行语音转文字管道。

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use altgo::config;
use altgo::key_listener;
use altgo::output;
use altgo::pipeline;
use altgo::polisher;
use altgo::recorder;
use altgo::state_machine;
use altgo::transcriber;
use clap::Parser;

#[derive(Parser)]
#[command(name = "altgo", about = "无需打字，言出法随 — 跨平台语音转文字工具")]
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
    cfg.as_ref().validate()?;

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cfg.logging.level));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    tracing::info!("altgo starting");

    let mut listener = key_listener::PlatformListener::new(&cfg.key_listener.key_name)?;
    #[cfg(target_os = "windows")]
    listener.set_poll_interval_ms(cfg.key_listener.poll_interval_ms);

    let mut recorder =
        recorder::PlatformRecorder::new(cfg.recorder.sample_rate, cfg.recorder.channels);

    let transcriber: transcriber::Transcriber = match cfg.transcriber.engine.as_str() {
        "local" => {
            tracing::info!(
                model = %cfg.transcriber.model,
                "using local whisper for transcription"
            );
            transcriber::Transcriber::Local(transcriber::LocalWhisper::new(
                cfg.transcriber.model.clone(),
                cfg.transcriber.language.clone(),
                cfg.transcriber.whisper_path.clone(),
            ))
        }
        engine => {
            tracing::info!(engine = engine, "using Whisper API for transcription");
            transcriber::Transcriber::Api(transcriber::WhisperApi::new(
                cfg.transcriber.api_key.clone(),
                cfg.transcriber.api_base_url.clone(),
                cfg.transcriber.model.clone(),
                cfg.transcriber.language.clone(),
                cfg.transcriber.temperature,
                cfg.transcriber.prompt.clone(),
                cfg.transcriber.timeout(),
            )?)
        }
    };

    let polish_level = polisher::PolishLevel::from_str(&cfg.polisher.level).unwrap_or_else(|_| {
        tracing::warn!(
            level = %cfg.polisher.level,
            "invalid polish level in config, using 'medium'"
        );
        polisher::PolishLevel::Medium
    });
    let polisher_protocol =
        polisher::ApiProtocol::from_str(&cfg.polisher.protocol).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "invalid polisher protocol, defaulting to openai");
            polisher::ApiProtocol::OpenAi
        });
    let formatter = polisher::LLMFormatter::with_config(
        cfg.polisher.api_key.clone(),
        cfg.polisher.api_base_url.clone(),
        cfg.polisher.model.clone(),
        cfg.polisher.timeout(),
        cfg.polisher.max_tokens,
        polisher_protocol,
        cfg.polisher.temperature,
        cfg.transcriber.language.clone(),
        cfg.polisher.system_prompt.clone(),
    )?;

    let key_events = listener.start()?;

    let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();
    let debounce_window = cfg.key_listener.debounce_window();
    tokio::spawn(key_listener::debounce_task(
        key_events,
        key_tx,
        debounce_window,
    ));

    let sm = state_machine::Machine::new(
        cfg.key_listener.long_press_threshold(),
        cfg.key_listener.double_click_interval(),
        cfg.key_listener.min_press_duration(),
    );
    let mut commands = sm.run(key_rx);

    tracing::info!("altgo initialized — waiting for trigger key");

    const MAX_RESTARTS: u32 = 3;
    const RESTART_COOLDOWN: Duration = Duration::from_secs(1);
    let mut restart_count = 0u32;

    'outer: loop {
        while let Some(cmd) = commands.recv().await {
            match cmd {
                state_machine::Command::StartRecord => {
                    tracing::info!("recording started");
                    let _ = output::show_recording_window();
                    if let Err(e) = recorder.start() {
                        tracing::error!(error = %e, "failed to start recording");
                    }
                }
                state_machine::Command::StopRecord => {
                    tracing::info!("recording stopped, processing...");
                    let _ = output::close_recording_window();
                    let wav_data = match recorder.stop() {
                        Ok(data) => data,
                        Err(e) => {
                            tracing::error!(error = %e, "failed to stop recording");
                            continue;
                        }
                    };

                    let cfg = Arc::clone(&cfg);
                    let formatter = formatter.clone();
                    let transcriber = transcriber.clone();

                    tokio::spawn(async move {
                        if let Err(e) =
                            process_audio(&cfg, &transcriber, &formatter, &wav_data, polish_level)
                                .await
                        {
                            tracing::error!(error = %e, "audio processing failed");
                        }
                    });
                }
            }
        }

        restart_count += 1;
        if restart_count > MAX_RESTARTS {
            tracing::error!(
                restart_count,
                "state machine exited too many times, giving up"
            );
            break 'outer;
        }

        tracing::warn!(
            restart_count,
            max_restarts = MAX_RESTARTS,
            "state machine exited, attempting restart after cooldown"
        );
        tokio::time::sleep(RESTART_COOLDOWN).await;

        listener.stop();
        let _ = recorder.stop();

        let key_events = match listener.start() {
            Ok(rx) => rx,
            Err(e) => {
                tracing::error!(error = %e, "failed to restart key listener");
                break 'outer;
            }
        };
        let (new_key_tx, new_key_rx) = tokio::sync::mpsc::unbounded_channel();
        let debounce_window = cfg.key_listener.debounce_window();
        tokio::spawn(key_listener::debounce_task(
            key_events,
            new_key_tx,
            debounce_window,
        ));

        let sm = state_machine::Machine::new(
            cfg.key_listener.long_press_threshold(),
            cfg.key_listener.double_click_interval(),
            cfg.key_listener.min_press_duration(),
        );
        commands = sm.run(new_key_rx);

        tracing::info!("state machine restarted — waiting for trigger key");
    }

    listener.stop();
    tracing::info!("altgo stopped");
    Ok(())
}

async fn process_audio(
    cfg: &Arc<config::Config>,
    transcriber: &transcriber::Transcriber,
    formatter: &polisher::LLMFormatter,
    wav_data: &[u8],
    polish_level: polisher::PolishLevel,
) -> anyhow::Result<()> {
    let output = pipeline::process_audio_core(transcriber, formatter, wav_data, polish_level)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "transcription failed");
            e
        })?;

    if output.text.is_empty() {
        return Ok(());
    }

    if output.polish_failed && cfg.output.enable_notify {
        let _ = output::notify(
            "altgo",
            "润色失败，已使用原始文本",
            cfg.output.notify_timeout_ms,
        );
    }

    let reason = output::output_text(
        &output.raw_text,
        &output.text,
        output.polish_failed,
        cfg.output.inject_at_cursor,
        cfg.output.prefer_polished,
        cfg.output.notify_timeout_ms,
    )
    .await
    .map_err(|e| anyhow::anyhow!("output failed: {}", e))?;

    tracing::info!(reason = reason, "done — text output");
    Ok(())
}
