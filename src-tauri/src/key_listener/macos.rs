//! macOS 按键监听器。
//!
//! 通过内联 Swift 脚本使用 CGEvent tap 监听全局键盘事件。
//! 需要用户在系统设置 → 隐私与安全性 → 辅助功能中授予权限。
//!
//! Swift 脚本创建一个只读的事件监听 tap，将按键事件输出到 stdout，
//! Rust 端解析 "PRESS <keycode>" / "RELEASE <keycode>" 格式的行。

use super::KeyEvent;
use anyhow::{Context, Result};
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// macOS 按键监听器。
///
/// 通过内联 Swift 脚本使用 CGEvent tap 监听全局键盘事件。
/// 需要辅助功能（Accessibility）权限。
pub struct MacOSListener {
    key_name: String,
    running: Arc<AtomicBool>,
    child: Option<Child>,
}

impl MacOSListener {
    /// 创建新的 macOS 监听器。
    pub fn new(key_name: &str) -> Result<Self> {
        Ok(Self {
            key_name: key_name.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            child: None,
        })
    }

    /// 开始监听按键事件，通过 Swift CGEvent tap 实现。
    pub fn start(&mut self) -> Result<mpsc::UnboundedReceiver<KeyEvent>> {
        let keycode = self.resolve_keycode();

        tracing::info!("resolved key '{}' to keycode {:?}", self.key_name, keycode);

        let (tx, rx) = mpsc::unbounded_channel();
        let running = Arc::clone(&self.running);
        running.store(true, Ordering::SeqCst);

        // Use a Swift script to monitor key events via CGEvent tap.
        // This requires Accessibility permissions.
        let script = Self::event_tap_script();

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
            for line in reader.lines().map_while(Result::ok) {
                if !running.load(Ordering::SeqCst) {
                    tracing::info!("key listener thread stopped by user");
                    break;
                }
                // Parse "PRESS <keycode>" or "RELEASE <keycode>" from swift script.
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("PRESS ") {
                    if let Ok(code) = rest.parse::<u32>() {
                        if code == target_keycode && tx.send(KeyEvent { pressed: true }).is_err() {
                            tracing::warn!(
                                "key event receiver dropped, key listener thread exiting"
                            );
                            break;
                        }
                    }
                } else if let Some(rest) = trimmed.strip_prefix("RELEASE ") {
                    if let Ok(code) = rest.parse::<u32>() {
                        if code == target_keycode && tx.send(KeyEvent { pressed: false }).is_err() {
                            tracing::warn!(
                                "key event receiver dropped, key listener thread exiting"
                            );
                            break;
                        }
                    }
                }
            }
            tracing::warn!("Swift key listener stdout closed, key listener thread exiting");
        });

        self.child = Some(child);
        Ok(rx)
    }

    /// 停止监听。
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

    fn event_tap_script() -> String {
        r#"
import Cocoa

let mask = (1 << CGEventType.keyDown.rawValue) | (1 << CGEventType.keyUp.rawValue)

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

guard let tap = CGEvent.tapCreate(tap: .cgSessionEventTap,
                                  place: .headInsertEventTap,
                                  options: .listenOnly,
                                  eventsOfInterest: CGEventMask(mask),
                                  callback: callback,
                                  userInfo: nil) else {
    fputs("Failed to create event tap. Grant Accessibility permission.\n", stderr)
    exit(1)
}

let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
let loop_ = CFRunLoopGetCurrent()
CFRunLoopAddSource(loop_, source, CFRunLoopMode.commonModes.rawValue)
CGEvent.tapEnable(tap: tap, enable: true)

CFRunLoopRun()
"#
        .to_string()
    }
}

impl Drop for MacOSListener {
    fn drop(&mut self) {
        self.stop();
    }
}
