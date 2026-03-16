# Audio System

> Status: draft
> Crate: amigo_audio
> Depends on: [engine/core](../engine/core.md)
> Last updated: 2026-03-16

## Zweck

Wrapper around `kira`. Three subsystems: SFX playback, Adaptive Music Engine, and Ambient layers.

For audio generation pipeline, see [ai-pipelines/audiogen](../ai-pipelines/audiogen.md).

## Public API

### 15.1 SFX Playback

```rust
pub struct SfxManager {
    definitions: HashMap<String, SfxDefinition>,    // loaded from data/sfx.ron
    active: Vec<SfxInstance>,
}

pub struct SfxDefinition {
    pub files: Vec<PathBuf>,           // multiple variants
    pub volume: f32,
    pub pitch_variance: f32,           // +/-random pitch shift
    pub max_concurrent: u32,           // max simultaneous instances
    pub cooldown: Option<f32>,         // min time between plays
}
```

API: `res.audio.play_sfx("cannon_fire")` -- picks random variant, applies pitch variance, respects max concurrent limit.

Spatial SFX: optional position parameter for distance-based volume falloff relative to camera.

### 15.2 Adaptive Music Engine

The core runtime system for dynamic soundtracks. Plays pre-generated stems and crossfades between musical sections based on game parameters. No AI at runtime -- everything is pre-authored assets controlled by data.

**Vertical Layering**: Multiple stems (drums, bass, melody, strings, brass) play simultaneously, synchronized. Each layer's volume is driven by game parameters (tension, danger, boss, etc.) via rules defined in RON.

**Horizontal Re-Sequencing**: Multiple musical sections (calm, tense, battle, boss, victory) with bar-synced transitions. The engine tracks bar position and only switches at bar boundaries.

**Stingers**: One-shot musical cues (wave start, boss spawn, tower placed) quantized to the next beat or bar.

```rust
pub struct AdaptiveMusicEngine {
    // Current playing section
    active_section: Option<MusicSection>,
    // Pending transition
    pending_transition: Option<(String, MusicTransition)>,
    // Bar tracking
    bar_clock: BarClock,
    // Game-driven parameters
    params: MusicParameters,
    // Stinger playback
    stinger_queue: Vec<StingerRequest>,
}

pub struct BarClock {
    pub bpm: f32,
    pub beats_per_bar: u32,
    pub current_beat: f32,          // fractional beat position
    pub current_bar: u32,
}

pub struct MusicSection {
    pub name: String,
    pub layers: Vec<MusicLayer>,
    pub rules: Vec<LayerRule>,
}

pub struct MusicLayer {
    pub name: String,               // "drums", "bass", etc.
    pub handle: SoundHandle,        // kira sound handle
    pub base_volume: f32,
    pub current_volume: f32,
    pub target_volume: f32,
    pub fade_speed: f32,
}

pub struct MusicParameters {
    pub tension: f32,               // 0.0..1.0
    pub danger: f32,                // 0.0..1.0
    pub victory: f32,               // 0.0..1.0
    pub boss: bool,
    pub menu_open: bool,
}
```

**Layer Rules** (evaluated per frame, drive layer volumes):

```rust
pub enum LayerRule {
    // Linear interpolation: param value maps to volume range
    Lerp { param: String, from: f32, to: f32 },
    // Step function: layer on above threshold, fades in/out
    Threshold { param: String, above: f32, fade_seconds: f32 },
    // Boolean: on/off based on bool param
    Toggle { param: String, fade_seconds: f32 },
}
```

**Horizontal Transitions** (bar-synced):

```rust
pub enum MusicTransition {
    CrossfadeOnBar { bars: u32 },
    StingerThen { stinger: String, then: Box<MusicTransition> },
    FadeOutThenPlay { fade_bars: u32 },
    CutOnBar,
    LayerSwap { bars_per_layer: u32 },
}
```

Engine update loop:

```rust
fn update_adaptive_music(res: &mut Resources, dt: f32) {
    let music = &mut res.adaptive_music;

    // 1. Advance bar clock
    music.bar_clock.advance(dt);

    // 2. Apply layer rules (smooth volume changes)
    if let Some(section) = &mut music.active_section {
        for (layer, rule) in section.layers.iter_mut().zip(&section.rules) {
            layer.target_volume = rule.evaluate(&music.params);
            layer.current_volume = lerp(
                layer.current_volume,
                layer.target_volume,
                layer.fade_speed * dt,
            );
            layer.handle.set_volume(layer.current_volume);
        }
    }

    // 3. Check pending horizontal transitions (bar-synced)
    if let Some((target, transition)) = &music.pending_transition {
        if music.bar_clock.is_on_bar_boundary() {
            execute_transition(music, target, transition);
        }
    }

    // 4. Play queued stingers (beat/bar quantized)
    for stinger in music.stinger_queue.drain(..) {
        match stinger.quantize {
            Immediate => play_stinger_now(stinger),
            NextBeat => schedule_stinger_at_next_beat(stinger, &music.bar_clock),
            NextBar => schedule_stinger_at_next_bar(stinger, &music.bar_clock),
        }
    }

    // 5. Menu muffle (low-pass filter when menu is open)
    if music.params.menu_open {
        music.master_filter.lerp_to(lowpass_800hz, dt);
    } else {
        music.master_filter.lerp_to(bypass, dt);
    }
}
```

### 15.3 Ambient Layer

Separate from music. Looping environmental audio per world (ocean waves, desert wind, rain, etc.). Crossfades when atmosphere changes. Always plays under music and SFX.

### 15.4 Volume Channels

Three independent channels with master:

```
Master Volume
+-- Music Volume (adaptive music engine)
+-- SFX Volume (gameplay sounds)
+-- Ambient Volume (environmental loops)
```

All configurable in settings. Saved to user preferences.

> For complete adaptive music configuration (RON definitions, stingers, world audio profiles, stem strategy), see [ai-pipelines/audiogen](../ai-pipelines/audiogen.md).
