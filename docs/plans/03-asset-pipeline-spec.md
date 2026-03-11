# Amigo Engine -- AI Asset Generation Pipeline Specification

## amigo_artgen + amigo_audiogen v1.0

---

## 1. Overview

The Amigo Engine uses two dedicated MCP servers for AI-powered asset generation. **amigo_artgen** connects to external ComfyUI instances for pixel art sprite, tileset, and animation generation with a full post-processing pipeline enforcing visual consistency. **amigo_audiogen** runs ACE-Step and AudioGen locally on GPU to produce music tracks, adaptive stems, sound effects, and ambient audio -- all royalty-free and commercially usable.

---

# Part I: Art Pipeline (amigo_artgen)

---

## 2. Art Architecture

```
+-------------------------------------------------+
|              Claude Code (MCP)                   |
|                                                  |
|  amigo_artgen_generate_sprite(...)               |
|  amigo_artgen_generate_tileset(...)              |
|  amigo_artgen_inpaint(...)                       |
|  amigo_artgen_palette_swap(...)                  |
|                                                  |
+--------------------------------------------------+
|              amigo_artgen (MCP Server)            |
|                                                  |
|  +------------+  +------------+  +------------+  |
|  | Workflow    |  | ComfyUI    |  | Post-      |  |
|  | Builder    |->| Client     |->| Processing |  |
|  |            |  | (HTTP)     |  | Pipeline   |  |
|  +------------+  +------------+  +------------+  |
|                                        |         |
|                                        v         |
|                                  assets/ folder   |
|                                  (hot reload)     |
+--------------------------------------------------+
|              ComfyUI (external)                   |
|                                                  |
|  - Local (localhost:8188)                        |
|  - Remote (LAN / Cloud)                          |
|  - Pixel Art Checkpoint + LoRA                   |
|  - ControlNet models                             |
|  - Custom nodes as needed                        |
+--------------------------------------------------+
```

amigo_artgen never runs AI models itself. It builds ComfyUI workflow JSONs, sends them to the ComfyUI HTTP API, receives generated images, runs post-processing in Rust, and saves to the assets folder.

---

## 3. Connection

Three modes, like Krita AI Diffusion:

| Mode   | Server URL                         | Use Case                           |
| ------ | ---------------------------------- | ---------------------------------- |
| Local  | `http://localhost:8188`            | ComfyUI on same machine (default)  |
| Remote | `http://192.168.x.x:8188`          | ComfyUI on another PC / GPU server |
| Cloud  | `https://api.rundiffusion.com/...` | Hosted ComfyUI service             |

Configuration in `amigo.toml`:

```toml
[artgen]
server = "http://localhost:8188"
timeout = 120                        # seconds per generation
output_dir = "assets/generated"      # where results land
```

On startup, amigo_artgen queries the ComfyUI server for available models, LoRAs, and custom nodes. Missing requirements are reported as warnings.

---

## 4. Style Definitions

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

---

## 5. Art MCP Tools

### Generation

```
amigo_artgen_generate_sprite(
    prompt: string,          # "pirate archer tower with skull flag"
    style: string,           # "caribbean" -> loads style file
    size: [u32, u32]?,       # default from style (e.g., [32, 32])
    variants: u32?,          # number of variations (default: 3)
    output: string?,         # output filename (auto-generated if omitted)
) -> { paths: [string], preview: string }

amigo_artgen_generate_tileset(
    theme: string,           # "caribbean_ground"
    style: string,           # "caribbean"
    tile_size: u32?,         # default from style
    tiles: [string],         # ["grass", "dirt", "water", "sand", "path"]
    seamless: bool?,         # ensure edges tile correctly (default: true)
) -> { path: string, tiles: [string] }

amigo_artgen_generate_spritesheet(
    base: string,            # path to base sprite
    animation: string,       # "walk", "attack", "death", "idle"
    frames: u32,             # number of animation frames
    directions: u32?,        # 1, 4, or 8 (default: 1)
) -> { path: string, frames: u32 }
```

### Modification

```
amigo_artgen_variation(
    input: string,           # path to existing sprite
    prompt: string,          # "smaller flag, more wood texture"
    strength: f32?,          # 0.0 = identical, 1.0 = completely new (default: 0.4)
    style: string?,          # style for post-processing
) -> { path: string }

amigo_artgen_inpaint(
    input: string,           # path to sprite
    mask: string,            # path to mask (white = replace, black = keep)
    prompt: string,          # what to fill in
    style: string?,
) -> { path: string }
```

### Post-Processing Only (no AI)

```
amigo_artgen_palette_swap(
    input: string,           # path to sprite
    palette: string,         # palette name or style name
) -> { path: string }

amigo_artgen_upscale(
    input: string,           # path to sprite
    factor: u32,             # 2 or 4
) -> { path: string }

amigo_artgen_post_process(
    input: string,           # path to any image
    style: string,           # apply style's post-processing
) -> { path: string }
```

### Utility

```
amigo_artgen_list_styles() -> { styles: [string] }
amigo_artgen_list_checkpoints() -> { checkpoints: [string] }
amigo_artgen_list_loras() -> { loras: [string] }
amigo_artgen_server_status() -> { connected: bool, gpu: string, vram: string }
```

---

## 6. Art Post-Processing Pipeline

Runs in Rust after ComfyUI returns a raw image. No AI involved -- pure image manipulation. Ensures every generated asset matches the pixel art style.

### Pipeline Steps

```
Raw AI Output (may have anti-aliasing, wrong colors, soft edges)
    |
    v
+- 1. Downscale ------------------------------------------+
|  If generated at higher resolution (e.g., 128x128       |
|  for a 32x32 sprite), downscale with nearest-           |
|  neighbor to target size.                                |
+---------------------------------------------------------+
    |
    v
+- 2. Palette Clamping -----------------------------------+
|  For each pixel: find nearest color in the style's      |
|  palette (Euclidean distance in RGB space).              |
|  Result: image uses only defined palette colors.         |
+---------------------------------------------------------+
    |
    v
+- 3. Anti-Aliasing Removal ------------------------------+
|  Detect pixels that are blends between two palette       |
|  colors (intermediate values). Snap to the nearest       |
|  palette color. Eliminates soft edges.                   |
+---------------------------------------------------------+
    |
    v
+- 4. Transparency Cleanup -------------------------------+
|  Pixels with alpha < 128 -> fully transparent (0).       |
|  Pixels with alpha >= 128 -> fully opaque (255).         |
|  No semi-transparent pixels in pixel art.                |
+---------------------------------------------------------+
    |
    v
+- 5. Outline Addition -----------------------------------+
|  For each opaque pixel adjacent to a transparent         |
|  pixel: add 1px outline in outline_color.                |
|  Configurable: inner outline, outer outline, or off.     |
+---------------------------------------------------------+
    |
    v
+- 6. Tile Edge Check (tilesets only) --------------------+
|  Verify that tile edges match for seamless tiling.       |
|  Left edge pixels must match right edge of neighbor.     |
|  If mismatch > threshold: flag for manual review         |
|  or re-run with inpainting on edges.                     |
+---------------------------------------------------------+
    |
    v
  Clean pixel art asset -> saved to assets/ -> engine hot reload
```

### Configuration

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

---

## 7. Workflow Builder

amigo_artgen builds ComfyUI workflow JSONs programmatically. No need for Claude Code or the user to understand ComfyUI node graphs.

### How It Works

1. Tool call comes in (e.g., `generate_sprite`)
2. Workflow Builder selects a template based on the operation
3. Fills in parameters from the tool call + style definition
4. Sends completed workflow JSON to ComfyUI HTTP API (`/prompt`)
5. Polls for completion (`/history`)
6. Downloads result image
7. Runs post-processing pipeline
8. Saves to output directory

### Template Workflows

Stored as JSON templates with placeholder values:

```
tools/amigo_artgen/workflows/
+-- txt2img_sprite.json          # text -> new sprite
+-- img2img_variation.json       # existing sprite -> variation
+-- inpaint.json                 # sprite + mask -> modified sprite
+-- spritesheet.json             # base sprite -> animation frames
+-- tileset.json                 # theme -> tileset grid
+-- upscale.json                 # small -> larger with detail
+-- custom/                      # user-provided workflows
```

### ComfyUI API Integration

```rust
pub struct ComfyUIClient {
    base_url: String,
    client: reqwest::Client,
}

impl ComfyUIClient {
    // Queue a workflow for execution
    pub async fn queue_prompt(&self, workflow: Value) -> Result<String>;

    // Poll for completion
    pub async fn get_history(&self, prompt_id: &str) -> Result<PromptResult>;

    // Download generated image
    pub async fn get_image(&self, filename: &str) -> Result<Vec<u8>>;

    // Query available models
    pub async fn get_checkpoints(&self) -> Result<Vec<String>>;
    pub async fn get_loras(&self) -> Result<Vec<String>>;

    // Server health
    pub async fn system_stats(&self) -> Result<SystemStats>;
}
```

---

# Part II: Audio Pipeline (amigo_audiogen)

---

## 8. Audio Architecture

```
+----------------------------------------------------+
|                  Claude Code (MCP)                  |
|                                                      |
|  amigo_audiogen_generate_track(...)                  |
|  amigo_audiogen_generate_sfx(...)                    |
|  amigo_audiogen_generate_ambient(...)                |
|  amigo_audiogen_stem_split(...)                      |
|  amigo_audiogen_loop_trim(...)                       |
|                                                      |
+------------------------------------------------------+
|             amigo_audiogen (MCP Server)               |
|                                                      |
|  +--------------+  +--------------+  +-----------+   |
|  | ACE-Step 1.5 |  |  AudioGen    |  |   Post-   |   |
|  | (Music)      |  |  (SFX)      |  | Processing|   |
|  | Gradio API   |  |  Python API  |  | (Rust)    |   |
|  +--------------+  +--------------+  +-----------+   |
|                                                      |
|  Runs on local GPU (RTX 3060/3080)                   |
|  or remote GPU server                                |
+------------------------------------------------------+
```

---

## 9. AI Models

### Music Generation: ACE-Step 1.5

Open-source (Apache 2.0), runs locally, commercial use permitted.

- Generates full tracks up to 5 minutes from text prompts
- Supports style tags, genre, mood, tempo, key signature
- LoRA fine-tuning with ~8 songs for custom style
- Stem separation built-in (vocals, drums, bass, melody, accompaniment)
- Remix, repaint, extend, and variation modes
- Under 4GB VRAM for generation, ~10s per full song on RTX 3090

ACE-Step runs as a Gradio server with API endpoints. amigo_audiogen calls these endpoints over HTTP, same pattern as ComfyUI for artgen.

```toml
# amigo.toml
[audiogen]
acestep_server = "http://localhost:7860"    # ACE-Step Gradio API
audiogen_server = "http://localhost:7861"    # AudioGen API (if separate)
output_dir = "assets/audio/generated"
```

### Sound Effects: Meta AudioGen (AudioCraft)

Open-source (MIT license for code), trained on public sound effects.

- Text-to-sound: "explosion with debris", "arrow whoosh", "coin pickup chime"
- Environmental audio: "tropical rain on wooden deck", "desert wind with sand"
- 285M and 1B parameter models, runs on same GPU as ACE-Step
- Short clips (1-10 seconds) ideal for game SFX

### Stem Separation

Both ACE-Step and third-party tools (Demucs) can split a mixed track into individual stems: vocals, drums, bass, other instruments. This is critical for the adaptive music system -- each stem becomes a layer that can be independently controlled at runtime.

---

## 10. Stem Strategy

Two workflows for different phases of development:

### Quick Mode: Generate & Split (Iteration)

Fast, good enough for prototyping. Generate a full mixed track, split into stems. Has stem bleed, but gets you from zero to playable adaptive music in minutes.

```
Full Track (ACE-Step) -> Stem Split (Demucs) -> Post-Processing -> Playable (with bleed)
```

Use for: early prototyping, testing adaptive system, exploring moods.

### Clean Mode: Per-Stem Generation (Release Quality)

Each stem generated individually, conditioned on a shared core melody. Zero bleed, professional quality. More generation time, but the result is how shipped games do adaptive music.

```
Step 1: Core Melody
    |
    |  Generate a simple melody line that defines the
    |  harmonic identity of this world. All moods share it.
    |  ACE-Step: "simple pirate melody, fiddle, 120 BPM, C minor, solo instrument"
    |  -> caribbean_core_melody.wav
    |
    v
Step 2: Per-Stem Generation (melody-conditioned)
    |
    |  Each stem generated separately, core melody as reference:
    |
    |  reference=caribbean_core_melody.wav
    |    + "war drums only, percussion, 120 BPM"
    |    -> caribbean_battle_drums.wav
    |
    |  reference=caribbean_core_melody.wav
    |    + "bass line only, deep cello, 120 BPM"
    |    -> caribbean_battle_bass.wav
    |
    |  reference=caribbean_core_melody.wav
    |    + "lead fiddle melody, adventurous, 120 BPM"
    |    -> caribbean_battle_melody.wav
    |
    |  (... strings, brass, etc.)
    |
    v
Step 3: Mix Validation
    |
    |  Play all stems simultaneously -> coherent?
    |  Adjust volumes, re-generate if needed.
    |
    v
Step 4: Mood Variations (same core melody)
    |
    |  Repeat Step 2 for each mood, conditioning on SAME core melody:
    |    caribbean_calm_*   -> gentle, sparse
    |    caribbean_tense_*  -> darker, building
    |    caribbean_battle_* -> full, intense
    |    caribbean_boss_*   -> maximum, menacing
    |    caribbean_victory_*-> triumphant, resolved
    |
    |  All moods share harmonic identity -> horizontal transitions
    |  sound like the SAME piece evolving, not different songs.
    |
    v
Step 5: Post-Processing -> Export
    |
    v
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

### Loop Safety

AI-generated audio rarely has perfect loop points. Post-processing handles this:

1. **BPM Detection** via onset analysis (in Rust)
2. **Bar Boundary Snap**: Trim to exact N bars using detected BPM
3. **Crossfade Window**: 1-bar cosine crossfade at loop boundary (tail fades out, head fades in, overlapped). Eliminates clicks.
4. **Validation**: Spectral similarity between first and last bar. Below threshold -> flag for re-generation.
5. **Fallback**: If loop detection fails, trim to closest power-of-2 seconds with 100ms fade.

---

## 11. Audio MCP Tools

### Music Generation

```
amigo_audiogen_generate_core_melody(
    style: string,            # world style preset
    bpm: u32?,                # tempo
    key: string?,             # musical key
    duration: f32?,           # seconds (default: 16, one phrase)
    instrument: string?,      # "fiddle", "piano", "synth lead", etc.
) -> { path: string, bpm: u32, key: string }

amigo_audiogen_generate_stem(
    reference: string,        # path to core melody (melody conditioning)
    stem_type: string,        # "drums", "bass", "melody", "strings", "brass", "synth", "choir"
    prompt: string,           # additional style description
    style: string?,           # world style for prompt prefix
    bpm: u32?,                # must match reference
    duration: f32?,           # seconds
) -> { path: string }

amigo_audiogen_generate_track(
    prompt: string,           # style tags + description
    duration: f32?,           # seconds (default: 120)
    bpm: u32?,                # tempo (default: auto)
    key: string?,             # musical key (default: auto)
    style: string?,           # world style preset name
    split_stems: bool?,       # auto-split via Demucs (default: false, Quick Mode)
) -> { path: string, stems: [string]?, duration: f32 }

amigo_audiogen_generate_variation(
    input: string,            # path to existing track
    prompt: string?,          # style modification
    strength: f32?,           # 0.0 = same, 1.0 = completely different
) -> { path: string }

amigo_audiogen_extend_track(
    input: string,            # path to existing track
    duration: f32,            # additional seconds
) -> { path: string }

amigo_audiogen_remix(
    input: string,            # path to existing track
    prompt: string,           # new style/mood
) -> { path: string }
```

### Sound Effects

```
amigo_audiogen_generate_sfx(
    prompt: string,           # "cannon fire with echo"
    duration: f32?,           # seconds (default: 2)
    variants: u32?,           # number of variations (default: 3)
) -> { paths: [string] }

amigo_audiogen_generate_ambient(
    prompt: string,           # "tropical ocean waves with seagulls"
    duration: f32?,           # seconds, longer for ambience (default: 30)
    loopable: bool?,          # ensure seamless loop (default: true)
) -> { path: string }
```

### Processing (no AI)

```
amigo_audiogen_stem_split(
    input: string,            # path to mixed track
    stems: [string]?,         # ["drums", "bass", "melody", "other"]
) -> { stems: { [string]: string } }

amigo_audiogen_loop_trim(
    input: string,            # path to audio
    target_bars: u32?,        # trim to N bars (auto-detect tempo)
) -> { path: string, bpm: f32, bars: u32 }

amigo_audiogen_normalize(
    input: string,
    target_lufs: f32?,        # loudness target (default: -14)
) -> { path: string }

amigo_audiogen_convert(
    input: string,
    format: string,           # "ogg", "wav", "mp3"
    sample_rate: u32?,        # default: 44100
) -> { path: string }
```

### Utility

```
amigo_audiogen_preview(path: string)           # play audio in terminal
amigo_audiogen_server_status()                  # GPU, models loaded
amigo_audiogen_list_styles()                    # available world presets
```

---

## 12. Adaptive Music System (Engine-Side)

This runs inside the Amigo Engine at runtime, powered by kira. No AI at runtime -- everything is pre-generated stems controlled by game parameters.

### Three Techniques Combined

**Vertical Layering** -- Multiple stems (drums, bass, melody, brass, strings) play simultaneously. Each layer has a volume that fades in/out based on game state. Calm moments: just strings and ambient. Battle: drums and bass kick in. Boss: brass stingers on top.

**Horizontal Re-Sequencing** -- Multiple musical sections (intro, loop_calm, loop_tense, loop_battle, outro) transition at musically meaningful points (bar boundaries). The engine doesn't jump mid-bar -- it waits for the next bar boundary to switch, keeping the music coherent.

**Dynamic Mixing** -- Real-time parameter adjustments: volume per layer, low-pass filter (muffle when in menus or cutscenes), reverb (more in large spaces), tempo shift (subtle, for urgency).

### Music Definition (RON)

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

### Engine Parameters (set by game logic)

```rust
pub struct MusicParameters {
    pub tension: f32,       // 0.0 = calm, 1.0 = max intensity
    pub danger: f32,        // 0.0 = safe, 1.0 = about to lose
    pub victory: f32,       // 0.0 = ongoing, 1.0 = wave cleared
    pub boss: bool,         // boss alive on field
    pub menu_open: bool,    // UI overlay active (muffle music)
}
```

The TD game sets these parameters every frame:

```rust
fn update_music_params(world: &World, res: &mut Resources) {
    let wave = res.wave_manager.current();
    let wave_progress = res.wave_manager.progress();   // 0.0..1.0
    let lives = world.get::<PlayerState>(player).lives;
    let max_lives = 20;
    let enemies_alive = world.query::<With<Enemy>>().count();
    let boss_alive = world.query::<With<BossMarker>>().count() > 0;

    let params = &mut res.music.params;

    // Tension rises with wave number and active enemies
    params.tension = (wave as f32 / 10.0)
        .max(enemies_alive as f32 / 30.0)
        .clamp(0.0, 1.0);

    // Danger based on remaining lives
    params.danger = 1.0 - (lives as f32 / max_lives as f32);

    // Boss override
    params.boss = boss_alive;
    if boss_alive {
        params.tension = params.tension.max(0.85);
    }

    // Low lives = max tension
    if lives <= 3 {
        params.tension = params.tension.max(0.9);
    }
}
```

### Horizontal Transitions

For switching between different musical pieces (not just layers), the engine uses bar-synced transitions:

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

### Transition Types

```rust
pub enum MusicTransition {
    // Crossfade old -> new at the next bar boundary
    CrossfadeOnBar { bars: u32 },

    // Play a one-shot stinger, then transition
    StingerThen { stinger: String, then: Box<MusicTransition> },

    // Fade out current, silence gap, then start new
    FadeOutThenPlay { fade_bars: u32 },

    // Hard cut at bar boundary (for dramatic moments)
    CutOnBar,

    // Gradual: swap one layer at a time over N bars
    LayerSwap { bars_per_layer: u32 },
}
```

### Stingers (One-Shot Musical Cues)

Short musical phrases triggered by game events, mixed on top of the current music. Quantized to the next beat or bar for musical coherence:

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

---

## 13. Sound Effects Pipeline

### Categories

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

### Variation System

Multiple variants per sound effect. Engine picks randomly at playback to avoid repetition:

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

---

## 14. World Audio Styles

Each world has a distinct sonic identity: different musical genre, different instruments, different SFX feel. The hybrid approach means no two worlds sound alike.

### Per-World Audio Profiles

| World | Music Genre | Key Instruments | SFX Style |
|-------|-------------|-----------------|-----------|
| Caribbean | Orchestral sea shanty | Fiddle, accordion, war drums, brass, harpsichord | Wooden, explosive, wet (splashes, creaking) |
| Lord of the Rings | Epic orchestral / Howard Shore | French horn, cello, choir, harp, bodhran | Metallic, reverberant, stone (clang, echo) |
| Dune | Ambient electronic / Hans Zimmer | Duduk, throat singing, deep synth pads, tabla | Sandy, dry, resonant (wind, rumble, vibration) |
| Matrix | Dark synthwave / industrial | Analog synth, drum machine, distorted bass, glitch | Digital, crisp, processed (beeps, whooshes, electric) |
| Game of Thrones | Dark medieval orchestral | Cello, war drums, raven calls, low brass | Cold, metallic, heavy (ice crack, fire roar, steel) |
| Stranger Things | 80s retro synth / John Carpenter | Moog synth, Juno pads, gated reverb drums, arpeggios | Eerie, analog, distorted (static, warble, flicker) |

### Style Definition Files

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

---

## 15. Audio Post-Processing Pipeline

Runs in Rust after AI generation, before assets are saved:

```
Raw AI Audio
    |
    v
+- 1. Loop Detection & Trim ---------------------------+
|  Analyze BPM, find bar boundaries.                    |
|  Trim to exact bar count for seamless looping.        |
|  Zero-crossing detection at loop points.              |
+------------------------------------------------------+
    |
    v
+- 2. Loudness Normalization ---------------------------+
|  Normalize to target LUFS:                            |
|    Music: -14 LUFS                                    |
|    SFX: -12 LUFS                                      |
|    Ambient: -18 LUFS                                  |
+------------------------------------------------------+
    |
    v
+- 3. Format Conversion --------------------------------+
|  Convert to OGG Vorbis (quality 6) for release.      |
|  Keep WAV for dev (hot reload is faster).             |
|  Resample to 44.1kHz if needed.                      |
+------------------------------------------------------+
    |
    v
+- 4. Loop Validation ----------------------------------+
|  Play loop boundary: does it click/pop?               |
|  Spectral analysis at loop point: smooth?             |
|  Flag for manual review if artifacts detected.        |
+------------------------------------------------------+
    |
    v
  Clean, normalized, loop-ready audio asset
```

---

# Part III: Shared Infrastructure

---

## 16. GPU Scheduling

ACE-Step and ComfyUI (artgen) both need the GPU. On a single RTX 3060/3080, they cannot run simultaneously.

### Solution: Sequential Scheduling

amigo_artgen and amigo_audiogen are separate MCP servers, but only one runs inference at a time. Claude Code naturally serializes this -- it calls one tool, waits for the result, then calls the next. No explicit scheduler needed.

For batch generation (e.g., "create all Caribbean audio"), the audiogen MCP server queues jobs internally and processes them sequentially. ComfyUI is not running during audio generation and vice versa.

```toml
# amigo.toml
[gpu]
# Explicit mode: only one GPU consumer at a time
# artgen and audiogen check this lock before starting inference
lock_file = "/tmp/amigo_gpu.lock"
timeout = 300                          # seconds before lock is considered stale
```

If running on a multi-GPU setup (e.g., RTX 3060 for audio, RTX 3080 for art), each server can be pinned to a specific GPU via CUDA_VISIBLE_DEVICES in its config.

---

## 17. Workspace Structure

```
amigo-engine/
+-- tools/
|   +-- amigo_artgen/
|   |   +-- Cargo.toml
|   |   +-- src/
|   |   |   +-- main.rs               # MCP server entry point
|   |   |   +-- comfyui_client.rs      # HTTP client for ComfyUI API
|   |   |   +-- workflow_builder.rs    # Builds workflow JSONs from templates
|   |   |   +-- post_processing.rs     # Palette clamp, outline, AA removal
|   |   |   +-- style.rs              # Style definition loader
|   |   |   +-- tools.rs              # MCP tool definitions
|   |   +-- workflows/
|   |       +-- txt2img_sprite.json
|   |       +-- img2img_variation.json
|   |       +-- inpaint.json
|   |       +-- spritesheet.json
|   |       +-- tileset.json
|   |       +-- upscale.json
|   +-- amigo_audiogen/
|       +-- Cargo.toml
|       +-- src/
|       |   +-- main.rs               # MCP server entry point
|       |   +-- acestep_client.rs      # HTTP client for ACE-Step Gradio API
|       |   +-- audiogen_client.rs     # Python bridge for AudioCraft/AudioGen
|       |   +-- stem_splitter.rs       # Stem separation orchestration
|       |   +-- post_processing.rs     # Loop trim, normalize, convert
|       |   +-- style.rs              # Audio style definition loader
|       |   +-- tools.rs              # MCP tool definitions
|       +-- scripts/
|           +-- audiogen_server.py     # AudioGen FastAPI wrapper
+-- styles/
|   +-- caribbean.style.ron            # Art style (visual)
|   +-- lotr.style.ron
|   +-- dune.style.ron
|   +-- matrix.style.ron
|   +-- got.style.ron
|   +-- stranger_things.style.ron
|   +-- audio/
|       +-- caribbean.audio_style.ron  # Audio style (sonic)
|       +-- lotr.audio_style.ron
|       +-- dune.audio_style.ron
|       +-- matrix.audio_style.ron
|       +-- got.audio_style.ron
|       +-- stranger_things.audio_style.ron
+-- assets/
    +-- generated/                     # artgen output lands here
    |   +-- sprites/
    |   +-- tilesets/
    |   +-- spritesheets/
    +-- audio/
        +-- music/                     # adaptive tracks + stems
        |   +-- caribbean/
        |   +-- lotr/
        |   +-- ...
        +-- sfx/                       # sound effects
        +-- ambient/                   # environmental loops
```

---

## 18. MCP Configuration

```json
// ~/.claude/claude_code_config.json
{
  "mcpServers": {
    "amigo": {
      "command": "amigo",
      "args": ["mcp-server", "--port", "9999"]
    },
    "amigo-artgen": {
      "command": "amigo-artgen",
      "args": ["--server", "http://localhost:8188"]
    },
    "amigo-audiogen": {
      "command": "amigo-audiogen",
      "args": ["--acestep", "http://localhost:7860"]
    }
  }
}
```

Three MCP servers side by side: `amigo` for engine control, `amigo-artgen` for pixel art asset generation via ComfyUI, `amigo-audiogen` for music and sound effect generation via ACE-Step/AudioGen. Claude Code sees all three tool sets simultaneously.

---

## 19. Licensing

All generated audio is royalty-free and commercially usable:

| Model | License | Training Data | Commercial Use |
|-------|---------|--------------|----------------|
| ACE-Step 1.5 | Apache 2.0 | Original training data | Yes |
| AudioGen (AudioCraft) | MIT (code) | Public sound effects | Yes (verify per-model) |
| Demucs (stem split) | MIT | N/A (inference only) | Yes |

Generated output is original -- not copies of training data. Standard disclaimer: verify uniqueness of generated tracks before commercial release.

---

## 20. Example Workflows

### Art Pipeline: Creating a Tower Sprite

```
Claude Code: "Create a cannon tower for the Caribbean world"

1. amigo_artgen_generate_sprite(
     prompt="pirate cannon tower, wooden platform, black cannon, skull decoration",
     style="caribbean",
     size=[32, 32],
     variants=3
   )
   -> Generates 3 variants, post-processed with Caribbean palette

2. Claude sees the 3 PNGs, picks the best one

3. amigo_artgen_variation(
     input="assets/generated/sprites/cannon_tower_v2.png",
     prompt="add small pirate flag on top",
     strength=0.3
   )
   -> Refined version with flag

4. amigo_artgen_generate_spritesheet(
     base="assets/generated/sprites/cannon_tower_v2_refined.png",
     animation="idle",
     frames=4
   )
   -> 4-frame idle animation (flag waving, cannon rotating slightly)

5. // Asset is now in assets/generated/sprites/
   // Engine hot-reloads it automatically
   // Claude can immediately test it in the running game:
   amigo_place_tower(x=5, y=3, tower_type="cannon")
   amigo_screenshot(path="/tmp/tower_placed.png")
   -> Claude sees the tower in-game, evaluates the look
```

### Audio Pipeline: Creating a Complete World Soundtrack

#### Quick Mode (Prototyping)

```
Claude Code: "Quick prototype Caribbean audio"

1. amigo_audiogen_generate_track(
     prompt="calm pirate harbor", style="caribbean",
     duration=60, split_stems=true
   ) -> caribbean_calm_full.wav + 4 stems (with bleed, good enough)

2. amigo_audiogen_generate_track(
     prompt="epic pirate battle", style="caribbean",
     duration=60, split_stems=true
   ) -> caribbean_battle_full.wav + 4 stems

3. Claude writes .music.ron configs, tests in-game immediately.
   Total time: ~5 minutes. Quality: prototype-grade.
```

#### Clean Mode (Release Quality)

```
Claude Code: "Create Caribbean world's release soundtrack"

== Step 1: Core Melody ==

1. amigo_audiogen_generate_core_melody(
     style="caribbean", bpm=120, key="C minor",
     instrument="solo fiddle", duration=16
   ) -> caribbean_core_melody.wav
   Claude listens: "Good melodic identity, memorable hook."

== Step 2: Battle Stems (conditioned on core melody) ==

2. amigo_audiogen_generate_stem(
     reference="caribbean_core_melody.wav",
     stem_type="drums",
     prompt="war drums, intense battle percussion, snare rolls",
     style="caribbean", bpm=120, duration=120
   ) -> caribbean_battle_drums.wav

3. amigo_audiogen_generate_stem(
     reference="caribbean_core_melody.wav",
     stem_type="bass",
     prompt="deep double bass, driving rhythm, pizzicato",
     style="caribbean", bpm=120, duration=120
   ) -> caribbean_battle_bass.wav

4. amigo_audiogen_generate_stem(
     reference="caribbean_core_melody.wav",
     stem_type="melody",
     prompt="fiddle melody, heroic and fast, adventurous",
     style="caribbean", bpm=120, duration=120
   ) -> caribbean_battle_melody.wav

5-6. (... strings, brass stems ...)

== Step 3: Calm Stems (same core melody, sparser) ==

7. amigo_audiogen_generate_stem(
     reference="caribbean_core_melody.wav",
     stem_type="strings",
     prompt="gentle string pad, peaceful, sustained",
     style="caribbean", bpm=120, duration=120
   ) -> caribbean_calm_strings.wav

8-9. (... calm melody, ambient percussion ...)

== Step 4: Stingers ==

10. amigo_audiogen_generate_sfx(
      prompt="short triumphant brass fanfare", variants=1, duration=2
    ) -> stinger_wave.ogg

== Step 5: SFX (world-specific) ==

11. amigo_audiogen_generate_sfx(
      prompt="cannon blast with wooden creak", style="caribbean", variants=3
    ) -> cannon_fire_01/02/03.ogg

12. amigo_audiogen_generate_sfx(
      prompt="skeleton bones scattering on wood deck", style="caribbean", variants=3
    ) -> skeleton_death_01/02/03.ogg

== Step 6: Ambient ==

13. amigo_audiogen_generate_ambient(
      prompt="tropical ocean, seagulls, gentle waves, harbor bells",
      duration=60, loopable=true
    ) -> caribbean_ambient.ogg

== Step 7: Configure & Test ==

14. Claude writes adaptive configs (.music.ron, .sequence.ron, stingers.ron)
15. amigo_start_wave() -> test in-game
    "tension=0.4, drums fading in with bass. Melody still muted.
     At 0.6 melody arrives. Sounds coherent -- same harmonic identity."

Total time: ~30 minutes. Quality: release-grade, zero stem bleed.
```

---

*For the engine specification, see 01-engine-spec.md. For game design and UI/UX, see 02-td-spec.md.*
