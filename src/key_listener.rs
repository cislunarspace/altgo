use anyhow::{Context, Result};
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Key event from the listener.
#[derive(Debug)]
pub struct KeyEvent {
    pub pressed: bool,
}

/// X11 key listener using `xinput test-xi2` to capture global key events.
///
/// This approach works without root and uses the XInput2 extension
/// which is available on all modern X11 systems.
pub struct X11Listener {
    key_name: String,
    running: Arc<AtomicBool>,
    child: Option<Child>,
}

impl X11Listener {
    pub fn new(key_name: &str) -> Result<Self> {
        Command::new("xinput")
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("xinput not found — install xinput package")?;

        Ok(Self {
            key_name: key_name.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            child: None,
        })
    }

    /// Start listening for key events. Returns a channel of events.
    pub fn start(&mut self) -> Result<mpsc::UnboundedReceiver<KeyEvent>> {
        let keycode = self.resolve_keycode()?;

        tracing::info!("resolved key '{}' to keycode {}", self.key_name, keycode);

        let (tx, rx) = mpsc::unbounded_channel();
        let running = Arc::clone(&self.running);
        running.store(true, Ordering::SeqCst);

        let mut child = Command::new("xinput")
            .args(["test-xi2", "--root"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to start xinput test-xi2")?;

        let stdout = child.stdout.take().context("no stdout from xinput")?;

        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
            let mut event_type: Option<bool> = None; // true=press, false=release

            for line in reader.lines() {
                if !running.load(Ordering::SeqCst) {
                    break;
                }

                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };

                let trimmed = line.trim();

                if trimmed.contains("KeyPress") && trimmed.starts_with("EVENT") {
                    event_type = Some(true);
                } else if trimmed.contains("KeyRelease") && trimmed.starts_with("EVENT") {
                    event_type = Some(false);
                } else if let Some(detail_str) = trimmed.strip_prefix("detail:") {
                    if let Some(pressed) = event_type.take() {
                        if let Ok(detail) = detail_str.trim().parse::<u8>() {
                            if detail == keycode && tx.send(KeyEvent { pressed }).is_err() {
                                break;
                            }
                        }
                    } else {
                        tracing::debug!(line = %trimmed, "detail line without preceding event type, skipping");
                    }
                } else if !trimmed.is_empty()
                    && !trimmed.starts_with("EVENT")
                    && !trimmed.contains(':')
                {
                    tracing::trace!(line = %trimmed, "unparsed xinput line");
                }
            }
        });

        self.child = Some(child);
        Ok(rx)
    }

    /// Stop listening.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn resolve_keycode(&self) -> Result<u8> {
        let output = Command::new("xmodmap")
            .arg("-pke")
            .output()
            .context("xmodmap not found")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let target_keysym = &self.key_name;

        for line in stdout.lines() {
            if line.contains(target_keysym) {
                if let Some(keycode_str) = line.split_whitespace().nth(1) {
                    if let Ok(keycode) = keycode_str.parse::<u8>() {
                        return Ok(keycode);
                    }
                }
            }
        }

        anyhow::bail!(
            "keycode for '{}' not found in xmodmap output",
            target_keysym
        )
    }
}

impl Drop for X11Listener {
    fn drop(&mut self) {
        self.stop();
    }
}
