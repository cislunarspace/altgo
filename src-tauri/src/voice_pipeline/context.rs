//! PipelineContext — 拥有所有组件并运行事件循环。

use std::sync::{Arc, Mutex};

use crate::key_listener::KeyListener;
use crate::polisher::{LLMFormatter, PolishLevel};
use crate::recorder::Recorder;
use crate::state_machine::{Command, Machine};
use crate::transcriber::Transcriber;

use super::handlers::{handle_start_record, handle_stop_record};
use super::sink::PipelineSink;
use crate::pipeline_controller::PipelineStatus;

/// Owns all components needed while the pipeline runs.
pub struct PipelineContext {
    pub(crate) recorder: Box<dyn Recorder>,
    pub(crate) transcriber: Box<dyn Transcriber>,
    pub(crate) formatter: LLMFormatter,
    pub(crate) polish_level: PolishLevel,
    pub(crate) listener: Mutex<Option<Box<dyn KeyListener>>>,
    // 状态机参数
    pub(crate) long_press_threshold: std::time::Duration,
    pub(crate) double_click_interval: std::time::Duration,
    pub(crate) min_press_duration: std::time::Duration,
}

impl PipelineContext {
    /// Run the pipeline event loop until `stop_rx` fires.
    pub async fn run(self, stop_rx: tokio::sync::oneshot::Receiver<()>, sink: impl PipelineSink) {
        let mut recorder = self.recorder;
        let transcriber = self.transcriber;
        let formatter = self.formatter;
        let polish_level = self.polish_level;
        // Wrap the sink in an Arc so handlers can keep using it after the
        // async task lifecycle (and so progress forwarders can hold it).
        let sink: Arc<dyn PipelineSink> = Arc::new(sink);

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

        // 创建状态机，直接集成到主循环
        let mut machine = Machine::new(
            self.long_press_threshold,
            self.double_click_interval,
            self.min_press_duration,
        );
        let mut deadline: Option<tokio::time::Instant> = None;

        sink.on_status_change(PipelineStatus::Idle);

        let mut stop_rx = stop_rx;
        loop {
            let cmd = tokio::select! {
                // 按键事件
                event = key_events.recv() => match event {
                    Some(ev) => machine.process(ev),
                    None => {
                        tracing::warn!("key event channel closed, stopping pipeline");
                        break;
                    }
                },
                // 超时事件
                _ = async { tokio::time::sleep_until(deadline.unwrap()).await }, if deadline.is_some() => {
                    machine.poll_timeout()
                }
                // 停止信号
                _ = &mut stop_rx => {
                    tracing::info!("pipeline stop requested");
                    break;
                }
            };

            if let Some(cmd) = cmd {
                match cmd {
                    Command::StartRecord => {
                        let _ = handle_start_record(&mut *recorder, &*sink);
                    }
                    Command::StopRecord => {
                        handle_stop_record(
                            &mut *recorder,
                            &*transcriber,
                            &formatter,
                            polish_level,
                            sink.clone(),
                        )
                        .await;
                    }
                }
            }
            deadline = machine.next_deadline().map(|d| d.into());
        }

        sink.on_status_change(PipelineStatus::Stopped);
        tracing::info!("pipeline stopped");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_listener::KeyListener;
    use crate::polisher::{LLMFormatter, PolishLevel};
    use crate::recorder::PlatformRecorder;

    fn test_polisher_config() -> crate::config::PolisherConfig {
        crate::config::PolisherConfig {
            api_key: "test-key".to_string(),
            api_base_url: "http://localhost".to_string(),
            model: "test-model".to_string(),
            protocol: "openai".to_string(),
            max_tokens: 256,
            temperature: 0.0,
            system_prompt: String::new(),
            timeout: std::time::Duration::from_secs(10),
            level: "none".to_string(),
        }
    }

    fn make_context(listener: Option<Box<dyn KeyListener>>) -> PipelineContext {
        PipelineContext {
            recorder: Box::new(PlatformRecorder::new(16000)),
            transcriber: Box::new(
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
            formatter: LLMFormatter::from_config(&test_polisher_config(), "en").unwrap(),
            polish_level: PolishLevel::None,
            listener: Mutex::new(listener),
            long_press_threshold: std::time::Duration::from_millis(400),
            double_click_interval: std::time::Duration::from_millis(200),
            min_press_duration: std::time::Duration::from_millis(80),
        }
    }

    #[test]
    fn pipeline_context_accepts_boxed_key_listener() {
        let fake: Box<dyn KeyListener> = Box::new(super::super::test_doubles::FakeListener {
            backend: "test-fake",
        });
        let ctx = make_context(Some(fake));
        let mut taken = ctx.listener.lock().unwrap().take().unwrap();
        assert_eq!(taken.start().unwrap().1, "test-fake");
    }

    #[test]
    fn pipeline_context_run_returns_early_when_listener_already_taken() {
        struct MockSink;
        impl super::super::sink::PipelineSink for MockSink {
            fn on_status_change(&self, _: crate::pipeline_controller::PipelineStatus) {}
            fn on_error(&self, _: &str) {}
            fn on_transcription_result(&self, _: &super::super::sink::PipelineOutput) {}
            fn on_progress(&self, _: &str, _: Option<f32>) {}
            fn on_key_listener_backend(&self, _: &str) {}
        }

        let ctx = make_context(None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        drop(stop_tx);
        rt.block_on(ctx.run(stop_rx, MockSink));
    }
}
