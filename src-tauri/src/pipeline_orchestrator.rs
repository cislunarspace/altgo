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
    // --- Build components using PipelineBuilder ---
    let builder = crate::pipeline_builder::PipelineBuilder::new(cfg.clone());

    let mut recorder = builder.build_recorder();

    let transcriber = match builder.build_transcriber() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "failed to create transcriber");
            sink.on_error(&e.message("zh"));
            return;
        }
    };

    let formatter = match builder.build_polisher() {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "failed to create polisher");
            sink.on_error(&e.message("zh"));
            return;
        }
    };

    let polish_level = builder.polish_level();

    // --- 构建按键监听器 ---
    let (raw_key_tx, raw_key_rx) = tokio::sync::mpsc::unbounded_channel();
    let poll_running = Arc::new(AtomicBool::new(true));
    let key_listener_cfg = builder.key_listener_config();
    let poll_interval_ms = key_listener_cfg.poll_interval_ms;

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
    let debounce_window = key_listener_cfg.debounce_window;
    tokio::spawn(crate::key_listener::debounce_task(
        raw_key_rx,
        key_tx,
        debounce_window,
    ));

    let sm = crate::state_machine::Machine::new(
        key_listener_cfg.long_press_threshold,
        key_listener_cfg.double_click_interval,
        key_listener_cfg.min_press_duration,
    );
    let mut commands = sm.run(key_rx);

    sink.on_status_change("idle");

    // --- 事件循环 ---
    loop {
        tokio::select! {
            cmd = commands.recv() => {
                match cmd {
                    Some(crate::state_machine::Command::StartRecord) => {
                        let _ = crate::pipeline_command_handler::handle_start_record(
                            &mut recorder,
                            &sink,
                        );
                    }
                    Some(crate::state_machine::Command::StopRecord) => {
                        crate::pipeline_command_handler::handle_stop_record(
                            &mut recorder,
                            &transcriber,
                            &formatter,
                            polish_level,
                            &sink,
                        )
                        .await;
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
