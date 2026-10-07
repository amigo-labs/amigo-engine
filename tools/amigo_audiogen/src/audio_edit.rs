//! The local processing tools: trimming, normalising, loop cutting,
//! previews and format conversion. No model is involved; WAV files are
//! read and written directly and other formats go through `ffmpeg`.

use crate::processing::{db_to_linear, linear_to_db};
use crate::wav::WavData;
use std::path::Path;

/// Below this level (dBFS) a frame counts as silence when trimming.
pub const SILENCE_DB: f32 = -50.0;

/// Drop leading and trailing frames whose every channel is below
/// `threshold_db`. Returns how many frames were cut from the start.
pub fn trim_silence(wav: &mut WavData, threshold_db: f32) -> usize {
    let threshold = db_to_linear(threshold_db);
    let loud = |i: usize| wav.frame(i).iter().any(|s| s.abs() > threshold);
    let frames = wav.frames();
    let start = (0..frames).find(|&i| loud(i)).unwrap_or(frames);
    let end = (0..frames).rposition(loud).map_or(start, |i| i + 1);
    *wav = wav.slice(start, end.max(start));
    start
}

/// Scale so the loudest sample is at `target_db` dBFS. Returns the peak
/// before, in dBFS; silence is left alone.
pub fn normalize(wav: &mut WavData, target_db: f32) -> f32 {
    let peak = wav.peak();
    if peak > 1e-9 {
        let gain = db_to_linear(target_db) / peak;
        wav.samples.iter_mut().for_each(|s| *s *= gain);
    }
    linear_to_db(peak)
}

/// The first `end` frames as a seamless loop: the `crossfade` frames after
/// `end` are faded out over the start of the loop while the start fades in,
/// so the jump from the last frame back to the first continues the audio
/// that followed it. Without audio after `end` the cut is plain.
pub fn make_loop(wav: &WavData, end: usize, crossfade: usize) -> WavData {
    let end = end.min(wav.frames());
    let xf = crossfade.min(wav.frames() - end).min(end);
    let mut out = wav.slice(0, end);
    let c = usize::from(wav.channels);
    for i in 0..xf {
        // Equal-power, so a fade between unrelated material does not dip.
        let t = (i as f32 + 0.5) / xf as f32;
        let (fade_in, fade_out) = (
            (t * std::f32::consts::FRAC_PI_2).sin(),
            (t * std::f32::consts::FRAC_PI_2).cos(),
        );
        for ch in 0..c {
            let tail = wav.samples[(end + i) * c + ch];
            let head = &mut out.samples[i * c + ch];
            *head = *head * fade_in + tail * fade_out;
        }
    }
    out
}

/// Where to cut a loop of about `target_frames`: the frame within ±50 ms
/// whose level is closest to zero and to the first frame's.
pub fn loop_end_near(wav: &WavData, target_frames: usize) -> usize {
    let mono = wav.mono().samples;
    let reach = (wav.sample_rate / 20) as usize;
    let lo = target_frames.saturating_sub(reach).max(1);
    let hi = (target_frames + reach).min(mono.len());
    let first = mono.first().copied().unwrap_or(0.0);
    (lo..hi)
        .min_by(|&a, &b| {
            let score = |i: usize| mono[i].abs() + (mono[i] - first).abs() * 0.5;
            score(a).total_cmp(&score(b))
        })
        .unwrap_or(target_frames.min(mono.len()))
}

/// The `frames`-long stretch with the most energy, starting on a quarter
/// second. Returns its first frame.
pub fn loudest_window(wav: &WavData, frames: usize) -> usize {
    let mono = wav.mono().samples;
    if frames >= mono.len() {
        return 0;
    }
    let mut energy = Vec::with_capacity(mono.len() + 1);
    energy.push(0.0f64);
    for s in &mono {
        energy.push(energy.last().copied().unwrap_or(0.0) + f64::from(s * s));
    }
    let hop = (wav.sample_rate as usize / 4).max(1);
    (0..=mono.len() - frames)
        .step_by(hop)
        .max_by(|&a, &b| {
            let e = |i: usize| energy[i + frames] - energy[i];
            e(a).total_cmp(&e(b))
        })
        .unwrap_or(0)
}

/// Linear fades over the first and last `frames` frames.
pub fn fade_edges(wav: &mut WavData, frames: usize) {
    let n = wav.frames();
    let f = frames.min(n / 2);
    let c = usize::from(wav.channels);
    for i in 0..f {
        let g = i as f32 / f as f32;
        for ch in 0..c {
            wav.samples[i * c + ch] *= g;
            wav.samples[(n - 1 - i) * c + ch] *= g;
        }
    }
}

/// `a` followed by `b` (conformed to `a`'s rate and channels), the last
/// `crossfade` frames of `a` overlapping the first of `b`.
pub fn append(a: &WavData, b: &WavData, crossfade: usize) -> WavData {
    let b = b.conform(a.sample_rate, a.channels);
    let xf = crossfade.min(a.frames()).min(b.frames());
    let c = usize::from(a.channels);
    let mut out = a.clone();
    let start = a.frames() - xf;
    for i in 0..xf {
        let t = (i as f32 + 0.5) / xf as f32;
        let (fade_in, fade_out) = (
            (t * std::f32::consts::FRAC_PI_2).sin(),
            (t * std::f32::consts::FRAC_PI_2).cos(),
        );
        for ch in 0..c {
            let s = &mut out.samples[(start + i) * c + ch];
            *s = *s * fade_out + b.samples[i * c + ch] * fade_in;
        }
    }
    out.samples.extend_from_slice(&b.samples[xf * c..]);
    out
}

/// Formats [`convert`] writes.
pub const FORMATS: [&str; 4] = ["wav", "ogg", "flac", "mp3"];

/// Convert `input` to `output` (format from its extension) with the
/// `ffmpeg` at `ffmpeg`. WAV to WAV needs no ffmpeg.
pub fn convert(ffmpeg: &Path, input: &Path, output: &Path) -> Result<(), String> {
    let ext = |p: &Path| {
        p.extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    };
    let (from, to) = (ext(input), ext(output));
    if from == "wav" && to == "wav" {
        let wav = crate::wav::read_wav(input)?;
        return crate::wav::write_wav(output, &wav);
    }
    let codec: &[&str] = match to.as_str() {
        "ogg" => &["-c:a", "libvorbis", "-q:a", "5"],
        "flac" => &["-c:a", "flac"],
        "mp3" => &["-c:a", "libmp3lame", "-q:a", "2"],
        "wav" => &["-c:a", "pcm_s16le"],
        other => {
            return Err(format!(
                "cannot write '{other}': use {}",
                FORMATS.join(", ")
            ));
        }
    };
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    let result = std::process::Command::new(ffmpeg)
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(input)
        .args(codec)
        .arg(output)
        .output()
        .map_err(|e| {
            format!(
                "converting to {to} needs ffmpeg, which could not be run ({}: {e}); install it or put it on PATH",
                ffmpeg.display()
            )
        })?;
    if !result.status.success() {
        return Err(format!(
            "ffmpeg failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono(samples: Vec<f32>) -> WavData {
        WavData {
            sample_rate: 100,
            channels: 1,
            samples,
        }
    }

    #[test]
    fn trimming_keeps_the_loud_middle() {
        let mut w = WavData {
            sample_rate: 100,
            channels: 2,
            samples: vec![0.0, 0.0, 0.0, 0.5, 0.2, 0.0, 0.0, 0.0],
        };
        assert_eq!(trim_silence(&mut w, -40.0), 1);
        assert_eq!(w.samples, [0.0, 0.5, 0.2, 0.0]);
        let mut silent = mono(vec![0.0; 10]);
        trim_silence(&mut silent, -40.0);
        assert_eq!(silent.frames(), 0);
    }

    #[test]
    fn normalising_sets_the_peak() {
        let mut w = mono(vec![0.25, -0.5]);
        let before = normalize(&mut w, -6.0206);
        assert!((before - -6.0206).abs() < 0.01, "{before}");
        assert!((w.peak() - 0.5).abs() < 1e-3);
        let mut w = mono(vec![0.1, -0.2]);
        normalize(&mut w, 0.0);
        assert!((w.samples[1] + 1.0).abs() < 1e-6);
        let mut silent = mono(vec![0.0; 3]);
        normalize(&mut silent, 0.0);
        assert_eq!(silent.samples, [0.0; 3]);
    }

    #[test]
    fn a_loop_blends_what_follows_the_cut_into_its_start() {
        let w = mono((0..10).map(|i| i as f32).collect());
        let looped = make_loop(&w, 6, 2);
        assert_eq!(looped.frames(), 6);
        // Frame 0 is mostly what came after the cut (6), frame 1 mostly 1.
        assert!(looped.samples[0] > 4.0, "{:?}", looped.samples);
        assert!(
            looped.samples[1] < 4.0 && looped.samples[1] > 1.0,
            "{:?}",
            looped.samples
        );
        assert_eq!(&looped.samples[2..], [2.0, 3.0, 4.0, 5.0]);
        // Cut at the very end: nothing to blend.
        assert_eq!(make_loop(&w, 10, 4).samples, w.samples);
    }

    #[test]
    fn the_loop_end_lands_on_a_quiet_frame() {
        let mut samples = vec![0.5; 100];
        samples[47] = 0.0;
        let w = mono(samples);
        assert_eq!(loop_end_near(&w, 50), 47);
    }

    #[test]
    fn the_loudest_window_is_found() {
        let mut samples = vec![0.01; 400];
        samples[200..250].fill(0.9);
        let w = mono(samples);
        assert_eq!(loudest_window(&w, 50), 200);
        assert_eq!(loudest_window(&w, 1000), 0);
    }

    #[test]
    fn appending_crossfades_and_conforms() {
        let a = mono(vec![1.0; 4]);
        let b = WavData {
            sample_rate: 100,
            channels: 2,
            samples: vec![0.0; 8],
        };
        let out = append(&a, &b, 2);
        assert_eq!(out.frames(), 6);
        assert_eq!(out.samples[..2], [1.0, 1.0]);
        assert!(out.samples[2] < 1.0 && out.samples[3] < out.samples[2]);
        assert_eq!(out.samples[5], 0.0);
    }

    #[test]
    fn edges_fade() {
        let mut w = mono(vec![1.0; 10]);
        fade_edges(&mut w, 2);
        assert_eq!(w.samples[0], 0.0);
        assert_eq!(w.samples[9], 0.0);
        assert_eq!(w.samples[5], 1.0);
    }

    #[test]
    fn conversion_without_ffmpeg_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.wav");
        crate::wav::write_wav(&input, &mono(vec![0.5; 10])).unwrap();
        let missing = dir.path().join("no-ffmpeg");
        let err = convert(&missing, &input, &dir.path().join("a.ogg")).unwrap_err();
        assert!(err.contains("needs ffmpeg"), "{err}");
        let err = convert(&missing, &input, &dir.path().join("a.xyz")).unwrap_err();
        assert!(err.contains("cannot write 'xyz'"), "{err}");
        // WAV to WAV is done here.
        convert(&missing, &input, &dir.path().join("out/b.wav")).unwrap();
        assert_eq!(
            crate::wav::read_wav(&dir.path().join("out/b.wav"))
                .unwrap()
                .frames(),
            10
        );
    }
}
