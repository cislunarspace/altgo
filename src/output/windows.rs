use super::truncate_text;
use std::process::Command;

/// Write text to the Windows clipboard via `clip.exe` or PowerShell.
pub async fn write_clipboard(text: &str) -> anyhow::Result<()> {
    let text = text.to_string();
    tokio::task::spawn_blocking(move || {
        // Try clip.exe first (available on all Windows).
        let output = Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    // clip.exe expects text in the system's OEM code page.
                    // Use PowerShell for proper UTF-8 support.
                    let pwsh = Command::new("powershell")
                        .args(["-NoProfile", "-Command", "Set-Clipboard -Value $input"])
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                    match pwsh {
                        Ok(mut ps_child) => {
                            if let Some(mut stdin) = ps_child.stdin.take() {
                                let _ = stdin.write_all(text.as_bytes());
                            }
                            let result = ps_child.wait_with_output();
                            // Kill the clip.exe child since we're using PowerShell instead.
                            let _ = child.kill();
                            let _ = child.wait();
                            return result;
                        }
                        Err(_) => {
                            // Fallback: use clip.exe with potential encoding issues.
                            stdin.write_all(text.as_bytes())?;
                            return child.wait_with_output();
                        }
                    }
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

/// Show a Windows toast notification via PowerShell.
pub fn notify(title: &str, body: &str, _timeout_ms: u64) -> anyhow::Result<()> {
    let truncated = truncate_text(body, 200);
    let title_escaped = title.replace('\'', "''");
    let body_escaped = truncated.replace('\'', "''");

    // Try PowerShell BurntToast module first, then fallback to simple MessageBox.
    let script = format!(
        r#"
try {{
    Import-Module BurntToast -ErrorAction Stop
    New-BurntToastNotification -Text '{title}', '{body}'
}} catch {{
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show('{body}', '{title}', 'OK', 'Information') | Out-Null
}}
"#,
        title = title_escaped,
        body = body_escaped
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output();

    match output {
        Ok(_) => Ok(()),
        Err(_) => {
            tracing::debug!("PowerShell notification not available");
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
