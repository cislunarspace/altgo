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

#[cfg(test)]
mod tests {
    #[test]
    fn clipboard_new_runs() {
        // 冒烟测试：验证 arboard 能在当前环境构造 Clipboard（构造不写入，不污染剪切板）。
        // 无桌面会话时 Clipboard::new 可能失败，故忽略结果 —— 真实写入靠 Windows 手动验证
        // （路线 B 选项 3）。此测试主要保证 windows.rs 在 Windows 上能编译并链接 arboard。
        let _ = arboard::Clipboard::new();
    }
}
