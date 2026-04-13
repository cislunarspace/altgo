use std::sync::Mutex;

/// Thread-safe byte buffer for accumulating PCM audio samples.
pub struct Buffer {
    data: Mutex<Vec<u8>>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(Vec::new()),
        }
    }

    pub fn write(&self, chunk: &[u8]) {
        let mut data = self.data.lock().unwrap();
        data.extend_from_slice(chunk);
    }

    /// Returns a copy of the current buffer contents.
    pub fn read_all(&self) -> Vec<u8> {
        self.data.lock().unwrap().clone()
    }

    pub fn reset(&self) {
        self.data.lock().unwrap().clear();
    }

    pub fn len(&self) -> usize {
        self.data.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.lock().unwrap().is_empty()
    }
}

/// Encode raw PCM data into WAV format with a 44-byte header.
pub fn encode_wav(
    pcm_data: &[u8],
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
) -> Vec<u8> {
    assert!(!pcm_data.is_empty(), "PCM data must not be empty");
    assert!(sample_rate > 0, "Sample rate must be positive");
    assert!(bits_per_sample > 0, "Bits per sample must be positive");

    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_size = pcm_data.len() as u32;
    let file_size = 36 + data_size; // RIFF header - 8 + data

    let mut wav = Vec::with_capacity(44 + pcm_data.len());

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm_data);

    wav
}

/// Decode 16-bit PCM WAV data to float32 samples in [-1.0, 1.0].
pub fn decode_wav_to_f32(wav_data: &[u8]) -> Result<Vec<f32>, &'static str> {
    if wav_data.len() < 44 {
        return Err("WAV data too short");
    }
    if &wav_data[0..4] != b"RIFF" || &wav_data[8..12] != b"WAVE" {
        return Err("Not a valid WAV file");
    }

    // Find the data chunk.
    let mut offset = 12u32;
    let mut data_offset = 0u32;
    let mut data_size = 0u32;

    while offset + 8 <= wav_data.len() as u32 {
        let chunk_id = &wav_data[offset as usize..offset as usize + 4];
        let chunk_size = u32::from_le_bytes(
            wav_data[offset as usize + 4..offset as usize + 8]
                .try_into()
                .unwrap(),
        );
        if chunk_id == b"data" {
            data_offset = offset + 8;
            data_size = chunk_size;
            break;
        }
        offset += 8 + chunk_size;
        // Pad to even boundary.
        if chunk_size % 2 != 0 {
            offset += 1;
        }
    }

    if data_offset == 0 {
        return Err("No data chunk found in WAV");
    }

    let end = (data_offset + data_size) as usize;
    let pcm_data = if end <= wav_data.len() {
        &wav_data[data_offset as usize..end]
    } else {
        &wav_data[data_offset as usize..]
    };

    let n_samples = pcm_data.len() / 2;
    let mut samples = Vec::with_capacity(n_samples);
    for i in 0..n_samples {
        let sample = i16::from_le_bytes([pcm_data[i * 2], pcm_data[i * 2 + 1]]);
        samples.push(sample as f32 / 32768.0);
    }

    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_buffer_write_and_read() {
        let buf = Buffer::new();
        buf.write(b"hello");
        buf.write(b" world");
        assert_eq!(buf.read_all(), b"hello world");
        assert_eq!(buf.len(), 11);
    }

    #[test]
    fn test_buffer_reset() {
        let buf = Buffer::new();
        buf.write(b"data");
        buf.reset();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_buffer_read_returns_copy() {
        let buf = Buffer::new();
        buf.write(b"original");
        let copy = buf.read_all();
        buf.reset();
        // The copy should still contain the original data.
        assert_eq!(copy, b"original");
    }

    #[test]
    fn test_buffer_concurrent_access() {
        let buf = Arc::new(Buffer::new());
        let mut handles = vec![];

        for i in 0..100 {
            let buf = Arc::clone(&buf);
            handles.push(thread::spawn(move || {
                buf.write(&[i as u8; 10]);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(buf.len(), 1000);
    }

    #[test]
    fn test_encode_wav_header() {
        let pcm = vec![0u8; 100];
        let wav = encode_wav(&pcm, 16000, 1, 16);

        // RIFF header
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 136); // 36 + 100
        assert_eq!(&wav[8..12], b"WAVE");

        // fmt chunk
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(u32::from_le_bytes(wav[16..20].try_into().unwrap()), 16); // chunk size
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1); // PCM
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1); // channels
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16000); // sample rate
        assert_eq!(u32::from_le_bytes(wav[28..32].try_into().unwrap()), 32000); // byte rate
        assert_eq!(u16::from_le_bytes(wav[32..34].try_into().unwrap()), 2); // block align
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16); // bits

        // data chunk
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 100); // data size
        assert_eq!(&wav[44..], &pcm[..]);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        // Create PCM data with known samples.
        let samples: Vec<i16> = vec![0, 1000, -1000, 32767, -32768];
        let mut pcm = Vec::new();
        for s in &samples {
            pcm.extend_from_slice(&s.to_le_bytes());
        }

        let wav = encode_wav(&pcm, 16000, 1, 16);
        let decoded = decode_wav_to_f32(&wav).unwrap();

        assert_eq!(decoded.len(), samples.len());
        for (i, (orig, dec)) in samples.iter().zip(decoded.iter()).enumerate() {
            let expected = *orig as f32 / 32768.0;
            assert!(
                (dec - expected).abs() < 1e-6,
                "sample {i}: expected {expected}, got {dec}"
            );
        }
    }

    #[test]
    fn test_decode_wav_too_short() {
        assert!(decode_wav_to_f32(b"short").is_err());
    }

    #[test]
    fn test_decode_wav_invalid_header() {
        let data = vec![0u8; 100];
        assert!(decode_wav_to_f32(&data).is_err());
    }
}
