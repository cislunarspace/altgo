//! 运行时捆绑资源定位模块。
//!
//! 查找与主程序一同安装的捆绑二进制文件（ffmpeg、whisper-cli）。
//! 当这些工具未安装在系统 PATH 中时，回退到捆绑位置。

use std::path::PathBuf;

/// 查找捆绑的二进制文件。
///
/// 搜索逻辑：
/// - `/usr/lib/altgo/bin/{name}`（系统安装）
/// - 可执行文件同级目录下的 `bin/{name}`（相对路径）
pub fn bundled_bin(name: &str) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();

    // If installed to /usr/bin, look for /usr/lib/altgo/bin/.
    if exe_dir.ends_with("/usr/bin") || exe_dir.ends_with("/usr/local/bin") {
        let candidate = PathBuf::from("/usr/lib/altgo/bin").join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    // Otherwise look relative to the exe.
    let candidate = exe_dir.join("bin").join(name);
    if candidate.exists() {
        return Some(candidate);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_bin_nonexistent() {
        // Should return None for a binary that doesn't exist.
        assert!(bundled_bin("nonexistent_tool_xyz").is_none());
    }
}
