use super::KeyEvent;
use anyhow::{Context, Result};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// macOS key listener using `hidutil` for monitoring key events.
///
/// On macOS, global key listening requires Accessibility permissions.
/// Users must grant access in System Settings → Privacy & Security → Accessibility.
///
/// This implementation uses a small Swift helper script to tap into key events
/// via CGEvent, since hidutil alone cannot provide real-time key event monitoring
/// in the way needed.
pub struct MacOSListener {
    key_name: String,
    running: Arc<AtomicBool>,
    child: Option<Child>,
}

impl MacOSListener {
    pub fn new(key_name: &str) -> Result<Self> {
        Ok(Self {
            key_name: key_name.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            child: None,
        })
    }

    /// Start listening for key events via a Swift CGEvent tap helper.
    pub fn start(&mut self) -> Result<mpsc::UnboundedReceiver<KeyEvent>> {
        let keycode = self.resolve_keycode();

        tracing::info!("resolved key '{}' to keycode {:?}", self.key_name, keycode);

        let (tx, rx) = mpsc::unbounded_channel();
        let running = Arc::clone(&self.running);
        running.store(true, Ordering::SeqCst);

        // Use a Swift script to monitor key events via CGEvent tap.
        // This requires Accessibility permissions.
        let script = Self::event_tap_script()?;

        let mut child = Command::new("swift")
            .arg("-e")
            .arg(&script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to start swift key listener — ensure Xcode CLI tools are installed")?;

        let stdout = child.stdout.take().context("no stdout from swift")?;

        let target_keycode = keycode?;
        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines().flatten() {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                // Parse "PRESS <keycode>" or "RELEASE <keycode>" from swift script.
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("PRESS ") {
                    if let Ok(code) = rest.parse::<u32>() {
                        if code == target_keycode && tx.send(KeyEvent { pressed: true }).is_err() {
                            break;
                        }
                    }
                } else if let Some(rest) = trimmed.strip_prefix("RELEASE ") {
                    if let Ok(code) = rest.parse::<u32>() {
                        if code == target_keycode && tx.send(KeyEvent { pressed: false }).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        self.child = Some(child);
        Ok(rx)
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn resolve_keycode(&self) -> Result<u32> {
        // Map common key names to macOS virtual keycodes.
        match self.key_name.as_str() {
            "ISO_Level3_Shift" | "Alt_R" | "RightAlt" => Ok(61), // kVK_RightOption
            "Alt_L" | "LeftAlt" => Ok(58),                       // kVK_Option
            "Super_L" | "Cmd_L" => Ok(55),                       // kVK_Command
            "Super_R" | "Cmd_R" => Ok(54),                       // kVK_RightCommand
            _ => anyhow::bail!("unsupported key name for macOS: {}", self.key_name),
        }
    }

    fn event_tap_script(&self) -> Result<String> {
        Ok(r#"
import Cocoa

let mask = (1 << CGEventType.keyDown.rawValue) | (1 << CGEventType.keyUp.rawValue)

guard let tap = CGEvent.tapCreate(tap: .cgSessionEventTap,
                                  place: .headInsertEventTap,
                                  options: .listenOnly,
                                  eventsOfInterest: CGEventMask(mask),
                                  callback: nil,
                                  userInfo: nil) else {
    fputs("Failed to create event tap. Grant Accessibility permission.\n", stderr)
    exit(1)
}

let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
let loop_ = CFRunLoopGetCurrent()
CFRunLoopAddSource(loop_, source, CFRunLoopMode.commonModes.rawValue)
CGEvent.tapEnable(tap: tap, enable: true)

let callback: CGEventTapCallBack = { _, type, event, _ in
    let keycode = event.getIntegerValueField(.keyboardEventKeycode)
    if type == .keyDown {
        print("PRESS \(keycode)")
        fflush(stdout)
    } else if type == .keyUp {
        print("RELEASE \(keycode)")
        fflush(stdout)
    }
}

// Re-create tap with callback
if let tapWithCallback = CGEvent.tapCreate(tap: .cgSessionEventTap,
                                            place: .headInsertEventTap,
                                            options: .listenOnly,
                                            eventsOfInterest: CGEventMask(mask),
                                            callback: callback,
                                            userInfo: nil) {
    let src = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tapWithCallback, 0)
    CFRunLoopAddSource(loop_, src, CFRunLoopMode.commonModes.rawValue)
    CGEvent.tapEnable(tap: tapWithCallback, enable: true)
}

CFRunLoopRun()
"#
        .to_string())
    }
}

impl Drop for MacOSListener {
    fn drop(&mut self) {
        self.stop();
    }
}

use std::io::BufRead;
