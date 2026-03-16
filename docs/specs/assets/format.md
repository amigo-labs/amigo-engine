# Asset Formats

> Status: draft
> Crate: amigo_assets
> Depends on: --
> Last updated: 2026-03-16

## Zweck

Defines all data formats used for assets in the Amigo Engine, including art style definitions for AI generation, audio style definitions, adaptive music configuration formats, and SFX definition formats.

## Public API

Style definitions are loaded by name:

```rust
// Art style loading
let style = styles.load("caribbean"); // -> loads styles/caribbean.style.ron

// Audio style loading
let audio_style = audio_styles.load("caribbean"); // -> loads styles/audio/caribbean.audio_style.ron
```

## Verhalten

### Art Style Definitions

Each world has a style file that constrains AI generation for visual consistency:

```ron
// styles/caribbean.style.ron
(
    name: "Caribbean",

    // Model selection
    checkpoint: "pixel_art_xl_v1.safetensors",
    lora: Some(("pixel_art_16bit.safetensors", 0.7)),

    // Color palette (enforced in post-processing)
    palette: [
        "#1a1a2e",  // outline/dark
        "#e8c170",  // sand
        "#8b5e3c",  // wood
        "#3b7dd8",  // water
        "#4caf50",  // leaf
        "#f5f5dc",  // bone/sail
        "#c0392b",  // red accent
        "#f39c12",  // gold
        "#2c3e50",  // shadow
        "#ecf0f1",  // highlight
    ],

    // Prompt engineering
    prompt_prefix: "pixel art, 16-bit style, tropical pirate theme,",
    negative_prompt: "realistic, 3d, smooth, anti-aliased, gradient, blurry, modern, photo",

    // Generation defaults
    default_size: (32, 32),
    steps: 20,
    cfg_scale: 7.0,

    // Post-processing flags
    post_processing: (
        palette_clamp: true,
        remove_anti_aliasing: true,
        add_outline: true,
        outline_color: "#1a1a2e",
        cleanup_transparency: true,
    ),

    // Reference images for img2img consistency
    reference_images: [
        "styles/ref/caribbean_tower.png",
        "styles/ref/caribbean_enemy.png",
        "styles/ref/caribbean_tiles.png",
    ],
)
```

### Post-Processing Configuration

Each step is toggleable per style:

```ron
post_processing: (
    palette_clamp: true,
    remove_anti_aliasing: true,
    add_outline: true,
    outline_color: "#1a1a2e",
    outline_mode: "outer",       // "outer", "inner", or "both"
    cleanup_transparency: true,
    tile_edge_check: false,      // only for tilesets
)
```

### Audio Style Definitions

Each world has a distinct sonic identity: different musical genre, different instruments, different SFX feel. The hybrid approach means no two worlds sound alike.

#### Per-World Audio Profiles

| World | Music Genre | Key Instruments | SFX Style |
|-------|-------------|-----------------|-----------|
| Caribbean | Orchestral sea shanty | Fiddle, accordion, war drums, brass, harpsichord | Wooden, explosive, wet (splashes, creaking) |
| Lord of the Rings | Epic orchestral / Howard Shore | French horn, cello, choir, harp, bodhran | Metallic, reverberant, stone (clang, echo) |
| Dune | Ambient electronic / Hans Zimmer | Duduk, throat singing, deep synth pads, tabla | Sandy, dry, resonant (wind, rumble, vibration) |
| Matrix | Dark synthwave / industrial | Analog synth, drum machine, distorted bass, glitch | Digital, crisp, processed (beeps, whooshes, electric) |
| Game of Thrones | Dark medieval orchestral | Cello, war drums, raven calls, low brass | Cold, metallic, heavy (ice crack, fire roar, steel) |
| Stranger Things | 80s retro synth / John Carpenter | Moog synth, Juno pads, gated reverb drums, arpeggios | Eerie, analog, distorted (static, warble, flicker) |

#### Caribbean Audio Style

```ron
// styles/audio/caribbean.audio_style.ron
(
    name: "Caribbean",
    genre: "orchestral sea shanty",

    // Core melody generation
    core_melody_instrument: "solo fiddle",
    default_bpm: 120,
    default_key: "C minor",

    // Stem instrument mapping
    stem_instruments: {
        "drums":   "war drums, snare, tambourine",
        "bass":    "double bass, pizzicato cello",
        "melody":  "fiddle, tin whistle",
        "strings": "string section, legato violins",
        "brass":   "brass fanfares, trumpet, french horn",
    },

    // Prompt engineering
    music_prompt_prefix: "pirate adventure, tropical, sea shanty influence,
        orchestral with fiddle and accordion, warm and adventurous,",
    negative_music_prompt: "electronic, synthesizer, modern, lo-fi, hip hop",

    // Per-mood overrides
    mood_prompts: {
        "calm":    "relaxed, gentle waves, soft strings, peaceful harbor, major key",
        "tense":   "building tension, low drums, ominous cello, distant thunder",
        "battle":  "epic battle, war drums, brass fanfares, fast strings, intense",
        "boss":    "dark epic, heavy percussion, choir, pipe organ, menacing",
        "victory": "triumphant, celebratory, fanfare, major key resolution",
    },

    // SFX generation style
    sfx_prompt_prefix: "fantasy pirate, wooden, nautical,",
    sfx_types: {
        "tower_fire":    "cannon blast with wooden creak",
        "tower_build":   "wood planks hammering, rope tying",
        "enemy_death":   "skeleton bones scattering on wood deck",
        "enemy_spawn":   "ghostly ship horn in the distance",
        "projectile":    "cannonball whoosh through salty air",
        "impact":        "heavy iron ball hitting wood",
        "gold_pickup":   "pirate coins clinking in chest",
        "life_lost":     "ship hull breach, water rushing",
    },

    // Ambient
    ambient_prompt: "tropical ocean, seagulls, gentle waves, wooden ship creaking, harbor bells",
)
```

#### Matrix Audio Style

```ron
// styles/audio/matrix.audio_style.ron
(
    name: "Matrix",
    genre: "dark synthwave industrial",

    core_melody_instrument: "analog synth lead",
    default_bpm: 140,
    default_key: "F# minor",

    stem_instruments: {
        "drums":   "TR-808 kick, hi-hat, industrial percussion",
        "bass":    "deep analog bass, sub bass, distorted 808",
        "melody":  "detuned saw lead, arp synth",
        "pads":    "dark ambient pads, filtered noise sweeps",
        "glitch":  "glitch percussion, digital artifacts, bit-crushed hits",
    },

    music_prompt_prefix: "dark synthwave, cyberpunk, matrix-inspired,
        analog synthesizers, industrial, moody and mechanical,",
    negative_music_prompt: "acoustic, orchestral, folk, warm, organic",

    mood_prompts: {
        "calm":    "ambient cyberpunk, distant city hum, soft pads, slow arp",
        "tense":   "rising synth tension, heartbeat bass, filtered sweeps",
        "battle":  "fast industrial, aggressive drums, distorted bass, chaos",
        "boss":    "slow heavy industrial, massive sub bass, alarm sirens, oppressive",
        "victory": "euphoric trance moment, major synth chord, release",
    },

    sfx_prompt_prefix: "digital, cyberpunk, electronic,",
    sfx_types: {
        "tower_fire":    "laser pulse, electric discharge",
        "tower_build":   "digital construction, data stream assembling",
        "enemy_death":   "glitch derez, digital dissolve",
        "enemy_spawn":   "matrix code rain materializing",
        "projectile":    "neon tracer bullet whoosh",
        "impact":        "electric shock hit, voltage surge",
        "gold_pickup":   "data packet acquired chime",
        "life_lost":     "system breach alarm, static burst",
    },

    ambient_prompt: "matrix digital rain, distant server hum, neon city ambience, electrical buzz",
)
```

#### Stranger Things Audio Style

```ron
// styles/audio/stranger_things.audio_style.ron
(
    name: "Stranger Things",
    genre: "80s retro synth horror",

    core_melody_instrument: "Moog synthesizer",
    default_bpm: 100,
    default_key: "D minor",

    stem_instruments: {
        "drums":   "gated reverb snare, LinnDrum, 80s electronic drums",
        "bass":    "Moog bass, analog sub",
        "melody":  "Juno-60 lead, detuned poly synth",
        "pads":    "lush 80s pads, chorus-drenched strings",
        "arp":     "sequenced arpeggio, clock-synced, pulsing",
    },

    music_prompt_prefix: "80s retro synth, John Carpenter inspired,
        analog synthesizers, gated reverb, nostalgic and eerie,",
    negative_music_prompt: "modern EDM, orchestral, acoustic guitar, folk",

    mood_prompts: {
        "calm":    "nostalgic 80s, warm synth pads, gentle arpeggio, suburban evening",
        "tense":   "eerie pulsing synth, reversed sounds, creeping dread, flickering",
        "battle":  "fast arpeggio, aggressive drums, distorted synth stabs, chase scene",
        "boss":    "deep Moog drone, terrifying low frequencies, demogorgon theme, dark",
        "victory": "triumphant 80s anthem, bright major synth, kids won, sunrise feeling",
    },

    sfx_prompt_prefix: "80s analog, retro, slightly distorted,",
    sfx_types: {
        "tower_fire":    "retro laser zap, arcade sound",
        "tower_build":   "walkie talkie static, bicycle bell, flashlight click",
        "enemy_death":   "demogorgon screech fading, dimensional tear closing",
        "enemy_spawn":   "upside down portal opening, wet organic rip",
        "projectile":    "slingshot whoosh, retro energy bolt",
        "impact":        "VHS distortion hit, analog crunch",
        "gold_pickup":   "arcade coin insert chime",
        "life_lost":     "christmas lights flickering, power surge",
    },

    ambient_prompt: "suburban night, crickets, distant 80s radio, gentle wind, occasional flickering electricity",
)
```

Style files for LotR, Dune, and GoT follow the same pattern. Each completely defines the sonic world for both Claude Code (generation prompts) and the engine (runtime playback).

### Adaptive Music Definition (RON)

Each world/level defines its adaptive music configuration:

```ron
// assets/audio/music/caribbean/caribbean_battle.music.ron
(
    bpm: 120,
    time_signature: (4, 4),
    bar_length_seconds: 2.0,       // 60 / 120 * 4

    // All stems loop-synced, same length, same BPM
    layers: {
        "drums":   (file: "caribbean_battle_drums.ogg",   base_volume: 1.0),
        "bass":    (file: "caribbean_battle_bass.ogg",     base_volume: 0.9),
        "melody":  (file: "caribbean_battle_melody.ogg",   base_volume: 0.8),
        "strings": (file: "caribbean_battle_strings.ogg",  base_volume: 0.7),
        "brass":   (file: "caribbean_battle_brass.ogg",    base_volume: 0.6),
    },

    // Game parameters control layer volumes
    // Parameter range: 0.0 to 1.0
    layer_rules: [
        // Strings always audible, fades with tension
        ("strings", Volume, Lerp(param: "tension", from: 0.3, to: 0.8)),

        // Drums kick in at medium tension
        ("drums",   Volume, Threshold(param: "tension", above: 0.3, fade: 0.5)),

        // Bass follows drums
        ("bass",    Volume, Threshold(param: "tension", above: 0.35, fade: 0.5)),

        // Melody at higher tension
        ("melody",  Volume, Threshold(param: "tension", above: 0.5, fade: 1.0)),

        // Brass only for boss/climax
        ("brass",   Volume, Threshold(param: "tension", above: 0.8, fade: 0.3)),
    ],
)
```

### Horizontal Transition Sequence (RON)

```ron
// assets/audio/music/caribbean/caribbean.sequence.ron
(
    bpm: 120,
    sections: {
        "calm":   (music: "caribbean_calm.music.ron"),
        "battle": (music: "caribbean_battle.music.ron"),
        "boss":   (music: "caribbean_boss.music.ron"),
        "victory": (music: "caribbean_victory.music.ron"),
    },

    transitions: [
        // from -> to: how to transition
        ("calm",   "battle",  CrossfadeOnBar(bars: 2)),
        ("battle", "boss",    StingerThen(stinger: "boss_intro.ogg", then: CrossfadeOnBar(bars: 1))),
        ("battle", "victory", FadeOutThenPlay(fade_bars: 1)),
        ("boss",   "victory", StingerThen(stinger: "boss_defeated.ogg", then: CrossfadeOnBar(bars: 2))),
        ("victory","calm",    CrossfadeOnBar(bars: 4)),
    ],
)
```

### Stinger Definitions (RON)

```ron
// assets/audio/music/caribbean/stingers.ron
(
    stingers: {
        "tower_placed":    (file: "stinger_build.ogg",    quantize: NextBeat),
        "tower_sold":      (file: "stinger_sell.ogg",     quantize: NextBeat),
        "wave_start":      (file: "stinger_wave.ogg",     quantize: NextBar),
        "boss_spawn":      (file: "stinger_boss.ogg",     quantize: NextBar),
        "boss_defeated":   (file: "stinger_victory.ogg",  quantize: NextBar),
        "life_lost":       (file: "stinger_damage.ogg",   quantize: Immediate),
        "game_over":       (file: "stinger_gameover.ogg", quantize: Immediate),
    },
)
```

### SFX Definition (RON)

```ron
// data/sfx.ron
(
    "cannon_fire": (
        files: ["towers/cannon_fire_01.ogg", "towers/cannon_fire_02.ogg", "towers/cannon_fire_03.ogg"],
        volume: 0.8,
        pitch_variance: 0.05,       // +/-5% random pitch shift
        max_concurrent: 3,           // max simultaneous instances
    ),
)
```

### SFX Directory Structure

```
assets/audio/sfx/
+-- towers/
|   +-- cannon_fire_01.ogg       # 3 variants for each
|   +-- cannon_fire_02.ogg
|   +-- cannon_fire_03.ogg
|   +-- arrow_shoot_01.ogg
|   +-- magic_cast_01.ogg
+-- enemies/
|   +-- skeleton_death_01.ogg
|   +-- ghost_moan_01.ogg
|   +-- footsteps_dirt_01.ogg
+-- ui/
|   +-- button_click.ogg
|   +-- menu_open.ogg
|   +-- coin_collect.ogg
+-- environment/
|   +-- ocean_waves_loop.ogg
|   +-- wind_desert_loop.ogg
|   +-- thunder_01.ogg
+-- impacts/
    +-- hit_physical_01.ogg
    +-- hit_magic_01.ogg
    +-- explosion_01.ogg
```

### Stem Output Structure

```
assets/audio/music/caribbean/
+-- caribbean_core_melody.wav          <- reference (dev only, not shipped)
+-- caribbean_calm_drums.ogg
+-- caribbean_calm_melody.ogg
+-- caribbean_calm_strings.ogg
+-- caribbean_calm.music.ron
+-- caribbean_battle_drums.ogg
+-- caribbean_battle_bass.ogg
+-- caribbean_battle_melody.ogg
+-- caribbean_battle_strings.ogg
+-- caribbean_battle_brass.ogg
+-- caribbean_battle.music.ron
+-- ...
+-- caribbean.sequence.ron             <- horizontal transitions
```

## Internes Design

All format definitions use RON (Rusty Object Notation) for game data and TOML for engine configuration. See [config/data-formats](../config/data-formats.md) for the rationale on when to use which format.

## Nicht-Ziele

- Binary format specifications (handled by the packing step in [assets/pipeline](../assets/pipeline.md))
- Runtime format conversion
- Supporting non-RON game data formats

## Offene Fragen

- Whether to add a schema validation step for RON files at build time
- Versioning strategy for format changes across engine updates
