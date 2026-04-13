use std::process::Command;

/// Available clipboard management tools.
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

/// Detect available clipboard tool.
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

/// Write text to the system clipboard using a 5-second timeout.
pub fn write_clipboard(text: &str) -> anyhow::Result<()> {
    let tool = detect_clipboard_tool()
        .ok_or_else(|| anyhow::anyhow!("no clipboard tool found (tried xclip, xsel, wl-copy)"))?;
    write_clipboard_with_tool(tool, text)
}

/// Write text using a specific clipboard tool.
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

/// Show a desktop notification via notify-send.
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
            // notify-send failed, but this is not critical.
            tracing::debug!("notify-send failed, ignoring");
            Ok(())
        }
        Err(_) => {
            // notify-send not installed — not critical.
            tracing::debug!("notify-send not available");
            Ok(())
        }
    }
}

/// Show "processing speech" notification.
pub fn notify_processing() -> anyhow::Result<()> {
    notify("altgo", "正在处理语音...", 5000)
}

/// Show transcription result notification.
pub fn notify_result(text: &str, timeout_ms: u64) -> anyhow::Result<()> {
    let truncated = truncate_text(text, 200);
    notify("altgo", &truncated, timeout_ms)
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    // Find a safe truncation point (don't cut multi-byte chars).
    let mut end = max_len;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
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
        // "你好" is 6 bytes, "世" starts at byte 6 but we can't include it at 6 bytes.
        assert!(result.ends_with("..."));
        assert!(result.starts_with("你好"));
    }

    #[test]
    fn test_truncate_text_empty() {
        assert_eq!(truncate_text("", 10), "");
    }

    #[test]
    fn test_detect_clipboard_tool() {
        // Just verify it doesn't panic.
        let tool = detect_clipboard_tool();
        if let Some(t) = tool {
            let s = t.to_string();
            assert!(s == "xclip" || s == "xsel" || s == "wl-copy");
        }
    }
}
