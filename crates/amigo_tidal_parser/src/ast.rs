/// Pitch class (chromatic scale, sharps only — flats normalized to sharps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PitchClass {
    C,
    Cs,
    D,
    Ds,
    E,
    F,
    Fs,
    G,
    Gs,
    A,
    As,
    B,
}

impl PitchClass {
    /// MIDI note number for this pitch class in octave 0.
    pub const fn midi_base(self) -> u8 {
        match self {
            Self::C => 0,
            Self::Cs => 1,
            Self::D => 2,
            Self::Ds => 3,
            Self::E => 4,
            Self::F => 5,
            Self::Fs => 6,
            Self::G => 7,
            Self::Gs => 8,
            Self::A => 9,
            Self::As => 10,
            Self::B => 11,
        }
    }

    /// Parse from string like "c", "cs", "d", "ds", "ef" (flat alias), etc.
    pub fn from_str_name(s: &str) -> Option<Self> {
        match s {
            "c" => Some(Self::C),
            "cs" | "db" => Some(Self::Cs),
            "d" => Some(Self::D),
            "ds" | "eb" | "ef" => Some(Self::Ds),
            "e" => Some(Self::E),
            "f" => Some(Self::F),
            "fs" | "gb" => Some(Self::Fs),
            "g" => Some(Self::G),
            "gs" | "ab" => Some(Self::Gs),
            "a" => Some(Self::A),
            "as" | "bb" | "bf" => Some(Self::As),
            "b" => Some(Self::B),
            _ => None,
        }
    }
}

/// Note with octave.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NoteValue {
    pub pitch_class: PitchClass,
    pub octave: i8,
}

impl NoteValue {
    pub const fn new(pitch_class: PitchClass, octave: i8) -> Self {
        Self {
            pitch_class,
            octave,
        }
    }

    /// MIDI note number (C4 = 60).
    pub fn midi_note(&self) -> u8 {
        let base = self.pitch_class.midi_base() as i16;
        let note = base + (self.octave as i16 + 1) * 12;
        note.clamp(0, 127) as u8
    }

    /// Frequency in Hz (A4 = 440 Hz, equal temperament).
    pub fn frequency(&self) -> f64 {
        let semitones_from_a4 = self.midi_note() as f64 - 69.0;
        440.0 * (2.0_f64).powf(semitones_from_a4 / 12.0)
    }
}

/// A single pattern element.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PatternAtom {
    /// Note value, e.g. "c4", "ds5".
    Note(NoteValue),
    /// Rest / silence.
    Rest,
    /// Numeric value (for amp, legato, etc.).
    Number(f64),
    /// Drum/sample name like "bd", "sd", "hh".
    Sample(String),
}

/// Pattern node in the AST.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Pattern {
    /// Single atom.
    Atom(PatternAtom),
    /// Space-separated sequence of patterns.
    Sequence(Vec<Pattern>),
    /// Subsequence / grouping [a b c].
    Group(Vec<Pattern>),
    /// Repetition a*n — same event repeated n times in the same slot.
    Repeat(Box<Pattern>, u32),
    /// Replication a!n — consolidated, n copies as separate sequence elements.
    Replicate(Box<Pattern>, u32),
    /// Slow division a/n — event spans n cycles.
    SlowDiv(Box<Pattern>, u32),
    /// Polyphonic layers (stack).
    Stack(Vec<Pattern>),
}

/// A voice layer with note pattern and optional parameter patterns.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Voice {
    pub note_pattern: Pattern,
    pub amp_pattern: Option<Pattern>,
    pub legato_pattern: Option<Pattern>,
}

/// A single stem (instrument track).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stem {
    pub name: String,
    pub voices: Vec<Voice>,
}

/// Full composition parsed from .amigo.tidal.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Composition {
    pub name: String,
    pub bpm: f64,
    /// Cycle length in beats (derived from slow factor, default 1.0).
    pub cycle_length: f64,
    pub stems: Vec<Stem>,
    pub metadata: CompositionMeta,
}

/// Optional metadata from the .amigo.tidal header.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CompositionMeta {
    pub source: Option<String>,
    pub license: Option<String>,
    pub author: Option<String>,
}

/// Tempo / pattern transformation.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Transform {
    Slow(f64),
    Fast(f64),
    Rev,
}

/// Chiptune instrument type for playback.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum Instrument {
    #[default]
    SquareWave,
    Pulse25,
    Pulse12,
    TriangleWave,
    SawtoothWave,
    NoiseChannel,
    SineWave,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_midi_numbers() {
        assert_eq!(NoteValue::new(PitchClass::C, 4).midi_note(), 60);
        assert_eq!(NoteValue::new(PitchClass::A, 4).midi_note(), 69);
        assert_eq!(NoteValue::new(PitchClass::C, -1).midi_note(), 0);
    }

    #[test]
    fn note_frequency_a440() {
        let a4 = NoteValue::new(PitchClass::A, 4);
        assert!((a4.frequency() - 440.0).abs() < 0.01);
    }

    #[test]
    fn pitch_class_from_str() {
        assert_eq!(PitchClass::from_str_name("c"), Some(PitchClass::C));
        assert_eq!(PitchClass::from_str_name("cs"), Some(PitchClass::Cs));
        assert_eq!(PitchClass::from_str_name("eb"), Some(PitchClass::Ds));
        assert_eq!(PitchClass::from_str_name("bb"), Some(PitchClass::As));
        assert_eq!(PitchClass::from_str_name("x"), None);
    }
}
