# TidalCycles Mini-Notation

Die Engine nutzt ein Subset der [TidalCycles](https://tidalcycles.org/) Mini-Notation als kompaktes, manipulierbares Format fuer Game-Musik. Der `amigo_tidal_parser` Crate parst diese Notation in einen AST, den die Engine zur Laufzeit mit Chiptune-Oszillatoren auswertet.

## Syntax-Referenz

| Syntax | Bedeutung | Beispiel |
|--------|-----------|----------|
| `"a b c"` | Sequenz | `n "c4 d4 e4"` |
| `~` | Pause | `n "c4 ~ e4"` |
| `a*n` | Repeat | `n "c4*4"` |
| `a!n` | Replicate (consolidated) | `n "c4!4"` |
| `[a b]` | Subsequenz (Gruppierung) | `n "[c4 d4] e4"` |
| `a/n` | Slow (ueber n Zyklen) | `n "c4/2"` |
| `stack [..]` | Polyphonie (mehrere Layer) | `stack [n "c4", n "e4"]` |
| `# param` | Parameter-Chain | `# amp "0.8"` |
| `$ slow n` | Tempo runter | `$ slow 2` |
| `$ fast n` | Tempo hoch | `$ fast 2` |
| `$ rev` | Pattern umkehren | `$ rev` |

### Noten-Format

Noten bestehen aus Pitch-Klasse + Oktave: `c4`, `ds5`, `bf3`, `e2`

| Pitch | Notation | Alternativen |
|-------|----------|-------------|
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

Drum-Samples: `bd` (Bass Drum), `sd` (Snare), `hh` (Hi-Hat)

## Tidal Playground (Editor-Widget)

Der Tidal Playground ist ein Panel im Amigo Editor, das beim Oeffnen einer `.amigo.tidal`-Datei erscheint.

### Features

- **Play/Stop** mit Chiptune-Oszillatoren
- **Per-Stem Controls**: Mute/Solo, Instrument-Auswahl, Volume
- **BPM-Kontrolle** (20-300 BPM, live aenderbar)
- **Transforms**: slow, fast, rev
- **Presets**: Speichern/Laden von Instrument-Konfigurationen
- **WAV-Export**: Einen Zyklus als WAV exportieren
- **Live-Preview**: Musik hoeren waehrend man am Level arbeitet

### Instrumente

| Instrument | Klang | Typischer Einsatz |
|-----------|-------|------------------|
| Square Wave | Klassisch, voll | Melodie (NES-Style Lead) |
| Pulse 25% | Duenn, nasal | Harmonie |
| Pulse 12.5% | Sehr duenn, metallisch | Effekte |
| Triangle Wave | Weich, warm | Bass (NES-Style) |
| Sawtooth Wave | Harsch, voll | Lead |
| Noise Channel | Rauschen | Percussion / Hi-Hats |
| Sine Wave | Rein | Sub-Bass, Testtoen |

## Engine-Integration

```rust
// .amigo.tidal laden
let comp = amigo_tidal_parser::load("assets/music/overworld.amigo.tidal")?;

// Playground erstellen
let mut playground = TidalPlayground::new(comp);
playground.set_bpm(140.0);
playground.set_instrument(0, Instrument::SquareWave);

// Im Game-Loop: Audio rendern
playground.render_audio(&mut buffer, 44100);

// Preset-Wechsel basierend auf Game-State
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
