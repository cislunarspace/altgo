use crate::audio::{self, Buffer};
use anyhow::Result;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// PulseAudio recorder that captures audio from the default source.
///
/// Uses `parecord` subprocess for reliable PulseAudio capture.
/// Records 16-bit PCM at 16kHz mono.
pub struct PulseRecorder {
    sample_rate: u32,
    channels: u32,
    shared_buffer: Arc<Buffer>,
    recording: Arc<AtomicBool>,
}

impl PulseRecorder {
    pub fn new(sample_rate: u32, channels: u32) -> Self {
        Self {
            sample_rate,
            channels,
            shared_buffer: Arc::new(Buffer::new()),
            recording: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start recording from the default PulseAudio source.
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

        std::thread::spawn(move || {
            let child = std::process::Command::new("parecord")
                .args([
                    "--format=s16le",
                    &format!("--rate={}", sample_rate),
                    &format!("--channels={}", channels),
                    "--raw",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn();

            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(error = %e, "failed to start parecord");
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

            // Kill parecord when recording stops.
            let _ = child.kill();
            let _ = child.wait();
        });

        Ok(())
    }

    /// Stop recording and return WAV-encoded audio data.
    pub fn stop(&self) -> Result<Vec<u8>> {
        self.recording.store(false, Ordering::SeqCst);

        // Give the recording thread a moment to finish.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let pcm_data = self.shared_buffer.read_all();
        if pcm_data.is_empty() {
            return Err(anyhow::anyhow!("no audio data recorded"));
        }

        let wav_data = audio::encode_wav(&pcm_data, self.sample_rate, self.channels as u16, 16);
        Ok(wav_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_creation() {
        let recorder = PulseRecorder::new(16000, 1);
        assert_eq!(recorder.sample_rate, 16000);
        assert_eq!(recorder.channels, 1);
    }
}
