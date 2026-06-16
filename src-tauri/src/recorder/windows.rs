//! Windows 录音器。
//!
//! 使用 cpal（WASAPI 后端）从默认输入设备捕获音频：回调里把采样转成 i16、
//! 降混到单声道后写入共享 `Buffer`；`stop_recording` 时若设备采样率 != 16kHz，
//! 再用 rubato 重采样到 16kHz，最后编码为 WAV。
//!
//! 设计要点（见 issue #20 设计澄清）：
//! - cpal `Stream` 存在 `Mutex<Option<Stream>>`，start 时放入、stop 时 take 出（drop 即停采集）。
//! - 回调保持轻量：只做格式转换 + 降混，不在实时音频线程上做重采样。
//! - 16kHz/i16 优先；设备不支持时用默认配置并在停止时批量重采样。
//! - 输出始终为单声道（回调内 downmix），满足 whisper 输入要求。
//! - 错误路径（无设备、流构建/启动失败）通过 `anyhow` 向上传播；
//!   这些 cpal 设备交互路径无法在无 Windows 音频硬件的 CI 上触发，靠 Windows 手动验证（路线 B 选项 3）。

use crate::audio::{self, Buffer};
use crate::recorder::{dsp, Recorder};
use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// cpal（WASAPI）录音器，捕获音频并产出 16kHz 单声道 WAV。
pub struct WindowsRecorder {
    sample_rate: u32,
    /// 构造时传入的目标声道数；输出始终为单声道（回调内 downmix），此字段仅作记录。
    #[allow(dead_code)]
    channels: u32,
    shared_buffer: Arc<Buffer>,
    recording: Arc<AtomicBool>,
    stream: Mutex<Option<cpal::Stream>>,
    /// 实际设备采样率；start 时写入、stop 时读取，决定是否需要重采样。
    /// `start_recording` 拿 `&mut self`、`stop_recording` 拿 `&self`，同线程顺序调用，
    /// 普通字段即可，无需内部可变性。
    device_sample_rate: u32,
}

impl WindowsRecorder {
    /// 创建新的录音器。`sample_rate`/`channels` 为目标值（默认 16000 / 1）。
    pub fn new(sample_rate: u32, channels: u32) -> Self {
        Self {
            sample_rate,
            channels,
            shared_buffer: Arc::new(Buffer::new()),
            recording: Arc::new(AtomicBool::new(false)),
            stream: Mutex::new(None),
            device_sample_rate: sample_rate,
        }
    }

    /// 选择输入配置：优先 16kHz/i16（零重采样最优路径），否则用设备默认配置。
    /// 返回 `(StreamConfig, SampleFormat, 实际采样率, 声道数)`。
    fn select_input_config(
        &self,
        device: &cpal::Device,
    ) -> Result<(
        cpal::StreamConfig,
        cpal::SampleFormat,
        u32,
        cpal::ChannelCount,
    )> {
        let target = self.sample_rate;

        if let Ok(ranges) = device.supported_input_configs() {
            for range in ranges {
                if range.sample_format() == cpal::SampleFormat::I16
                    && range.min_sample_rate() <= target
                    && range.max_sample_rate() >= target
                {
                    let channels = range.channels();
                    let config = cpal::StreamConfig {
                        channels,
                        sample_rate: target,
                        buffer_size: cpal::BufferSize::Default,
                    };
                    tracing::debug!(
                        "selected 16kHz/i16 input config ({} ch, zero resample)",
                        channels
                    );
                    return Ok((config, cpal::SampleFormat::I16, target, channels));
                }
            }
        }

        let default = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("no supported input config: {e}"))?;
        let fmt = default.sample_format();
        let rate = default.sample_rate();
        let channels = default.channels();
        let config = cpal::StreamConfig {
            channels,
            sample_rate: rate,
            buffer_size: cpal::BufferSize::Default,
        };
        tracing::debug!(
            "falling back to default input config: {}Hz / {:?} / {}ch (will resample to {}Hz)",
            rate,
            fmt,
            channels,
            target
        );
        Ok((config, fmt, rate, channels))
    }

    /// 开始录音。
    pub fn start(&mut self) -> Result<()> {
        if self.recording.load(Ordering::SeqCst) {
            anyhow::bail!("already recording");
        }
        self.shared_buffer.reset();

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("no input device available"))?;

        let (config, sample_format, dev_rate, dev_channels) = self.select_input_config(&device)?;
        let channels = dev_channels as usize;

        // 各分支返回 `Result<Stream, anyhow::Error>`；不支持的格式用 bail（散布分支 `!`）。
        let stream: cpal::Stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let buffer = Arc::clone(&self.shared_buffer);
                let recording = Arc::clone(&self.recording);
                device
                    .build_input_stream::<f32, _, _>(
                        &config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            let i16_samples = dsp::f32_to_i16(data);
                            write_mono_pcm(&i16_samples, channels, &buffer, &recording);
                        },
                        stream_error_callback,
                        None,
                    )
                    .map_err(build_stream_err)?
            }
            cpal::SampleFormat::I16 => {
                let buffer = Arc::clone(&self.shared_buffer);
                let recording = Arc::clone(&self.recording);
                device
                    .build_input_stream::<i16, _, _>(
                        &config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            write_mono_pcm(data, channels, &buffer, &recording);
                        },
                        stream_error_callback,
                        None,
                    )
                    .map_err(build_stream_err)?
            }
            cpal::SampleFormat::U16 => {
                let buffer = Arc::clone(&self.shared_buffer);
                let recording = Arc::clone(&self.recording);
                device
                    .build_input_stream::<u16, _, _>(
                        &config,
                        move |data: &[u16], _: &cpal::InputCallbackInfo| {
                            // u16 以 32768 为零点，减去后映射到 i16。
                            let i16_samples: Vec<i16> = data
                                .iter()
                                .map(|&u| u as i32 - 32768)
                                .map(|v| v as i16)
                                .collect();
                            write_mono_pcm(&i16_samples, channels, &buffer, &recording);
                        },
                        stream_error_callback,
                        None,
                    )
                    .map_err(build_stream_err)?
            }
            other => anyhow::bail!("unsupported sample format: {:?}", other),
        };

        stream
            .play()
            .map_err(|e| anyhow::anyhow!("failed to start input stream: {e}"))?;

        *self.stream.lock().unwrap_or_else(|p| p.into_inner()) = Some(stream);
        self.device_sample_rate = dev_rate;
        self.recording.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// 停止录音并返回 16kHz 单声道 WAV。
    pub fn stop(&self) -> Result<Vec<u8>> {
        self.recording.store(false, Ordering::SeqCst);

        // 取出并 drop stream，cpal 保证 drop 后不再触发回调。
        if let Some(stream) = self.stream.lock().unwrap_or_else(|p| p.into_inner()).take() {
            drop(stream);
        }

        let pcm_bytes = self.shared_buffer.read_all();
        if pcm_bytes.is_empty() {
            anyhow::bail!("no audio data recorded");
        }

        if self.device_sample_rate == self.sample_rate {
            // 设备已是目标采样率：buffer 里就是 16kHz mono i16 LE，直接编码。
            audio::encode_wav(&pcm_bytes, self.sample_rate, 1, 16)
        } else {
            // 否则：还原 i16 → 重采样到 16kHz → 再编码字节 → WAV。
            let samples = dsp::i16_from_le_bytes(&pcm_bytes);
            let resampled = dsp::resample(&samples, self.device_sample_rate, self.sample_rate)?;
            let resampled_bytes = dsp::i16_to_le_bytes(&resampled);
            audio::encode_wav(&resampled_bytes, self.sample_rate, 1, 16)
        }
    }
}

/// 把 i16 采样降混到单声道并以 LE 字节写入 `Buffer`（回调里调用）。
fn write_mono_pcm(samples: &[i16], channels: usize, buffer: &Buffer, recording: &AtomicBool) {
    if !recording.load(Ordering::SeqCst) {
        return;
    }
    let mono = dsp::downmix_to_mono(samples, channels);
    buffer.write(&dsp::i16_to_le_bytes(&mono));
}

/// cpal 流错误回调（fn item，可在多个 match 分支复用而不被 move）。
fn stream_error_callback(err: cpal::StreamError) {
    tracing::error!(error = %err, "cpal input stream error");
}

/// `build_input_stream` 错误转换（fn item，多个分支复用）。
fn build_stream_err(e: cpal::BuildStreamError) -> anyhow::Error {
    anyhow::anyhow!("failed to build input stream: {e}")
}

impl Recorder for WindowsRecorder {
    fn start_recording(&mut self) -> Result<(), crate::error::RecorderError> {
        self.start().map_err(|e| match e.downcast::<crate::error::RecorderError>() {
            Ok(rec_err) => rec_err,
            Err(other) => crate::error::RecorderError::StartFailed(other.to_string()),
        })
    }

    fn stop_recording(&self) -> Result<Vec<u8>, crate::error::RecorderError> {
        self.stop().map_err(|e| match e.downcast::<crate::error::RecorderError>() {
            Ok(rec_err) => rec_err,
            Err(other) => crate::error::RecorderError::StopFailed(other.to_string()),
        })
    }

    fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_creation() {
        let recorder = WindowsRecorder::new(16000, 1);
        assert_eq!(recorder.sample_rate, 16000);
        assert!(!recorder.is_recording());
    }
}
