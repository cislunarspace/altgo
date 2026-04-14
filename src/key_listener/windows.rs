//! Windows 按键监听器。
//!
//! 使用 PowerShell 脚本调用 `GetAsyncKeyState` 进行按键状态轮询（约 50ms 间隔）。
//! 通过内联 C# P/Invoke 声明访问 user32.dll，无需 COM 或 Win32 API 绑定。
//!
//! 将常见的按键名称（如 `ISO_Level3_Shift`、`Alt_R`）映射到 Windows 虚拟键码。

use super::KeyEvent;
use anyhow::{Context, Result};
use std::io::BufRead;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

// Windows flag to prevent subprocess from creating a console window.
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Windows 按键监听器.
///
/// 使用 PowerShell + `GetAsyncKeyState` 进行按键状态轮询。
pub struct WindowsListener {
    key_name: String,
    poll_interval_ms: u64,
    running: Arc<AtomicBool>,
    child: Option<Child>,
}

impl WindowsListener {
    /// 创建新的 Windows 监听器，验证 PowerShell 是否可用。
    pub fn new(key_name: &str) -> Result<Self> {
        // Verify PowerShell is available.
        Command::new("powershell")
            .arg("-Command")
            .arg("exit 0")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("PowerShell not found")?;

        Ok(Self {
            key_name: key_name.to_string(),
            poll_interval_ms: 50, // default, can be set before start()
            running: Arc::new(AtomicBool::new(false)),
            child: None,
        })
    }

    /// Set the poll interval in milliseconds.
    pub fn set_poll_interval_ms(&mut self, ms: u64) {
        self.poll_interval_ms = ms;
    }

    /// 开始监听按键事件，通过 PowerShell 轮询实现。
    pub fn start(&mut self) -> Result<mpsc::UnboundedReceiver<KeyEvent>> {
        let vk_code = self.resolve_vkcode()?;

        tracing::info!("resolved key '{}' to VK code {}", self.key_name, vk_code);

        let (tx, rx) = mpsc::unbounded_channel();
        let running = Arc::clone(&self.running);
        running.store(true, Ordering::SeqCst);

        let script = format!(
            r#"Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class KeyState {{
    [DllImport("user32.dll")]
    public static extern short GetAsyncKeyState(int vKey);
}}
"@
$vk = {vk_code}
$wasDown = $false
while ($true) {{
    $state = [KeyState]::GetAsyncKeyState($vk)
    $isDown = ($state -band 0x8000) -ne 0
    if ($isDown -and -not $wasDown) {{
        Write-Output "PRESS"
        [Console]::Out.Flush()
    }} elseif (-not $isDown -and $wasDown) {{
        Write-Output "RELEASE"
        [Console]::Out.Flush()
    }}
    $wasDown = $isDown
    Start-Sleep -Milliseconds {poll_interval_ms}
}}"#,
            vk_code = vk_code,
            poll_interval_ms = self.poll_interval_ms
        );

        let mut child = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(&script)
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .context("failed to start PowerShell key listener")?;

        let stdout = child.stdout.take().context("no stdout from powershell")?;

        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if !running.load(Ordering::SeqCst) {
                    tracing::info!("key listener thread stopped by user");
                    break;
                }
                let trimmed = line.trim();
                let send_result = match trimmed {
                    "PRESS" => tx.send(KeyEvent { pressed: true }),
                    "RELEASE" => tx.send(KeyEvent { pressed: false }),
                    _ => continue,
                };
                if send_result.is_err() {
                    tracing::warn!("key event receiver dropped, key listener thread exiting");
                    break;
                }
            }
            tracing::warn!("PowerShell key listener stdout closed, key listener thread exiting");
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

    fn resolve_vkcode(&self) -> Result<i32> {
        // Map common key names to Windows virtual key codes.
        match self.key_name.as_str() {
            "ISO_Level3_Shift" | "Alt_R" | "RightAlt" => Ok(0xA5), // VK_RMENU
            "Alt_L" | "LeftAlt" => Ok(0xA4),                       // VK_LMENU
            "Super_L" | "Win_L" => Ok(0x5B),                       // VK_LWIN
            "Super_R" | "Win_R" => Ok(0x5C),                       // VK_RWIN
            "Control_R" => Ok(0xA3),                               // VK_RCONTROL
            "Shift_R" => Ok(0xA1),                                 // VK_RSHIFT
            _ => anyhow::bail!("unsupported key name for Windows: {}", self.key_name),
        }
    }
}

impl Drop for WindowsListener {
    fn drop(&mut self) {
        self.stop();
    }
}
