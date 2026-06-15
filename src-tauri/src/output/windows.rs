//! Windows 输出模块。
//!
//! 使用 arboard 写入系统剪切板。结果展示由 Tauri overlay 负责（跨平台），
//! 本模块仅处理剪切板。
//!
//! `arboard::Clipboard` 非 `Send`，在 `spawn_blocking` 闭包内创建、使用、drop。
//! 错误通过 `anyhow` 向上传播；剪切板写入路径靠 Windows 手动验证（路线 B 选项 3）。

use anyhow::Result;

/// 将文本写入系统剪切板（异步）。
///
/// 匹配 Linux 的 `write_clipboard` 签名，供 `output::write_clipboard` 跨平台派发。
/// arboard 的 API 是同步的，且 `Clipboard` 非 `Send`，故在 `spawn_blocking` 闭包内
/// 完成构造 + 写入 + drop。
pub async fn write_clipboard(text: &str) -> Result<()> {
    let text = text.to_string();
    tokio::task::spawn_blocking(move || {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| anyhow::anyhow!("failed to access clipboard: {e}"))?;
        clipboard
            .set_text(text)
            .map_err(|e| anyhow::anyhow!("failed to set clipboard text: {e}"))
    })
    .await
    .map_err(|e| anyhow::anyhow!("clipboard task panicked: {e}"))?
}

/// Windows `Output` adapter — arboard for clipboard, overlay for notifications.
pub struct WindowsOutput;

impl WindowsOutput {
    pub fn new() -> Self {
        Self
    }
}

impl super::Output for WindowsOutput {
    fn write_clipboard(&self, text: &str) -> Result<()> {
        // arboard::Clipboard is !Send; caller wraps this in spawn_blocking.
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| anyhow::anyhow!("failed to access clipboard: {e}"))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|e| anyhow::anyhow!("failed to set clipboard text: {e}"))
    }

    fn notify(&self, _title: &str, _body: &str) -> Result<()> {
        // Notifications on Windows are handled by the Tauri overlay.
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn super::Output> {
        Box::new(WindowsOutput)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clipboard_new_runs() {
        let _ = arboard::Clipboard::new();
    }
}
