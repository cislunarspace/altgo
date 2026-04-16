//! altgo 核心库。
//!
//! 包含所有平台的语音转文字管道逻辑：
//! 按键监听 → 状态机 → 录音 → 语音识别 → 文本润色 → 输出

pub mod audio;
pub mod config;
pub mod key_listener;
pub mod model;
pub mod output;
pub mod pipeline;
pub mod polisher;
pub mod recorder;
pub mod resource;
pub mod state_machine;
pub mod transcriber;

pub use pipeline::PipelineOutput;
