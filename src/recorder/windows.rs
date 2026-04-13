use crate::audio::{self, Buffer};
use anyhow::Result;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

/// Windows recorder using `ffmpeg` or `sox` for audio capture.
///
/// Records 16-bit PCM at 16kHz mono via subprocess.
/// Requires `ffmpeg` or `sox` to be installed and available in PATH.
pub struct WindowsRecorder {
    sample_rate: u32,
    channels: u32,
    shared_buffer: Arc<Buffer>,
    recording: Arc<AtomicBool>,
    done: std::sync::Mutex<Option<JoinHandle<()>>>,
}

/// Find the ffmpeg binary, searching PATH and common install locations.
fn find_ffmpeg() -> String {
    // Try PATH first via cmd's `where` (more reliable than PowerShell's where).
    if let Ok(output) = std::process::Command::new("cmd")
        .args(["/C", "where", "ffmpeg"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    {
        let path = String::from_utf8_lossy(&output.stdout);
        if let Some(first) = path.lines().next() {
            let p = first.trim();
            if !p.is_empty() && std::path::Path::new(p).exists() {
                return p.to_string();
            }
        }
    }

    // Search common winget install location.
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let winget_base = std::path::PathBuf::from(local_app_data)
            .join("Microsoft")
            .join("WinGet")
            .join("Packages");
        if let Ok(entries) = std::fs::read_dir(&winget_base) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("Gyan.FFmpeg") {
                    // Try any version subdirectory.
                    if let Ok(sub) = std::fs::read_dir(entry.path()) {
                        for s in sub.flatten() {
                            let cand = s.path().join("bin").join("ffmpeg.exe");
                            if cand.exists() {
                                tracing::debug!(path = %cand.display(), "found ffmpeg");
                                return cand.to_string_lossy().to_string();
                            }
                        }
                    }
                }
            }
        }
    }

    tracing::warn!("ffmpeg not found in PATH or winget, using bare 'ffmpeg'");
    "ffmpeg".to_string()
}

/// Resolve the dshow audio device string (e.g. `"audio=@device_..."`).
///
/// On many Windows systems `audio=default` does not work because dshow
/// requires the exact device name.  Chinese device names cannot be passed
/// reliably through PowerShell→ffmpeg due to encoding issues, so we use
/// the device's **alternative PnP name** (ASCII-safe `@device_...` string).
///
/// Returns `"audio=<alt_name>"` on success, `"audio=default"` on failure.
fn resolve_audio_device() -> &'static str {
    static DEVICE: OnceLock<String> = OnceLock::new();
    DEVICE.get_or_init(|| {
        let ffmpeg_path = find_ffmpeg();
        tracing::info!(ffmpeg = %ffmpeg_path, "resolving dshow audio device");

        // ffmpeg writes device list to stderr.  Capture both stdout and stderr.
        let output = std::process::Command::new(&ffmpeg_path)
            .args(["-list_devices", "true", "-f", "dshow", "-i", "dummy"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        let (out, err) = match output {
            Ok(o) => {
                let stdout_text = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr_text = String::from_utf8_lossy(&o.stderr).to_string();
                tracing::debug!(
                    stdout_len = stdout_text.len(),
                    stderr_len = stderr_text.len(),
                    "ffmpeg device list output received"
                );
                (stdout_text, stderr_text)
            }
            Err(e) => {
                tracing::debug!(error = %e, "failed to enumerate dshow devices");
                return "audio=default".to_string();
            }
        };

        // ffmpeg outputs device info to stderr, but some builds use stdout.
        let combined = if err.contains("(audio)") || err.contains("Alternative name") {
            err
        } else {
            out
        };

        // Scan for the first audio device and its alternative name.
        // ffmpeg stderr output looks like:
        //   "Device Name" (audio)
        //     Alternative name "@device_cm_...\wave_..."
        let mut found_audio = false;
        for line in combined.lines() {
            if line.contains("(audio)") {
                found_audio = true;
                continue;
            }
            if found_audio && line.contains("Alternative name") {
                // Extract the quoted alternative name.
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        let alt_name = &line[start + 1..start + 1 + end];
                        tracing::info!(
                            device = %alt_name,
                            "resolved dshow audio device"
                        );
                        return format!("audio={alt_name}");
                    }
                }
                found_audio = false;
            }
        }

        tracing::warn!("could not resolve dshow audio device, falling back to audio=default");
        "audio=default".to_string()
    })
}

impl WindowsRecorder {
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
            // Try ffmpeg with dshow, then sox as fallback.
            let audio_device = resolve_audio_device();
            let ffmpeg_path = find_ffmpeg();
            let result = std::process::Command::new(&ffmpeg_path)
                .args([
                    "-f",
                    "dshow",
                    "-i",
                    audio_device,
                    "-ar",
                    &sample_rate.to_string(),
                    "-ac",
                    &channels.to_string(),
                    "-sample_fmt",
                    "s16",
                    "-f",
                    "s16le",
                    "-acodec",
                    "pcm_s16le",
                    "pipe:1",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn();

            let mut child = match result {
                Ok(c) => c,
                Err(_) => {
                    // Fallback to sox.
                    let sox_result = std::process::Command::new("sox")
                        .args([
                            "-d",
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
                            "-",
                        ])
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::null())
                        .spawn();

                    match sox_result {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                "failed to start ffmpeg or sox — install ffmpeg or sox"
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
        let recorder = WindowsRecorder::new(16000, 1);
        assert_eq!(recorder.sample_rate, 16000);
        assert_eq!(recorder.channels, 1);
    }
}
