# TidalCycles Mini-Notation

The engine uses a subset of [TidalCycles](https://tidalcycles.org/) mini-notation as a compact, manipulable format for game music. The `amigo_tidal_parser` crate parses this notation into an AST that the engine evaluates at runtime using built-in chiptune oscillators.

## Syntax Reference

| Syntax | Meaning | Example |
|--------|---------|---------|
| `"a b c"` | Sequence | `n "c4 d4 e4"` |
| `~` | Rest (silence) | `n "c4 ~ e4"` |
| `a*n` | Repeat | `n "c4*4"` |
| `a!n` | Replicate (consolidated) | `n "c4!4"` |
| `[a b]` | Subsequence (grouping) | `n "[c4 d4] e4"` |
| `a/n` | Slow (spans n cycles) | `n "c4/2"` |
| `stack [..]` | Polyphony (multiple layers) | `stack [n "c4", n "e4"]` |
| `# param` | Parameter chain | `# amp "0.8"` |
| `$ slow n` | Slow down tempo | `$ slow 2` |
| `$ fast n` | Speed up tempo | `$ fast 2` |
| `$ rev` | Reverse pattern | `$ rev` |

### Note Format

Notes consist of pitch class + octave: `c4`, `ds5`, `bf3`, `e2`

| Pitch | Notation | Aliases |
|-------|----------|---------|
| C | `c` | |
| C# | `cs` | `db` |
| D | `d` | |
| D# | `ds` | `eb`, `ef` |
| E | `e` | |
| F | `f` | |
| F# | `fs` | `gb` |
| G | `g` | |
| G# | `gs` | `ab` |
| A | `a` | |
| A# | `as` | `bb`, `bf` |
| B | `b` | |

Drum samples: `bd` (bass drum), `sd` (snare), `hh` (hi-hat)

## Tidal Playground (Editor Widget)

The Tidal Playground is a panel in the Amigo Editor that appears when opening a `.amigo.tidal` file.

### Features

- **Play/Stop** with chiptune oscillators
- **Per-stem controls**: mute/solo, instrument selection, volume
- **BPM control** (20-300 BPM, live adjustable)
- **Transforms**: slow, fast, rev
- **Presets**: save/load instrument configurations
- **WAV export**: export one cycle as WAV
- **Live preview**: hear music while editing levels

### Instruments

| Instrument | Sound | Typical Use |
|-----------|-------|-------------|
| Square Wave | Classic, full | Melody (NES-style lead) |
| Pulse 25% | Thin, nasal | Harmony |
| Pulse 12.5% | Very thin, metallic | Effects |
| Triangle Wave | Soft, warm | Bass (NES-style) |
| Sawtooth Wave | Harsh, full | Lead |
| Noise Channel | Noise | Percussion / hi-hats |
| Sine Wave | Pure | Sub-bass, test tones |

## Engine Integration

```rust
// Load .amigo.tidal file
let comp = amigo_tidal_parser::load("assets/music/overworld.amigo.tidal")?;

// Create playground
let mut playground = TidalPlayground::new(comp);
playground.set_bpm(140.0);
playground.set_instrument(0, Instrument::SquareWave);

// In the game loop: render audio
playground.render_audio(&mut buffer, 44100);

// Switch presets based on game state
if entering_combat {
    playground.set_bpm(180.0);
    playground.set_transform(Some(Transform::Fast(1.5)));
}
```

### Presets (.preset.yaml)

```yaml
name: "Boss Fight Version"
base_file: "overworld.amigo.tidal"
bpm: 180
transform: "fast 1.5"
stems:
  melody:
    instrument: sawtooth
    volume: 1.0
  bass:
    instrument: square_wave
    volume: 0.9
  percussion:
    instrument: noise_channel
    volume: 0.85
  harmony:
    enabled: false
```
