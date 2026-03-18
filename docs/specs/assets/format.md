---
status: done
crate: amigo_assets
depends_on: []
last_updated: 2026-03-18
---

# Asset Definition Formats

## Purpose

Define the RON-based file formats used to describe art styles, music
configurations, audio style presets, and SFX definitions.  These files are the
primary data-driven interface between content creators (human or AI pipeline) and
the Amigo Engine runtime.  A central format registry validates and loads all
asset definition files at startup.

## Existierende Bausteine

### StyleDef (`tools/amigo_artgen/src/style.rs`)

| Type | Description |
|------|-------------|
| `StyleDef` | Art generation style: checkpoint, LoRA, palette, prompt prefix/negative, sprite size, diffusion steps, CFG scale, post-processing config, reference images |
| `PostProcessConfig` | Palette clamping, anti-alias removal, outline settings, transparency cleanup, tile edge check |
| `OutlineMode` | `Outer`, `Inner`, `Both` |
| `StyleError` | IO and parse error variants |
| `StyleDef::load_from_file(path)` | Load one style from a RON file |
| `StyleDef::load_all(dir)` | Load all `.ron` files in a directory |
| `StyleDef::builtin_defaults()` | 6 hardcoded styles (caribbean, lotr, dune, matrix, got, stranger_things) |
| `StyleDef::find(name)` | Lookup in builtin defaults |
| `StyleDef::parse_hex_color(hex)` | `"#RRGGBB"` to `[u8; 3]` |
| `StyleDef::palette_rgb()` | Convert palette to RGB arrays |
| `StyleDef::outline_rgba()` | Outline color as RGBA |

### MusicConfig types (`crates/amigo_audio/src/lib.rs`)

| Type | Description |
|------|-------------|
| `MusicLayer` | Named stem with base/current/target volume, fade speed, kira handle |
| `MusicSection` | Named section with layers + per-layer `LayerRule` |
| `LayerRule` | `Lerp { param, from, to }`, `Threshold { param, above, fade_seconds }`, `Toggle { param, fade_seconds }` |
| `MusicTransition` | `CrossfadeOnBar`, `FadeOutThenPlay`, `CutOnBar`, `StingerThen`, `LayerSwap` |
| `Stinger` | One-shot cue with `StingerQuantize` (Beat, Bar, Immediate) |
| `MusicParameters` | Float/bool parameter maps driving adaptive music |
| `BarClock` | BPM, beats_per_bar, elapsed time, beat/bar position |
| `AdaptiveMusicEngine` | Core adaptive music runtime |
| `SfxDefinition` | Files, volume, pitch variance, max concurrent, cooldown |
| `SfxManager` | Register/load/play SFX with cooldowns and concurrency |
| `AudioManager` | kira wrapper: load_sfx, play_sfx, play_sfx_at, play_music, volume channels |

### WorldAudioStyle (`tools/amigo_audiogen/src/lib.rs`)

| Type | Description |
|------|-------------|
| `WorldAudioStyle` | Per-world audio config: genre, genre_tags, default BPM, SFX style prefix, key instruments |
| `MusicSection` (audiogen) | `Calm`, `Tense`, `Battle`, `Boss`, `Victory`, `Menu`, `Custom(String)` |
| `SfxCategory` | `Gameplay`, `UI`, `Ambient`, `Impact`, `Explosion`, `Magic`, `Voice`, `Custom(String)` |
| `MusicRequest` | Request to generate music via ACE-Step |
| `SfxRequest` | Request to generate SFX via AudioGen |
| `MusicResult` / `SfxResult` | Generation output metadata |

## Public API

### Proposed: `.style.ron` Format

```ron
StyleDef(
    name: "caribbean",
    checkpoint: "pixel_art_xl_v1.safetensors",
    lora: Some(("pixel_art_16bit.safetensors", 0.7)),
    palette: ["#1a1a2e", "#e8c170", "#8b5e3c", "#3b7dd8", "#4caf50",
              "#f5f5dc", "#c0392b", "#f39c12", "#2c3e50", "#ecf0f1"],
    prompt_prefix: "pixel art, 16-bit style, tropical pirate theme,",
    negative_prompt: "realistic, 3d, smooth, anti-aliased, gradient, blurry",
    default_size: (32, 32),
    steps: 20,
    cfg_scale: 7.0,
    post_processing: PostProcessConfig(
        palette_clamp: true,
        remove_anti_aliasing: true,
        add_outline: true,
        outline_color: "#1a1a2e",
        outline_mode: Outer,
        cleanup_transparency: true,
        tile_edge_check: false,
    ),
    reference_images: ["styles/ref/caribbean_tower.png"],
)
```

Loaded via existing `StyleDef::load_from_file()`.  No new code needed for this
format, only documentation and validation.

### Proposed: `.music.ron` Format and Loader

```ron
// assets/music/caribbean.music.ron
MusicConfig(
    bpm: 130,
    beats_per_bar: 4,
    sections: [
        SectionDef(
            name: "calm",
            layers: [
                LayerDef(name: "bass",    file: "music/caribbean/calm_bass.ogg",    base_volume: 0.8, rule: None),
                LayerDef(name: "melody",  file: "music/caribbean/calm_melody.ogg",  base_volume: 0.7, rule: Some(Lerp(param: "tension", from: 0.0, to: 0.3))),
                LayerDef(name: "drums",   file: "music/caribbean/calm_drums.ogg",   base_volume: 0.6, rule: Some(Threshold(param: "danger", above: 0.2, fade_seconds: 1.5))),
            ],
        ),
        SectionDef(
            name: "battle",
            layers: [
                LayerDef(name: "bass",    file: "music/caribbean/battle_bass.ogg",    base_volume: 1.0, rule: None),
                LayerDef(name: "melody",  file: "music/caribbean/battle_melody.ogg",  base_volume: 0.9, rule: None),
                LayerDef(name: "drums",   file: "music/caribbean/battle_drums.ogg",   base_volume: 1.0, rule: None),
                LayerDef(name: "choir",   file: "music/caribbean/battle_choir.ogg",   base_volume: 0.5, rule: Some(Toggle(param: "boss", fade_seconds: 2.0))),
            ],
        ),
    ],
    transitions: {
        ("calm", "battle"): CrossfadeOnBar(bars: 2),
        ("battle", "calm"): FadeOutThenPlay(fade_bars: 4),
    },
    stingers: [
        StingerDef(name: "victory", file: "music/caribbean/victory_sting.ogg", quantize: Bar),
        StingerDef(name: "death",   file: "music/caribbean/death_sting.ogg",   quantize: Beat),
    ],
)
```

```rust
/// Serialisable music configuration loaded from `.music.ron`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MusicConfig {
    pub bpm: u32,
    pub beats_per_bar: u32,
    pub sections: Vec<SectionDef>,
    pub transitions: HashMap<(String, String), MusicTransition>,
    pub stingers: Vec<StingerDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectionDef {
    pub name: String,
    pub layers: Vec<LayerDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerDef {
    pub name: String,
    pub file: String,
    pub base_volume: f32,
    pub rule: Option<LayerRule>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StingerDef {
    pub name: String,
    pub file: String,
    pub quantize: StingerQuantize,
}

impl MusicConfig {
    pub fn load(path: &Path) -> Result<Self, FormatError>;
    /// Build an AdaptiveMusicEngine from this config (does not load audio data).
    pub fn build_engine(&self) -> AdaptiveMusicEngine;
}
```

### Proposed: `.audio_style.ron` Format and Loader

```ron
// assets/audio_styles/caribbean.audio_style.ron
WorldAudioStyle(
    name: "caribbean",
    genre: "pirate shanty",
    genre_tags: ["folk", "sea shanty", "accordion", "fiddle"],
    default_bpm: 130,
    sfx_style: "wooden, ocean, cannon, ",
    key_instruments: ["accordion", "fiddle", "drums", "bass"],
)
```

```rust
impl WorldAudioStyle {
    pub fn load(path: &Path) -> Result<Self, FormatError>;
    pub fn load_all(dir: &Path) -> Vec<Self>;
}
```

### Proposed: `.sfx.ron` Format and Loader

```ron
// assets/sfx/combat.sfx.ron
SfxBundle(
    sounds: {
        "sword_hit": SfxDef(
            files: ["sfx/combat/sword_hit_01.ogg", "sfx/combat/sword_hit_02.ogg", "sfx/combat/sword_hit_03.ogg"],
            volume: 0.9,
            pitch_variance: 0.15,
            max_concurrent: 3,
            cooldown: Some(0.05),
            category: Impact,
        ),
        "arrow_fire": SfxDef(
            files: ["sfx/combat/arrow_fire_01.ogg"],
            volume: 0.7,
            pitch_variance: 0.1,
            max_concurrent: 5,
            cooldown: None,
            category: Gameplay,
        ),
    },
)
```

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfxBundle {
    pub sounds: HashMap<String, SfxDefinition>,
}

impl SfxBundle {
    pub fn load(path: &Path) -> Result<Self, FormatError>;
    /// Register all sounds into an SfxManager.
    pub fn register_all(&self, manager: &mut SfxManager);
}
```

### Proposed: Central Format Registry

```rust
/// Unified loader for all RON-based asset definition formats.
pub struct FormatRegistry {
    styles: Vec<StyleDef>,
    music_configs: HashMap<String, MusicConfig>,
    audio_styles: Vec<WorldAudioStyle>,
    sfx_bundles: Vec<SfxBundle>,
}

impl FormatRegistry {
    pub fn new() -> Self;
    /// Scan a directory tree and load all recognised `.ron` files
    /// by extension pattern: `.style.ron`, `.music.ron`,
    /// `.audio_style.ron`, `.sfx.ron`.
    pub fn load_directory(&mut self, root: &Path) -> Result<(), FormatError>;

    pub fn style(&self, name: &str) -> Option<&StyleDef>;
    pub fn music_config(&self, name: &str) -> Option<&MusicConfig>;
    pub fn audio_style(&self, name: &str) -> Option<&WorldAudioStyle>;
    pub fn sfx_bundle(&self, index: usize) -> Option<&SfxBundle>;

    /// Validate all cross-references (e.g. stinger names in transitions,
    /// file paths exist on disk).
    pub fn validate(&self, asset_root: &Path) -> Vec<FormatWarning>;
}

#[derive(Debug)]
pub enum FormatError {
    Io(std::io::Error),
    Parse { file: PathBuf, message: String },
    UnknownExtension(String),
}

#[derive(Debug)]
pub struct FormatWarning {
    pub file: PathBuf,
    pub message: String,
}
```

## Behavior

### File Discovery

`FormatRegistry::load_directory(root)` walks the directory tree recursively.
Files are dispatched by their compound extension:

| Pattern | Loader |
|---------|--------|
| `*.style.ron` | `StyleDef::load_from_file` |
| `*.music.ron` | `MusicConfig::load` |
| `*.audio_style.ron` | `WorldAudioStyle::load` |
| `*.sfx.ron` | `SfxBundle::load` |

Unrecognised `.ron` files are silently skipped (other systems may own them).

### Validation

After loading, `validate()` checks:
- All audio file paths referenced in `MusicConfig` and `SfxBundle` exist.
- Stinger names referenced in `MusicTransition::StingerThen` are defined.
- Palette hex strings are valid 6-digit hex.
- No duplicate section names within a `MusicConfig`.

Warnings are collected, not fatal -- allows partial asset sets during development.

### Hot Reload

File watchers (future) will detect changes to `.ron` files and reload the
affected definition.  The registry emits a `FormatReloaded` event that
downstream systems (audio engine, art pipeline) can subscribe to.

## Internal Design

- All format types derive `Serialize, Deserialize` via serde + the `ron` crate.
- The registry holds owned data; downstream systems borrow or clone as needed.
- File paths inside RON files are relative to the asset root, not absolute.
- The `FormatRegistry` is designed to be created once at startup and stored in
  the engine's resource table.

## Non-Goals

- Binary asset formats (`.bin`, `.pak`) -- handled by the asset pipeline crate.
- Texture atlas definitions (see `assets/atlas` spec).
- Shader definition files (WGSL is loaded directly).
- Level/map file formats (handled by tilemap serialisation).

## Open Questions

1. Should `.sfx.ron` bundle multiple sounds, or should each sound be its own
   file (one SfxDefinition per `.sfx.ron`)?
2. Should `MusicConfig` transitions use a map keyed by `(from, to)` section
   names, or a flat list with explicit from/to fields?
3. Should the registry support versioning (a `format_version` field in each RON
   file) for backwards compatibility?
4. Should style files reference palettes by name from a shared palette registry,
   or inline the hex colors directly?

## Referenzen

- Art style definitions: `tools/amigo_artgen/src/style.rs` (381 lines)
- Audio types: `crates/amigo_audio/src/lib.rs` (~1000 lines)
- Audio generation types: `tools/amigo_audiogen/src/lib.rs` (317 lines)
- RON format: github.com/ron-rs/ron
