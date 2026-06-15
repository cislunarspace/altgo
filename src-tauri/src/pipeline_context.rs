//! 管道上下文 — 拥有管道运行期间的全部组件。
//!
//! `PipelineContext` 在 `run()` 期间存在，由 `PipelineBuilder::build_context()` 构造。
//! 将组件所有权集中在一处，使集成测试可以构造部分模拟组件。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::key_listener::{KeyListener, KeyListenerConfig};
use crate::pipeline_command_handler::{handle_start_record, handle_stop_record};
use crate::pipeline_sink::PipelineSink;
use crate::polisher::{LLMFormatter, PolishLevel};
use crate::recorder::PlatformRecorder;
use crate::state_machine::{Command, Machine};
use crate::transcriber::Transcriber;

/// 管道上下文 — 拥有管道运行期间所需的全部组件。
pub struct PipelineContext {
    pub recorder: PlatformRecorder,
    pub transcriber: Transcriber,
    pub formatter: LLMFormatter,
    pub polish_level: PolishLevel,
    pub poll_running: Arc<AtomicBool>,
    pub key_listener_config: KeyListenerConfig,
    pub poll_interval_ms: u64,
    pub(crate) listener: Mutex<Option<Box<dyn KeyListener>>>,
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

        let mut listener: Box<dyn KeyListener> = match self.listener.lock().unwrap().take() {
            Some(l) => l,
            None => {
                sink.on_error("pipeline context already used");
                return;
            }
        };

        let (mut key_events, key_backend): (
            tokio::sync::mpsc::UnboundedReceiver<crate::key_listener::KeyEvent>,
            &'static str,
        ) = match listener.start() {
            Ok(pair) => pair,
            Err(e) => {
                sink.on_error(&format!("key listener start: {}", e));
                return;
            }
        };
        tracing::info!(backend = key_backend, "key listener active");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polisher::PolisherConfig;
    use tokio::sync::mpsc;

    struct FakeListener {
        backend: &'static str,
    }

    impl KeyListener for FakeListener {
        fn start(
            &mut self,
        ) -> anyhow::Result<(
            mpsc::UnboundedReceiver<crate::key_listener::KeyEvent>,
            &'static str,
        )> {
            let (_, rx) = mpsc::unbounded_channel();
            Ok((rx, self.backend))
        }
    }

    fn test_polisher_config() -> PolisherConfig {
        PolisherConfig {
            api_key: "test-key".to_string(),
            api_base_url: "http://localhost".to_string(),
            model: "test-model".to_string(),
            protocol: "openai".to_string(),
            max_tokens: 256,
            temperature: 0.0,
            system_prompt: String::new(),
            timeout: std::time::Duration::from_secs(10),
            level: "none".to_string(),
            language: "en".to_string(),
        }
    }

    #[test]
    fn pipeline_context_accepts_boxed_key_listener() {
        let fake: Box<dyn KeyListener> = Box::new(FakeListener {
            backend: "test-fake",
        });
        let ctx = PipelineContext {
            recorder: PlatformRecorder::new(16000, 1),
            transcriber: crate::transcriber::Transcriber::Api(
                crate::transcriber::WhisperApi::new(
                    "test-key".to_string(),
                    "http://localhost".to_string(),
                    "test-model".to_string(),
                    "en".to_string(),
                    0.0,
                    String::new(),
                    std::time::Duration::from_secs(10),
                )
                .unwrap(),
            ),
            formatter: LLMFormatter::from_config(&test_polisher_config()).unwrap(),
            polish_level: PolishLevel::None,
            poll_running: Arc::new(AtomicBool::new(true)),
            key_listener_config: KeyListenerConfig {
                key_name: "Alt_R".to_string(),
                linux_evdev_code: None,
                windows_vk: None,
                long_press_threshold: std::time::Duration::from_millis(400),
                double_click_interval: std::time::Duration::from_millis(200),
                debounce_window: std::time::Duration::from_millis(30),
                poll_interval_ms: 10,
                min_press_duration: std::time::Duration::from_millis(80),
            },
            poll_interval_ms: 10,
            listener: Mutex::new(Some(fake)),
        };

        // 从 ctx 取出 listener 并验证类型
        let mut taken = ctx.listener.lock().unwrap().take().unwrap();
        assert_eq!(taken.start().unwrap().1, "test-fake");
    }

    #[test]
    fn pipeline_context_run_returns_early_when_listener_already_taken() {
        use crate::pipeline::PipelineOutput;

        struct MockSink;
        impl PipelineSink for MockSink {
            fn on_status_change(&self, _: &str) {}
            fn on_error(&self, _: &str) {}
            fn on_transcription_result(&self, _: &PipelineOutput) {}
            fn on_progress(&self, _: &str, _: Option<f32>) {}
            fn on_key_listener_backend(&self, _: &str) {}
        }

        let ctx = PipelineContext {
            recorder: PlatformRecorder::new(16000, 1),
            transcriber: crate::transcriber::Transcriber::Api(
                crate::transcriber::WhisperApi::new(
                    "test-key".to_string(),
                    "http://localhost".to_string(),
                    "test-model".to_string(),
                    "en".to_string(),
                    0.0,
                    String::new(),
                    std::time::Duration::from_secs(10),
                )
                .unwrap(),
            ),
            formatter: LLMFormatter::from_config(&test_polisher_config()).unwrap(),
            polish_level: PolishLevel::None,
            poll_running: Arc::new(AtomicBool::new(true)),
            key_listener_config: KeyListenerConfig {
                key_name: "Alt_R".to_string(),
                linux_evdev_code: None,
                windows_vk: None,
                long_press_threshold: std::time::Duration::from_millis(400),
                double_click_interval: std::time::Duration::from_millis(200),
                debounce_window: std::time::Duration::from_millis(30),
                poll_interval_ms: 10,
                min_press_duration: std::time::Duration::from_millis(80),
            },
            poll_interval_ms: 10,
            listener: Mutex::new(None), // 已被取走
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        drop(stop_tx);

        // run() 应立即返回（listener 为 None 时 sink.on_error 被调用后 return）
        rt.block_on(ctx.run(stop_rx, MockSink));
    }
}
