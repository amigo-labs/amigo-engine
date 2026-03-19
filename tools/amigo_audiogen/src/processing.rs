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
        let peak = self.samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

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

        let fade_len = crossfade_samples
            .min(loop_point)
            .min(self.samples.len() - loop_point);

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

// ---------------------------------------------------------------------------
// Audio Post-Processing Pipeline (RS-22)
// ---------------------------------------------------------------------------

/// Steps in the audio post-processing pipeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostProcessStep {
    Normalize,
    BpmDetect,
    BarSnap,
    LoopCrossfade,
    SpectralValidation,
}

/// Result of spectral validation against a target style.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpectralReport {
    /// Whether the audio passes validation.
    pub passed: bool,
    /// Frequency band energy ratios (low, mid, high).
    pub band_energies: [f32; 3],
    /// Diagnostic message.
    pub message: String,
}

/// Result of running the full post-processing pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostProcessResult {
    pub detected_bpm: Option<f32>,
    pub bar_snap_samples: Option<usize>,
    pub loop_point: Option<usize>,
    pub spectral_report: Option<SpectralReport>,
    pub duration_secs: f32,
}

impl AudioBuffer {
    /// Snap audio length to the nearest bar boundary based on BPM.
    /// A bar = `beats_per_bar` beats at the given BPM.
    pub fn bar_snap(&mut self, bpm: f32, beats_per_bar: u32) {
        if bpm <= 0.0 || beats_per_bar == 0 {
            return;
        }
        let samples_per_beat = self.sample_rate as f32 * 60.0 / bpm;
        let samples_per_bar = (samples_per_beat * beats_per_bar as f32) as usize;
        if samples_per_bar == 0 || self.samples.is_empty() {
            return;
        }
        // Round to nearest complete bar
        let num_bars = (self.samples.len() as f32 / samples_per_bar as f32).round() as usize;
        let target_len = (num_bars.max(1) * samples_per_bar).min(self.samples.len());
        self.samples.truncate(target_len);
    }

    /// Simple spectral validation: compute energy in low/mid/high bands
    /// and check against expected ranges for a given style.
    pub fn spectral_validate(&self, style_hint: &str) -> SpectralReport {
        if self.samples.is_empty() {
            return SpectralReport {
                passed: false,
                band_energies: [0.0, 0.0, 0.0],
                message: "Empty audio buffer".into(),
            };
        }

        // Compute energy in 3 frequency bands using simple windowed analysis.
        // Low: 0-300Hz, Mid: 300-3000Hz, High: 3000Hz+
        // We approximate using sample-domain energy of differently smoothed signals.
        let total_energy: f32 =
            self.samples.iter().map(|s| s * s).sum::<f32>() / self.samples.len() as f32;

        if total_energy < 1e-10 {
            return SpectralReport {
                passed: false,
                band_energies: [0.0, 0.0, 0.0],
                message: "Silent audio".into(),
            };
        }

        // Low-pass approximation: running average with large window
        let low_window = (self.sample_rate as usize / 300).max(1);
        let low_energy = windowed_energy(&self.samples, low_window);

        // High-pass approximation: difference signal
        let high_energy = high_pass_energy(&self.samples);

        // Mid = total - low - high (clamped)
        let mid_energy = (total_energy - low_energy - high_energy).max(0.0);

        let sum = low_energy + mid_energy + high_energy;
        let band_energies = if sum > 0.0 {
            [low_energy / sum, mid_energy / sum, high_energy / sum]
        } else {
            [0.33, 0.34, 0.33]
        };

        // Style-specific validation
        let (passed, message) = match style_hint {
            "ambient" | "drone" => {
                if band_energies[0] > 0.15 {
                    (true, "Ambient profile: good low-end presence".into())
                } else {
                    (false, "Ambient expected more low-end energy".into())
                }
            }
            "battle" | "intense" => {
                if band_energies[2] > 0.1 {
                    (true, "Battle profile: good high-frequency energy".into())
                } else {
                    (false, "Battle expected more high-frequency energy".into())
                }
            }
            _ => (true, format!("Style '{}': generic pass", style_hint)),
        };

        SpectralReport {
            passed,
            band_energies,
            message,
        }
    }
}

/// Run the full post-processing pipeline on an AudioBuffer.
/// Order: Normalize → BPM Detect → Bar Snap → Loop Crossfade → Spectral Validate
pub fn run_pipeline(
    buf: &mut AudioBuffer,
    normalize_db: f32,
    beats_per_bar: u32,
    target_loop_secs: Option<f32>,
    style_hint: &str,
) -> PostProcessResult {
    // 1. Normalize
    buf.normalize(normalize_db);

    // 2. BPM Detect
    let detected_bpm = buf.detect_bpm();

    // 3. Bar Snap
    let bar_snap_samples = if let Some(bpm) = detected_bpm {
        let before = buf.samples.len();
        buf.bar_snap(bpm, beats_per_bar);
        Some(before - buf.samples.len())
    } else {
        None
    };

    // 4. Loop Crossfade
    let loop_point = if let Some(target_secs) = target_loop_secs {
        let lp = buf.find_loop_point(target_secs);
        if let Some(point) = lp {
            let crossfade = (buf.sample_rate as usize / 100).max(64); // ~10ms
            buf.apply_loop_crossfade(point, crossfade);
        }
        lp
    } else {
        None
    };

    // 5. Spectral Validation
    let spectral_report = Some(buf.spectral_validate(style_hint));

    let duration_secs = buf.duration_secs();

    PostProcessResult {
        detected_bpm,
        bar_snap_samples,
        loop_point,
        spectral_report,
        duration_secs,
    }
}

fn windowed_energy(samples: &[f32], window: usize) -> f32 {
    if samples.len() < window * 2 {
        return 0.0;
    }
    // Compute smoothed signal then measure energy
    let mut smoothed = vec![0.0f32; samples.len()];
    let mut sum = 0.0f32;
    for i in 0..samples.len() {
        sum += samples[i];
        if i >= window {
            sum -= samples[i - window];
        }
        let count = (i + 1).min(window) as f32;
        smoothed[i] = sum / count;
    }
    smoothed.iter().map(|s| s * s).sum::<f32>() / smoothed.len() as f32
}

fn high_pass_energy(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    // Simple first-order difference as high-pass
    let mut energy = 0.0f32;
    for i in 1..samples.len() {
        let d = samples[i] - samples[i - 1];
        energy += d * d;
    }
    energy / (samples.len() - 1) as f32
}

// ---------------------------------------------------------------------------
// Adaptive music config
// ---------------------------------------------------------------------------

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
    Threshold {
        param: String,
        above: f32,
        fade_secs: f32,
    },
    Lerp {
        param: String,
        from: f32,
        to: f32,
    },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── dB conversions ─────────────────────────────────────────

    #[test]
    fn db_conversions() {
        let linear = db_to_linear(-6.0);
        assert!((linear - 0.501).abs() < 0.01);

        let db = linear_to_db(1.0);
        assert!((db - 0.0).abs() < 0.001);

        assert!(linear_to_db(0.0) < -100.0);
    }

    // ── Trim and normalize ──────────────────────────────────────

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

    // ── Duration and loop points ─────────────────────────────────

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

    // ── Bar snapping ────────────────────────────────────────────

    #[test]
    fn bar_snap_truncates_to_bar_boundary() {
        let mut buf = AudioBuffer {
            // 44100 samples = 1 second. At 120 BPM, 4 beats/bar = 2 seconds/bar = 88200 samples
            samples: vec![0.5; 100000], // ~2.27 seconds
            sample_rate: 44100,
        };
        buf.bar_snap(120.0, 4);
        // Should snap to nearest bar: 2 bars = 176400 (too many), 1 bar = 88200
        assert_eq!(buf.samples.len(), 88200);
    }

    #[test]
    fn bar_snap_zero_bpm_noop() {
        let mut buf = AudioBuffer {
            samples: vec![0.5; 1000],
            sample_rate: 44100,
        };
        buf.bar_snap(0.0, 4);
        assert_eq!(buf.samples.len(), 1000);
    }

    // ── Spectral validation ─────────────────────────────────────

    #[test]
    fn spectral_validate_silent() {
        let buf = AudioBuffer {
            samples: vec![0.0; 1000],
            sample_rate: 44100,
        };
        let report = buf.spectral_validate("ambient");
        assert!(!report.passed);
        assert!(report.message.contains("Silent"));
    }

    #[test]
    fn spectral_validate_generic_passes() {
        let mut buf = AudioBuffer::new(44100);
        for i in 0..44100 {
            buf.samples.push((i as f32 * 0.1).sin() * 0.5);
        }
        let report = buf.spectral_validate("custom");
        assert!(report.passed);
    }

    // ── Full pipeline ───────────────────────────────────────────

    #[test]
    fn run_pipeline_basic() {
        let mut buf = AudioBuffer::new(1000);
        for i in 0..5000 {
            buf.samples.push((i as f32 * 0.05).sin() * 0.3);
        }
        let result = run_pipeline(&mut buf, -1.0, 4, None, "custom");
        assert!(result.duration_secs > 0.0);
        assert!(result.spectral_report.is_some());
    }
}
