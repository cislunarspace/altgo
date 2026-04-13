//! altgo 入口模块。
//!
//! 负责 CLI 参数解析（clap）、初始化各子模块，并运行主事件循环。
//! 整个程序是一个线性管道：
//!
//! ```text
//! 按键监听 → 状态机 → 录音 → 语音识别 → 文本润色 → 输出（剪切板 + 通知）
//! ```
//!
//! 主事件循环从状态机接收 `Command`，按需启动/停止录音，
//! 并将录音数据交给 `process_audio` 进行异步处理（转写 → 润色 → 复制到剪切板）。

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
use std::time::Duration;

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

/// 语音识别后端的包装枚举，支持 API 和本地两种引擎。
#[derive(Clone)]
enum Transcriber {
    Api(transcriber::WhisperApi),
    Local(transcriber::LocalWhisper),
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
    let mut listener = key_listener::PlatformListener::new(&cfg.key_listener.key_name)?;

    // Initialize recorder.
    let mut recorder =
        recorder::PlatformRecorder::new(cfg.recorder.sample_rate, cfg.recorder.channels);

    // Initialize transcriber.
    let transcriber = match cfg.transcriber.engine.as_str() {
        "local" => {
            tracing::info!(
                model = %cfg.transcriber.model,
                "using local whisper for transcription"
            );
            Transcriber::Local(transcriber::LocalWhisper::new(
                cfg.transcriber.model.clone(),
                cfg.transcriber.language.clone(),
            ))
        }
        engine => {
            tracing::info!(engine = engine, "using Whisper API for transcription");
            Transcriber::Api(transcriber::WhisperApi::new(
                cfg.transcriber.api_key.clone(),
                cfg.transcriber.api_base_url.clone(),
                cfg.transcriber.model.clone(),
                cfg.transcriber.language.clone(),
                cfg.transcriber.timeout(),
            ))
        }
    };

    // Initialize polisher.
    let polish_level = polisher::PolishLevel::from_str(&cfg.polisher.level)
        .unwrap_or(polisher::PolishLevel::Medium);
    let formatter = polisher::LLMFormatter::with_max_tokens(
        cfg.polisher.api_key.clone(),
        cfg.polisher.api_base_url.clone(),
        cfg.polisher.model.clone(),
        cfg.polisher.timeout(),
        cfg.polisher.max_tokens,
    );

    // Start key listener.
    let key_events = listener.start()?;

    // Convert key events for the state machine with debounce.
    //
    // On Windows with Chinese IME, Right Alt (VK_RMENU) can oscillate rapidly
    // between pressed/released due to AltGr handling.  A release event that is
    // immediately followed by a press within `debounce_window` is suppressed,
    // preventing the state machine from seeing a spurious press-release-press
    // sequence that would trigger continuous recording instead of long-press.
    let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();
    let debounce_window = cfg.key_listener.debounce_window();
    tokio::spawn(debounce_task(key_events, key_tx, debounce_window));

    // Start state machine.
    let sm = state_machine::Machine::new(
        cfg.key_listener.long_press_threshold(),
        cfg.key_listener.double_click_interval(),
    );
    let mut commands = sm.run(key_rx);

    tracing::info!("altgo initialized — waiting for trigger key");

    // Main event loop with restart support.
    //
    // If the state machine exits (e.g., key listener subprocess crashed,
    // channel dropped), we restart it up to MAX_RESTARTS times with a cooldown.
    // This makes the program resilient to transient failures.
    const MAX_RESTARTS: u32 = 3;
    const RESTART_COOLDOWN: Duration = Duration::from_secs(1);
    let mut restart_count = 0u32;

    'outer: loop {
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

        // State machine exited (key event channel closed or command receiver dropped).
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

        // Clean up before restart: stop the old key listener subprocess
        // and reset the recorder in case it was mid-recording.
        listener.stop();
        let _ = recorder.stop();

        // Restart key listener and state machine.
        let key_events = match listener.start() {
            Ok(rx) => rx,
            Err(e) => {
                tracing::error!(error = %e, "failed to restart key listener");
                break 'outer;
            }
        };
        let (new_key_tx, new_key_rx) = tokio::sync::mpsc::unbounded_channel();
        let debounce_window = cfg.key_listener.debounce_window();
        tokio::spawn(debounce_task(key_events, new_key_tx, debounce_window));

        let sm = state_machine::Machine::new(
            cfg.key_listener.long_press_threshold(),
            cfg.key_listener.double_click_interval(),
        );
        commands = sm.run(new_key_rx);

        tracing::info!("state machine restarted — waiting for trigger key");
    }

    listener.stop();
    tracing::info!("altgo stopped");
    Ok(())
}

/// 防抖任务：过滤 IME 引起的按键抖动，将稳定的按键事件转发给状态机。
async fn debounce_task(
    mut raw_events: tokio::sync::mpsc::UnboundedReceiver<key_listener::KeyEvent>,
    key_tx: tokio::sync::mpsc::UnboundedSender<state_machine::KeyEvent>,
    debounce_window: Duration,
) {
    let mut is_pressed = false;
    let mut pending_release: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;

    loop {
        tokio::select! {
            evt = raw_events.recv() => {
                match evt {
                    Some(evt) if evt.pressed => {
                        // Press cancels any pending release.
                        pending_release = None;
                        if !is_pressed {
                            is_pressed = true;
                            if key_tx
                                .send(state_machine::KeyEvent { pressed: true })
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    Some(_) => {
                        // Release — start debounce timer if not already pending.
                        if is_pressed && pending_release.is_none() {
                            pending_release =
                                Some(Box::pin(tokio::time::sleep(debounce_window)));
                        }
                    }
                    None => break,
                }
            }
            // Debounce timer fired — forward the release to the state machine.
            _ = async {
                if let Some(timer) = &mut pending_release {
                    timer.as_mut().await;
                } else {
                    std::future::pending::<()>().await;
                }
            }, if pending_release.is_some() => {
                pending_release = None;
                is_pressed = false;
                if key_tx
                    .send(state_machine::KeyEvent { pressed: false })
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

/// 语音处理管道：语音识别 → 文本润色 → 输出到剪切板。
async fn process_audio(
    cfg: &Arc<config::Config>,
    transcriber: &Transcriber,
    formatter: &polisher::LLMFormatter,
    wav_data: &[u8],
    polish_level: polisher::PolishLevel,
) -> anyhow::Result<()> {
    // Step 1: Transcribe.
    let result = match transcriber {
        Transcriber::Local(lw) => lw.transcribe(wav_data).await,
        Transcriber::Api(api) => api.transcribe(wav_data).await,
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
        let _ = output::notify(
            "altgo",
            "润色失败，已使用原始文本",
            cfg.output.notify_timeout_ms,
        );
    }

    tracing::info!(text = %polished, "polished");

    // Step 3: Output.
    if let Err(e) = output::write_clipboard(&polished).await {
        tracing::error!(error = %e, "clipboard write failed");
    }

    if cfg.output.enable_notify {
        let _ = output::notify_result(&polished, cfg.output.notify_timeout_ms);
    }

    tracing::info!("done — text copied to clipboard");
    Ok(())
}
