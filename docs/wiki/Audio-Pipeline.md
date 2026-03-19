# Audio Pipeline

The pipeline converts audio files into `.amigo.tidal` mini-notation that the engine plays back at runtime using built-in chiptune oscillators.

**Prerequisite:** `amigo setup --only audio` (see [AI Setup](AI-Setup))

## Pipeline Stages

```
Audio (.wav/.ogg/.mp3)
    |
    v
[1] Demucs (Stem Separation)
    |  melody.wav, bass.wav, drums.wav, harmony.wav
    v
[2] Basic Pitch (Audio-to-MIDI)
    |  melody.mid, bass.mid, drums.mid, harmony.mid
    v
[3] midi_to_tidalcycles (MIDI-to-Notation)
    |  TidalCycles mini-notation per stem
    v
[4] Assembler (.amigo.tidal)
    |  Metadata + stem definitions
    v
overworld.amigo.tidal
```

## Commands

```sh
# Full pipeline: Audio -> .amigo.tidal
amigo pipeline convert \
  --input overworld.wav \
  --output overworld.amigo.tidal \
  --bpm 140 \
  --name "overworld_theme" \
  --license "CC-BY-4.0" \
  --author "Ozzed"

# Only stem separation
amigo pipeline separate --input track.wav --output ./stems/

# Only audio-to-MIDI
amigo pipeline transcribe --input ./stems/ --output ./midi/

# Only MIDI-to-TidalCycles
amigo pipeline notate --input ./midi/ --output track.amigo.tidal

# Batch processing
amigo pipeline batch --input ./tracks/ --output ./tidal/

# Preview / play a file
amigo pipeline play overworld.amigo.tidal
```

## Configuration (pipeline.toml)

```toml
name = "chiptune-default"

[separation]
model = "htdemucs"
skip_if_mono = true

[separation.stem_mapping]
vocals = "melody"
bass = "bass"
drums = "percussion"
other = "harmony"

[transcription]
onset_threshold = 0.5
frame_threshold = 0.3
min_note_length_ms = 50
pitch_bend = false
quantize_grid = 16

[conversion]
resolution = 16
include_legato = true
include_amplitude = true
consolidate = true

[postprocessing]
remove_ghost_notes = true
ghost_note_threshold = 0.1
normalize_velocity = true
merge_short_rests = true
```

## .amigo.tidal File Format

```
-- amigo:meta
-- name: "overworld_theme"
-- bpm: 140
-- source: "ozzed_adventure.wav"
-- license: "CC-BY-4.0"
-- author: "Ozzed"

-- amigo:stem melody
d1 $ slow 8 $ n "c5 d5 e5 ~ g5 a5 g5 e5" # amp "0.8 0.7 0.9 0 0.8 0.7 0.9 0.8"

-- amigo:stem bass
d2 $ slow 8 $ n "c3 ~ c3 ~ g2 ~ g2 ~" # amp "0.9!8"

-- amigo:stem percussion
d3 $ slow 8 $ n "bd ~ sd ~ bd ~ sd bd"
```

See [TidalCycles](TidalCycles) for the full notation reference.
