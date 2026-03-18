---
status: draft
crate: amigo_audio_pipeline
depends_on: ["engine/audio", "ai-pipelines/audiogen"]
last_updated: 2026-03-18
author: Daniel
---

# Audio-to-TidalCycles Pipeline

## 1. Motivation

Die Amigo Engine braucht eine Pipeline, die bestehende Chiptune-/Retro-Audio-Tracks analysiert, in ihre Bestandteile zerlegt und als TidalCycles-Mini-Notation ausgibt. Diese Notation dient als kompaktes, manipulierbares Datenformat für die Sound-Engine. Der Workflow ermöglicht es, CC-lizenzierte Tracks zu importieren, zu transformieren und algorithmisch im Spiel einzusetzen.

## 2. Übersicht

```
┌──────────┐    ┌──────────┐    ┌─────────────┐    ┌──────────────┐    ┌────────────┐
│  Audio   │───▶│  Demucs  │───▶│ Basic Pitch │───▶│ MIDI→Tidal   │───▶│ Amigo IR   │
│  (.wav/  │    │  (Stem   │    │ (Audio→MIDI)│    │ (MIDI→Mini-  │    │ (Internal  │
│  .ogg/   │    │  Split)  │    │             │    │  Notation)   │    │  Repr.)    │
│  .mp3)   │    └──────────┘    └─────────────┘    └──────────────┘    └────────────┘
└──────────┘         │                                                       │
                     │ Optional: Skip                                        ▼
                     │ bei Mono-Tracks                              ┌────────────────┐
                     └─────────────────────────────────────────────▶│  .amigo.tidal   │
                                                                    │  (Dateiformat)  │
                                                                    └────────────────┘
```

## 3. Pipeline-Stufen

### 3.1 Stufe 1 — Source Separation (Demucs)

**Zweck:** Polyphonen Audio-Track in isolierte Stems aufteilen.

**Tool:** Demucs v4 (Meta/Facebook Research), Python CLI.

**Verhalten:**
- Input: Audio-Datei (WAV, OGG, MP3, FLAC)
- Output: Separate Stems als WAV-Dateien
  - Bei Chiptune typisch: `melody`, `bass`, `drums`, `other`
  - Demucs htdemucs-Modell für beste Trennung
- **Skip-Logik:** Wenn der Input als Mono/Single-Voice erkannt wird (z.B. via RMS-Analyse oder User-Flag `--skip-separation`), Stufe überspringen und direkt zu Stufe 2

**Konfiguration:**
```yaml
separation:
  model: htdemucs
  stem_count: 4          # Standard: 4 Stems
  output_format: wav
  sample_rate: 44100
  skip_if_mono: true     # Automatische Mono-Erkennung
  custom_stems:          # Optional: Custom Stem-Mapping
    - name: melody
      demucs_stem: vocals  # Chiptune "Melodie" oft im Vocals-Stem
    - name: bass
      demucs_stem: bass
    - name: percussion
      demucs_stem: drums
    - name: harmony
      demucs_stem: other
```

**CLI:**
```bash
amigo-pipeline separate --input track.wav --output ./stems/
amigo-pipeline separate --input track.wav --skip-separation  # Direkt zu MIDI
```

### 3.2 Stufe 2 — Audio-to-MIDI Transcription (Basic Pitch)

**Zweck:** Jeden Stem (oder den Gesamt-Track) in MIDI transkribieren.

**Tool:** Basic Pitch (Spotify), Python-Library (`pip install basic-pitch`).

**Verhalten:**
- Input: WAV-Datei (einzelner Stem oder Gesamt-Track)
- Output: MIDI-Datei (.mid) pro Stem
- Polyphon: Ja — mehrere Noten gleichzeitig erkannt
- Pitch Bend: Erkannt und in MIDI-Events übertragen

**Parameter (konfigurierbar):**
```yaml
transcription:
  onset_threshold: 0.5       # Empfindlichkeit für Notenanfang (0.0–1.0)
  frame_threshold: 0.3       # Empfindlichkeit für Note-Sustain
  min_note_length_ms: 50     # Minimum Notenlänge in ms
  min_frequency_hz: 27.5     # A0 — untere Grenze
  max_frequency_hz: 4186.0   # C8 — obere Grenze (Chiptune selten höher)
  midi_tempo_bpm: null       # null = auto-detect, sonst fixiert
  pitch_bend: true           # Pitch Bend Events mitschreiben
```

**CLI:**
```bash
amigo-pipeline transcribe --input ./stems/ --output ./midi/
amigo-pipeline transcribe --input track.wav --output ./midi/track.mid
```

**Nachbearbeitung (intern):**
- Quantisierung auf konfigurierbares Grid (z.B. 1/16, 1/32)
- Velocity-Normalisierung (optional)
- Entfernen von Ghost Notes unter konfigurierbarem Threshold

### 3.3 Stufe 3 — MIDI-to-TidalCycles Conversion

**Zweck:** MIDI-Dateien in TidalCycles Mini-Notation umwandeln.

**Tool:** `midi_to_tidalcycles` (Python CLI) als Basis, erweitert um Amigo-spezifische Ausgabe.

**Verhalten:**
- Input: MIDI-Datei (.mid)
- Output: TidalCycles-Notation als Text
- Polyphonie: Via `stack []` — jede Stimme ein Layer
- Konsolidierung: Wiederholende Werte mit `!`-Notation komprimiert

**Parameter:**
```yaml
conversion:
  resolution: 16              # Quanta pro Viertelnote (8, 16, 32)
  include_legato: true        # Legato-Pattern generieren
  include_amplitude: true     # Amplitude/Velocity-Pattern generieren
  consolidate: true           # Wiederholungen mit ! komprimieren
  output_format: amigo_tidal  # "tidal_raw" | "amigo_tidal" | "strudel"
```

**Beispiel-Output (TidalCycles raw):**
```haskell
-- stem: melody
d1 $ slow 8 $ stack [
  n "c5 d5 e5 ~ g5 a5 g5 e5" # amp "0.8 0.7 0.9 0 0.8 0.7 0.9 0.8",
  n "e4 ~ g4 ~ e4 ~ g4 ~" # amp "0.5 0 0.5 0 0.5 0 0.5 0"
] # legato "1.0 0.5 1.0 0 1.0 0.5 1.0 1.0"
```

### 3.4 Stufe 4 — Amigo Internal Representation (IR)

**Zweck:** TidalCycles-Notation in ein Rust-natives Format parsen, das die Engine direkt verarbeiten kann.

**Implementierung:** Rust-Crate `amigo_tidal_parser`

#### 3.4.1 Mini-Notation Parser

Ein Subset der TidalCycles Mini-Notation parsen:

**Unterstützte Syntax:**

| Syntax       | Bedeutung                  | Beispiel              |
|-------------|----------------------------|-----------------------|
| `"a b c"`   | Sequenz                    | `n "c4 d4 e4"`       |
| `"~"`       | Pause (Rest)               | `n "c4 ~ e4"`        |
| `"a*n"`     | Repeat                     | `n "c4*4"`            |
| `"a!n"`     | Replicate (consolidated)   | `n "c4!4"`            |
| `"[a b]"`   | Subsequenz (Gruppierung)   | `n "[c4 d4] e4"`     |
| `"a/n"`     | Slow (über n Zyklen)       | `n "c4/2"`            |
| `"a b" ? x` | Zufällige Auswahl          | Spätere Phase         |
| `stack [..]`| Polyphonie                 | Mehrere Layer         |
| `# param`   | Parameter-Chain            | `# amp "0.8"`        |
| `$ slow n`  | Tempo-Transformation       | `$ slow 2`           |
| `$ fast n`  | Tempo-Transformation       | `$ fast 2`           |
| `$ rev`     | Pattern umkehren           | `$ rev`              |

**AST-Struktur (Rust):**

```rust
/// Ein einzelnes Pattern-Element
#[derive(Debug, Clone, PartialEq)]
pub enum PatternAtom {
    /// Notenwert, z.B. "c4", "ds5"
    Note(NoteValue),
    /// Pause
    Rest,
    /// Numerischer Wert (für amp, legato etc.)
    Number(f64),
}

/// Note mit Oktave
#[derive(Debug, Clone, PartialEq)]
pub struct NoteValue {
    pub pitch_class: PitchClass,  // C, Cs, D, Ds, E, F, Fs, G, Gs, A, As, B
    pub octave: i8,               // -1 bis 9
}

#[derive(Debug, Clone, PartialEq)]
pub enum PitchClass {
    C, Cs, D, Ds, E, F, Fs, G, Gs, A, As, B,
}

/// Pattern-Knoten im AST
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Einzelnes Atom
    Atom(PatternAtom),
    /// Sequenz von Patterns (Leerzeichen-getrennt)
    Sequence(Vec<Pattern>),
    /// Subsequenz / Gruppierung [a b c]
    Group(Vec<Pattern>),
    /// Wiederholung a*n
    Repeat(Box<Pattern>, u32),
    /// Replikation a!n (consolidated)
    Replicate(Box<Pattern>, u32),
    /// Slow-Division a/n
    SlowDiv(Box<Pattern>, u32),
    /// Polyphone Layer (stack)
    Stack(Vec<Pattern>),
}

/// Ein vollständiger Voice-Layer mit Parametern
#[derive(Debug, Clone)]
pub struct Voice {
    pub note_pattern: Pattern,
    pub amp_pattern: Option<Pattern>,
    pub legato_pattern: Option<Pattern>,
}

/// Gesamte Komposition
#[derive(Debug, Clone)]
pub struct Composition {
    pub name: String,
    pub bpm: f64,
    pub cycle_length: f64,       // in Beats (default: slow-Faktor)
    pub stems: Vec<Stem>,
}

#[derive(Debug, Clone)]
pub struct Stem {
    pub name: String,             // z.B. "melody", "bass", "percussion"
    pub voices: Vec<Voice>,       // Polyphon: mehrere Voices pro Stem
}
```

#### 3.4.2 Dateiformat `.amigo.tidal`

Eigenes Textformat, das die TidalCycles-Notation um Amigo-Metadaten erweitert:

```
-- amigo:meta
-- name: "overworld_theme"
-- bpm: 140
-- source: "ozzed_adventure.wav"
-- license: "CC-BY-4.0"
-- author: "Ozzed"

-- amigo:stem melody
d1 $ slow 8 $ stack [
  n "c5 d5 e5 ~ g5 a5 g5 e5" # amp "0.8 0.7 0.9 0 0.8 0.7 0.9 0.8"
]

-- amigo:stem bass
d2 $ slow 8 $
  n "c3 ~ c3 ~ g2 ~ g2 ~" # amp "0.9!8"

-- amigo:stem percussion
d3 $ slow 8 $
  n "bd ~ sd ~ bd ~ sd bd" # amp "1.0 0 0.8 0 1.0 0 0.8 0.6"
```

**Parser-Regeln:**
- Zeilen mit `-- amigo:meta` leiten den Metadaten-Block ein
- Zeilen mit `-- amigo:stem <name>` leiten einen Stem-Block ein
- Alles zwischen Stem-Markern ist TidalCycles-Notation
- Standard-Kommentare (`--`) ohne `amigo:`-Prefix werden ignoriert

## 4. CLI-Interface (Gesamt-Pipeline)

```bash
# Volle Pipeline: Audio → .amigo.tidal
amigo-pipeline convert \
  --input overworld.wav \
  --output overworld.amigo.tidal \
  --bpm 140 \
  --name "overworld_theme" \
  --license "CC-BY-4.0" \
  --author "Ozzed"

# Nur Separation
amigo-pipeline separate --input track.wav --output ./stems/

# Nur Transkription
amigo-pipeline transcribe --input ./stems/ --output ./midi/

# Nur MIDI→Tidal
amigo-pipeline notate --input ./midi/ --output track.amigo.tidal

# Pipeline mit Konfig-Datei
amigo-pipeline convert --input track.wav --config pipeline.yaml

# Batch: Ganzen Ordner verarbeiten
amigo-pipeline batch --input ./tracks/ --output ./tidal/ --config pipeline.yaml
```

## 5. Konfigurations-Datei (`pipeline.yaml`)

```yaml
pipeline:
  name: "default"

separation:
  enabled: true
  model: htdemucs
  skip_if_mono: true
  stem_mapping:
    vocals: melody
    bass: bass
    drums: percussion
    other: harmony

transcription:
  onset_threshold: 0.5
  frame_threshold: 0.3
  min_note_length_ms: 50
  min_frequency_hz: 27.5
  max_frequency_hz: 4186.0
  pitch_bend: false           # Chiptune: typisch kein Pitch Bend
  quantize_grid: 16           # 1/16-Noten Grid

conversion:
  resolution: 16
  include_legato: true
  include_amplitude: true
  consolidate: true
  output_format: amigo_tidal

postprocessing:
  remove_ghost_notes: true
  ghost_note_threshold: 0.1   # amp < 0.1 = Ghost Note
  normalize_velocity: true
  merge_short_rests: true     # Kurze Pausen zwischen gleichen Noten mergen
  min_rest_length_ms: 30
```

## 6. Rust-Crate-Struktur

```
amigo-engine/
├── crates/
│   ├── amigo_tidal_parser/        # Mini-Notation Parser
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── lexer.rs           # Tokenizer für Mini-Notation
│   │   │   ├── parser.rs          # AST-Parser
│   │   │   ├── ast.rs             # Pattern/Voice/Composition Types
│   │   │   ├── eval.rs            # Pattern → zeitliche Events auflösen
│   │   │   └── file.rs            # .amigo.tidal Datei-Parser
│   │   ├── tests/
│   │   │   ├── lexer_tests.rs
│   │   │   ├── parser_tests.rs
│   │   │   ├── eval_tests.rs
│   │   │   └── fixtures/          # Test-.amigo.tidal Dateien
│   │   └── Cargo.toml
│   │
│   └── amigo_audio_pipeline/      # Python-Wrapper + Orchestrierung
│       ├── src/
│       │   ├── lib.rs             # FFI / subprocess orchestration
│       │   ├── config.rs          # pipeline.yaml Parsing
│       │   ├── separation.rs      # Demucs-Aufruf
│       │   ├── transcription.rs   # Basic Pitch-Aufruf
│       │   ├── conversion.rs      # midi_to_tidalcycles-Aufruf
│       │   └── postprocess.rs     # Nachbearbeitung
│       ├── python/                # Python-Scripts für externe Tools
│       │   ├── run_demucs.py
│       │   ├── run_basic_pitch.py
│       │   └── run_midi_to_tidal.py
│       ├── Cargo.toml
│       └── pipeline.yaml          # Default-Konfiguration
```

## 7. Pattern-Evaluation (Runtime)

Der Parser allein reicht nicht — die Engine muss Patterns in zeitliche Events auflösen.

**`eval.rs` — Pattern zu Events:**

```rust
/// Ein zeitlich aufgelöstes Event
#[derive(Debug, Clone)]
pub struct NoteEvent {
    pub time: f64,        // Position im Zyklus (0.0 – 1.0)
    pub duration: f64,    // Dauer relativ zum Zyklus
    pub note: NoteValue,
    pub amplitude: f64,   // 0.0 – 1.0
    pub legato: f64,      // Multiplikator für Duration
}

/// Pattern in eine Liste von Events für einen Zyklus auflösen
pub fn evaluate_pattern(
    composition: &Composition,
    cycle: u64,
) -> Vec<NoteEvent>;

/// Transformationen anwenden
pub fn apply_transform(
    events: Vec<NoteEvent>,
    transform: Transform,
) -> Vec<NoteEvent>;

pub enum Transform {
    Slow(f64),
    Fast(f64),
    Rev,
}
```

## 8. Integration mit Amigo Engine

Die Amigo Engine konsumiert `Composition`-Objekte:

```rust
// In der Game-Loop:
let composition = amigo_tidal_parser::load("assets/music/overworld.amigo.tidal")?;

// Pro Audio-Frame:
let events = evaluate_pattern(&composition, current_cycle);
for event in events {
    audio_engine.play_note(
        event.note,
        event.amplitude,
        event.duration * event.legato,
    );
}
```

**Synthese:** Die Engine nutzt eigene Chiptune-Oszillatoren (Square, Triangle, Sawtooth, Noise) — die TidalCycles-Notation liefert nur *was* gespielt wird, nicht *wie* es klingt. Klangfarbe wird pro Stem über die Engine-Konfiguration gesteuert:

```yaml
# In der Welt-/Level-Konfiguration:
music:
  file: "overworld.amigo.tidal"
  stem_instruments:
    melody: square_wave       # Klassischer Chiptune-Lead
    bass: triangle_wave       # Weicher Bass
    percussion: noise_channel  # NES-Style Noise
    harmony: pulse_25          # 25% Duty Cycle Pulse
```

## 9. Testplan

| Test                              | Typ         | Beschreibung                                                    |
|-----------------------------------|-------------|-----------------------------------------------------------------|
| Lexer: Note-Parsing               | Unit        | `"c4"`, `"ds5"`, `"~"`, `"0.8"` korrekt tokenisiert           |
| Lexer: Operatoren                 | Unit        | `*`, `!`, `/`, `[]`, `stack` erkannt                            |
| Parser: Einfache Sequenz          | Unit        | `"c4 d4 e4"` → `Sequence([Note, Note, Note])`                  |
| Parser: Verschachtelt             | Unit        | `"[c4 d4] e4"` → `Sequence([Group([Note, Note]), Note])`       |
| Parser: Stack                     | Unit        | Polyphonie korrekt geparst                                      |
| Parser: Repeat/Replicate          | Unit        | `"c4*4"` und `"c4!4"` unterschieden                            |
| Eval: Timing                      | Unit        | Events gleichmäßig über Zyklus verteilt                         |
| Eval: Nested Groups               | Unit        | `"[c4 d4] e4"` → c4@0.0, d4@0.25, e4@0.5                      |
| Eval: Slow/Fast                   | Unit        | Tempo-Transformationen korrekt                                   |
| File-Parser: Metadaten            | Unit        | `-- amigo:meta` korrekt extrahiert                               |
| File-Parser: Stems                | Unit        | Mehrere Stems korrekt getrennt                                   |
| Pipeline: Roundtrip               | Integration | WAV → Pipeline → .amigo.tidal → Engine-Playback klingt korrekt |
| Pipeline: Batch                   | Integration | Ordner mit 10 Tracks korrekt verarbeitet                        |
| Pipeline: Mono-Skip               | Integration | Mono-Track überspringt Separation                               |

## 10. Offene Fragen / Spätere Phasen

- **Live-Transformation:** TidalCycles-Patterns zur Laufzeit manipulieren (z.B. Boss-Kampf → `$ fast 2`, leiser Bereich → Stems entfernen)
- **Generative Patterns:** Zufall/Algorithmus-basierte Pattern-Modifikation (`?`, `choose`, `degrade`)
- **ACE-Step Integration:** Pipeline rückwärts — aus TidalCycles-Notation Audio generieren lassen für Preview/Prototyping
- **Strudel-Export:** Web-basierter Preview via Strudel (JavaScript TidalCycles-Port)
- **Drum-Mapping:** Standard-Percussion-Mapping für NES/GB/C64-Style Drum Channels
- **Welt-spezifische Presets:** Pro Threadwalker-Welt / TD-Welt ein Preset mit Stem→Instrument-Mapping und Genre-spezifischen Transformationen
