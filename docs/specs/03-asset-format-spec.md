# Amigo Engine - Asset Format & Import/Export Specification

**Version:** 0.2.0-draft
**Status:** Design Phase
**Author:** Daniel (via Claude)
**Depends on:** Amigo RomKit Spec v0.1

---

## 1. Overview

The Amigo Asset Format (AAF) is the native asset pipeline for the Amigo Engine — a Rust-based 2D game engine supporting both pixel art and hand-drawn raster art styles. It defines how all game assets (sprites, tilesets, maps, audio, entities, palettes) are authored, stored, imported, exported, and compiled for runtime.

### Design Principles

- **Human-readable sources** — All asset definitions are TOML files, commentable, diffable, and Git-friendly
- **Deterministic builds** — Same sources always produce identical binary output
- **Round-trip capable** — Import from external tools, export back without data loss
- **Separation of data and metadata** — Binary assets (PNG, JPG, WAV) alongside TOML descriptors
- **Convention over configuration** — Sensible defaults, override only what differs
- **Pattern-native audio** — Music and SFX defined via a TidalCycles/Strudel-inspired pattern language
- **Dual art pipeline** — First-class support for both indexed pixel art and full-color raster art
- **Royalty-free stack** — All runtime formats are patent-free (WebP, OGG Vorbis, FLAC, LZ4)

### Art Pipeline Summary

|                        | Pixel Art                     | Raster Art                  |
| ---------------------- | ----------------------------- | --------------------------- |
| **Ideal for**          | Celeste, Shovel Knight, retro | Cuphead, Hollow Knight, Ori |
| **Source formats**     | PNG (indexed)                 | PNG, JPG                    |
| **Runtime format**     | Amigo Indexed Tile (.ait)     | WebP (lossy)                |
| **Compression**        | LZ4 on indexed data           | WebP built-in               |
| **Color depth**        | 2/4/8 bpp (4–256 colors)      | 24/32-bit RGBA              |
| **Transparency**       | 1-bit (index 0 = transparent) | 8-bit alpha channel         |
| **Palette swaps**      | Native (instant, zero cost)   | Shader-based                |
| **Typical frame size** | 8×8 – 64×64 px                | 256×256 – 1024×1024 px      |

### Audio Pipeline Summary

| Context                       | Source                 | Runtime          | Rationale                                                     |
| ----------------------------- | ---------------------- | ---------------- | ------------------------------------------------------------- |
| Synth music (chiptune)        | `.music.toml` patterns | Pattern bytecode | No audio file needed — generated at runtime                   |
| Recorded music / long samples | WAV, FLAC              | **OGG Vorbis**   | Royalty-free, industry standard, pure Rust decoder (`lewton`) |
| Short SFX (synth)             | `.sfx.toml` patterns   | Pattern bytecode | Generated at runtime                                          |
| Short SFX (sampled)           | WAV                    | **OGG Vorbis**   | Same codec for consistency                                    |
| Lossless archival (optional)  | WAV                    | **FLAC**         | Pure Rust decoder (`claxon`), ~50% smaller than WAV           |

### Supported Import Formats

| Format                   | Tool                          | Asset Types                                     |
| ------------------------ | ----------------------------- | ----------------------------------------------- |
| `.aseprite` / `.ase`     | Aseprite                      | Sprites, animations, palettes, hitboxes         |
| `.tmx` / `.tmj` / `.tsx` | Tiled Map Editor              | Tilemaps, tilesets, entity spawns, collision    |
| `.ldtk`                  | LDTK (Level Designer Toolkit) | Tilemaps, entity defs, world layout, auto-tiles |
| `.mml`                   | Music Macro Language          | Music sequences (converted to Amigo patterns)   |
| `.vgm` / `.vgz`          | VGM (Video Game Music)        | Chiptune playback data                          |
| `.gb` / `.gbc`           | Game Boy ROM                  | Tiles, sprites, palettes (via RomKit)           |
| `.nes`                   | NES ROM (iNES)                | CHR tiles, sprites, palettes (via RomKit)       |
| `.sms` / `.gg`           | SMS/Game Gear ROM             | Tiles, sprites, palettes (via RomKit)           |

### Supported Export Formats

| Format      | Direction              | Notes                                        |
| ----------- | ---------------------- | -------------------------------------------- |
| `.tmx`      | Map → Tiled            | Full round-trip including custom properties  |
| `.ldtk`     | Maps + Entities → LDTK | World layout, entity definitions             |
| `.aseprite` | Sprite → Aseprite      | Layers, tags, slices preserved               |
| `.mml`      | Music → MML            | Lossy — pattern transforms not representable |

---

## 2. Project Structure

```
my-game/
├── Amigo.toml                    # Project manifest
├── assets/
│   ├── sprites/
│   │   ├── player.sprite.toml    # Sprite definition
│   │   ├── player.png            # Source sprite sheet (PNG for pixel art, PNG/JPG for raster)
│   │   ├── enemies.sprite.toml
│   │   └── enemies.png
│   ├── tilesets/
│   │   ├── overworld.tileset.toml
│   │   └── overworld.png
│   ├── maps/
│   │   ├── world1-1.map.toml
│   │   ├── world1-1.main.csv     # Layer data (external)
│   │   └── world1-2.map.toml
│   ├── audio/
│   │   ├── tracks/
│   │   │   ├── overworld.music.toml
│   │   │   └── boss.music.toml
│   │   ├── sfx/
│   │   │   ├── jump.sfx.toml
│   │   │   └── coin.sfx.toml
│   │   ├── instruments/
│   │   │   ├── chiptune.bank.toml
│   │   │   └── retro-drums.bank.toml
│   │   └── samples/
│   │       ├── kick.wav           # Source samples always WAV
│   │       └── orchestra-loop.wav
│   ├── palettes/
│   │   ├── default.palette.toml
│   │   └── gameboy.palette.toml
│   └── entities/
│       ├── goomba.entity.toml
│       └── coin-block.entity.toml
└── build/
    └── game.amigo-pak            # Compiled binary bundle
```

### 2.1 Project Manifest (`Amigo.toml`)

```toml
[project]
name = "super-adventure"
version = "0.1.0"
engine = "amigo 0.1"
authors = ["Daniel"]

[display]
resolution = [256, 224]        # Native resolution in pixels
pixel_scale = 4                # Default upscale factor
fps = 60

# --- Art Pipeline ---

[art]
mode = "pixel"                 # "pixel" | "raster" | "hybrid"

[art.pixel]
# Active when mode = "pixel" or "hybrid"
# Source: PNG → Runtime: Amigo Indexed Tile (.ait) + LZ4
palette_enforce = true         # Reject off-palette colors at build time
default_palette = "default"

[art.raster]
# Active when mode = "raster" or "hybrid"
# Source: PNG/JPG → Runtime: WebP (lossy)
webp_quality = 85              # WebP lossy quality (0–100)
webp_lossless_ui = true        # Use lossless WebP for UI elements (sharp edges)
max_atlas_size = 4096          # Larger atlases for raster art
alpha_mode = "premultiplied"   # premultiplied | straight
trim_whitespace = true         # Auto-trim transparent borders on sprites

# --- Audio Pipeline ---

[audio]
sample_rate = 44100
channels = 8                   # Max simultaneous audio channels
master_volume = 0.8

[audio.build]
# Source: WAV → Runtime: OGG Vorbis (lossy) or FLAC (lossless)
lossy_format = "ogg"           # OGG Vorbis — royalty-free, pure Rust decoder (lewton)
lossy_quality = 0.6            # Vorbis quality (-0.1 to 1.0); 0.6 ≈ 192kbps
lossless_format = "flac"       # FLAC — pure Rust decoder (claxon), ~50% smaller than WAV
prefer_lossless = false        # true = encode samples as FLAC instead of OGG
stream_threshold = 10          # Seconds — tracks over this are streamed, not fully preloaded

# --- Build ---

[build]
output = "build/game.amigo-pak"
compression = "lz4"            # Pak-level block compression
```

---

## 3. Runtime Formats — Technical Specification

### 3.1 Amigo Indexed Tile (.ait) — Pixel Art Runtime Format

The `.ait` format stores pixel art assets as palette-indexed data, enabling instant palette swaps and minimal memory footprint. It is the core runtime format for pixel art mode.

**Why not PNG at runtime?** PNG decoding is CPU-intensive (zlib inflate + filter reconstruction). AIT with LZ4 decompresses 10–50× faster while being comparable in size for indexed images.

**Why not QOI?** QOI stores full RGBA per pixel. For indexed pixel art (4–256 colors), storing palette indices at 2–8 bpp is fundamentally more compact, and enables zero-cost palette swaps that RGBA formats cannot.

#### Binary Layout

```
┌──────────────────────────────────────────────────┐
│ Magic: "AIT\0"            (4 bytes)              │
│ Version: u8               (1 byte)   — currently 1│
│ Width: u16 LE             (2 bytes)              │
│ Height: u16 LE            (2 bytes)              │
│ Bit Depth: u8             (1 byte)   — 2, 4, or 8│
│ Compression: u8           (1 byte)   — 0=raw, 1=LZ4│
│ Palette Hash: u64 LE      (8 bytes)  — FNV-1a of palette name│
│ Reserved: [u8; 5]         (5 bytes)              │
├──────────────────────────────────────────────────┤
│ Pixel Data                (variable)             │
│   If compression=0: raw indexed pixels           │
│   If compression=1: LZ4 compressed block         │
│                                                  │
│   Pixels are stored left-to-right, top-to-bottom │
│   Packed by bit depth:                           │
│     2bpp: 4 pixels per byte (MSB first)          │
│     4bpp: 2 pixels per byte (MSB first)          │
│     8bpp: 1 pixel per byte                       │
└──────────────────────────────────────────────────┘
```

**Header: 24 bytes fixed.** Pixel data follows immediately.

The palette itself is NOT embedded — it's referenced by hash. This means:

- Palette swaps are free: change the palette reference, pixels stay identical
- Multiple sprites can share a palette without duplication
- Palette data lives in the `.palette.toml` → compiled palette block in the `.amigo-pak`

#### Size Comparison (typical 256×256 sprite sheet, 16 colors)

| Format           | Size   |
| ---------------- | ------ |
| PNG (indexed)    | ~12 KB |
| QOI (RGBA)       | ~28 KB |
| AIT (4bpp + LZ4) | ~8 KB  |
| Raw RGBA         | 256 KB |

### 3.2 WebP — Raster Art Runtime Format

For hand-drawn, full-color art (Cuphead/Hollow Knight style), the Amigo Engine uses WebP as the sole raster runtime format.

**Why WebP over JPEG?** WebP handles both opaque and transparent assets in a single format. No need for separate pipelines or JPEG+alpha-mask hacks. WebP lossy is 25–34% smaller than JPEG at equivalent quality, and WebP lossless is 26% smaller than PNG.

**Why WebP over AVIF?** AVIF has better lossy compression but worse lossless compression, slower encoding, and no pure-Rust decoder. WebP has mature Rust support and is the established standard across game engines and web platforms.

#### Build Behavior

The build step automatically converts source images to WebP:

```
Source PNG/JPG → amigo build → WebP in .amigo-pak

Decision logic:
1. Is art_mode = "pixel"?          → AIT (indexed)
2. Is art_mode = "raster"?         → WebP (lossy, quality from config)
3. Is art_mode = "hybrid"?         → Per-asset override via art_mode field
4. Is the asset tagged as UI?      → WebP (lossless, for sharp edges)
```

#### Raster Sprite Features

Raster sprites support additional features not relevant to pixel art:

```toml
[sprite]
name = "boss_baroness"
sheet = "boss_baroness.png"
art_mode = "raster"                # Explicit override in hybrid mode
tile_size = [512, 512]             # Larger frames for hand-drawn art
origin = [256, 512]
framerate = 24                     # Cuphead-style: 24fps animation in 60fps game
playback = "hold"                  # "hold" = frame persists until next (for 24fps in 60fps)

# Raster-specific: trim transparent pixels to save VRAM
trim = true
mesh_mode = "tight"                # "rect" | "tight" — tight cuts away transparent areas
mesh_alpha_threshold = 10          # Pixels with alpha < 10 are trimmed

# Texture packing
pack_group = "bosses"              # Group related sprites in same atlas
```

### 3.3 OGG Vorbis — Audio Runtime Format

All lossy audio in the Amigo Engine uses OGG Vorbis.

**Why OGG Vorbis?**

- **Royalty-free** — No patents, no licensing fees, no legal risk
- **Industry standard** — Used by Unity, Godot, Unreal, and virtually every indie engine
- **Pure Rust decoder** — `lewton` crate, zero C dependencies, no unsafe code
- **Good quality** — Transparent at ~192kbps for music, excellent for SFX at lower bitrates
- **Streaming-capable** — Page-based format supports seeking and streaming

**Why not AAC?** Patent-encumbered ($15,000 initial license + per-unit fees), no pure Rust decoder.

**Why not Opus?** Technically superior at low bitrates, but recent patent pool attempts (Vectis IP, 2023) create licensing uncertainty. Marginal quality benefit at typical game audio bitrates (128–192kbps).

**Why not MP3?** Patents expired (2017) but the format is technically inferior to Vorbis at every bitrate. No advantage.

#### Build Behavior

```
Source WAV → amigo build → OGG Vorbis in .amigo-pak

Decision logic:
1. Is the audio a pattern/synth definition? → Pattern bytecode (no audio file)
2. Is prefer_lossless = true?               → FLAC
3. Is duration > stream_threshold?          → OGG Vorbis, marked as streaming
4. Otherwise                                → OGG Vorbis, fully preloaded
```

### 3.4 FLAC — Lossless Audio (Optional)

For projects requiring lossless audio (audiophile soundtracks, archival quality):

- **Pure Rust decoder** — `claxon` crate
- **~50% smaller than WAV** — Significant savings with zero quality loss
- **Activated per-project** — Set `prefer_lossless = true` in `Amigo.toml`

### 3.5 Pattern Bytecode — Synth Audio Runtime Format

Pattern-based music and SFX (the Strudel/TidalCycles-inspired system) are compiled from `.music.toml` and `.sfx.toml` into a compact bytecode representation. No audio file is generated — the engine's built-in synthesizer generates audio at runtime.

This means a full chiptune soundtrack can be a few kilobytes of bytecode, compared to megabytes of recorded audio.

---

## 4. Sprite Format (`.sprite.toml`)

Sprites are the primary visual entity format. Each sprite definition references a sprite sheet image and describes its frames, animations, hitboxes, and attachment points.

```toml
[sprite]
name = "player"
sheet = "player.png"               # Source image (PNG for pixel, PNG/JPG for raster)
art_mode = "pixel"                 # "pixel" | "raster" — override project default (hybrid mode)
tile_size = [16, 16]               # Frame size in pixels
columns = 8                        # Frames per row (auto-detected if omitted)
palette = "default"                # Palette reference (pixel art only)
origin = [8, 16]                   # Sprite origin/pivot point (default: bottom-center)

# --- Animations ---

[animations.idle]
frames = [0, 1]                    # Frame indices from sheet
timing = [400, 400]                # Duration per frame in ms
loop = true

[animations.walk]
frames = [2, 3, 4, 5]
timing = [100, 100, 100, 100]
loop = true

[animations.jump]
frames = [6]
timing = [0]
loop = false

[animations.die]
frames = [8, 9, 10, 11]
timing = [100, 100, 200, 300]
loop = false
on_complete = "destroy"            # Entity action on animation end

# --- Hitboxes ---

[hitboxes.body]
rect = { x = 2, y = 0, w = 12, h = 16 }
applies_to = "*"                   # All frames

[hitboxes.feet]
rect = { x = 3, y = 14, w = 10, h = 2 }
applies_to = "*"

# --- Attachment Points ---

[attachments.hand]
position = { x = 14, y = 8 }
applies_to = "*"

# --- Variants (pixel art: palette swaps; raster: shader-based) ---

[variants.player2]
palette = "player2-palette"        # Pixel art: instant palette swap
# shader = "hue_shift"             # Raster art: shader-based recolor
```

### 4.1 Aseprite Import Mapping

| Aseprite Concept | Amigo Equivalent                |
| ---------------- | ------------------------------- |
| Tags             | `[animations.*]` sections       |
| Tag direction    | `loop_mode` field               |
| Frame duration   | `timing` array                  |
| Slices           | `[hitboxes.*]` sections         |
| Slice pivot      | `origin`                        |
| Layers           | Flattened into composite frames |
| Palette          | Exported to `.palette.toml`     |
| Tilemap mode     | Exported to `.tileset.toml`     |

---

## 5. Tileset Format (`.tileset.toml`)

```toml
[tileset]
name = "overworld"
image = "overworld.png"
tile_size = [8, 8]
palette = "default"

# --- Tile Properties ---

[properties]
solid = [0, 1, 2, "5..7", 12, 13]
platform = [20, 21, 22]                    # One-way: solid from above
breakable = [10, 11]
damage = { tiles = [30, 31], value = 1 }
ladder = [45, 46]
water = [50, 51, 52]

# --- Animated Tiles ---

[[animated_tiles]]
frames = [40, 41, 42, 43]
timing = 150
loop = true

# --- Auto-Tile Rules ---

[auto_tiles.grass]
inner = 1
top = 2
bottom = 3
left = 4
right = 5
top_left_outer = 6
top_right_outer = 7
bottom_left_outer = 8
bottom_right_outer = 9
```

---

## 6. Map Format (`.map.toml`)

```toml
[map]
name = "World 1-1"
size = [256, 18]
tile_size = [8, 8]
tileset = "overworld"
background_color = "#5C94FC"
scroll = "horizontal"              # horizontal | vertical | free | locked

# --- Layers ---

[layers.background]
data = "world1-1.bg.csv"
parallax = [0.5, 1.0]
tileset = "background-decor"       # Layer-specific tileset override

[layers.main]
data = "world1-1.main.csv"
collision = true

[layers.foreground]
data = "world1-1.fg.csv"
parallax = [1.2, 1.0]

# --- Entities ---

[[entities]]
type = "goomba"
position = [320, 128]
properties = { direction = "left" }

[[entities]]
type = "pipe-warp"
position = [480, 112]
properties = { target_map = "world1-1-bonus", target_spawn = "entry" }

# --- Spawn Points ---

[[spawns]]
name = "start"
position = [24, 128]
default = true

# --- Triggers ---

[[triggers]]
name = "boss-arena"
region = { x = 2000, y = 0, w = 160, h = 144 }
on_enter = "camera_lock"
properties = { boss = "king_totomesu", music = "boss" }

# --- Camera ---

[camera]
mode = "follow_x"
bounds = { left = 0, right = 2560, top = 0, bottom = 144 }
dead_zone = { x = 32, y = 16 }
look_ahead = 48
```

---

## 7. Entity Format (`.entity.toml`)

```toml
[entity]
name = "goomba"
sprite = "enemies"
default_animation = "goomba-walk"
category = "enemy"

[physics]
gravity = true
body = { w = 8, h = 8 }
max_velocity = { x = 0.5, y = 4.0 }
friction = 0.8
solid = true

[behavior]
type = "patrol"
direction = "left"
reverse_on_wall = true
reverse_on_edge = true
activate_distance = 256

[interactions.stomp]
condition = "player_above"
effect = "die"
player_bounce = true
score = 100

[interactions.touch]
condition = "player_touch"
effect = "damage_player"
damage = 1

[drops.on_death]
type = "score_popup"
value = 100
```

---

## 8. Palette Format (`.palette.toml`)

```toml
[palette]
name = "default"
mode = "indexed"                   # indexed | rgba
bit_depth = 4                      # 2, 4, or 8 → 4/16/256 colors

colors = [
    "#00000000",    # 0: Transparent
    "#0f380f",      # 1: Darkest
    "#306230",      # 2: Dark
    "#8bac0f",      # 3: Light
    "#9bbc0f",      # 4: Lightest
]

[roles]
transparent = 0
outline = 1
primary = 3
highlight = 4

[swaps.player2]
mapping = { 3 = "#2244aa", 4 = "#4466cc" }

[swaps.damage_flash]
mapping = { 1 = "#ffffff", 2 = "#ffffff", 3 = "#ffffff" }
duration = 100                     # ms — auto-revert
```

---

## 9. Audio System — Pattern-Based (Strudel/TidalCycles-Inspired)

### 9.1 Core Concepts

The Amigo audio system uses a cycle-based pattern language inspired by TidalCycles/Strudel. Music is defined as patterns that repeat over cycles, enabling compact, composable, reactive game music.

### 9.2 Mini-Notation

| Symbol  | Meaning                         | Example          |
| ------- | ------------------------------- | ---------------- |
| ` `     | Sequence — divide cycle evenly  | `"c d e f"`      |
| `~`     | Rest / silence                  | `"c ~ e ~"`      |
| `*n`    | Speed up — repeat n times       | `"c*4"`          |
| `/n`    | Slow down — once every n cycles | `"c/2"`          |
| `[...]` | Subdivide a step                | `"c [d e] f"`    |
| `<...>` | Alternate per cycle             | `"c <d e> f"`    |
| `,`     | Stack / chord                   | `"[c,e,g]"`      |
| `(n,m)` | Euclidean rhythm                | `"c(3,8)"`       |
| `?`     | Random (50%)                    | `"c d? e"`       |
| `_`     | Elongate previous               | `"c _ e f"`      |
| `!n`    | Replicate                       | `"c!3"`          |
| `{...}` | Polymetric                      | `"{c d e, f g}"` |

### 9.3 Instrument Bank (`.bank.toml`)

```toml
[bank]
name = "chiptune"

[instruments.square50]
type = "synth"
waveform = "square"
duty_cycle = 0.5
volume = 0.7

[instruments.triangle]
type = "synth"
waveform = "triangle"
volume = 0.8

[instruments.kick]
type = "sample"
file = "../samples/kick.wav"       # Source: WAV → built as OGG Vorbis
base_note = "c3"
volume = 0.9

[effects.retro_reverb]
type = "reverb"
room = 0.3
mix = 0.2
```

### 9.4 Music Track (`.music.toml`)

```toml
[track]
name = "Overworld Theme"
bpm = 140
time_signature = [4, 4]
bank = "chiptune"
key = "c major"

[channels.melody]
instrument = "square50"
pattern = """
  [e5 e5 ~ e5] [~ c5 e5 ~] [g5 ~ ~ ~] [~ g4 ~ ~]
"""
volume = 0.8
effects = ["retro_reverb"]

[channels.bass]
instrument = "triangle"
pattern = """
  [c2 ~ ~ g2] [~ ~ c3 ~] [g2 ~ ~ ~] [~ c2 ~ ~]
"""
volume = 0.9

[channels.drums]
instrument = "kick"
pattern = "[kick snare kick snare]"
volume = 0.7

# --- Sections for reactive game music ---

[sections.intense]
bpm = 160
channels.melody.pattern = """
  [e5 e5 e5 e5] [c5 e5 g5 e5] [g5 g5 g5 g5] [e5 c5 g4 c5]
"""
channels.drums.pattern = "[kick snare kick [snare kick]]"

[sections.victory]
bpm = 120
channels.melody.pattern = "[c5 e5 g5 c6] [~ _ _ _]"
loop = false

# --- Transitions ---

[transitions.default_to_intense]
from = "default"
to = "intense"
mode = "crossfade"
duration = 2

# --- Reusable pattern variables ---

[variables]
drum_basic = "kick snare kick snare"
drum_fill = "kick kick snare [kick snare]"
```

### 9.5 Sound Effect (`.sfx.toml`)

```toml
[sfx]
name = "jump"
priority = 5
category = "player"
type = "synth"

[synth]
waveform = "square"
duty_cycle = 0.5
frequency = "[400 500 600 800]"
volume = "[1.0 0.8 0.5 0.0]"
duration = 150
```

### 9.6 Reactive Music API (Conceptual)

```rust
audio.play_music("overworld");
audio.set_section("intense");                    // Uses defined transition
audio.queue_section("victory", Quantize::NextCycle);
audio.play_sfx("jump");
audio.play_sfx_pitched("coin", 1.2);
```

---

## 10. Pattern Language — Formal Grammar

```peg
pattern     = sequence ("|" sequence)*
sequence    = element+
element     = group / atom modifiers?
group       = "[" sequence "]"
            / "<" sequence ">"
            / "{" sequence ("," sequence)* "}"
            / "(" number "," number ("," number)? ")"
atom        = note / rest / sample_name
note        = [a-g] ("#" / "b")? [0-9]?
rest        = "~"
sample_name = [a-z][a-z0-9_-]*
modifiers   = ("*" number)? ("/" number)? ("!" number)? "_"* "?" number?
number      = [0-9]+ ("." [0-9]+)?
```

Patterns are compiled at build time to bytecode:

```
PatternOp::Note { pitch, velocity, start, duration }
PatternOp::Rest { start, duration }
PatternOp::Sample { id, start, pitch }
PatternOp::Alternate { variants, index_mode }
PatternOp::Euclidean { hits, steps, offset, inner }
PatternOp::Random { probability, inner }
```

---

## 11. Build System

### 11.1 Build Pipeline

```
Source Assets (TOML + PNG/JPG + WAV)
  │
  ├─► Validation
  │     • All references resolve
  │     • Palette compliance (pixel art mode)
  │     • Pattern syntax validation
  │     • Dimension checks
  │
  ├─► Image Processing
  │     ├── Pixel art PNG → Indexed → LZ4 → .ait blocks
  │     ├── Raster PNG/JPG → WebP lossy (quality from config) → WebP blocks
  │     ├── UI assets → WebP lossless → WebP blocks
  │     └── Atlas packing (per pack_group)
  │
  ├─► Audio Processing
  │     ├── Pattern .music.toml → Pattern bytecode
  │     ├── Pattern .sfx.toml → Pattern bytecode
  │     ├── WAV samples (≤ stream_threshold) → OGG Vorbis → preload blocks
  │     ├── WAV samples (> stream_threshold) → OGG Vorbis → streaming blocks
  │     └── (If prefer_lossless) WAV → FLAC
  │
  ├─► Data Processing
  │     ├── Tilemaps → binary tile arrays
  │     ├── Entity defs → binary structs
  │     └── Auto-tiles → resolved neighbor lookups
  │
  └─► Packaging
        • All assets → single .amigo-pak file
        • LZ4 block compression (on non-compressed blocks)
        • TOC for O(1) asset lookup
        • SHA256 manifest
```

### 11.2 Build Commands

```bash
amigo build                        # Full build
amigo build --watch                # Rebuild on changes
amigo build --validate             # Validate only
amigo build --asset sprites/       # Build subset
amigo build --release              # Optimized (max WebP compression, strip debug)
```

### 11.3 `.amigo-pak` Binary Format

```
┌──────────────────────────────────────┐
│ Magic: "AMIG" (4 bytes)             │
│ Version: u16                         │
│ Flags: u16                           │
│   bit 0: has pixel art assets        │
│   bit 1: has raster art assets       │
│   bit 2: has streaming audio         │
│ TOC Offset: u64                      │
│ TOC Count: u32                       │
├──────────────────────────────────────┤
│ Asset Block 0                        │
│   Type tag: u8                       │
│     0x01 = AIT (pixel art)           │
│     0x02 = WebP (raster art)         │
│     0x03 = OGG Vorbis (audio)        │
│     0x04 = FLAC (lossless audio)     │
│     0x05 = Pattern bytecode          │
│     0x06 = Binary data (maps, etc)   │
│     0x07 = Palette data              │
│   Compression: u8 (0=none, 1=LZ4)   │
│   Data...                            │
├──────────────────────────────────────┤
│ Asset Block 1...                     │
├──────────────────────────────────────┤
│ Table of Contents                    │
│   name_hash: u64                     │
│   asset_type: u8                     │
│   offset: u64                        │
│   compressed_size: u32               │
│   uncompressed_size: u32             │
│   flags: u8 (streaming, preload)     │
│   checksum: u32                      │
├──────────────────────────────────────┤
│ SHA256 Manifest                      │
└──────────────────────────────────────┘
```

---

## 12. Import / Export CLI

### 12.1 Import

```bash
amigo import aseprite player.aseprite              # → sprites/
amigo import tiled world1.tmx                      # → maps/ + tilesets/
amigo import ldtk game.ldtk                        # → maps/ + tilesets/ + entities/
amigo import mml theme.mml                         # → audio/tracks/
amigo import vgm soundtrack.vgm                    # → audio/tracks/

# ROM imports (via RomKit — see RomKit Spec)
amigo romkit extract game.gb                       # → raw tiles
amigo romkit extract game.gb --profile super-mario-land  # → structured project

# Batch
amigo import aseprite assets/raw/*.aseprite
```

### 12.2 Export

```bash
amigo export tiled maps/world1-1.map.toml          # → .tmx
amigo export aseprite sprites/player.sprite.toml   # → .aseprite
amigo export ldtk                                  # → .ldtk project
amigo export mml audio/tracks/overworld.music.toml # → .mml (lossy)
```

### 12.3 Round-Trip Fidelity

| Format           | Fidelity                                    |
| ---------------- | ------------------------------------------- |
| Aseprite ↔ Amigo | High — frames, tags, slices                 |
| Tiled ↔ Amigo    | High — layers, objects, properties          |
| LDTK ↔ Amigo     | Medium — some auto-tile rules simplify      |
| MML → Amigo      | Medium — basic patterns map well            |
| Amigo → MML      | Low — sections/transforms not representable |
| VGM → Amigo      | Low — lossy conversion from register writes |

### 12.4 Import Metadata

Every imported asset stores provenance:

```toml
[_import]
source = "player.aseprite"
format = "aseprite"
imported_at = "2026-03-15T14:30:00Z"
checksum = "sha256:abc123..."
```

---

## 13. Rust Crate Dependencies (Runtime)

| Crate                  | Purpose                        | Pure Rust | no_std |
| ---------------------- | ------------------------------ | --------- | ------ |
| `lz4_flex`             | LZ4 compression/decompression  | Yes       | Yes    |
| `lewton`               | OGG Vorbis decoding            | Yes       | No     |
| `claxon`               | FLAC decoding                  | Yes       | No     |
| `image` (webp feature) | WebP decoding                  | Partial   | No     |
| `toml`                 | TOML parsing (build-time only) | Yes       | No     |

All runtime decoders are royalty-free and available as pure Rust implementations. No C dependencies required for the core runtime.

---

## 14. Future Extensions

### 14.1 Planned

- **World Format** (`.world.toml`) — Multi-map world layout with connections
- **Dialogue Format** (`.dialogue.toml`) — Branching dialogue trees
- **Shader Format** (`.shader.toml`) — Palette-aware post-processing (CRT, scanlines)
- **PICO-8 Import** — Extract assets from `.p8` cartridges
- **Godot Import** — Import 2D scenes from `.tscn` files
- **Tier 2 ROM support** — SNES, Mega Drive, GBA (see RomKit Spec)

### 14.2 Community

- **Game Profile Registry** — Community ROM extraction profiles
- **Instrument Bank Sharing** — Publish `.bank.toml` collections
- **Pattern Library** — Reusable drum patterns, bass lines, arpeggios
