//! 线程数配置的中性工具模块。
//!
//! 把 `effective_threads` 从 `whisper_server` 提取出来，供 transcriber 等
//! 多个模块复用，避免跨模块依赖 `whisper_server` 的内部函数。

/// whisper-server 默认线程数下限的回退值（取不到 CPU 并行度时使用）。
const DEFAULT_THREADS_FALLBACK: u32 = 4;

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
