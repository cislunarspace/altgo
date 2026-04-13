//! macOS 输出模块。
//!
//! 剪切板通过 `pbcopy` 写入，通知通过 `osascript` 的 `display notification` 发送。

use super::truncate_text;
use std::process::Command;

/// 通过 `pbcopy` 将文本写入 macOS 剪切板。
pub async fn write_clipboard(text: &str) -> anyhow::Result<()> {
    let text = text.to_string();
    tokio::task::spawn_blocking(move || {
        let output = Command::new("pbcopy")
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
                "pbcopy failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )),
            Err(e) => Err(anyhow::anyhow!("failed to run pbcopy: {}", e)),
        }
    })
    .await
    .map_err(|e| anyhow::anyhow!("clipboard task panicked: {e}"))?
}

/// 通过 `osascript` 显示 macOS 通知。
pub fn notify(title: &str, body: &str, _timeout_ms: u64) -> anyhow::Result<()> {
    let truncated = truncate_text(body, 200);
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        truncated.replace('\\', "\\\\").replace('"', "\\\""),
        title.replace('\\', "\\\\").replace('"', "\\\""),
    );

    let output = Command::new("osascript").arg("-e").arg(&script).output();

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(_) => {
            tracing::debug!("osascript notification failed, ignoring");
            Ok(())
        }
        Err(_) => {
            tracing::debug!("osascript not available");
            Ok(())
        }
    }
}

/// 显示"正在处理语音"通知。
pub fn notify_processing() -> anyhow::Result<()> {
    notify("altgo", "正在处理语音...", 5000)
}

/// 显示语音识别结果通知。
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
