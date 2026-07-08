//! Windows 录音器 DSP 工具（纯函数，平台无关）。
//!
//! 处理 cpal 回调返回的采样：f32→i16 转换、多声道降混、重采样。
//! 这些函数不依赖 cpal，因此在 Linux 上也能编译和单元测试（见 issue #20 路线 B）。
//! Windows 录音器（`windows.rs`）在回调和 `stop_recording` 时调用这些函数。

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use crate::error::RecorderError;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

/// 将 f32 采样（范围约 [-1.0, 1.0]）转换为 i16，超范围值钳位到 [-1.0, 1.0]。
pub(crate) fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect()
}

/// 将 i16 采样序列编码为小端（LE）字节，用于写入 `Buffer`。
pub(crate) fn i16_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

/// 将小端（LE）字节解码为 i16 采样（stop 时从 `Buffer` 还原 PCM）。
///
/// 不完整的末尾字节（奇数字节）被丢弃。
pub(crate) fn i16_from_le_bytes(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// 将 N 声道交错的 i16 采样降混为单声道（每帧取均值）。
///
/// 不完整的末尾帧（样本数不是 `channels` 的整数倍）被丢弃。
/// `channels` 为 0 时返回空。
pub(crate) fn downmix_to_mono(samples: &[i16], channels: usize) -> Vec<i16> {
    if channels == 0 {
        return Vec::new();
    }
    samples
        .chunks_exact(channels)
        .map(|frame| {
            let sum: i64 = frame.iter().map(|&x| x as i64).sum();
            (sum / channels as i64) as i16
        })
        .collect()
}

/// 将 i16 采样从 `from_rate` 重采样到 `to_rate`（单声道）。
///
/// 使用 rubato `SincFixedIn` 做一次性整段重采样。同采样率直接返回副本，
/// 空输入返回空。`stop_recording` 在设备采样率 != 16kHz 时调用。
pub(crate) fn resample(
    samples: &[i16],
    from_rate: u32,
    to_rate: u32,
) -> Result<Vec<i16>, RecorderError> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    if from_rate == 0 || to_rate == 0 {
        return Err(RecorderError::CaptureFailed(
            "sample rates must be positive".to_string(),
        ));
    }
    if from_rate == to_rate {
        return Ok(samples.to_vec());
    }

    let ratio = to_rate as f64 / from_rate as f64;
    let chunk_size = 1024usize;
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1)
        .map_err(|e| RecorderError::CaptureFailed(format!("failed to create resampler: {e}")))?;

    // i16 -> f32（单声道，一个声道向量）。
    let frames: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
    let delay = resampler.output_delay();
    let new_length = (samples.len() as f64 * ratio).round() as usize;

    let mut out: Vec<f32> = Vec::new();
    let mut pos = 0;
    while pos < frames.len() {
        let needed = resampler.input_frames_next();
        let avail = frames.len() - pos;
        if avail < needed {
            // 末尾不完整帧。
            let input_buf = vec![frames[pos..].to_vec()];
            let res = resampler
                .process_partial(Some(&input_buf), None)
                .map_err(|e| RecorderError::CaptureFailed(format!("resample failed: {e}")))?;
            if let Some(ch) = res.first() {
                out.extend_from_slice(ch);
            }
            pos = frames.len();
        } else {
            let input_buf = vec![frames[pos..pos + needed].to_vec()];
            let res = resampler
                .process(&input_buf, None)
                .map_err(|e| RecorderError::CaptureFailed(format!("resample failed: {e}")))?;
            if let Some(ch) = res.first() {
                out.extend_from_slice(ch);
            }
            pos += needed;
        }
    }

    // 排空内部缓冲，直到产出足够帧数（new_length + delay）。
    while out.len() < delay + new_length {
        let res = resampler
            .process_partial::<Vec<f32>>(None, None)
            .map_err(|e| RecorderError::CaptureFailed(format!("resample drain failed: {e}")))?;
        match res.first() {
            Some(ch) if !ch.is_empty() => out.extend_from_slice(ch),
            _ => break,
        }
    }

    // 跳过 delay 帧（滤波器预热），取 new_length 帧，再 f32 -> i16。
    let start = delay.min(out.len());
    let end = (start + new_length).min(out.len());
    let result: Vec<i16> = out[start..end]
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_to_i16_converts_basic_values() {
        let out = f32_to_i16(&[0.0, 1.0, -1.0]);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 32767);
        assert_eq!(out[2], -32767);
    }

    #[test]
    fn f32_to_i16_clamps_out_of_range() {
        let out = f32_to_i16(&[2.0, -2.0, 1.5, -1.5, f32::INFINITY, f32::NEG_INFINITY]);
        assert_eq!(out, vec![32767, -32767, 32767, -32767, 32767, -32767]);
    }

    #[test]
    fn f32_to_i16_empty() {
        assert!(f32_to_i16(&[]).is_empty());
    }

    #[test]
    fn i16_to_le_bytes_encodes_known_values() {
        // 0x0100 (256) -> LE bytes [0x00, 0x01]; -1 (0xFFFF) -> [0xFF, 0xFF]
        let bytes = i16_to_le_bytes(&[256, -1, 0]);
        assert_eq!(bytes, vec![0x00, 0x01, 0xFF, 0xFF, 0x00, 0x00]);
    }

    #[test]
    fn i16_to_le_bytes_empty() {
        assert!(i16_to_le_bytes(&[]).is_empty());
    }

    #[test]
    fn i16_from_le_bytes_roundtrips() {
        let samples = vec![256i16, -1, 0, 32000, -32000];
        assert_eq!(i16_from_le_bytes(&i16_to_le_bytes(&samples)), samples);
    }

    #[test]
    fn i16_from_le_bytes_empty() {
        assert!(i16_from_le_bytes(&[]).is_empty());
    }

    #[test]
    fn i16_from_le_bytes_drops_trailing_byte() {
        // 5 bytes = 2 complete i16 + 1 orphan byte (dropped)
        assert_eq!(
            i16_from_le_bytes(&[0x00, 0x01, 0xFF, 0xFF, 0x42]),
            vec![256, -1]
        );
    }

    #[test]
    fn downmix_single_channel_is_passthrough() {
        assert_eq!(downmix_to_mono(&[1, 2, 3], 1), vec![1, 2, 3]);
    }

    #[test]
    fn downmix_stereo_averages_pairs() {
        assert_eq!(downmix_to_mono(&[100, 200, 300, 400], 2), vec![150, 350]);
    }

    #[test]
    fn downmix_four_channels_averages_frames() {
        assert_eq!(downmix_to_mono(&[100, 200, 300, 400], 4), vec![250]);
    }

    #[test]
    fn downmix_sums_without_overflow() {
        // 32000 + 31000 = 63000, must not overflow i16; avg = 31500
        assert_eq!(downmix_to_mono(&[32000, 31000], 2), vec![31500]);
    }

    #[test]
    fn downmix_empty() {
        assert!(downmix_to_mono(&[], 2).is_empty());
    }

    #[test]
    fn downmix_drops_incomplete_trailing_frame() {
        // 3 samples, 2 channels: one complete frame + one orphan (dropped)
        assert_eq!(downmix_to_mono(&[100, 200, 300], 2), vec![150]);
    }

    #[test]
    fn resample_same_rate_is_identity() {
        let input = vec![100i16, -50, 200, 0, 32000, -32000];
        let out = resample(&input, 16000, 16000).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn resample_downsample_length_ratio() {
        // 4800 frames at 48kHz -> ~1600 frames at 16kHz (3:1)
        let input: Vec<i16> = (0..4800).map(|i| (i % 1000) as i16).collect();
        let out = resample(&input, 48000, 16000).unwrap();
        assert!(
            (out.len() as i64 - 1600).abs() <= 4,
            "got len {}",
            out.len()
        );
        assert!(!out.is_empty());
    }

    #[test]
    fn resample_empty() {
        assert!(resample(&[], 48000, 16000).unwrap().is_empty());
    }
}
