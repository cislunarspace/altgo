//! 常驻 whisper-server 语音识别后端。
//!
//! 与一次性 `whisper-cli` 不同，`ResidentWhisper` 在管道启动时 **一次性** 拉起
//! `whisper-server` 子进程：模型常驻内存，之后每句话只发一次本地 HTTP 请求，
//! 不再重复支付「从磁盘冷载入模型」的成本（medium ≈ 1.5GB / large ≈ 2.9GB，
//! 冷载入往往比转写本身还久）。
//!
//! 服务器只监听 `127.0.0.1`（无需鉴权），随 `ResidentWhisper` 的最后一个克隆一同
//! `Drop` 时被杀死。任何失败路径（二进制缺失、端口冲突、就绪超时、运行期崩溃）
//! 都会 **回退到内置的一次性 `LocalWhisper`**，因此即使旧安装只带了 `whisper-cli`
//! 也能照常工作。

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::sync::OnceCell;

use crate::error::TranscriberError;
use crate::resource::expand_tilde;
use crate::transcriber::{LocalWhisper, TranscribeResult, Transcriber};

/// 就绪探测总预算 —— large 模型从磁盘冷载入可能数十秒。
const READY_TIMEOUT: Duration = Duration::from_secs(120);
/// 就绪探测轮询间隔。
const READY_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// 单次 `/health` 探测请求超时。
const HEALTH_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// whisper-server 默认线程数下限的回退值（取不到 CPU 并行度时使用）。
const DEFAULT_THREADS_FALLBACK: u32 = 4;

/// `/inference` 默认返回 `{"text": "..."}`（`response_format=json`）。
#[derive(Debug, Deserialize)]
struct InferenceResponse {
    text: String,
}

/// 持有 whisper-server 子进程；在最后一个克隆 `Drop` 时杀死它。
struct ServerProc {
    child: Mutex<Option<tokio::process::Child>>,
}

impl Drop for ServerProc {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(child) = guard.as_mut() {
                // start_kill 发送 SIGKILL 且不阻塞，适合在 Drop 中调用。
                let _ = child.start_kill();
            }
        }
    }
}

/// 常驻 whisper-server 后端。
///
/// `Clone` 极廉价：子进程句柄与就绪状态共享在 `Arc` 后，普通字段为浅拷贝。
/// 这让 `handle_stop_record` 中 `transcriber.clone()` 进 `tokio::spawn` 的现有写法
/// 能在转写在途时保活服务器。
#[derive(Clone)]
pub struct ResidentWhisper {
    /// 子进程；spawn 失败时其 `child` 为 `None`。
    /// 仅为持有生命周期而存在（最后一个克隆 `Drop` 时杀死服务器），不直接读取。
    #[allow(dead_code)]
    proc: Arc<ServerProc>,
    /// 就绪探测的一次性结果（`Ok` = 可服务，`Err` = 探测失败）。
    ready: Arc<OnceCell<Result<(), String>>>,
    /// 服务器基址，如 `http://127.0.0.1:38123`；spawn 失败时为空。
    base_url: String,
    /// spawn 是否成功 —— 为 `false` 时直接走回退，不做就绪探测。
    spawned: bool,
    client: Client,
    language: String,
    temperature: f32,
    /// beam search 宽度；`<= 1` 时使用贪心（最快）。
    beam_size: u32,
    /// 服务器不可用时的一次性回退后端。
    fallback: LocalWhisper,
}

impl std::fmt::Debug for ResidentWhisper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResidentWhisper")
            .field("base_url", &self.base_url)
            .field("spawned", &self.spawned)
            .field("language", &self.language)
            .field("beam_size", &self.beam_size)
            .finish()
    }
}

impl ResidentWhisper {
    /// 创建常驻后端并 **立即** 拉起 whisper-server（模型在管道启动时即开始载入，
    /// 与「用户尚未开口」的时间重叠）。
    ///
    /// 永不返回错误：spawn 失败时降级为纯回退模式，转写时走一次性 `whisper-cli`。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model_path: String,
        language: String,
        whisper_path: String,
        temperature: f32,
        threads: u32,
        beam_size: u32,
        timeout: Duration,
    ) -> Self {
        let fallback = LocalWhisper::new(
            model_path.clone(),
            language.clone(),
            whisper_path.clone(),
            threads,
            beam_size,
        );

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "failed to build whisper-server HTTP client; using default");
                Client::new()
            });

        match spawn_server(&model_path, &language, &whisper_path, threads, beam_size) {
            Ok((child, port)) => {
                tracing::info!(port, "whisper-server spawned; model resident");
                Self {
                    proc: Arc::new(ServerProc {
                        child: Mutex::new(Some(child)),
                    }),
                    ready: Arc::new(OnceCell::new()),
                    base_url: format!("http://127.0.0.1:{}", port),
                    spawned: true,
                    client,
                    language,
                    temperature,
                    beam_size,
                    fallback,
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "whisper-server unavailable; transcription will fall back to one-shot whisper-cli"
                );
                Self {
                    proc: Arc::new(ServerProc {
                        child: Mutex::new(None),
                    }),
                    ready: Arc::new(OnceCell::new()),
                    base_url: String::new(),
                    spawned: false,
                    client,
                    language,
                    temperature,
                    beam_size,
                    fallback,
                }
            }
        }
    }

    /// 转写音频：优先走常驻服务器，任何失败都回退到一次性 `whisper-cli`。
    ///
    /// `progress` 仅在回退路径生效（服务器无流式进度）。
    pub async fn transcribe(
        &self,
        audio_data: &[u8],
        progress: Option<UnboundedSender<f32>>,
    ) -> Result<TranscribeResult, TranscriberError> {
        if audio_data.is_empty() {
            return Err(TranscriberError::EmptyAudio);
        }

        if self.ensure_ready().await.is_ok() {
            match self.transcribe_via_server(audio_data).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "resident whisper-server transcription failed; falling back to whisper-cli"
                    );
                }
            }
        } else if self.spawned {
            tracing::warn!("whisper-server not ready in time; falling back to whisper-cli");
        }

        self.fallback.transcribe(audio_data, progress).await
    }

    /// 一次性探测服务器就绪（结果缓存）。spawn 失败时立即返回 `Err`。
    async fn ensure_ready(&self) -> Result<(), String> {
        if !self.spawned {
            return Err("whisper-server not started".to_string());
        }
        self.ready
            .get_or_init(|| async { self.probe_ready().await })
            .await
            .clone()
    }

    /// 轮询 `GET /health` 直到 200 或超时。
    async fn probe_ready(&self) -> Result<(), String> {
        let url = format!("{}/health", self.base_url);
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            match self
                .client
                .get(&url)
                .timeout(HEALTH_PROBE_TIMEOUT)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => return Ok(()),
                _ => {}
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "whisper-server readiness timed out after {:?}",
                    READY_TIMEOUT
                ));
            }
            tokio::time::sleep(READY_POLL_INTERVAL).await;
        }
    }

    /// 向 `POST /inference` 发送 multipart 请求（field `file` = 原始 16kHz WAV）。
    async fn transcribe_via_server(
        &self,
        audio_data: &[u8],
    ) -> Result<TranscribeResult, TranscriberError> {
        let url = format!("{}/inference", self.base_url);

        let audio_part = reqwest::multipart::Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| TranscriberError::HttpError(e.to_string()))?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", audio_part)
            .text("response_format", "json")
            .text("no_timestamps", "true")
            .text("temperature", format!("{}", self.temperature));

        if !self.language.is_empty() {
            form = form.text("language", self.language.clone());
        }
        if self.beam_size > 1 {
            form = form.text("beam_size", format!("{}", self.beam_size));
        }

        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| TranscriberError::HttpError(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "failed to read error body".to_string());
            return Err(TranscriberError::ApiError {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: InferenceResponse = resp
            .json()
            .await
            .map_err(|e| TranscriberError::JsonError(e.to_string()))?;

        Ok(TranscribeResult {
            text: parsed.text.trim().to_string(),
            language: self.language.clone(),
        })
    }
}

impl Transcriber for ResidentWhisper {
    fn transcribe<'life0, 'life1>(
        &'life0 self,
        audio: &'life1 [u8],
        on_progress: Arc<dyn Fn(f32) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<TranscribeResult, TranscriberError>> + Send + 'life0>>
    where
        'life1: 'life0,
    {
        Box::pin(async move {
            let cb = Arc::clone(&on_progress);
            let (tx, mut rx) = unbounded_channel::<f32>();
            let forward = tokio::spawn(async move {
                while let Some(fr) = rx.recv().await {
                    (cb)(fr);
                }
            });
            let result = ResidentWhisper::transcribe(self, audio, Some(tx)).await;
            let _ = forward.await;
            if result.is_ok() {
                (on_progress)(1.0);
            }
            result
        })
    }
}

/// 解析有效线程数：配置 `> 0` 时用配置值，否则用 CPU 并行度（默认 whisper 仅用 min(4,hw)，
/// 显式给满核数能直接提速）。
pub fn effective_threads(configured: u32) -> u32 {
    if configured > 0 {
        configured
    } else {
        std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(DEFAULT_THREADS_FALLBACK)
    }
}

/// 从 127.0.0.1 上获取一个空闲端口（绑定后立即释放，交给 whisper-server 复用）。
fn pick_free_port() -> std::io::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

/// 拉起 whisper-server 子进程，返回 `(child, port)`。
fn spawn_server(
    model_path: &str,
    language: &str,
    whisper_path: &str,
    threads: u32,
    beam_size: u32,
) -> Result<(tokio::process::Child, u16), String> {
    let bin = find_whisper_server(whisper_path)?;
    let model = expand_tilde(model_path);
    let port = pick_free_port().map_err(|e| format!("allocate local port: {}", e))?;

    let mut cmd = tokio::process::Command::new(&bin);
    cmd.arg("-m")
        .arg(&model)
        .arg("-l")
        .arg(language)
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("-t")
        .arg(effective_threads(threads).to_string());

    if beam_size > 1 {
        cmd.arg("-bs").arg(beam_size.to_string());
    }

    // 丢弃 server 的 stdout/stderr，避免管道缓冲写满导致进程阻塞。
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let child = cmd
        .spawn()
        .map_err(|e| format!("spawn whisper-server ({}): {}", bin.display(), e))?;

    Ok((child, port))
}

/// 查找 whisper-server 二进制：
/// 1. 已配置 `whisper_path`（cli 路径）的同级目录
/// 2. 捆绑安装位置
/// 3. 系统 PATH
fn find_whisper_server(whisper_path: &str) -> Result<PathBuf, String> {
    if !whisper_path.is_empty() {
        if let Some(dir) = Path::new(whisper_path).parent() {
            #[cfg(windows)]
            {
                let candidate = dir.join("whisper-server.exe");
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
            let candidate = dir.join("whisper-server");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    if let Some(bundled) = crate::resource::bundled_bin("whisper-server") {
        return Ok(bundled);
    }

    if let Ok(found) = which_whisper_server() {
        return Ok(found);
    }

    Err("whisper-server not found (sibling of whisper_path, bundled bin, or PATH)".to_string())
}

/// 在 PATH 上查找 `whisper-server`。
#[cfg(unix)]
fn which_whisper_server() -> Result<PathBuf, String> {
    let output = std::process::Command::new("which")
        .arg("whisper-server")
        .output()
        .map_err(|e| format!("which whisper-server: {}", e))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }
    Err("whisper-server not on PATH".to_string())
}

/// 在 PATH 上查找 `whisper-server.exe`（Windows）。
#[cfg(windows)]
fn which_whisper_server() -> Result<PathBuf, String> {
    let output = std::process::Command::new("where")
        .arg("whisper-server")
        .output()
        .map_err(|e| format!("where whisper-server: {}", e))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // `where` 可能返回多行，取第一行。
        let first_line = stdout.lines().next().unwrap_or("").trim();
        let p = PathBuf::from(first_line);
        if p.exists() {
            return Ok(p);
        }
    }
    Err("whisper-server not on PATH".to_string())
}

#[cfg(test)]
impl ResidentWhisper {
    /// 测试构造器：跳过真实 spawn，直接指向给定基址并预设为就绪。
    fn for_test(base_url: String) -> Self {
        let ready = OnceCell::new();
        let _ = ready.set(Ok(()));
        Self {
            proc: Arc::new(ServerProc {
                child: Mutex::new(None),
            }),
            ready: Arc::new(ready),
            base_url,
            spawned: true,
            client: Client::new(),
            language: "zh".to_string(),
            temperature: 0.0,
            beam_size: 0,
            fallback: LocalWhisper::new(
                "/nonexistent/model".to_string(),
                "zh".to_string(),
                String::new(),
                0,
                0,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_threads_respects_config() {
        assert_eq!(effective_threads(8), 8);
        // 0 -> 自动探测（至少 1）
        assert!(effective_threads(0) >= 1);
    }

    #[test]
    fn test_pick_free_port_returns_nonzero() {
        let port = pick_free_port().unwrap();
        assert!(port > 0);
    }

    #[test]
    fn test_find_whisper_server_missing() {
        // 空配置 + 大概率不存在的捆绑/PATH -> 应报错而非 panic。
        // （CI 环境通常没有 whisper-server）
        let _ = find_whisper_server("/definitely/not/here/whisper-cli");
    }

    #[tokio::test]
    async fn test_transcribe_empty_audio() {
        let rw = ResidentWhisper::for_test("http://127.0.0.1:1".to_string());
        let result = rw.transcribe(&[], None).await;
        assert!(matches!(result, Err(TranscriberError::EmptyAudio)));
    }

    #[tokio::test]
    async fn test_transcribe_via_server_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/inference")
            .with_status(200)
            .with_body(serde_json::json!({ "text": "  你好世界  " }).to_string())
            .create_async()
            .await;

        let rw = ResidentWhisper::for_test(server.url());
        let result = rw.transcribe_via_server(&[0u8; 44]).await.unwrap();
        assert_eq!(result.text, "你好世界");
        assert_eq!(result.language, "zh");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_transcribe_via_server_api_error() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/inference")
            .with_status(500)
            .with_body("boom")
            .create_async()
            .await;

        let rw = ResidentWhisper::for_test(server.url());
        let result = rw.transcribe_via_server(&[0u8; 44]).await;
        assert!(matches!(
            result,
            Err(TranscriberError::ApiError { status: 500, .. })
        ));
    }

    #[tokio::test]
    async fn test_transcribe_falls_back_when_not_spawned() {
        // spawned=false -> ensure_ready Err -> 回退到 LocalWhisper（指向不存在模型，故报错而非 panic）。
        let rw = ResidentWhisper::new(
            "/nonexistent/model".to_string(),
            "zh".to_string(),
            "/nonexistent/whisper-cli".to_string(),
            0.0,
            0,
            0,
            Duration::from_secs(5),
        );
        assert!(!rw.spawned);
        let result = rw.transcribe(&[0u8; 44], None).await;
        // 回退路径会因 cli/model 缺失而失败，但不应 panic。
        assert!(result.is_err());
    }
}
