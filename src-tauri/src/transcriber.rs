//! 语音识别模块。
//!
//! `Transcriber` trait 抽象本地语音识别后端；当前生产实现为
//! `SherpaTranscriber`（`crate::sherpa`）内嵌 sherpa-onnx 的 SenseVoice。
//!
//! 所有实现都返回 `TranscribeResult`（文本 + 语言信息），进度通过闭包回调上报。
//!
//! Transcription module.
//!
//! The `Transcriber` trait abstracts local speech-to-text backends; the current production
//! implementation is `SherpaTranscriber` (`crate::sherpa`) — local SenseVoice via embedded sherpa-onnx.
//!
//! All implementations return a `TranscribeResult` (text + language info); progress is reported
//! through a closure callback.

use crate::error::TranscriberError;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// 语音识别结果。
/// Speech recognition result.
#[derive(Debug)]
pub struct TranscribeResult {
    /// 识别出的文本
    /// Recognized text
    pub text: String,
    /// 检测到的语言代码
    /// Detected language code
    pub language: String,
}

/// 统一的转写后端 trait——`on_progress` 回调由调用方提供，trait 表面不携带
/// 通道类型，新后端接入无需改动 trait。
/// Unified transcription backend trait—the `on_progress` callback is supplied by the caller, so
/// the trait surface carries no channel types and adding new backends needs no trait changes.
pub trait Transcriber: Send + Sync {
    /// 转写 WAV 音频数据。
    ///
    /// `on_progress` 收到 0.0–1.0 的进度；无流式进度的后端成功时也应调用一次
    /// `1.0`，让 UI 收到最终一帧。
    /// Transcribe WAV audio data.
    ///
    /// `on_progress` receives progress in the 0.0–1.0 range; backends without streaming progress
    /// should still invoke it once with `1.0` on success, letting the UI receive a final frame.
    fn transcribe<'life0, 'life1>(
        &'life0 self,
        audio: &'life1 [u8],
        on_progress: Arc<dyn Fn(f32) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<TranscribeResult, TranscriberError>> + Send + 'life0>>
    where
        'life1: 'life0;
}
