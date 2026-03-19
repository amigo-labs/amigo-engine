/// Tidal Playground — interactive editor widget for `.amigo.tidal` files.
///
/// Provides real-time playback of TidalCycles compositions using chiptune
/// oscillators, with per-stem mute/solo/instrument/volume controls, BPM
/// adjustment, and pattern transformations.
use amigo_tidal_parser::{
    apply_transform, evaluate_pattern, Composition, Instrument, NoteEvent, NoteValue, PitchClass,
    Transform,
};

// ---------------------------------------------------------------------------
// Playground state
// ---------------------------------------------------------------------------

/// Interactive playground for TidalCycles compositions.
pub struct TidalPlayground {
    pub composition: Composition,
    pub playback: PlaybackState,
    pub stem_settings: Vec<StemSettings>,
    pub global_bpm: f64,
    pub transform: Option<Transform>,
}

/// Playback state.
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub playing: bool,
    pub current_cycle: u64,
    /// Position within the current cycle (0.0 .. 1.0).
    pub cycle_position: f64,
    pub looping: bool,
}

/// Per-stem settings.
#[derive(Debug, Clone)]
pub struct StemSettings {
    pub name: String,
    pub enabled: bool,
    pub solo: bool,
    pub instrument: Instrument,
    pub volume: f64,
}

/// Preset configuration for quick switching.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaygroundPreset {
    pub name: String,
    pub base_file: String,
    pub bpm: f64,
    pub transform: Option<String>,
    pub stems: std::collections::HashMap<String, PresetStemConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PresetStemConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_instrument")]
    pub instrument: String,
    #[serde(default = "default_volume")]
    pub volume: f64,
}

fn default_true() -> bool {
    true
}
fn default_instrument() -> String {
    "square_wave".into()
}
fn default_volume() -> f64 {
    1.0
}

impl TidalPlayground {
    /// Create a new playground from a composition.
    pub fn new(composition: Composition) -> Self {
        let stem_settings = composition
            .stems
            .iter()
            .enumerate()
            .map(|(i, stem)| StemSettings {
                name: stem.name.clone(),
                enabled: true,
                solo: false,
                instrument: default_instrument_for_stem(&stem.name, i),
                volume: 1.0,
            })
            .collect();

        let bpm = composition.bpm;

        Self {
            composition,
            playback: PlaybackState {
                playing: false,
                current_cycle: 0,
                cycle_position: 0.0,
                looping: true,
            },
            stem_settings,
            global_bpm: bpm,
            transform: None,
        }
    }

    /// Advance playback by the given number of seconds.
    pub fn advance(&mut self, dt_seconds: f64) {
        if !self.playback.playing {
            return;
        }

        let beats_per_second = self.global_bpm / 60.0;
        let cycles_per_second = beats_per_second / self.composition.cycle_length.max(1.0);
        let advance = dt_seconds * cycles_per_second;

        self.playback.cycle_position += advance;
        while self.playback.cycle_position >= 1.0 {
            self.playback.cycle_position -= 1.0;
            self.playback.current_cycle += 1;
        }
    }

    /// Get events for the current cycle, respecting stem settings and transforms.
    pub fn current_events(&self) -> Vec<NoteEvent> {
        let mut events = evaluate_pattern(&self.composition, self.playback.current_cycle);

        if let Some(t) = self.transform {
            apply_transform(&mut events, t);
        }

        // Apply stem mute/solo.
        let any_solo = self.stem_settings.iter().any(|s| s.solo);
        events.retain(|ev| {
            let settings = &self.stem_settings[ev.stem_index];
            if any_solo {
                settings.solo
            } else {
                settings.enabled
            }
        });

        // Apply volume.
        for ev in &mut events {
            let settings = &self.stem_settings[ev.stem_index];
            ev.amplitude *= settings.volume;
        }

        events
    }

    /// Render audio samples into a buffer (mono, f32, -1.0 to 1.0).
    pub fn render_audio(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.playback.playing {
            buffer.fill(0.0);
            return;
        }

        let beats_per_second = self.global_bpm / 60.0;
        let cycle_duration_secs = self.composition.cycle_length.max(1.0) / beats_per_second;
        let samples_per_cycle = (cycle_duration_secs * sample_rate as f64) as usize;

        let events = self.current_events();

        for (i, sample) in buffer.iter_mut().enumerate() {
            let sample_pos = self.playback.cycle_position + (i as f64 / samples_per_cycle as f64);
            let t_seconds = i as f64 / sample_rate as f64;

            let mut value = 0.0_f32;

            for ev in &events {
                let event_end = ev.time + ev.duration * ev.legato;
                if sample_pos >= ev.time && sample_pos < event_end {
                    let freq = ev.note.frequency();
                    let instrument = self.stem_settings[ev.stem_index].instrument;
                    let phase = (freq * t_seconds) % 1.0;
                    let osc = oscillator(instrument, phase);
                    value += osc * ev.amplitude as f32 * 0.3; // master gain
                }
            }

            *sample = value.clamp(-1.0, 1.0);
        }

        // Advance playback state.
        let advance = buffer.len() as f64 / samples_per_cycle as f64;
        self.playback.cycle_position += advance;
        while self.playback.cycle_position >= 1.0 {
            self.playback.cycle_position -= 1.0;
            self.playback.current_cycle += 1;
        }
    }

    /// Toggle mute for a stem.
    pub fn toggle_stem(&mut self, index: usize) {
        if index < self.stem_settings.len() {
            self.stem_settings[index].enabled = !self.stem_settings[index].enabled;
        }
    }

    /// Solo a stem (mute all others).
    pub fn solo_stem(&mut self, index: usize) {
        if index < self.stem_settings.len() {
            let is_already_solo = self.stem_settings[index].solo;
            // Clear all solo flags.
            for s in &mut self.stem_settings {
                s.solo = false;
            }
            if !is_already_solo {
                self.stem_settings[index].solo = true;
            }
        }
    }

    /// Change instrument for a stem.
    pub fn set_instrument(&mut self, stem_index: usize, instrument: Instrument) {
        if stem_index < self.stem_settings.len() {
            self.stem_settings[stem_index].instrument = instrument;
        }
    }

    /// Set BPM (immediate, no glitch).
    pub fn set_bpm(&mut self, bpm: f64) {
        self.global_bpm = bpm.max(20.0).min(300.0);
    }

    /// Set or clear transform.
    pub fn set_transform(&mut self, transform: Option<Transform>) {
        self.transform = transform;
    }

    /// Save current settings as a preset.
    pub fn save_preset(&self, path: &str) -> Result<(), std::io::Error> {
        let mut stems = std::collections::HashMap::new();
        for s in &self.stem_settings {
            stems.insert(
                s.name.clone(),
                PresetStemConfig {
                    enabled: s.enabled,
                    instrument: instrument_name(s.instrument).to_string(),
                    volume: s.volume,
                },
            );
        }

        let transform_str = self.transform.map(|t| match t {
            Transform::Slow(n) => format!("slow {n}"),
            Transform::Fast(n) => format!("fast {n}"),
            Transform::Rev => "rev".into(),
        });

        let preset = PlaygroundPreset {
            name: self.composition.name.clone(),
            base_file: String::new(),
            bpm: self.global_bpm,
            transform: transform_str,
            stems,
        };

        let yaml = serde_json::to_string_pretty(&preset)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, yaml)
    }

    /// Load and apply a preset.
    pub fn load_preset(&mut self, path: &str) -> Result<(), std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let preset: PlaygroundPreset = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        self.global_bpm = preset.bpm;

        if let Some(ref t) = preset.transform {
            self.transform = parse_transform(t);
        }

        for s in &mut self.stem_settings {
            if let Some(cfg) = preset.stems.get(&s.name) {
                s.enabled = cfg.enabled;
                s.volume = cfg.volume;
                if let Some(inst) = parse_instrument(&cfg.instrument) {
                    s.instrument = inst;
                }
            }
        }

        Ok(())
    }

    /// Export one cycle as WAV (mono, 16-bit PCM).
    pub fn export_wav(&mut self, path: &str, sample_rate: u32) -> Result<(), std::io::Error> {
        let beats_per_second = self.global_bpm / 60.0;
        let cycle_duration_secs = self.composition.cycle_length.max(1.0) / beats_per_second;
        let total_samples = (cycle_duration_secs * sample_rate as f64) as usize;

        // Save and restore playback state.
        let saved_playing = self.playback.playing;
        let saved_pos = self.playback.cycle_position;
        let saved_cycle = self.playback.current_cycle;

        self.playback.playing = true;
        self.playback.cycle_position = 0.0;

        let mut buffer = vec![0.0_f32; total_samples];
        self.render_audio(&mut buffer, sample_rate);

        // Restore state.
        self.playback.playing = saved_playing;
        self.playback.cycle_position = saved_pos;
        self.playback.current_cycle = saved_cycle;

        // Write WAV file (minimal implementation).
        write_wav(path, &buffer, sample_rate)
    }

    /// Get playhead positions per stem (for visualization).
    pub fn playhead_positions(&self) -> Vec<(String, f64)> {
        self.stem_settings
            .iter()
            .map(|s| (s.name.clone(), self.playback.cycle_position))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Oscillators
// ---------------------------------------------------------------------------

fn oscillator(instrument: Instrument, phase: f64) -> f32 {
    let p = phase as f32;
    match instrument {
        Instrument::SquareWave => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Instrument::Pulse25 => {
            if p < 0.25 {
                1.0
            } else {
                -1.0
            }
        }
        Instrument::Pulse12 => {
            if p < 0.125 {
                1.0
            } else {
                -1.0
            }
        }
        Instrument::TriangleWave => {
            if p < 0.5 {
                4.0 * p - 1.0
            } else {
                3.0 - 4.0 * p
            }
        }
        Instrument::SawtoothWave => 2.0 * p - 1.0,
        Instrument::NoiseChannel => {
            // Simple pseudo-noise based on phase.
            let bits = (phase * 32768.0) as u32;
            let noise = bits.wrapping_mul(1103515245).wrapping_add(12345);
            ((noise >> 16) as f32 / 32768.0) - 1.0
        }
        Instrument::SineWave => (p * std::f32::consts::TAU).sin(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_instrument_for_stem(name: &str, _index: usize) -> Instrument {
    match name {
        "melody" => Instrument::SquareWave,
        "bass" => Instrument::TriangleWave,
        "percussion" | "drums" => Instrument::NoiseChannel,
        "harmony" | "other" => Instrument::Pulse25,
        _ => Instrument::SquareWave,
    }
}

fn instrument_name(inst: Instrument) -> &'static str {
    match inst {
        Instrument::SquareWave => "square_wave",
        Instrument::Pulse25 => "pulse_25",
        Instrument::Pulse12 => "pulse_12",
        Instrument::TriangleWave => "triangle_wave",
        Instrument::SawtoothWave => "sawtooth_wave",
        Instrument::NoiseChannel => "noise_channel",
        Instrument::SineWave => "sine_wave",
    }
}

fn parse_instrument(s: &str) -> Option<Instrument> {
    match s {
        "square_wave" | "square" => Some(Instrument::SquareWave),
        "pulse_25" | "pulse25" => Some(Instrument::Pulse25),
        "pulse_12" | "pulse12" => Some(Instrument::Pulse12),
        "triangle_wave" | "triangle" => Some(Instrument::TriangleWave),
        "sawtooth_wave" | "sawtooth" | "saw" => Some(Instrument::SawtoothWave),
        "noise_channel" | "noise" => Some(Instrument::NoiseChannel),
        "sine_wave" | "sine" => Some(Instrument::SineWave),
        _ => None,
    }
}

fn parse_transform(s: &str) -> Option<Transform> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    match parts.as_slice() {
        ["slow", n] => n.parse().ok().map(Transform::Slow),
        ["fast", n] => n.parse().ok().map(Transform::Fast),
        ["rev"] => Some(Transform::Rev),
        _ => None,
    }
}

/// Write a minimal WAV file (mono, 16-bit PCM).
fn write_wav(path: &str, samples: &[f32], sample_rate: u32) -> Result<(), std::io::Error> {
    use std::io::Write;

    let num_samples = samples.len() as u32;
    let data_size = num_samples * 2; // 16-bit = 2 bytes per sample
    let file_size = 36 + data_size;

    let mut f = std::fs::File::create(path)?;

    // RIFF header
    f.write_all(b"RIFF")?;
    f.write_all(&file_size.to_le_bytes())?;
    f.write_all(b"WAVE")?;

    // fmt chunk
    f.write_all(b"fmt ")?;
    f.write_all(&16_u32.to_le_bytes())?; // chunk size
    f.write_all(&1_u16.to_le_bytes())?; // PCM format
    f.write_all(&1_u16.to_le_bytes())?; // mono
    f.write_all(&sample_rate.to_le_bytes())?;
    f.write_all(&(sample_rate * 2).to_le_bytes())?; // byte rate
    f.write_all(&2_u16.to_le_bytes())?; // block align
    f.write_all(&16_u16.to_le_bytes())?; // bits per sample

    // data chunk
    f.write_all(b"data")?;
    f.write_all(&data_size.to_le_bytes())?;

    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let value = (clamped * 32767.0) as i16;
        f.write_all(&value.to_le_bytes())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use amigo_tidal_parser::{
        ast::{CompositionMeta, Pattern, PatternAtom, Stem, Voice},
        NoteValue, PitchClass,
    };

    fn test_composition() -> Composition {
        Composition {
            name: "test".into(),
            bpm: 120.0,
            cycle_length: 1.0,
            stems: vec![
                Stem {
                    name: "melody".into(),
                    voices: vec![Voice {
                        note_pattern: Pattern::Sequence(vec![
                            Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4))),
                            Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::E, 4))),
                            Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::G, 4))),
                        ]),
                        amp_pattern: None,
                        legato_pattern: None,
                    }],
                },
                Stem {
                    name: "bass".into(),
                    voices: vec![Voice {
                        note_pattern: Pattern::Sequence(vec![
                            Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 2))),
                            Pattern::Atom(PatternAtom::Rest),
                        ]),
                        amp_pattern: None,
                        legato_pattern: None,
                    }],
                },
            ],
            metadata: CompositionMeta::default(),
        }
    }

    #[test]
    fn playground_creates_stem_settings() {
        let pg = TidalPlayground::new(test_composition());
        assert_eq!(pg.stem_settings.len(), 2);
        assert_eq!(pg.stem_settings[0].name, "melody");
        assert_eq!(pg.stem_settings[0].instrument, Instrument::SquareWave);
        assert_eq!(pg.stem_settings[1].name, "bass");
        assert_eq!(pg.stem_settings[1].instrument, Instrument::TriangleWave);
    }

    #[test]
    fn toggle_stem_mutes() {
        let mut pg = TidalPlayground::new(test_composition());
        assert!(pg.stem_settings[0].enabled);
        pg.toggle_stem(0);
        assert!(!pg.stem_settings[0].enabled);
        pg.toggle_stem(0);
        assert!(pg.stem_settings[0].enabled);
    }

    #[test]
    fn solo_stem_exclusive() {
        let mut pg = TidalPlayground::new(test_composition());
        pg.solo_stem(0);
        assert!(pg.stem_settings[0].solo);
        assert!(!pg.stem_settings[1].solo);

        // Solo again to unsolo.
        pg.solo_stem(0);
        assert!(!pg.stem_settings[0].solo);
    }

    #[test]
    fn current_events_respects_mute() {
        let mut pg = TidalPlayground::new(test_composition());
        pg.toggle_stem(1); // mute bass
        let events = pg.current_events();
        // Only melody events (stem_index 0).
        assert!(events.iter().all(|e| e.stem_index == 0));
    }

    #[test]
    fn current_events_respects_solo() {
        let mut pg = TidalPlayground::new(test_composition());
        pg.solo_stem(1); // solo bass
        let events = pg.current_events();
        // Only bass events. Bass has c2 and rest, so only 1 note event.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].stem_index, 1);
    }

    #[test]
    fn set_bpm_clamps() {
        let mut pg = TidalPlayground::new(test_composition());
        pg.set_bpm(10.0);
        assert!((pg.global_bpm - 20.0).abs() < 0.01); // min 20
        pg.set_bpm(999.0);
        assert!((pg.global_bpm - 300.0).abs() < 0.01); // max 300
    }

    #[test]
    fn render_audio_produces_samples() {
        let mut pg = TidalPlayground::new(test_composition());
        pg.playback.playing = true;
        let mut buffer = vec![0.0_f32; 1024];
        pg.render_audio(&mut buffer, 44100);
        // Should have non-zero samples.
        assert!(buffer.iter().any(|&s| s.abs() > 0.001));
    }

    #[test]
    fn oscillator_square_wave() {
        assert_eq!(oscillator(Instrument::SquareWave, 0.25), 1.0);
        assert_eq!(oscillator(Instrument::SquareWave, 0.75), -1.0);
    }

    #[test]
    fn oscillator_triangle_wave() {
        let mid = oscillator(Instrument::TriangleWave, 0.25);
        assert!((mid - 0.0).abs() < 0.01);
        let peak = oscillator(Instrument::TriangleWave, 0.5);
        assert!((peak - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_transform_roundtrip() {
        assert_eq!(parse_transform("slow 2"), Some(Transform::Slow(2.0)));
        assert_eq!(parse_transform("fast 1.5"), Some(Transform::Fast(1.5)));
        assert_eq!(parse_transform("rev"), Some(Transform::Rev));
        assert_eq!(parse_transform("invalid"), None);
    }

    #[test]
    fn parse_instrument_names() {
        assert_eq!(
            parse_instrument("square_wave"),
            Some(Instrument::SquareWave)
        );
        assert_eq!(parse_instrument("triangle"), Some(Instrument::TriangleWave));
        assert_eq!(parse_instrument("noise"), Some(Instrument::NoiseChannel));
        assert_eq!(parse_instrument("unknown"), None);
    }
}
