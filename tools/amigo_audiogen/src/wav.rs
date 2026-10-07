//! Reading and writing WAV files for the processing tools.
//!
//! Reads PCM (8, 16, 24 and 32 bit), IEEE float (32 and 64 bit) and
//! `WAVE_FORMAT_EXTENSIBLE` files of those; writes 16-bit PCM. Samples are
//! kept as interleaved `f32` in -1..1.

use crate::processing::AudioBuffer;
use std::path::Path;

/// Decoded audio: interleaved samples, `channels` per frame.
#[derive(Clone, Debug, PartialEq)]
pub struct WavData {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl WavData {
    /// Sample frames (one sample per channel each).
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }

    pub fn duration_secs(&self) -> f32 {
        self.frames() as f32 / self.sample_rate.max(1) as f32
    }

    /// Frame `i` as a slice of one sample per channel.
    pub fn frame(&self, i: usize) -> &[f32] {
        let c = usize::from(self.channels);
        &self.samples[i * c..(i + 1) * c]
    }

    /// The frames `start..end` as a new buffer.
    pub fn slice(&self, start: usize, end: usize) -> WavData {
        let c = usize::from(self.channels);
        WavData {
            samples: self.samples[start * c..end * c].to_vec(),
            ..*self
        }
    }

    /// The channels averaged, for analysis.
    pub fn mono(&self) -> AudioBuffer {
        let c = usize::from(self.channels.max(1));
        AudioBuffer {
            samples: self
                .samples
                .chunks_exact(c)
                .map(|f| f.iter().sum::<f32>() / c as f32)
                .collect(),
            sample_rate: self.sample_rate,
        }
    }

    /// The largest absolute sample.
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    /// The same audio at `sample_rate` with `channels` channels: linear
    /// interpolation between frames, mono spread to every channel and
    /// other layouts averaged down to mono first.
    pub fn conform(&self, sample_rate: u32, channels: u16) -> WavData {
        let src = if self.channels == channels {
            self.clone()
        } else {
            let mono = self.mono().samples;
            WavData {
                sample_rate: self.sample_rate,
                channels,
                samples: mono
                    .iter()
                    .flat_map(|&s| std::iter::repeat_n(s, usize::from(channels)))
                    .collect(),
            }
        };
        if src.sample_rate == sample_rate || src.frames() == 0 {
            return WavData { sample_rate, ..src };
        }
        let c = usize::from(channels);
        let ratio = f64::from(src.sample_rate) / f64::from(sample_rate);
        let frames = (src.frames() as f64 / ratio).floor() as usize;
        let last = src.frames() - 1;
        let mut samples = Vec::with_capacity(frames * c);
        for i in 0..frames {
            let pos = i as f64 * ratio;
            let a = (pos.floor() as usize).min(last);
            let b = (a + 1).min(last);
            let t = (pos - a as f64) as f32;
            for ch in 0..c {
                let (x, y) = (src.samples[a * c + ch], src.samples[b * c + ch]);
                samples.push(x + (y - x) * t);
            }
        }
        WavData {
            sample_rate,
            channels,
            samples,
        }
    }
}

/// Read a WAV file.
pub fn read_wav(path: &Path) -> Result<WavData, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    parse_wav(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Decode WAV bytes.
pub fn parse_wav(bytes: &[u8]) -> Result<WavData, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file (no RIFF/WAVE header)".into());
    }
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);

    let mut format = None;
    let mut data = None;
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32_at(bytes, pos + 4) as usize;
        let body = pos + 8;
        let end = body.saturating_add(len).min(bytes.len());
        match id {
            b"fmt " if end - body >= 16 => {
                let b = &bytes[body..end];
                let mut tag = u16_at(b, 0);
                if tag == 0xFFFE && b.len() >= 26 {
                    // WAVE_FORMAT_EXTENSIBLE: the sub-format GUID starts
                    // with the real tag.
                    tag = u16_at(b, 24);
                }
                format = Some((tag, u16_at(b, 2), u32_at(b, 4), u16_at(b, 14)));
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }
        // Chunks are padded to an even length.
        pos = body.saturating_add(len).saturating_add(len & 1);
    }
    let (tag, channels, sample_rate, bits) = format.ok_or("no fmt chunk")?;
    let data = data.ok_or("no data chunk")?;
    if channels == 0 || sample_rate == 0 {
        return Err("zero channels or sample rate".into());
    }
    let samples: Vec<f32> = match (tag, bits) {
        (1, 8) => data
            .iter()
            .map(|&b| (f32::from(b) - 128.0) / 128.0)
            .collect(),
        (1, 16) => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| f32::from(i16::from_le_bytes(c)) / 32768.0)
            .collect(),
        (1, 24) => data
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.0)
            .collect(),
        (1, 32) => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&c| i32::from_le_bytes(c) as f32 / 2_147_483_648.0)
            .collect(),
        (3, 32) => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&c| f32::from_le_bytes(c))
            .collect(),
        (3, 64) => data
            .as_chunks::<8>()
            .0
            .iter()
            .map(|&c| f64::from_le_bytes(c) as f32)
            .collect(),
        _ => {
            return Err(format!(
                "unsupported WAV encoding (format {tag}, {bits} bit)"
            ));
        }
    };
    let whole = samples.len() - samples.len() % usize::from(channels);
    let mut samples = samples;
    samples.truncate(whole);
    Ok(WavData {
        sample_rate,
        channels,
        samples,
    })
}

/// Encode as 16-bit PCM WAV bytes.
pub fn encode_wav(wav: &WavData) -> Vec<u8> {
    let data_len = (wav.samples.len() * 2) as u32;
    let block = wav.channels * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&wav.channels.to_le_bytes());
    out.extend_from_slice(&wav.sample_rate.to_le_bytes());
    out.extend_from_slice(&(wav.sample_rate * u32::from(block)).to_le_bytes());
    out.extend_from_slice(&block.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in &wav.samples {
        let s = if s.is_finite() {
            s.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        out.extend_from_slice(&((s * 32767.0).round() as i16).to_le_bytes());
    }
    out
}

/// Write `wav` as 16-bit PCM, creating the directory.
pub fn write_wav(path: &Path, wav: &WavData) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    std::fs::write(path, encode_wav(wav))
        .map_err(|e| format!("could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(channels: u16, samples: Vec<f32>) -> WavData {
        WavData {
            sample_rate: 100,
            channels,
            samples,
        }
    }

    #[test]
    fn sixteen_bit_round_trips() {
        let original = wav(2, vec![0.0, 0.5, -0.5, 1.0, -1.0, 0.25]);
        let back = parse_wav(&encode_wav(&original)).unwrap();
        assert_eq!(
            (back.sample_rate, back.channels, back.frames()),
            (100, 2, 3)
        );
        for (a, b) in original.samples.iter().zip(&back.samples) {
            assert!((a - b).abs() < 1e-4, "{a} vs {b}");
        }
    }

    /// A WAV with the given format fields and data, plus an odd-sized chunk
    /// before `fmt ` to check padding is skipped.
    fn raw(tag: u16, bits: u16, channels: u16, data: &[u8]) -> Vec<u8> {
        let mut out = b"RIFF\0\0\0\0WAVE".to_vec();
        out.extend_from_slice(b"LIST\x03\0\0\0abc\0");
        out.extend_from_slice(b"fmt \x10\0\0\0");
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&8000u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn reads_other_encodings() {
        let w = parse_wav(&raw(1, 8, 1, &[128, 255, 0])).unwrap();
        assert_eq!(w.samples[0], 0.0);
        assert!(w.samples[1] > 0.99 && w.samples[2] == -1.0);
        let w = parse_wav(&raw(1, 24, 1, &[0, 0, 0x40, 0, 0, 0xC0])).unwrap();
        assert_eq!(w.samples, [0.5, -0.5]);
        let w = parse_wav(&raw(3, 32, 1, &0.25f32.to_le_bytes())).unwrap();
        assert_eq!(w.samples, [0.25]);
        assert!(
            parse_wav(&raw(2, 4, 1, &[0]))
                .unwrap_err()
                .contains("unsupported")
        );
        assert!(parse_wav(b"OggS").is_err());
    }

    #[test]
    fn conform_changes_rate_and_layout() {
        let stereo = wav(2, vec![1.0, 0.0, 0.0, 1.0]);
        let mono = stereo.conform(100, 1);
        assert_eq!(mono.samples, [0.5, 0.5]);
        let up = wav(1, vec![0.0, 1.0]).conform(200, 2);
        assert_eq!(up.frames(), 4);
        assert_eq!(up.frame(1), [0.5, 0.5]);
        assert_eq!(up.channels, 2);
    }
}
