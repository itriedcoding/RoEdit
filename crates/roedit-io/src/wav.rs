//! WAV audio reading (PCM 8/16/24/32-bit and IEEE float 32/64) and PCM16 writing.

use crate::ImportError;

#[derive(Clone, Debug, PartialEq)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    /// Interleaved samples in [-1, 1].
    pub samples: Vec<f32>,
}

impl AudioBuffer {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate: sample_rate.max(1),
            channels: channels.max(1),
            samples: Vec::new(),
        }
    }

    pub fn frame_count(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn duration_secs(&self) -> f32 {
        self.frame_count() as f32 / self.sample_rate as f32
    }

    /// Equal-power-free straight average down to mono.
    pub fn mixdown(&self) -> Vec<f32> {
        let ch = self.channels.max(1) as usize;
        if ch == 1 {
            return self.samples.clone();
        }
        self.samples
            .chunks(ch)
            .map(|frame| frame.iter().sum::<f32>() / ch as f32)
            .collect()
    }

    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    pub fn gain(&mut self, gain: f32) {
        for s in &mut self.samples {
            *s = (*s * gain).clamp(-1.0, 1.0);
        }
    }

    /// Linear resampling to a new rate (adequate for preview and export mixes).
    pub fn resample(&self, new_rate: u32) -> AudioBuffer {
        if new_rate == self.sample_rate || self.samples.is_empty() {
            return self.clone();
        }
        let ch = self.channels as usize;
        let frames = self.frame_count();
        let ratio = self.sample_rate as f64 / new_rate as f64;
        let out_frames = ((frames as f64) / ratio).round() as usize;
        let mut out = AudioBuffer::new(new_rate, self.channels);
        out.samples.reserve(out_frames * ch);
        for f in 0..out_frames {
            let src = f as f64 * ratio;
            let i0 = src.floor() as usize;
            let i1 = (i0 + 1).min(frames.saturating_sub(1));
            let t = (src - i0 as f64) as f32;
            for c in 0..ch {
                let a = self.samples.get(i0 * ch + c).copied().unwrap_or(0.0);
                let b = self.samples.get(i1 * ch + c).copied().unwrap_or(a);
                out.samples.push(a + (b - a) * t);
            }
        }
        out
    }

    /// Mixes another buffer in at `offset_secs` with a volume, growing this
    /// buffer as needed.
    pub fn mix_into(&mut self, other: &AudioBuffer, offset_secs: f32, volume: f32) {
        let other = if other.sample_rate != self.sample_rate {
            other.resample(self.sample_rate)
        } else {
            other.clone()
        };
        let ch = self.channels.max(1) as usize;
        let och = other.channels.max(1) as usize;
        let offset_frames = (offset_secs.max(0.0) * self.sample_rate as f32).round() as usize;
        let needed = (offset_frames + other.frame_count()) * ch;
        if self.samples.len() < needed {
            self.samples.resize(needed, 0.0);
        }
        for f in 0..other.frame_count() {
            for c in 0..ch {
                let src = other
                    .samples
                    .get(f * och + (c.min(och - 1)))
                    .copied()
                    .unwrap_or(0.0);
                let dst = self.samples.get_mut((offset_frames + f) * ch + c);
                if let Some(dst) = dst {
                    *dst = (*dst + src * volume).clamp(-1.0, 1.0);
                }
            }
        }
    }
}

fn read_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn read_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Parses a WAV file.
pub fn read_wav(bytes: &[u8]) -> Result<AudioBuffer, ImportError> {
    if bytes.len() < 44 || !bytes.starts_with(b"RIFF") || &bytes[8..12] != b"WAVE" {
        return Err(ImportError::Audio("not a RIFF/WAVE file".into()));
    }
    let mut offset = 12;
    let mut format = 1u16;
    let mut channels = 1u16;
    let mut sample_rate = 44_100u32;
    let mut bits = 16u16;
    let mut data: Option<&[u8]> = None;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = read_u32(bytes, offset + 4) as usize;
        let start = offset + 8;
        if start + size > bytes.len() {
            return Err(ImportError::Audio("chunk length exceeds file size".into()));
        }
        let chunk = &bytes[start..start + size];
        match id {
            b"fmt " => {
                if chunk.len() < 16 {
                    return Err(ImportError::Audio("fmt chunk is truncated".into()));
                }
                format = read_u16(chunk, 0);
                channels = read_u16(chunk, 2).max(1);
                sample_rate = read_u32(chunk, 4).max(1);
                bits = read_u16(chunk, 14);
                if format == 0xFFFE && chunk.len() >= 26 {
                    // WAVE_FORMAT_EXTENSIBLE: the real format is the first two
                    // bytes of the sub-format GUID.
                    format = read_u16(chunk, 24);
                }
            }
            b"data" => data = Some(chunk),
            _ => {}
        }
        offset = start + size + (size & 1);
    }
    let data = data.ok_or_else(|| ImportError::Audio("missing data chunk".into()))?;
    let channels = channels.max(1);
    let mut samples: Vec<f32> = Vec::with_capacity(data.len());
    match (format, bits) {
        (1, 8) => {
            for b in data {
                samples.push((*b as f32 - 128.0) / 128.0);
            }
        }
        (1, 16) => {
            for c in data.chunks_exact(2) {
                samples.push(i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0);
            }
        }
        (1, 24) => {
            for c in data.chunks_exact(3) {
                let v = ((c[2] as i32) << 16 | (c[1] as i32) << 8 | c[0] as i32) << 8 >> 8;
                samples.push(v as f32 / 8_388_608.0);
            }
        }
        (1, 32) => {
            for c in data.chunks_exact(4) {
                samples.push(i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.0);
            }
        }
        (3, 32) => {
            for c in data.chunks_exact(4) {
                samples.push(f32::from_le_bytes([c[0], c[1], c[2], c[3]]).clamp(-1.0, 1.0));
            }
        }
        (3, 64) => {
            for c in data.chunks_exact(8) {
                let mut b = [0u8; 8];
                b.copy_from_slice(c);
                samples.push((f64::from_le_bytes(b) as f32).clamp(-1.0, 1.0));
            }
        }
        (f, b) => {
            return Err(ImportError::Audio(format!(
                "unsupported WAV encoding (format {f}, {b}-bit)"
            )))
        }
    }
    Ok(AudioBuffer {
        sample_rate,
        channels,
        samples,
    })
}

/// Encodes a buffer as 16-bit PCM WAV.
pub fn write_wav_pcm16(buffer: &AudioBuffer) -> Vec<u8> {
    let channels = buffer.channels.max(1);
    let data_len = buffer.samples.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&buffer.sample_rate.to_le_bytes());
    out.extend_from_slice(&(buffer.sample_rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in &buffer.samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm16_roundtrip() {
        let mut buf = AudioBuffer::new(48_000, 2);
        for i in 0..480 {
            let t = i as f32 / 48_000.0;
            buf.samples
                .push((t * 440.0 * std::f32::consts::TAU).sin() * 0.5);
            buf.samples.push(0.25);
        }
        let bytes = write_wav_pcm16(&buf);
        assert_eq!(&bytes[0..4], b"RIFF");
        let back = read_wav(&bytes).unwrap();
        assert_eq!(back.sample_rate, 48_000);
        assert_eq!(back.channels, 2);
        assert_eq!(back.frame_count(), 480);
        assert!((back.duration_secs() - 0.01).abs() < 1e-4);
        assert!((back.samples[1] - 0.25).abs() < 1e-3);
        assert!(back.samples[0].abs() < 1e-3);
    }

    #[test]
    fn mixdown_and_peak() {
        let mut buf = AudioBuffer::new(1000, 2);
        buf.samples = vec![1.0, 0.0, 0.0, 0.0, -0.5, -0.5];
        assert_eq!(buf.frame_count(), 3);
        let mono = buf.mixdown();
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.5).abs() < 1e-6);
        assert!((buf.peak() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn resample_preserves_duration() {
        let mut buf = AudioBuffer::new(48_000, 1);
        buf.samples = vec![0.0; 48_000];
        let out = buf.resample(24_000);
        assert_eq!(out.sample_rate, 24_000);
        assert!((out.duration_secs() - 1.0).abs() < 1e-2);
    }

    #[test]
    fn mixes_at_offset() {
        let mut base = AudioBuffer::new(1000, 1);
        base.samples = vec![0.0; 1000];
        let mut other = AudioBuffer::new(1000, 1);
        other.samples = vec![0.5; 100];
        base.mix_into(&other, 0.5, 1.0);
        assert!((base.samples[500] - 0.5).abs() < 1e-6);
        assert!(base.samples[100].abs() < 1e-6);
    }

    #[test]
    fn rejects_non_wav() {
        assert!(read_wav(b"nope").is_err());
        let mut bad = write_wav_pcm16(&AudioBuffer::new(8000, 1));
        bad[8] = b'X';
        assert!(read_wav(&bad).is_err());
    }
}
