//! Windows 输出模块。
//!
//! 使用 arboard 写入系统剪切板。结果展示由 Tauri overlay 负责（跨平台），
//! 本模块仅处理剪切板。
//!
//! `arboard::Clipboard` 非 `Send`，在 `spawn_blocking` 闭包内创建、使用、drop。
//! 错误通过 `anyhow` 向上传播；剪切板写入路径靠 Windows 手动验证（路线 B 选项 3）。

use anyhow::Result;
use std::sync::Arc;

/// Windows `Output` adapter — arboard for clipboard.
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

    fn clone_box(&self) -> Arc<dyn super::Output> {
        Arc::new(WindowsOutput)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clipboard_new_runs() {
        let _ = arboard::Clipboard::new();
    }
}
