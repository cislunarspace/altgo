//! 管道上下文 — 拥有管道运行期间的全部组件。
//!
//! `PipelineContext` 在 `run()` 期间存在，由 `PipelineBuilder::build_context()` 构造。
//! 将组件所有权集中在一处，使集成测试可以构造部分模拟组件。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::key_listener::{KeyListenerConfig, X11Listener};
use crate::pipeline_command_handler::{handle_start_record, handle_stop_record};
use crate::pipeline_sink::PipelineSink;
use crate::polisher::{LLMFormatter, PolishLevel};
use crate::recorder::PulseRecorder;
use crate::state_machine::{Command, Machine};
use crate::transcriber::Transcriber;

/// 管道上下文 — 拥有管道运行期间所需的全部组件。
pub struct PipelineContext {
    pub recorder: PulseRecorder,
    pub transcriber: Transcriber,
    pub formatter: LLMFormatter,
    pub polish_level: PolishLevel,
    pub poll_running: Arc<AtomicBool>,
    pub key_listener_config: KeyListenerConfig,
    pub poll_interval_ms: u64,
    pub(crate) listener: Mutex<Option<X11Listener>>,
}

impl PipelineContext {
    /// 运行管道事件循环。
    ///
    /// 阻塞当前异步任务直到收到 `stop_rx` 信号。
    /// 所有状态变化和处理结果通过 `sink` 报告。
    pub async fn run(
        mut self,
        stop_rx: tokio::sync::oneshot::Receiver<()>,
        sink: impl PipelineSink,
    ) {
        // Extract fields we need from self before the async loop borrows self mutably
        let poll_running = self.poll_running.clone();
        let poll_interval_ms = self.poll_interval_ms;
        let key_listener_config = self.key_listener_config.clone();

        let mut listener = match self.listener.lock().unwrap().take() {
            Some(l) => l,
            None => {
                sink.on_error("pipeline context already used");
                return;
            }
        };

        let (mut key_events, key_backend) = match listener.start() {
            Ok(pair) => pair,
            Err(e) => {
                sink.on_error(&format!("key listener start: {}", e));
                return;
            }
        };
        tracing::info!(backend = key_backend, "Linux key listener active");
        sink.on_key_listener_backend(key_backend);

        let (raw_key_tx, raw_key_rx) = tokio::sync::mpsc::unbounded_channel();
        let poll_running_for_thread = poll_running.clone();

        // Bridge key_events (tokio receiver) to raw_key_rx
        std::thread::spawn(move || {
            use tokio::sync::mpsc::error::TryRecvError;
            while poll_running_for_thread.load(Ordering::SeqCst) {
                match key_events.try_recv() {
                    Ok(ev) => {
                        if raw_key_tx.send(ev).is_err() {
                            break;
                        }
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

        // Debounce task: raw_key_rx -> key_rx -> state machine
        let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(crate::key_listener::debounce_task(
            raw_key_rx,
            key_tx.clone(),
        ));

        let sm = Machine::new(
            key_listener_config.long_press_threshold,
            key_listener_config.double_click_interval,
            key_listener_config.min_press_duration,
        );
        let mut commands = sm.run(key_rx);

        sink.on_status_change("idle");

        // Use a named variable so we can borrow it mutably in the select
        let mut stop_rx = stop_rx;
        loop {
            tokio::select! {
                cmd = commands.recv() => {
                    match cmd {
                        Some(Command::StartRecord) => {
                            let _ = handle_start_record(&mut self.recorder, &sink);
                        }
                        Some(Command::StopRecord) => {
                            handle_stop_record(
                                &mut self.recorder,
                                &self.transcriber,
                                &self.formatter,
                                self.polish_level,
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
}
