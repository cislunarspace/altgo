//! Windows 输出模块。
//!
//! 剪切板通过 `clip.exe` 写入（需要 UTF-16LE 编码）。
//! 通知通过 PowerShell/WPF 创建半透明浮动窗口实现，
//! 位于屏幕右下角，超时后自动消失。如果 WPF 不可用，
//! 回退到 MessageBox。

use super::truncate_text;
use std::process::Command;

/// 通过 `clip.exe` 将文本写入 Windows 剪切板（UTF-16LE 编码）。
pub async fn write_clipboard(text: &str) -> anyhow::Result<()> {
    let text = text.to_string();
    tokio::task::spawn_blocking(move || {
        // Convert to UTF-16LE (Windows native clipboard encoding).
        let utf16: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();

        let output = Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(&utf16);
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

/// 通过 PowerShell/WPF 显示浮动通知窗口。
///
/// 在屏幕右下角创建半透明无边框窗口，超时后自动消失。
/// 如果 WPF 不可用，回退到 MessageBox。
pub fn notify(title: &str, body: &str, timeout_ms: u64) -> anyhow::Result<()> {
    let truncated = truncate_text(body, 200);
    let timeout_sec = (timeout_ms as f64) / 1000.0;

    // Escape single quotes for PowerShell string interpolation.
    let title_escaped = title.replace('\'', "''");
    let body_escaped = truncated.replace('\n', " ").replace('\'', "''");

    // Write the PowerShell script to a temp file to avoid format! escaping issues
    // with XAML (hex colors like #CC2D2D2D clash with Rust token parsing).
    let tmp = tempfile::NamedTempFile::with_suffix(".ps1")?;
    let script = format_ps1_script(&title_escaped, &body_escaped, timeout_sec);
    std::fs::write(tmp.path(), script)?;

    // Spawn in background so it doesn't block the main loop.
    let child = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &tmp.path().to_string_lossy(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match child {
        Ok(mut child) => {
            // Move tmp into background thread — it auto-deletes when PowerShell exits.
            let _ = child.stdin.take();
            std::thread::spawn(move || {
                let _ = child.wait();
                // tmp (NamedTempFile) is dropped here, cleaning up the .ps1 file.
            });
        }
        Err(e) => {
            tracing::debug!(error = %e, "failed to spawn PowerShell notification");
            // tmp is dropped here on the main thread, cleaning up immediately.
        }
    }
    Ok(())
}

fn format_ps1_script(title: &str, body: &str, timeout_sec: f64) -> String {
    // Build the script by concatenation to avoid format!/r#"..."# parsing
    // issues with XAML hex colors like #CC2D2D2D.
    let mut s = String::new();
    s.push_str("try {\n");
    s.push_str("    Add-Type -AssemblyName PresentationFramework\n");
    s.push_str("    Add-Type -AssemblyName PresentationCore\n");
    s.push_str("    Add-Type -AssemblyName WindowsBase\n\n");
    s.push_str("    $xaml = @\"\n");
    s.push_str("<Window xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\"\n");
    s.push_str("        xmlns:x=\"http://schemas.microsoft.com/winfx/2006/xaml\"\n");
    s.push_str("        WindowStyle=\"None\" AllowsTransparency=\"True\"\n");
    s.push_str("        Background=\"#CC2D2D2D\" Opacity=\"0.92\"\n");
    s.push_str("        ShowInTaskbar=\"False\" Topmost=\"True\"\n");
    s.push_str("        SizeToContent=\"Height\" Width=\"320\"\n");
    s.push_str("        WindowStartupLocation=\"Manual\">\n");
    s.push_str("    <Border CornerRadius=\"12\" Padding=\"16,12\" Background=\"#CC2D2D2D\"\n");
    s.push_str("            BorderBrush=\"#44FFFFFF\" BorderThickness=\"1\">\n");
    s.push_str("        <StackPanel>\n");
    s.push_str("            <TextBlock Foreground=\"#CCFFFFFF\"\n");
    s.push_str(
        "                       FontSize=\"13\" FontWeight=\"SemiBold\" Margin=\"0,0,0,4\"/>\n",
    );
    s.push_str("            <TextBlock Foreground=\"#AAFFFFFF\"\n");
    s.push_str("                       FontSize=\"12\" TextWrapping=\"Wrap\"/>\n");
    s.push_str("        </StackPanel>\n");
    s.push_str("    </Border>\n");
    s.push_str("</Window>\n");
    s.push_str("\"@\n\n");
    s.push_str(
        "    $reader = [System.Xml.XmlReader]::Create([System.IO.StringReader]::new($xaml))\n",
    );
    s.push_str("    $window = [System.Windows.Markup.XamlReader]::Load($reader)\n\n");
    s.push_str("    # Set text after loading XAML to avoid parsing issues.\n");
    s.push_str("    $window.Content.Child.Children[0].Text = '");
    s.push_str(title);
    s.push_str("'\n");
    s.push_str("    $window.Content.Child.Children[1].Text = '");
    s.push_str(body);
    s.push_str("'\n\n");
    s.push_str("    # Position near bottom-right of work area.\n");
    s.push_str("    $screen = [System.Windows.SystemParameters]::WorkArea\n");
    s.push_str("    $window.Left = $screen.Right - $window.Width - 24\n");
    s.push_str("    $window.Top = $screen.Bottom - $window.Height - 24\n\n");
    s.push_str("    # Auto-dismiss timer.\n");
    s.push_str("    $timer = New-Object System.Windows.Threading.DispatcherTimer\n");
    s.push_str("    $timer.Interval = [TimeSpan]::FromSeconds(");
    s.push_str(&timeout_sec.to_string());
    s.push_str(")\n");
    s.push_str("    $timer.Add_Tick({ $window.Close(); $timer.Stop() })\n");
    s.push_str("    $timer.Start()\n\n");
    s.push_str("    $window.ShowDialog() | Out-Null\n");
    s.push_str("} catch {\n");
    s.push_str("    Add-Type -AssemblyName System.Windows.Forms\n");
    s.push_str("    [System.Windows.Forms.MessageBox]::Show('");
    s.push_str(body);
    s.push_str("', '");
    s.push_str(title);
    s.push_str("', 'OK', 'Information') | Out-Null\n");
    s.push_str("}\n");
    s
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
