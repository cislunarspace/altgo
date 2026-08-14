//! SenseVoice 模型管理模块。
//!
//! 提供 SenseVoice（sherpa-onnx）模型的注册、下载、切换功能。
//! 模型存储在 altgo 配置目录的 `models/<name>/` 子目录下，每个模型
//! 一个目录，内含 `model.int8.onnx` 与 `tokens.txt` 两个文件。

use crate::error::ModelError;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

/// SenseVoice 模型仓库基址（sherpa-onnx 官方 HF 仓库）。
const MODEL_BASE_URL: &str =
    "https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main";

/// 可通过环境变量覆盖下载基址（勿以 `/` 结尾），便于国内等网络环境使用镜像，例如：
/// `ALTGO_MODEL_BASE_URL=https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main`
const ENV_MODEL_BASE_URL: &str = "ALTGO_MODEL_BASE_URL";

const DOWNLOAD_ATTEMPTS: u32 = 3;

/// 主模型文件最小可接受大小（字节）。小于此值视为下载损坏。
const MIN_MODEL_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// 国内常用 HF 镜像（与官方路径一致，仅替换域名）。
const HF_MIRROR_BASE_URL: &str =
    "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main";

/// 主模型文件名（其余文件为配套资源）。
const MAIN_MODEL_FILENAME: &str = "model.int8.onnx";
const TOKENS_FILENAME: &str = "tokens.txt";
const MAIN_MODEL_SHA256: &str = "c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51";
const TOKENS_SHA256: &str = "4d14b174af75c64af4b9879a7f2d60c774b4dcea74fddee64510d7e4d7347590";

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
                " (sherpa-onnx sense-voice model download)"
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

/// 模型内单个文件。
pub struct ModelFile {
    pub filename: &'static str,
    /// 近似大小（用于进度条；与 Content-Length 接近即可）。
    pub size_bytes: u64,
    /// 官方发布文件的 SHA-256，用于识别中断下载和损坏缓存。
    pub sha256: &'static str,
}

/// 已知模型信息。
pub struct ModelInfo {
    pub name: &'static str,
    pub files: &'static [ModelFile],
    pub description: &'static str,
}

/// SenseVoice int8：中/英/日/韩/粤自动检测，CPU 实时率远高于 whisper。
const SENSE_VOICE_FILES: &[ModelFile] = &[
    ModelFile {
        filename: MAIN_MODEL_FILENAME,
        size_bytes: 230 * 1024 * 1024,
        sha256: MAIN_MODEL_SHA256,
    },
    ModelFile {
        filename: TOKENS_FILENAME,
        size_bytes: 8 * 1024,
        sha256: TOKENS_SHA256,
    },
];

const MODELS: &[ModelInfo] = &[ModelInfo {
    name: "sense-voice",
    files: SENSE_VOICE_FILES,
    description: "SenseVoice（中英日韩粤自动检测，速度快）",
}];

pub fn models_info() -> &'static [ModelInfo] {
    MODELS
}

/// 返回模型存储根目录（`~/.config/altgo/models/` 或 `%APPDATA%/altgo/models/`）。
pub fn models_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("altgo")
        .join("models")
}

/// 返回指定模型的目录。
pub fn model_dir(name: &str) -> PathBuf {
    models_dir().join(name)
}

fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn model_file_ready(file: &ModelFile, path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file()
        || (file.filename == MAIN_MODEL_FILENAME && metadata.len() < MIN_MODEL_FILE_BYTES)
        || (file.filename != MAIN_MODEL_FILENAME && metadata.len() == 0)
    {
        return false;
    }

    file_sha256(path).is_ok_and(|sha256| sha256 == file.sha256)
}

fn model_files_ready_with(dir: &Path, files: &[ModelFile]) -> bool {
    files
        .iter()
        .all(|file| model_file_ready(file, &dir.join(file.filename)))
}

/// 模型文件是否齐全且校验和匹配官方发布版本。
fn model_files_ready(dir: &Path) -> bool {
    model_files_ready_with(dir, SENSE_VOICE_FILES)
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
            let path = entry.path();
            if path.is_dir() && model_files_ready(&path) {
                if let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) {
                    if MODELS.iter().any(|m| m.name == name) {
                        downloaded.push(name);
                    }
                }
            }
        }
    }
    downloaded
}

/// 检查指定模型是否已下载。
pub fn is_downloaded(name: &str) -> bool {
    MODELS
        .iter()
        .find(|m| m.name == name)
        .map(|_| model_files_ready(&model_dir(name)))
        .unwrap_or(false)
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
    models_info()
        .iter()
        .map(|m| ModelEntry {
            name: m.name.to_string(),
            filename: m.files[0].filename.to_string(),
            size_bytes: m.files.iter().map(|f| f.size_bytes).sum(),
            description: m.description.to_string(),
            downloaded: is_downloaded(m.name),
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

/// 删除指定模型的本地目录。
pub fn delete(name: &str) -> Result<(), ModelError> {
    validate_name(name)?;
    let path = model_dir(name);
    if path.exists() {
        std::fs::remove_dir_all(&path)?;
    }
    Ok(())
}

/// 解析配置中的模型值，返回模型目录（含 `model.int8.onnx` 与 `tokens.txt`）。
///
/// 如果 `config_model` 是模型名称（如 "sense-voice"），返回已下载的模型目录。
/// 如果是目录路径，直接返回；如果是 `.onnx` 文件路径，返回其父目录。
/// 如果为空或目录不完整，返回 None。
pub fn resolve_model_dir(config_model: &str) -> Option<PathBuf> {
    if config_model.is_empty() {
        return None;
    }

    // Check if it's a model name.
    if MODELS.iter().any(|m| m.name == config_model) {
        let dir = model_dir(config_model);
        if model_files_ready(&dir) {
            return Some(dir);
        }
        return None;
    }

    // Check if it's a directory path.
    let path = Path::new(config_model);
    if path.is_dir() && model_files_ready(path) {
        return Some(path.to_path_buf());
    }

    // Check if it's a direct .onnx file path.
    if path.is_file() && path.file_name().is_some_and(|n| n == MAIN_MODEL_FILENAME) {
        if let Some(parent) = path.parent() {
            if model_files_ready(parent) {
                return Some(parent.to_path_buf());
            }
        }
    }

    None
}

/// 下载指定模型（全部文件），通过回调报告进度。
///
/// `on_progress` 参数为 `(downloaded_bytes, total_bytes)`，跨文件累计。
pub async fn download_with_progress<F>(name: &str, on_progress: F) -> Result<PathBuf, ModelError>
where
    F: FnMut(u64, u64),
{
    download_with_progress_to(name, model_download_bases(), models_dir(), on_progress).await
}

async fn download_with_progress_to<F>(
    name: &str,
    bases: Vec<String>,
    root_dir: PathBuf,
    on_progress: F,
) -> Result<PathBuf, ModelError>
where
    F: FnMut(u64, u64),
{
    let info = MODELS
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| ModelError::UnknownModel(name.to_string()))?;
    download_model_with_progress_to(info, bases, root_dir, on_progress).await
}

async fn download_model_with_progress_to<F>(
    info: &ModelInfo,
    bases: Vec<String>,
    root_dir: PathBuf,
    mut on_progress: F,
) -> Result<PathBuf, ModelError>
where
    F: FnMut(u64, u64),
{
    let dir = root_dir.join(info.name);
    std::fs::create_dir_all(&dir)?;

    let total_bytes: u64 = info.files.iter().map(|f| f.size_bytes).sum();
    let mut done_bytes: u64 = 0;

    for file in info.files {
        let dest = dir.join(file.filename);
        if model_file_ready(file, &dest) {
            done_bytes += file.size_bytes;
            continue;
        }
        if dest.exists() {
            if dest.is_file() {
                std::fs::remove_file(&dest)?;
            } else {
                return Err(ModelError::DownloadFailed(format!(
                    "模型资源路径不是文件: {}",
                    dest.display()
                )));
            }
        }

        let tmp_path = dir.join(format!("{}.tmp", file.filename));
        let mut last_err: Option<ModelError> = None;

        for attempt in 0..DOWNLOAD_ATTEMPTS {
            if attempt > 0 {
                let _ = std::fs::remove_file(&tmp_path);
                tokio::time::sleep(Duration::from_secs(2 * u64::from(attempt))).await;
            }

            for base in &bases {
                let url = format!("{}/{}", base, file.filename);
                match download_once_to_tmp(
                    &url,
                    done_bytes,
                    total_bytes,
                    &tmp_path,
                    &mut on_progress,
                )
                .await
                {
                    Ok(()) => {
                        if !model_file_ready(file, &tmp_path) {
                            let _ = std::fs::remove_file(&tmp_path);
                            return Err(ModelError::DownloadFailed(format!(
                                "下载的模型文件校验失败: {}",
                                file.filename
                            )));
                        }
                        std::fs::rename(&tmp_path, &dest)?;
                        done_bytes += file.size_bytes;
                        last_err = None;
                        break;
                    }
                    Err(e) => {
                        last_err = Some(e);
                        let _ = std::fs::remove_file(&tmp_path);
                    }
                }
                if last_err.is_none() {
                    break;
                }
            }
            if last_err.is_none() {
                break;
            }
        }

        if let Some(e) = last_err {
            return Err(e);
        }
    }

    Ok(dir)
}

async fn download_once_to_tmp<F>(
    url: &str,
    base_done: u64,
    total: u64,
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

    on_progress(base_done, total);
    let mut file_handle = std::fs::File::create(tmp_path)?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ModelError::HttpError(format!("读取下载数据失败: {e}")))?;
        std::io::Write::write_all(&mut file_handle, &chunk)?;
        downloaded += chunk.len() as u64;
        on_progress(base_done + downloaded, total);
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

    fn test_model_info(model_payload: &[u8], tokens_payload: &[u8]) -> ModelInfo {
        let model_sha256: &'static str =
            Box::leak(format!("{:x}", Sha256::digest(model_payload)).into_boxed_str());
        let tokens_sha256: &'static str =
            Box::leak(format!("{:x}", Sha256::digest(tokens_payload)).into_boxed_str());
        let files = vec![
            ModelFile {
                filename: MAIN_MODEL_FILENAME,
                size_bytes: model_payload.len() as u64,
                sha256: model_sha256,
            },
            ModelFile {
                filename: TOKENS_FILENAME,
                size_bytes: tokens_payload.len() as u64,
                sha256: tokens_sha256,
            },
        ];

        ModelInfo {
            name: "sense-voice",
            files: Box::leak(files.into_boxed_slice()),
            description: "test model",
        }
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(75 * 1024 * 1024), "75 MB");
        assert_eq!(format_size(230 * 1024 * 1024), "230 MB");
        assert_eq!(format_size(2900 * 1024 * 1024), "2.8 GB");
        assert_eq!(format_size(500 * 1024), "500 KB");
    }

    #[test]
    fn test_resolve_model_dir_empty() {
        assert!(resolve_model_dir("").is_none());
    }

    #[test]
    fn test_resolve_model_dir_nonexistent() {
        assert!(resolve_model_dir("/nonexistent/model").is_none());
    }

    #[test]
    fn test_resolve_model_dir_unknown_name() {
        assert!(resolve_model_dir("nonexistent-name").is_none());
    }

    #[test]
    fn test_resolve_model_dir_incomplete_dir() {
        let dir = tempfile::tempdir().unwrap();
        // 只有 tokens.txt 没有主模型 → 视为未下载
        std::fs::write(dir.path().join("tokens.txt"), b"tok").unwrap();
        assert!(resolve_model_dir(dir.path().to_str().unwrap()).is_none());
    }

    #[test]
    fn test_resolve_model_dir_missing_tokens() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(MAIN_MODEL_FILENAME), b"model").unwrap();
        assert!(resolve_model_dir(dir.path().to_str().unwrap()).is_none());
    }

    #[test]
    fn test_resolve_model_dir_rejects_small_model() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(MAIN_MODEL_FILENAME), b"model").unwrap();
        std::fs::write(dir.path().join(TOKENS_FILENAME), b"tok").unwrap();
        assert!(resolve_model_dir(dir.path().to_str().unwrap()).is_none());
    }

    #[test]
    fn test_resolve_model_dir_rejects_empty_tokens() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(MAIN_MODEL_FILENAME),
            vec![0; MIN_MODEL_FILE_BYTES as usize],
        )
        .unwrap();
        std::fs::write(dir.path().join(TOKENS_FILENAME), b"").unwrap();
        assert!(resolve_model_dir(dir.path().to_str().unwrap()).is_none());
    }

    #[test]
    fn test_model_file_ready_requires_matching_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-model");
        std::fs::write(&path, b"valid model").unwrap();
        let sha256: &'static str =
            Box::leak(format!("{:x}", Sha256::digest(b"valid model")).into_boxed_str());
        let file = ModelFile {
            filename: "test-model",
            size_bytes: 11,
            sha256,
        };

        assert!(model_file_ready(&file, &path));
        std::fs::write(&path, b"corrupted model").unwrap();
        assert!(!model_file_ready(&file, &path));
    }

    #[test]
    fn test_models_dir_contains_altgo() {
        let dir = models_dir();
        assert!(dir.to_string_lossy().contains("altgo"));
        assert!(dir.to_string_lossy().contains("models"));
    }

    #[test]
    fn test_validate_name_known() {
        assert!(validate_name("sense-voice").is_ok());
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
        assert!(entries.iter().any(|e| e.name == "sense-voice"));
        // 主模型文件名应暴露给前端展示
        assert!(entries.iter().all(|e| e.filename == MAIN_MODEL_FILENAME));
    }

    #[test]
    fn test_delete_unknown_model_errors() {
        assert!(delete("nonexistent_model").is_err());
    }

    #[test]
    fn test_delete_missing_dir_ok() {
        // 模型目录大概率不存在，删除应静默成功
        let _ = delete("sense-voice");
    }

    #[tokio::test]
    async fn test_download_success_writes_dest_and_reports_progress() {
        let mut server = mockito::Server::new_async().await;
        let model_payload = vec![0u8; MIN_MODEL_FILE_BYTES as usize + 1];
        let tokens_payload = b"token list";
        let model_mock = server
            .mock("GET", "/model.int8.onnx")
            .with_status(200)
            .with_header("content-type", "application/octet-stream")
            .with_body(&model_payload)
            .create_async()
            .await;
        let tokens_mock = server
            .mock("GET", "/tokens.txt")
            .with_status(200)
            .with_body(tokens_payload)
            .create_async()
            .await;

        let tmp_dir = tempfile::tempdir().unwrap();
        let dest_dir = tmp_dir.path().join("sense-voice");

        let info = test_model_info(&model_payload, tokens_payload);
        let progress_calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let calls = progress_calls.clone();
        let result = download_model_with_progress_to(
            &info,
            vec![server.url()],
            tmp_dir.path().to_path_buf(),
            move |d, t| calls.lock().unwrap().push((d, t)),
        )
        .await;

        let path = result.unwrap();
        assert_eq!(path, dest_dir);
        assert!(dest_dir.join(MAIN_MODEL_FILENAME).exists());
        assert_eq!(
            std::fs::metadata(dest_dir.join(MAIN_MODEL_FILENAME))
                .unwrap()
                .len(),
            model_payload.len() as u64
        );
        assert_eq!(
            std::fs::read(dest_dir.join("tokens.txt")).unwrap(),
            tokens_payload
        );
        assert!(!dest_dir.join("model.int8.onnx.tmp").exists());
        {
            let calls = progress_calls.lock().unwrap();
            assert!(!calls.is_empty());
            // total 恒为声明总大小；进度按实际下载字节累计
            let total = calls.last().unwrap().1;
            assert_eq!(
                total,
                info.files.iter().map(|file| file.size_bytes).sum::<u64>()
            );
            assert!(calls.iter().any(|(d, _)| *d == model_payload.len() as u64));
        }
        model_mock.assert_async().await;
        tokens_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_replaces_incomplete_existing_model() {
        let mut server = mockito::Server::new_async().await;
        let model_payload = vec![0u8; MIN_MODEL_FILE_BYTES as usize + 1];
        let tokens_payload = b"tok";
        let model_mock = server
            .mock("GET", "/model.int8.onnx")
            .with_status(200)
            .with_body(&model_payload)
            .create_async()
            .await;

        let info = test_model_info(&model_payload, tokens_payload);
        let tmp_dir = tempfile::tempdir().unwrap();
        let dest_dir = tmp_dir.path().join("sense-voice");
        std::fs::create_dir_all(&dest_dir).unwrap();
        std::fs::write(dest_dir.join(MAIN_MODEL_FILENAME), b"incomplete").unwrap();
        std::fs::write(dest_dir.join(TOKENS_FILENAME), tokens_payload).unwrap();

        download_model_with_progress_to(
            &info,
            vec![server.url()],
            tmp_dir.path().to_path_buf(),
            |_d, _t| {},
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::metadata(dest_dir.join(MAIN_MODEL_FILENAME))
                .unwrap()
                .len(),
            model_payload.len() as u64
        );
        model_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_http_error_clears_tmp() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/model.int8.onnx")
            .with_status(500)
            .expect_at_least(1)
            .create_async()
            .await;

        let model_payload = vec![0u8; MIN_MODEL_FILE_BYTES as usize + 1];
        let info = test_model_info(&model_payload, b"tok");
        let tmp_dir = tempfile::tempdir().unwrap();
        let dest_dir = tmp_dir.path().join("sense-voice");

        let result = download_model_with_progress_to(
            &info,
            vec![server.url()],
            tmp_dir.path().to_path_buf(),
            |_d, _t| {},
        )
        .await;

        assert!(result.is_err());
        assert!(!dest_dir.join(MAIN_MODEL_FILENAME).exists());
        assert!(!dest_dir.join("model.int8.onnx.tmp").exists());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_too_small_detected_as_corrupt() {
        let mut server = mockito::Server::new_async().await;
        let model_payload = vec![0u8; 1024];
        let mock = server
            .mock("GET", "/model.int8.onnx")
            .with_status(200)
            .with_body(&model_payload)
            .create_async()
            .await;

        let info = test_model_info(&model_payload, b"tok");
        let tmp_dir = tempfile::tempdir().unwrap();
        let dest_dir = tmp_dir.path().join("sense-voice");

        let result = download_model_with_progress_to(
            &info,
            vec![server.url()],
            tmp_dir.path().to_path_buf(),
            |_d, _t| {},
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("校验失败"));
        assert!(!dest_dir.join(MAIN_MODEL_FILENAME).exists());
        assert!(!dest_dir.join("model.int8.onnx.tmp").exists());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_retries_then_succeeds() {
        let mut server = mockito::Server::new_async().await;
        let model_payload = vec![0u8; MIN_MODEL_FILE_BYTES as usize + 1];
        let tokens_payload = b"tok";
        let fail_mock = server
            .mock("GET", "/model.int8.onnx")
            .with_status(500)
            .expect_at_least(1)
            .create_async()
            .await;
        let success_mock = server
            .mock("GET", "/model.int8.onnx")
            .with_status(200)
            .with_body(&model_payload)
            .create_async()
            .await;
        let tokens_mock = server
            .mock("GET", "/tokens.txt")
            .with_status(200)
            .with_body(tokens_payload)
            .create_async()
            .await;

        let info = test_model_info(&model_payload, tokens_payload);
        let tmp_dir = tempfile::tempdir().unwrap();

        let result = download_model_with_progress_to(
            &info,
            vec![server.url()],
            tmp_dir.path().to_path_buf(),
            |_d, _t| {},
        )
        .await;

        // mockito 同名 mock 按创建顺序匹配，只要最终成功即可验证重试语义。
        assert!(result.is_ok());
        assert_eq!(
            std::fs::metadata(result.unwrap().join(MAIN_MODEL_FILENAME))
                .unwrap()
                .len(),
            model_payload.len() as u64
        );
        fail_mock.assert_async().await;
        success_mock.assert_async().await;
        tokens_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_skips_when_dest_exists() {
        let model_payload = vec![0; MIN_MODEL_FILE_BYTES as usize];
        let tokens_payload = b"tok";
        let info = test_model_info(&model_payload, tokens_payload);
        let tmp_dir = tempfile::tempdir().unwrap();
        let dest_dir = tmp_dir.path().join("sense-voice");
        std::fs::create_dir_all(&dest_dir).unwrap();
        std::fs::write(dest_dir.join(MAIN_MODEL_FILENAME), &model_payload).unwrap();
        std::fs::write(dest_dir.join(TOKENS_FILENAME), tokens_payload).unwrap();

        // 全部文件已存在：即使下载源不可达也应直接成功
        let result = download_model_with_progress_to(
            &info,
            vec!["http://127.0.0.1:1".to_string()],
            tmp_dir.path().to_path_buf(),
            |_d, _t| {},
        )
        .await;

        assert_eq!(result.unwrap(), dest_dir);
        assert_eq!(
            std::fs::metadata(dest_dir.join(MAIN_MODEL_FILENAME))
                .unwrap()
                .len(),
            MIN_MODEL_FILE_BYTES
        );
    }
}
