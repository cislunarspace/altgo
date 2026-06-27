//! Linux 输出模块。
//!
//! 剪切板支持三种后端：`xclip`、`xsel`、`wl-copy`（Wayland）。
//! 根据 `XDG_SESSION_TYPE` 自动检测可用工具。

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
        // Wayland 下没有 wl-copy 时，回退到 xclip/xsel（可能通过 XWayland 工作）
        if which("xclip") {
            return Some(ClipboardTool::XClip);
        }
        if which("xsel") {
            return Some(ClipboardTool::XSel);
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

/// Check if a command exists in PATH.
fn which(cmd: &str) -> bool {
    crate::resource::which_binary(cmd).is_some()
}

/// Linux `Output` adapter — wraps clipboard tools.
pub struct LinuxOutput {
    tool: Option<ClipboardTool>,
}

impl LinuxOutput {
    pub fn new() -> Self {
        Self {
            tool: detect_clipboard_tool(),
        }
    }
}

impl super::Output for LinuxOutput {
    fn write_clipboard(&self, text: &str) -> anyhow::Result<()> {
        let tool = self
            .tool
            .ok_or_else(|| anyhow::anyhow!("no clipboard tool found"))?;
        write_clipboard_with_tool(tool, text)
    }

    fn clone_box(&self) -> Box<dyn super::Output> {
        Box::new(LinuxOutput { tool: self.tool })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_clipboard_tool() {
        let tool = detect_clipboard_tool();
        if let Some(t) = tool {
            let s = t.to_string();
            assert!(s == "xclip" || s == "xsel" || s == "wl-copy");
        }
    }
}
