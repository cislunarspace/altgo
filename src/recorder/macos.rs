//! macOS 录音器。
//!
//! 优先使用 `sox`，如果不可用则回退到 `ffmpeg`（使用 avfoundation 输入）。
//! 在独立线程中捕获 16 位 PCM 音频（16kHz 单声道），
//! 通过共享的 `Buffer` 累积音频数据。

use crate::audio::{self, Buffer};
use anyhow::Result;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// macOS 录音器，优先使用 `sox`，不可用时回退到 `ffmpeg`。
pub struct SoxRecorder {
    sample_rate: u32,
    channels: u32,
    shared_buffer: Arc<Buffer>,
    recording: Arc<AtomicBool>,
    done: std::sync::Mutex<Option<JoinHandle<()>>>,
}

impl SoxRecorder {
    /// 创建新的录音器。
    pub fn new(sample_rate: u32, channels: u32) -> Self {
        Self {
            sample_rate,
            channels,
            shared_buffer: Arc::new(Buffer::new()),
            recording: Arc::new(AtomicBool::new(false)),
            done: std::sync::Mutex::new(None),
        }
    }

    /// 开始录音。
    pub fn start(&mut self) -> Result<()> {
        if self.recording.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("already recording"));
        }

        self.shared_buffer.reset();
        self.recording.store(true, Ordering::SeqCst);

        let sample_rate = self.sample_rate;
        let channels = self.channels;
        let buffer = Arc::clone(&self.shared_buffer);
        let recording = Arc::clone(&self.recording);

        let handle = std::thread::spawn(move || {
            // Try sox first, then ffmpeg.
            let child = std::process::Command::new("sox")
                .args([
                    "-d", // default input device
                    "-r",
                    &sample_rate.to_string(),
                    "-c",
                    &channels.to_string(),
                    "-b",
                    "16",
                    "-e",
                    "signed-integer",
                    "--endian",
                    "little",
                    "-t",
                    "raw",
                    "-", // output to stdout
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn();

            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "sox not found, trying ffmpeg");
                    match std::process::Command::new("ffmpeg")
                        .args([
                            "-f",
                            "avfoundation",
                            "-i",
                            ":default",
                            "-ar",
                            &sample_rate.to_string(),
                            "-ac",
                            &channels.to_string(),
                            "-f",
                            "s16le",
                            "pipe:1",
                        ])
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                "failed to start sox or ffmpeg — install with: brew install sox"
                            );
                            recording.store(false, Ordering::SeqCst);
                            return;
                        }
                    }
                }
            };

            let stdout = match child.stdout.take() {
                Some(s) => s,
                None => {
                    recording.store(false, Ordering::SeqCst);
                    return;
                }
            };

            let mut reader = std::io::BufReader::new(stdout);
            let mut chunk = [0u8; 4096];

            while recording.load(Ordering::SeqCst) {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => buffer.write(&chunk[..n]),
                    Err(e) => {
                        tracing::debug!(error = %e, "read error in recorder, stopping");
                        break;
                    }
                }
            }

            let _ = child.kill();
            let _ = child.wait();
        });

        *self
            .done
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(handle);
        Ok(())
    }

    /// 停止录音并返回 WAV 编码的音频数据。
    pub fn stop(&self) -> Result<Vec<u8>> {
        self.recording.store(false, Ordering::SeqCst);

        if let Some(handle) = self
            .done
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
        {
            if let Err(e) = handle.join() {
                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                return Err(anyhow::anyhow!("recording thread panicked: {}", msg));
            }
        }

        let pcm_data = self.shared_buffer.read_all();
        if pcm_data.is_empty() {
            return Err(anyhow::anyhow!("no audio data recorded"));
        }

        let wav_data = audio::encode_wav(&pcm_data, self.sample_rate, self.channels as u16, 16)?;
        Ok(wav_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_creation() {
        let recorder = SoxRecorder::new(16000, 1);
        assert_eq!(recorder.sample_rate, 16000);
        assert_eq!(recorder.channels, 1);
    }
}
