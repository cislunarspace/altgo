//! 管道编排器。
//!
//! 负责语音转文字管道的事件循环：按键事件 → 状态机 → 录音 → 转写 → 润色。
//! 通过 `PipelineSink` trait 报告状态和结果，不依赖 Tauri 或具体的输出方式。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::pipeline_sink::PipelineSink;

/// 运行语音管道。
///
/// 阻塞当前异步任务直到收到 `stop_rx` 信号。
/// 所有状态变化和处理结果通过 `sink` 报告。
pub async fn run(
    cfg: Arc<crate::config::Config>,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    sink: impl PipelineSink,
) {
    // --- 构建录音器 ---
    let mut recorder =
        crate::recorder::PlatformRecorder::new(cfg.recorder.sample_rate, cfg.recorder.channels);

    // --- 构建转写器 ---
    let model_path = if cfg.transcriber.engine == "local" {
        match crate::model::resolve_model_path(&cfg.transcriber.model) {
            Some(p) => p.to_string_lossy().to_string(),
            None => {
                sink.on_error(&format!(
                    "本地模型未找到（配置值: {:?}）。请在 GUI 设置中下载模型，或将 [transcriber] model 设为已下载模型的名称（如 \"base\"）或完整文件路径。",
                    cfg.transcriber.model
                ));
                return;
            }
        }
    } else {
        cfg.transcriber.model.clone()
    };

    let transcriber: crate::transcriber::Transcriber = match cfg.transcriber.engine.as_str() {
        "local" => crate::transcriber::Transcriber::Local(crate::transcriber::LocalWhisper::new(
            model_path,
            cfg.transcriber.language.clone(),
            cfg.transcriber.whisper_path.clone(),
        )),
        _ => match crate::transcriber::WhisperApi::new(
            cfg.transcriber.api_key.clone(),
            cfg.transcriber.api_base_url.clone(),
            cfg.transcriber.model.clone(),
            cfg.transcriber.language.clone(),
            cfg.transcriber.temperature,
            cfg.transcriber.prompt.clone(),
            cfg.transcriber.timeout(),
        ) {
            Ok(api) => crate::transcriber::Transcriber::Api(api),
            Err(e) => {
                tracing::error!(error = %e, "failed to create transcriber");
                sink.on_error(&format!("transcriber: {}", e));
                return;
            }
        },
    };

    // --- 构建润色器 ---
    let polish_level = crate::polisher::PolishLevel::effective(&cfg.polisher.level);
    let mut formatter = match crate::polisher::LLMFormatter::try_from(&*cfg) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "failed to create polisher");
            sink.on_error(&format!("polisher: {}", e));
            return;
        }
    };

    // Try to load PromptStore from resources/prompts/
    let prompts_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("resources/prompts")))
        .or_else(|| Some(std::path::PathBuf::from("resources/prompts")));

    if let Some(dir) = prompts_dir {
        if dir.exists() {
            let store = crate::prompt_store::PromptStore::new(dir);
            if let Err(e) = store.ensure_loaded() {
                tracing::warn!(error = %e, "failed to load prompts from PromptStore, using fallback");
            } else {
                tracing::info!("PromptStore loaded successfully");
                formatter = formatter.with_prompt_store(store);
            }
        } else {
            tracing::debug!("prompts directory not found, using hardcoded prompts");
        }
    }

    // --- 构建按键监听器 ---
    let (raw_key_tx, raw_key_rx) = tokio::sync::mpsc::unbounded_channel();
    let poll_running = Arc::new(AtomicBool::new(true));
    let poll_interval_ms = cfg.key_listener.poll_interval_ms;

    let _linux_key_listener = {
        let mut listener = match crate::key_listener::PlatformListener::new(&cfg.key_listener) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(error = %e, "failed to create key listener");
                sink.on_error(&format!("key listener: {}", e));
                return;
            }
        };
        let (mut key_events, key_backend) = match listener.start() {
            Ok(pair) => pair,
            Err(e) => {
                tracing::error!(error = %e, "failed to start key listener");
                sink.on_error(&format!("key listener start: {}", e));
                return;
            }
        };
        tracing::info!(backend = key_backend, "Linux key listener active");
        sink.on_key_listener_backend(key_backend);

        let poll_running = poll_running.clone();
        std::thread::spawn(move || {
            use tokio::sync::mpsc::error::TryRecvError;
            while poll_running.load(Ordering::SeqCst) {
                match key_events.try_recv() {
                    Ok(ev) => {
                        let _ = raw_key_tx.send(ev);
                    }
                    Err(TryRecvError::Empty) => {
                        std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
                    }
                    Err(TryRecvError::Disconnected) => {
                        tracing::error!("key listener channel closed unexpectedly");
                        break;
                    }
                }
            }
        });
        listener
    };

    // --- 防抖 + 状态机 ---
    let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();
    let debounce_window = cfg.key_listener.debounce_window();
    tokio::spawn(crate::key_listener::debounce_task(
        raw_key_rx,
        key_tx,
        debounce_window,
    ));

    let sm = crate::state_machine::Machine::new(
        cfg.key_listener.long_press_threshold(),
        cfg.key_listener.double_click_interval(),
        cfg.key_listener.min_press_duration(),
    );
    let mut commands = sm.run(key_rx);

    sink.on_status_change("idle");

    // --- 事件循环 ---
    loop {
        tokio::select! {
            cmd = commands.recv() => {
                match cmd {
                    Some(crate::state_machine::Command::StartRecord) => {
                        tracing::info!("recording started");
                        if let Err(e) = recorder.start() {
                            tracing::error!(error = %e, "failed to start recording");
                            continue;
                        }
                        sink.on_status_change("recording");
                    }
                    Some(crate::state_machine::Command::StopRecord) => {
                        tracing::info!("recording stopped, processing...");
                        sink.on_status_change("processing");

                        let wav_data = match recorder.stop() {
                            Ok(data) => data,
                            Err(e) => {
                                tracing::error!(error = %e, "failed to stop recording");
                                sink.on_status_change("idle");
                                continue;
                            }
                        };

                        match crate::pipeline::process_audio_core(
                            &transcriber,
                            &formatter,
                            &wav_data,
                            polish_level,
                            |phase, fraction| {
                                sink.on_progress(phase, fraction);
                            },
                        )
                        .await
                        {
                            Ok(output) => {
                                sink.on_transcription_result(&output);
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "audio processing failed");
                                sink.on_error(&format!("processing: {}", e));
                            }
                        }
                    }
                    None => break,
                }
            }
            _ = &mut stop_rx => {
                tracing::info!("pipeline stop requested");
                poll_running.store(false, Ordering::SeqCst);
                break;
            }
        }
    }

    sink.on_status_change("stopped");
    tracing::info!("pipeline stopped");
}
