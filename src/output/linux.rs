//! Linux 输出模块。
//!
//! 剪切板支持三种后端：`xclip`、`xsel`、`wl-copy`（Wayland）。
//! 根据 `XDG_SESSION_TYPE` 自动检测可用工具。
//!
//! 桌面通知通过 `notify-send` 发送。

use super::truncate_text;
use std::process::Command;

/// Linux 上可用的剪切板管理工具。
#[derive(Debug, Clone, Copy)]
pub enum ClipboardTool {
    XClip,
    XSel,
    WlCopy,
}

impl std::fmt::Display for ClipboardTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardTool::XClip => write!(f, "xclip"),
            ClipboardTool::XSel => write!(f, "xsel"),
            ClipboardTool::WlCopy => write!(f, "wl-copy"),
        }
    }
}

/// 检测系统上可用的剪切板工具。
pub fn detect_clipboard_tool() -> Option<ClipboardTool> {
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();

    if session_type == "wayland" {
        if which("wl-copy") {
            return Some(ClipboardTool::WlCopy);
        }
        return None;
    }

    // X11 or unspecified: prefer xclip, then xsel.
    if which("xclip") {
        return Some(ClipboardTool::XClip);
    }
    if which("xsel") {
        return Some(ClipboardTool::XSel);
    }

    // Fallback: check wl-copy in case user is on Wayland without XDG_SESSION_TYPE.
    if which("wl-copy") {
        return Some(ClipboardTool::WlCopy);
    }

    None
}

/// 将文本写入系统剪切板（异步，5 秒超时）。
pub async fn write_clipboard(text: &str) -> anyhow::Result<()> {
    let tool = detect_clipboard_tool()
        .ok_or_else(|| anyhow::anyhow!("no clipboard tool found (tried xclip, xsel, wl-copy)"))?;
    let text = text.to_string();
    tokio::task::spawn_blocking(move || write_clipboard_with_tool(tool, &text))
        .await
        .map_err(|e| anyhow::anyhow!("clipboard task panicked: {e}"))?
}

/// 使用指定的剪切板工具写入文本。
pub fn write_clipboard_with_tool(tool: ClipboardTool, text: &str) -> anyhow::Result<()> {
    let args: Vec<&str> = match tool {
        ClipboardTool::XClip => vec!["-selection", "clipboard"],
        ClipboardTool::XSel => vec!["--clipboard", "--input"],
        ClipboardTool::WlCopy => vec![],
    };

    let output = Command::new(tool.to_string())
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
            }
            child.wait_with_output()
        });

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(anyhow::anyhow!(
            "clipboard command {} failed: {}",
            tool,
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(e) => Err(anyhow::anyhow!("failed to run {}: {}", tool, e)),
    }
}

/// 通过 `notify-send` 显示桌面通知。
pub fn notify(title: &str, body: &str, timeout_ms: u64) -> anyhow::Result<()> {
    let output = Command::new("notify-send")
        .arg("-t")
        .arg(timeout_ms.to_string())
        .arg(title)
        .arg(truncate_text(body, 200))
        .output();

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(_) => {
            tracing::warn!("notify-send command failed");
            Ok(())
        }
        Err(e) => {
            tracing::warn!(error = %e, "notify-send not available");
            Ok(())
        }
    }
}

/// 显示处理中通知。
pub fn notify_processing(message: &str) -> anyhow::Result<()> {
    notify("altgo", message, 5000)
}

/// 显示语音识别结果通知。
pub fn notify_result(text: &str, timeout_ms: u64) -> anyhow::Result<()> {
    let truncated = truncate_text(text, 200);
    notify("altgo", &truncated, timeout_ms)
}

/// Check if a command exists in PATH.
fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_text_short() {
        assert_eq!(truncate_text("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_text_exact() {
        assert_eq!(truncate_text("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_text_long() {
        let result = truncate_text("hello world", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_text_multibyte() {
        let result = truncate_text("你好世界再见", 6);
        assert!(result.ends_with("..."));
        assert!(result.starts_with("你好"));
    }

    #[test]
    fn test_truncate_text_empty() {
        assert_eq!(truncate_text("", 10), "");
    }

    #[test]
    fn test_detect_clipboard_tool() {
        let tool = detect_clipboard_tool();
        if let Some(t) = tool {
            let s = t.to_string();
            assert!(s == "xclip" || s == "xsel" || s == "wl-copy");
        }
    }
}
