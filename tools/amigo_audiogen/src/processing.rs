//! Audio post-processing utilities.
//!
//! Trim silence, normalize, detect BPM, find loop points.
//! Operates on raw PCM sample buffers.

use serde::{Deserialize, Serialize};

/// A buffer of audio samples (mono, f32, at a given sample rate).
#[derive(Clone, Debug)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl AudioBuffer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            samples: Vec::new(),
            sample_rate,
        }
    }

    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }

    /// Trim leading and trailing silence below a threshold (in dB).
    pub fn trim_silence(&mut self, threshold_db: f32) {
        let threshold = db_to_linear(threshold_db);

        let start = self
            .samples
            .iter()
            .position(|&s| s.abs() > threshold)
            .unwrap_or(0);

        let end = self
            .samples
            .iter()
            .rposition(|&s| s.abs() > threshold)
            .map(|i| i + 1)
            .unwrap_or(0);

        if start < end {
            self.samples = self.samples[start..end].to_vec();
        } else {
            self.samples.clear();
        }
    }

    /// Normalize peak amplitude to target dB.
    pub fn normalize(&mut self, target_db: f32) {
        let peak = self
            .samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);

        if peak < 1e-10 {
            return; // silence
        }

        let target = db_to_linear(target_db);
        let gain = target / peak;

        for sample in &mut self.samples {
            *sample *= gain;
        }
    }

    /// Detect approximate BPM using autocorrelation on energy envelope.
    pub fn detect_bpm(&self) -> Option<f32> {
        if self.samples.len() < self.sample_rate as usize * 2 {
            return None; // Need at least 2 seconds
        }

        // Compute energy envelope (RMS over windows)
        let window_size = self.sample_rate as usize / 20; // 50ms windows
        let hop = window_size / 2;
        let mut envelope = Vec::new();

        let mut i = 0;
        while i + window_size <= self.samples.len() {
            let rms: f32 = self.samples[i..i + window_size]
                .iter()
                .map(|s| s * s)
                .sum::<f32>()
                / window_size as f32;
            envelope.push(rms.sqrt());
            i += hop;
        }

        if envelope.len() < 100 {
            return None;
        }

        // Autocorrelation to find tempo
        // BPM range: 60-200 → period in envelope frames
        let envelope_rate = self.sample_rate as f32 / hop as f32;
        let min_lag = (envelope_rate * 60.0 / 200.0) as usize; // 200 BPM
        let max_lag = (envelope_rate * 60.0 / 60.0) as usize; // 60 BPM
        let max_lag = max_lag.min(envelope.len() / 2);

        if min_lag >= max_lag {
            return None;
        }

        let mean: f32 = envelope.iter().sum::<f32>() / envelope.len() as f32;
        let mut best_lag = min_lag;
        let mut best_corr = f32::NEG_INFINITY;

        for lag in min_lag..max_lag {
            let mut corr = 0.0f32;
            let n = envelope.len() - lag;
            for j in 0..n {
                corr += (envelope[j] - mean) * (envelope[j + lag] - mean);
            }
            corr /= n as f32;
            if corr > best_corr {
                best_corr = corr;
                best_lag = lag;
            }
        }

        let bpm = 60.0 * envelope_rate / best_lag as f32;
        Some(bpm)
    }

    /// Find the best loop point near the end of the buffer.
    /// Returns the sample index where a seamless loop cut can be made.
    pub fn find_loop_point(&self, target_duration_secs: f32) -> Option<usize> {
        let target_samples = (target_duration_secs * self.sample_rate as f32) as usize;
        if target_samples >= self.samples.len() {
            return None;
        }

        // Search in a window around the target for a zero crossing
        let search_window = (self.sample_rate as usize / 10).min(self.samples.len() / 4);
        let search_start = target_samples.saturating_sub(search_window / 2);
        let search_end = (target_samples + search_window / 2).min(self.samples.len() - 1);

        let mut best_idx = target_samples;
        let mut best_score = f32::MAX;

        for i in search_start..search_end {
            // Score: prefer zero crossings with low energy
            let zero_dist = self.samples[i].abs();
            let matches_start = (self.samples[i] - self.samples[0]).abs();
            let score = zero_dist + matches_start * 0.5;

            if score < best_score {
                best_score = score;
                best_idx = i;
            }
        }

        Some(best_idx)
    }

    /// Apply a short crossfade at the loop point for seamless looping.
    pub fn apply_loop_crossfade(&mut self, loop_point: usize, crossfade_samples: usize) {
        if loop_point >= self.samples.len() || crossfade_samples == 0 {
            return;
        }

        let fade_len = crossfade_samples.min(loop_point).min(self.samples.len() - loop_point);

        for i in 0..fade_len {
            let t = i as f32 / fade_len as f32;
            let end_idx = loop_point + i;
            let start_idx = i;

            if end_idx < self.samples.len() {
                // Crossfade: blend end of loop with start
                self.samples[end_idx] =
                    self.samples[end_idx] * (1.0 - t) + self.samples[start_idx] * t;
            }
        }
    }
}

/// Convert decibels to linear amplitude.
pub fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

/// Convert linear amplitude to decibels.
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        -120.0
    } else {
        20.0 * linear.log10()
    }
}

/// Adaptive music config that can be generated from stem analysis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdaptiveMusicConfig {
    pub section_name: String,
    pub bpm: f32,
    pub beats_per_bar: u32,
    pub layers: Vec<LayerConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerConfig {
    pub name: String,
    pub stem_file: String,
    pub base_volume: f32,
    pub rule: LayerRule,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LayerRule {
    AlwaysOn,
    Threshold { param: String, above: f32, fade_secs: f32 },
    Lerp { param: String, from: f32, to: f32 },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_conversions() {
        let linear = db_to_linear(-6.0);
        assert!((linear - 0.501).abs() < 0.01);

        let db = linear_to_db(1.0);
        assert!((db - 0.0).abs() < 0.001);

        assert!(linear_to_db(0.0) < -100.0);
    }

    #[test]
    fn trim_silence_basic() {
        let mut buf = AudioBuffer {
            samples: vec![0.0, 0.0, 0.5, 0.8, 0.3, 0.0, 0.0],
            sample_rate: 44100,
        };
        buf.trim_silence(-40.0); // threshold ~0.01
        assert_eq!(buf.samples.len(), 3); // [0.5, 0.8, 0.3]
        assert_eq!(buf.samples[0], 0.5);
    }

    #[test]
    fn normalize_peak() {
        let mut buf = AudioBuffer {
            samples: vec![0.0, 0.25, -0.5, 0.1],
            sample_rate: 44100,
        };
        buf.normalize(0.0); // normalize to 0dB = 1.0 peak
        let peak = buf.samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!((peak - 1.0).abs() < 0.001);
    }

    #[test]
    fn normalize_silence_noop() {
        let mut buf = AudioBuffer {
            samples: vec![0.0, 0.0, 0.0],
            sample_rate: 44100,
        };
        buf.normalize(0.0);
        assert_eq!(buf.samples, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn duration_calculation() {
        let buf = AudioBuffer {
            samples: vec![0.0; 44100],
            sample_rate: 44100,
        };
        assert!((buf.duration_secs() - 1.0).abs() < 0.001);
    }

    #[test]
    fn find_loop_point_returns_some() {
        let mut buf = AudioBuffer::new(1000);
        // Create a simple waveform
        for i in 0..3000 {
            buf.samples.push((i as f32 * 0.01).sin());
        }
        let point = buf.find_loop_point(2.0);
        assert!(point.is_some());
        let p = point.unwrap();
        assert!(p > 1500 && p < 2500);
    }
}
