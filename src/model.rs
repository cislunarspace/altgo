//! Whisper 模型管理模块。
//!
//! 提供 whisper.cpp GGML 模型的注册、下载、切换功能。
//! 模型存储在 altgo 配置目录的 `models/` 子目录下。

use anyhow::{Context, Result};
use console::style;
use dialoguer::{Confirm, Select};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// 已知模型信息。
pub struct ModelInfo {
    pub name: &'static str,
    pub filename: &'static str,
    pub size_bytes: u64,
    pub description: &'static str,
}

const MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "tiny",
        filename: "ggml-tiny.bin",
        size_bytes: 75 * 1024 * 1024,
        description: "最小模型，速度最快",
    },
    ModelInfo {
        name: "base",
        filename: "ggml-base.bin",
        size_bytes: 142 * 1024 * 1024,
        description: "推荐日常使用",
    },
    ModelInfo {
        name: "small",
        filename: "ggml-small.bin",
        size_bytes: 466 * 1024 * 1024,
        description: "更好的准确率",
    },
    ModelInfo {
        name: "medium",
        filename: "ggml-medium.bin",
        size_bytes: 1500 * 1024 * 1024,
        description: "推荐中文使用",
    },
    ModelInfo {
        name: "large",
        filename: "ggml-large-v3.bin",
        size_bytes: 2900 * 1024 * 1024,
        description: "最佳准确率",
    },
];

pub fn models_info() -> &'static [ModelInfo] {
    MODELS
}

/// 返回模型存储目录（`~/.config/altgo/models/` 或 `%APPDATA%/altgo/models/`）。
pub fn models_dir() -> PathBuf {
    dirs::config_dir()
        .expect("could not determine config directory")
        .join("altgo")
        .join("models")
}

/// 列出所有已知模型信息。
pub fn list_available() -> Vec<(&'static str, &'static str, u64)> {
    MODELS
        .iter()
        .map(|m| (m.name, m.description, m.size_bytes))
        .collect()
}

/// 扫描已下载的模型，返回存在的模型名称列表。
pub fn list_downloaded() -> Vec<String> {
    let dir = models_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut downloaded = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("ggml-") && name_str.ends_with(".bin") {
                // Find the model name from the filename.
                if let Some(info) = MODELS.iter().find(|m| m.filename == name_str) {
                    downloaded.push(info.name.to_string());
                }
            }
        }
    }
    downloaded
}

/// 检查指定模型是否已下载。
pub fn is_downloaded(name: &str) -> bool {
    let info = match MODELS.iter().find(|m| m.name == name) {
        Some(i) => i,
        None => return false,
    };
    models_dir().join(info.filename).exists()
}

/// 解析配置中的模型值。
///
/// 如果 `config_model` 是模型名称（如 "base"），返回对应文件路径。
/// 如果是文件路径，直接返回。
/// 如果为空，返回 None。
pub fn resolve_model_path(config_model: &str) -> Option<PathBuf> {
    if config_model.is_empty() {
        return None;
    }

    // Check if it's a model name.
    if let Some(info) = MODELS.iter().find(|m| m.name == config_model) {
        let path = models_dir().join(info.filename);
        if path.exists() {
            return Some(path);
        }
    }

    // Check if it's a direct file path.
    let path = Path::new(config_model);
    if path.exists() {
        return Some(path.to_path_buf());
    }

    None
}

/// 下载指定模型，通过回调报告进度。
///
/// `on_progress` 参数为 `(downloaded_bytes, total_bytes)` 回调。
pub async fn download_with_progress<F>(
    name: &str,
    mut on_progress: F,
) -> Result<PathBuf>
where
    F: FnMut(u64, u64),
{
    let info = MODELS
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| anyhow::anyhow!("未知模型: {}", name))?;

    let dir = models_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("创建模型目录失败: {}", dir.display()))?;

    let dest = dir.join(info.filename);

    if dest.exists() {
        return Ok(dest);
    }

    let url = format!("{}/{}", MODEL_BASE_URL, info.filename);

    let response = reqwest::get(&url)
        .await
        .with_context(|| format!("下载模型失败: {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("下载模型失败: HTTP {} for {}", response.status(), url);
    }

    let total_size = response.content_length().unwrap_or(info.size_bytes);
    let tmp_path = dest.with_extension("bin.tmp");
    let mut file = std::fs::File::create(&tmp_path).with_context(|| "创建临时文件失败")?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("读取下载数据失败")?;
        std::io::Write::write_all(&mut file, &chunk).context("写入下载数据失败")?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total_size);
    }

    let file_size = std::fs::metadata(&tmp_path)?.len();
    if file_size < 10 * 1024 * 1024 {
        let _ = std::fs::remove_file(&tmp_path);
        anyhow::bail!("下载的模型文件过小 ({} bytes)，可能损坏", file_size);
    }

    std::fs::rename(&tmp_path, &dest).with_context(|| "重命名临时文件失败")?;

    Ok(dest)
}

/// 下载指定模型，带进度条显示。
pub async fn download(name: &str) -> Result<PathBuf> {
    let info = MODELS
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| anyhow::anyhow!("未知模型: {}", name))?;

    let dir = models_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("创建模型目录失败: {}", dir.display()))?;

    let dest = dir.join(info.filename);

    // Already downloaded.
    if dest.exists() {
        println!("{} 模型已存在: {}", style("✓").green(), dest.display());
        return Ok(dest);
    }

    let url = format!("{}/{}", MODEL_BASE_URL, info.filename);
    println!(
        "{} 正在下载模型 {} ({})...",
        style("↓").cyan(),
        style(info.name).bold(),
        format_size(info.size_bytes)
    );

    let response = reqwest::get(&url)
        .await
        .with_context(|| format!("下载模型失败: {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("下载模型失败: HTTP {} for {}", response.status(), url);
    }

    let total_size = response.content_length().unwrap_or(info.size_bytes);

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .progress_chars("█▓░"),
    );

    // Download to temp file first, then rename atomically.
    let tmp_path = dest.with_extension("bin.tmp");
    let mut file = std::fs::File::create(&tmp_path).with_context(|| "创建临时文件失败")?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("读取下载数据失败")?;
        std::io::Write::write_all(&mut file, &chunk).context("写入下载数据失败")?;
        pb.inc(chunk.len() as u64);
    }
    pb.finish_and_clear();

    // Validate minimum size (10 MB).
    let file_size = std::fs::metadata(&tmp_path)?.len();
    if file_size < 10 * 1024 * 1024 {
        let _ = std::fs::remove_file(&tmp_path);
        anyhow::bail!("下载的模型文件过小 ({} bytes)，可能损坏", file_size);
    }

    // Atomic rename.
    std::fs::rename(&tmp_path, &dest).with_context(|| "重命名临时文件失败")?;

    println!(
        "{} 模型 {} 下载完成: {}",
        style("✓").green(),
        style(info.name).bold(),
        dest.display()
    );

    Ok(dest)
}

/// 交互式模型选择与下载菜单。
///
/// 如果已有模型，提供切换选项。如果无模型，引导下载。
/// 返回选中的模型文件路径。
pub async fn interactive_prompt() -> Result<PathBuf> {
    let downloaded = list_downloaded();

    println!();
    println!("{}", style("Whisper 模型管理").bold().dim());
    println!();

    if !downloaded.is_empty() {
        // Build menu: existing models + download new + cancel.
        let mut items: Vec<String> = downloaded
            .iter()
            .map(|n| {
                let info = MODELS.iter().find(|m| m.name == n.as_str());
                match info {
                    Some(i) => format!("{} — {} [已下载]", n, i.description),
                    None => format!("{} [已下载]", n),
                }
            })
            .collect();
        items.push("下载新模型...".to_string());
        items.push("取消".to_string());

        let selection = Select::new()
            .with_prompt("选择要使用的模型")
            .items(&items)
            .default(0)
            .interact()?;

        if selection == items.len() - 1 {
            // Cancel.
            anyhow::bail!("未选择模型，退出");
        }

        if selection == items.len() - 2 {
            // Download new model.
            return download_menu().await;
        }

        // Use an existing model.
        let name = &downloaded[selection];
        let info = MODELS.iter().find(|m| m.name == name.as_str()).unwrap();
        let path = models_dir().join(info.filename);
        println!("{} 已选择模型: {}", style("✓").green(), name);
        return Ok(path);
    }

    // No models downloaded — guide user to download.
    println!(
        "{}",
        style("未检测到 whisper 模型，需要下载一个才能使用本地语音识别。").yellow()
    );
    println!();

    download_menu().await
}

/// Show model download selection menu.
async fn download_menu() -> Result<PathBuf> {
    let items: Vec<String> = MODELS
        .iter()
        .map(|m| {
            let status = if is_downloaded(m.name) {
                "[已下载]"
            } else {
                ""
            };
            format!(
                "{} ({}) — {} {}",
                m.name,
                format_size(m.size_bytes),
                m.description,
                status
            )
        })
        .collect();

    let selection = Select::new()
        .with_prompt("选择要下载的模型")
        .items(&items)
        .default(1) // Default to "base"
        .interact()?;

    let chosen = &MODELS[selection];

    let confirm = Confirm::new()
        .with_prompt(format!(
            "确认下载 {} ({}) ?",
            chosen.name,
            format_size(chosen.size_bytes)
        ))
        .default(true)
        .interact()?;

    if !confirm {
        anyhow::bail!("取消下载");
    }

    download(chosen.name).await
}

/// Format bytes as human-readable size.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(75 * 1024 * 1024), "75 MB");
        assert_eq!(format_size(142 * 1024 * 1024), "142 MB");
        assert_eq!(format_size(1500 * 1024 * 1024), "1.5 GB");
        assert_eq!(format_size(2900 * 1024 * 1024), "2.8 GB");
        assert_eq!(format_size(500 * 1024), "500 KB");
    }

    #[test]
    fn test_list_available() {
        let available = list_available();
        assert_eq!(available.len(), 5);
        assert_eq!(available[0].0, "tiny");
        assert_eq!(available[1].0, "base");
    }

    #[test]
    fn test_resolve_model_path_empty() {
        assert!(resolve_model_path("").is_none());
    }

    #[test]
    fn test_resolve_model_path_nonexistent() {
        assert!(resolve_model_path("/nonexistent/model.bin").is_none());
    }

    #[test]
    fn test_models_dir_contains_altgo() {
        let dir = models_dir();
        assert!(dir.to_string_lossy().contains("altgo"));
        assert!(dir.to_string_lossy().contains("models"));
    }
}
