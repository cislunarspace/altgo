//! 音频处理管道模块。
//!
//! 提供 `PipelineOutput` 数据结构，供管道各阶段使用。

/// 管道处理结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PipelineOutput {
    /// 处理后的文本（润色成功时为润色文本，否则为原始转写文本）
    pub text: String,
    /// 原始转写文本（润色前）
    pub raw_text: String,
    /// 润色是否失败
    pub polish_failed: bool,
}
