use crate::audio::{self, Buffer};
use anyhow::Result;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// macOS recorder using `sox` (Sound eXchange) or `ffmpeg` for audio capture.
///
/// Records 16-bit PCM at 16kHz mono via subprocess.
/// Requires `sox` to be installed (`brew install sox`).
pub struct SoxRecorder {
    sample_rate: u32,
    channels: u32,
    shared_buffer: Arc<Buffer>,
    recording: Arc<AtomicBool>,
    done: std::sync::Mutex<Option<JoinHandle<()>>>,
}

impl SoxRecorder {
    pub fn new(sample_rate: u32, channels: u32) -> Self {
        Self {
            sample_rate,
            channels,
            shared_buffer: Arc::new(Buffer::new()),
            recording: Arc::new(AtomicBool::new(false)),
            done: std::sync::Mutex::new(None),
        }
    }

    /// Start recording from the default audio input device.
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
                    tracing::error!(error = %e, "failed to start sox — install with: brew install sox");
                    recording.store(false, Ordering::SeqCst);
                    return;
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
                    Err(_) => break,
                }
            }

            let _ = child.kill();
            let _ = child.wait();
        });

        *self.done.lock().unwrap() = Some(handle);
        Ok(())
    }

    /// Stop recording and return WAV-encoded audio data.
    pub fn stop(&self) -> Result<Vec<u8>> {
        self.recording.store(false, Ordering::SeqCst);

        let handle = self.done.lock().unwrap().take();
        if let Some(h) = handle {
            let _ = h.join();
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
