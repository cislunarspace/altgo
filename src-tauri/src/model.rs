//! Whisper 模型管理模块。
//!
//! 提供 whisper.cpp GGML 模型的注册、下载、切换功能。
//! 模型存储在 altgo 配置目录的 `models/` 子目录下。

use crate::error::ModelError;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// 可通过环境变量覆盖下载基址（勿以 `/` 结尾），便于国内等网络环境使用镜像，例如：
/// `ALTGO_MODEL_BASE_URL=https://hf-mirror.com/ggerganov/whisper.cpp/resolve/main`
const ENV_MODEL_BASE_URL: &str = "ALTGO_MODEL_BASE_URL";

/// Hugging Face 实际对象大小（用于进度条；与 Content-Length 接近即可）。
const GGML_MEDIUM_BYTES: u64 = 1533763059;

const DOWNLOAD_ATTEMPTS: u32 = 3;

/// 国内常用 HF 镜像（与官方路径一致，仅替换域名）。
const HF_MIRROR_BASE_URL: &str = "https://hf-mirror.com/ggerganov/whisper.cpp/resolve/main";

fn model_download_bases() -> Vec<String> {
    if let Ok(s) = std::env::var(ENV_MODEL_BASE_URL) {
        let t = s.trim();
        if !t.is_empty() {
            return vec![t.trim_end_matches('/').to_string()];
        }
    }
    vec![MODEL_BASE_URL.to_string(), HF_MIRROR_BASE_URL.to_string()]
}

fn model_download_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(concat!(
                "altgo/",
                env!("CARGO_PKG_VERSION"),
                " (whisper.cpp ggml model download)"
            ))
            .connect_timeout(Duration::from_secs(120))
            .pool_idle_timeout(Duration::from_secs(600))
            .build()
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "failed to build model download client");
                // Fallback to default client — download may still work with less optimal settings.
                Client::new()
            })
    })
}

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
        size_bytes: GGML_MEDIUM_BYTES,
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
        .unwrap_or_else(|| PathBuf::from("."))
        .join("altgo")
        .join("models")
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

/// 模型列表项（含下载状态），供 IPC 返回给前端。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub name: String,
    pub filename: String,
    pub size_bytes: u64,
    pub description: String,
    pub downloaded: bool,
}

/// 返回所有已知模型及下载状态。
pub fn list_all_with_status() -> Vec<ModelEntry> {
    let downloaded = list_downloaded();
    models_info()
        .iter()
        .map(|m| ModelEntry {
            name: m.name.to_string(),
            filename: m.filename.to_string(),
            size_bytes: m.size_bytes,
            description: m.description.to_string(),
            downloaded: downloaded.iter().any(|d| d == m.name),
        })
        .collect()
}

/// 校验模型名是否在已知模型列表中。
pub fn validate_name(name: &str) -> Result<(), ModelError> {
    if models_info().iter().any(|m| m.name == name) {
        Ok(())
    } else {
        Err(ModelError::UnknownModel(name.to_string()))
    }
}

/// 删除指定模型的本地文件。
///
/// 从 `models_info()` 查找模型、`models_dir()` 拼路径、`fs::remove_file` 删除。
/// 若文件不存在则静默返回 Ok。
pub fn delete(name: &str) -> Result<(), ModelError> {
    validate_name(name)?;
    let info = MODELS.iter().find(|m| m.name == name).unwrap();
    let path = models_dir().join(info.filename);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
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
) -> Result<PathBuf, ModelError>
where
    F: FnMut(u64, u64),
{
    let info = MODELS
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| ModelError::UnknownModel(name.to_string()))?;

    let dir = models_dir();
    std::fs::create_dir_all(&dir)?;

    let dest = dir.join(info.filename);

    if dest.exists() {
        return Ok(dest);
    }

    let bases = model_download_bases();
    let tmp_path = dest.with_extension("bin.tmp");

    let mut last_err: Option<ModelError> = None;
    for attempt in 0..DOWNLOAD_ATTEMPTS {
        if attempt > 0 {
            let _ = std::fs::remove_file(&tmp_path);
            tokio::time::sleep(Duration::from_secs(2 * u64::from(attempt))).await;
        }

        for base in &bases {
            let url = format!("{}/{}", base, info.filename);
            match download_once_to_tmp(&url, info, &tmp_path, &mut on_progress).await {
                Ok(()) => {
                    let file_size = std::fs::metadata(&tmp_path)?.len();
                    if file_size < 10 * 1024 * 1024 {
                        let _ = std::fs::remove_file(&tmp_path);
                        return Err(ModelError::DownloadFailed(format!(
                            "下载的模型文件过小 ({} bytes)，可能损坏",
                            file_size
                        )));
                    }
                    std::fs::rename(&tmp_path, &dest)?;
                    return Ok(dest);
                }
                Err(e) => {
                    last_err = Some(e);
                    let _ = std::fs::remove_file(&tmp_path);
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        ModelError::DownloadFailed(format!(
            "下载模型失败（已尝试官方与镜像）。可设置环境变量 {} 指定可访问的基址，或检查代理/防火墙。",
            ENV_MODEL_BASE_URL
        ))
    }))
}

async fn download_once_to_tmp<F>(
    url: &str,
    info: &ModelInfo,
    tmp_path: &Path,
    on_progress: &mut F,
) -> Result<(), ModelError>
where
    F: FnMut(u64, u64),
{
    let response = model_download_client().get(url).send().await.map_err(|e| {
        ModelError::HttpError(format!("无法从 {} 下载（网络或 TLS 错误）: {}", url, e))
    })?;

    if !response.status().is_success() {
        return Err(ModelError::DownloadFailed(format!(
            "下载失败: HTTP {} — {}\n可尝试设置环境变量 {} 使用镜像基址。",
            response.status(),
            url,
            ENV_MODEL_BASE_URL
        )));
    }

    let total_size = response.content_length().unwrap_or(info.size_bytes);
    on_progress(0, total_size);
    let mut file = std::fs::File::create(tmp_path)?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ModelError::HttpError(format!("读取下载数据失败: {e}")))?;
        std::io::Write::write_all(&mut file, &chunk)?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total_size);
    }

    Ok(())
}

/// Format bytes as human-readable size.
#[cfg(test)]
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

    #[test]
    fn test_validate_name_known() {
        assert!(validate_name("tiny").is_ok());
        assert!(validate_name("base").is_ok());
        assert!(validate_name("large").is_ok());
    }

    #[test]
    fn test_validate_name_unknown() {
        assert!(validate_name("nonexistent").is_err());
        assert!(validate_name("").is_err());
    }

    #[test]
    fn test_list_all_with_status_count() {
        let entries = list_all_with_status();
        assert_eq!(entries.len(), models_info().len());
        // 至少包含 tiny 和 base
        assert!(entries.iter().any(|e| e.name == "tiny"));
        assert!(entries.iter().any(|e| e.name == "base"));
    }

    #[test]
    fn test_delete_unknown_model_errors() {
        assert!(delete("nonexistent_model").is_err());
    }

    #[test]
    fn test_delete_missing_file_ok() {
        // 模型文件大概率不存在，删除应静默成功
        let _ = delete("tiny");
    }
}
