//! Windows 输出模块。
//!
//! 剪切板通过 PowerShell Set-Clipboard 写入（原生支持 Unicode）。
//! 文本注入通过 SendInput 实现（支持中日韩字符）。
//! 悬浮窗通过 PowerShell/WPF 创建，位于屏幕中下方（任务栏上方）。
//! 如果 WPF 不可用，回退到 MessageBox。

use super::truncate_text;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 通过 PowerShell Set-Clipboard 将文本写入 Windows 剪切板。
#[allow(dead_code)]
pub async fn write_clipboard(text: &str) -> anyhow::Result<()> {
    let text = text.to_string();
    tokio::task::spawn_blocking(move || {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$input | Set-Clipboard",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait_with_output()
            });

        match output {
            Ok(out) if out.status.success() => Ok(()),
            Ok(out) => Err(anyhow::anyhow!(
                "clipboard write failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )),
            Err(e) => Err(anyhow::anyhow!("failed to write clipboard: {}", e)),
        }
    })
    .await
    .map_err(|e| anyhow::anyhow!("clipboard task panicked: {e}"))?
}

/// 检查当前焦点元素是否为接受文本输入的控件。
fn is_text_input_focused() -> bool {
    let ps_script = r#"
        Add-Type -AssemblyName UIAutomationClient
        Add-Type -AssemblyName UIAutomationTypes
        $el = [System.Windows.Automation.AutomationElement]::FocusedElement
        if ($el -eq [System.Windows.Automation.AutomationElement]::None) { Write-Output 'false'; exit }
        $ct = $el.Current.ControlType
        if ($ct.ProgrammaticName -match 'Text|Edit') { Write-Output 'true'; exit }
        $class = $el.Current.ClassName
        if ($class -match 'Edit|TextBox|RichEdit|WpfText|IME') { Write-Output 'true' } else { Write-Output 'false' }
    "#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .output();

    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim() == "true",
        Err(_) => false,
    }
}

/// 通过 SendInput 发送 Unicode 文本到当前焦点窗口。
fn send_unicode_text(text: &str) -> anyhow::Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
        VIRTUAL_KEY,
    };

    let inputs: Vec<INPUT> = text
        .chars()
        .flat_map(|c| {
            let down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: c as u16,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let up = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: c as u16,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            [down, up]
        })
        .collect();

    unsafe {
        let result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if result == 0 {
            return Err(anyhow::anyhow!("SendInput returned 0"));
        }
    }
    Ok(())
}

/// 尝试将文本注入到当前光标位置。返回原因描述。
pub fn try_inject_at_cursor(text: &str) -> &'static str {
    if !is_text_input_focused() {
        return "not_text_field";
    }

    if let Err(e) = send_unicode_text(text) {
        tracing::warn!(error = %e, "SendInput failed");
        return "send_failed";
    }

    "injected"
}

// ─── Floating windows ──────────────────────────────────────────────────────────

/// 从系统获取屏幕工作区尺寸，返回 (screen_width, screen_height)。
fn get_screen_workarea() -> (i32, i32) {
    let ps_script = r#"
        $s = [System.Windows.SystemParameters]::WorkArea
        Write-Output "$($s.Width),$($s.Height)"
    "#;
    let output = match Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(_) => return (1920, 1080),
    };
    let s = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = s.trim().split(',').collect();
    if parts.len() < 2 {
        return (1920, 1080);
    }
    let w: i32 = parts[0].parse().unwrap_or(1920);
    let h: i32 = parts[1].parse().unwrap_or(1080);
    (w, h)
}

/// 计算居中靠下的窗口 Left 坐标。
fn center_left(window_width: i32) -> i32 {
    let (sw, _) = get_screen_workarea();
    (sw - window_width) / 2
}

/// 计算靠下的 Top 坐标（距任务栏上方 offset 像素）。
fn bottom_top(window_height: i32, offset: i32) -> i32 {
    let (_, sh) = get_screen_workarea();
    sh - window_height - offset
}

fn spawn_ps_script(script: &str) {
    let tmp = match tempfile::NamedTempFile::with_suffix(".ps1") {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "failed to create temp ps1 file");
            return;
        }
    };
    if let Err(e) = std::fs::write(tmp.path(), script) {
        tracing::warn!(error = %e, "failed to write ps1 script");
        return;
    }

    let mut child = match Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &tmp.path().to_string_lossy(),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "failed to spawn PowerShell");
            return;
        }
    };

    let tmp_path = tmp.into_temp_path();
    let _ = tmp_path.keep();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

/// 显示录音悬浮窗口（居中靠下）。
pub fn show_recording_window() -> anyhow::Result<()> {
    let left = center_left(320);
    let top = bottom_top(120, 80);

    let script = include_str!("windows_recording.ps1")
        .replace("{left}", &left.to_string())
        .replace("{top}", &top.to_string());

    tokio::task::spawn_blocking(move || {
        spawn_ps_script(&script);
    });
    Ok(())
}

/// 关闭录音悬浮窗口（静默，窗口由下一次录音/结果覆盖）。
pub fn close_recording_window() -> anyhow::Result<()> {
    Ok(())
}

/// 显示结果悬浮窗口（注入失败时调用）。
pub fn show_result_window(
    raw_text: &str,
    polished_text: &str,
    polish_failed: bool,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let left = center_left(420);
    let top = bottom_top(300, 100);
    let title = if polish_failed {
        "识别结果"
    } else {
        "润色结果"
    };

    // Escape single quotes for PowerShell string
    let escape_ps = |s: &str| s.replace('\'', "''").replace('\n', " ").replace('\r', "");

    let script = include_str!("windows_result.ps1")
        .replace("{left}", &left.to_string())
        .replace("{top}", &top.to_string())
        .replace("{title}", title)
        .replace("{raw_text}", &escape_ps(raw_text))
        .replace("{polished_text}", &escape_ps(polished_text))
        .replace("{timeout_ms}", &timeout_ms.to_string());

    tokio::task::spawn_blocking(move || {
        spawn_ps_script(&script);
    });
    Ok(())
}

// ─── Unified output API ─────────────────────────────────────────────────────

/// 统一的输出函数：尝试注入光标，失败则显示悬浮窗口。
pub async fn output_text(
    raw_text: &str,
    polished_text: &str,
    polish_failed: bool,
    inject_at_cursor: bool,
    prefer_polished: bool,
    timeout_ms: u64,
) -> anyhow::Result<&'static str> {
    let text_to_use = if prefer_polished && !polish_failed {
        polished_text
    } else {
        raw_text
    };

    let reason = if inject_at_cursor {
        try_inject_at_cursor(text_to_use)
    } else {
        "disabled"
    };

    match reason {
        "injected" => {
            tracing::info!("text injected at cursor");
            Ok("injected")
        }
        _ => {
            tracing::info!(
                reason = reason,
                "cursor injection skipped, showing result window"
            );
            show_result_window(raw_text, polished_text, polish_failed, timeout_ms)?;
            Ok(reason)
        }
    }
}

/// 通过 PowerShell/WPF 显示浮动通知窗口。
pub fn notify(title: &str, body: &str, timeout_ms: u64) -> anyhow::Result<()> {
    let truncated = truncate_text(body, 200);
    let timeout_sec = (timeout_ms as f64) / 1000.0;

    let title_escaped = title
        .replace('\'', "''")
        .replace('$', "`$")
        .replace('`', "``");
    let body_escaped = truncated
        .replace('\'', "''")
        .replace('$', "`$")
        .replace('`', "``");

    let script = include_str!("windows_notify.ps1")
        .replace("{title}", &title_escaped)
        .replace("{body}", &body_escaped)
        .replace("{timeout_sec}", &timeout_sec.to_string());

    tokio::task::spawn_blocking(move || {
        spawn_ps_script(&script);
    });
    Ok(())
}

/// 显示处理中通知。
#[allow(dead_code)]
pub fn notify_processing(message: &str) -> anyhow::Result<()> {
    notify("altgo", message, 5000)
}

/// 显示语音识别结果通知。
#[allow(dead_code)]
pub fn notify_result(text: &str, timeout_ms: u64) -> anyhow::Result<()> {
    let truncated = truncate_text(text, 200);
    notify("altgo", &truncated, timeout_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_text_multibyte() {
        let result = truncate_text("你好世界再见", 6);
        assert!(result.ends_with("..."));
        assert!(result.starts_with("你好"));
    }
}
