---
status: spec
crate: amigo_core
depends_on: ["engine/tween", "engine/camera"]
last_updated: 2026-03-18
---

# Timeline / Cutscene System

## Purpose

Keyframe-based timeline system for cutscenes and scripted sequences. A timeline
is a collection of typed tracks (camera paths, dialogue triggers, sound events,
entity spawns, tween animations) synchronized to a shared clock. Designed to be
authored in RON files and played back by a `TimelinePlayer` that supports
play, pause, seek, speed control, and skip. Builds on the tween system for
value interpolation and the spline system for camera paths.

## Public API

### Keyframe

```rust
use amigo_core::math::{Fix, SimVec2, RenderVec2};

/// A single keyframe: a value at a specific time with an easing curve to
/// the next keyframe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyframe<T> {
    /// Time in seconds from timeline start.
    pub time: f32,
    /// Value at this keyframe.
    pub value: T,
    /// Easing function used to interpolate from this keyframe to the next.
    /// Defaults to Linear.
    pub easing: EasingFn,
}
```

### Track Types

```rust
/// A named track within a timeline. Each track type holds its own keyframes
/// and knows how to apply its effect at a given time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Track {
    /// Camera follows a spline path. Keyframes define control points;
    /// the camera position is interpolated along a CatmullRomSpline.
    CameraPath {
        name: String,
        points: Vec<Keyframe<RenderVec2>>,
        /// Optional zoom track synchronized with the path.
        zoom: Option<Vec<Keyframe<f32>>>,
    },

    /// Triggers a dialogue tree node at a specific time.
    Dialogue {
        name: String,
        triggers: Vec<DialogueTrigger>,
    },

    /// Overrides an entity's animation at keyframed times.
    AnimOverride {
        name: String,
        entity_tag: String,
        keyframes: Vec<Keyframe<AnimOverrideValue>>,
    },

    /// Plays sound effects or music transitions at specific times.
    Sound {
        name: String,
        cues: Vec<SoundCue>,
    },

    /// Spawns or despawns entities at keyframed times.
    EntitySpawn {
        name: String,
        events: Vec<SpawnEvent>,
    },

    /// Drives an arbitrary tweenable value over the timeline duration.
    /// The target is identified by a string tag that game code maps to
    /// a concrete field (e.g., "screen_fade_alpha", "vignette_intensity").
    TweenTrack {
        name: String,
        target_tag: String,
        keyframes: Vec<Keyframe<f32>>,
    },
}

/// A dialogue trigger at a specific timestamp.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogueTrigger {
    pub time: f32,
    /// Key into the dialogue tree system.
    pub dialogue_key: String,
    /// If true, the timeline pauses until the dialogue completes.
    pub blocking: bool,
}

/// An animation override value at a keyframe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimOverrideValue {
    /// Animation clip name to switch to.
    pub clip: String,
    /// Playback speed multiplier.
    pub speed: f32,
}

/// A sound cue at a specific timestamp.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundCue {
    pub time: f32,
    /// SFX name as registered in SfxManager, or music section name.
    pub sound_name: String,
    pub kind: SoundCueKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SoundCueKind {
    /// Play a one-shot SFX via SfxManager.
    Sfx,
    /// Transition adaptive music to a new section.
    MusicTransition,
    /// Stop a currently playing sound.
    Stop { fade_seconds: f32 },
}

/// An entity spawn or despawn event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnEvent {
    pub time: f32,
    pub kind: SpawnEventKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SpawnEventKind {
    /// Spawn an entity from a prefab/template name at a world position.
    Spawn {
        template: String,
        position: SimVec2,
        tag: String,
    },
    /// Despawn an entity identified by its tag.
    Despawn { tag: String },
}
```

### Timeline

```rust
/// A complete timeline definition, typically loaded from a RON file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timeline {
    /// Unique name for this timeline (used for lookup).
    pub name: String,
    /// Total duration in seconds. Playback stops at this time.
    pub duration: f32,
    /// Whether the player can skip this timeline (e.g., cutscene skip).
    pub skippable: bool,
    /// All tracks in this timeline, evaluated in parallel.
    pub tracks: Vec<Track>,
}
```

### TimelinePlayer

```rust
/// Runtime player that advances a timeline and applies track effects.
pub struct TimelinePlayer {
    /// The timeline being played (None if idle).
    timeline: Option<Timeline>,
    /// Current playback time in seconds.
    current_time: f32,
    /// Playback speed multiplier (1.0 = normal, 0.0 = paused via speed).
    speed: f32,
    /// Current playback state.
    state: PlaybackState,
    /// Tracks which one-shot events (dialogue, sound, spawn) have already
    /// fired so they are not re-triggered on seek.
    fired_events: FxHashSet<(usize, usize)>,
    /// Tags -> EntityId mapping for spawned entities within the timeline.
    entity_tags: FxHashMap<String, EntityId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    /// No timeline loaded.
    Idle,
    /// Playing forward.
    Playing,
    /// Paused at current time.
    Paused,
    /// Waiting for a blocking dialogue to complete.
    WaitingForDialogue,
    /// Playback finished (reached duration).
    Finished,
}

impl TimelinePlayer {
    pub fn new() -> Self;

    /// Load and start playing a timeline from the beginning.
    pub fn play(&mut self, timeline: Timeline);

    /// Pause playback at the current time.
    pub fn pause(&mut self);

    /// Resume playback from the current time.
    pub fn resume(&mut self);

    /// Stop playback and unload the timeline.
    pub fn stop(&mut self);

    /// Seek to a specific time. Fires any missed one-shot events between
    /// current_time and the target if seeking forward.
    pub fn seek(&mut self, time: f32);

    /// Set playback speed. 1.0 = normal, 2.0 = double speed.
    pub fn set_speed(&mut self, speed: f32);

    /// Skip the current timeline: jump to the end, apply all remaining
    /// effects (spawn entities, trigger final states), and set Finished.
    /// Only works if `timeline.skippable` is true.
    pub fn skip(&mut self) -> bool;

    /// Advance playback by dt seconds (scaled by speed).
    /// Evaluates all tracks at the new time, fires events, applies
    /// interpolated values. Returns the current PlaybackState.
    pub fn update(&mut self, dt: f32, ctx: &mut TimelineContext) -> PlaybackState;

    /// Current playback state.
    pub fn state(&self) -> PlaybackState;

    /// Current playback time in seconds.
    pub fn current_time(&self) -> f32;

    /// Fraction of timeline completed (0.0 to 1.0).
    pub fn progress(&self) -> f32;

    /// Notify that a blocking dialogue has completed, resuming playback.
    pub fn on_dialogue_complete(&mut self);
}

/// Context passed to update() providing mutable access to engine systems
/// that tracks need to apply their effects.
pub struct TimelineContext<'a> {
    pub camera: &'a mut Camera,
    pub sfx: &'a mut SfxManager,
    pub music: &'a mut AdaptiveMusicEngine,
    pub world: &'a mut World,
    /// Callback for tween track values: (target_tag, interpolated_value).
    pub tween_apply: &'a mut dyn FnMut(&str, f32),
    /// Callback to start a dialogue sequence.
    pub dialogue_start: &'a mut dyn FnMut(&str),
}
```

## Behavior

- **Parallel tracks**: All tracks in a timeline are evaluated simultaneously at
  the current playback time. There is no ordering dependency between tracks.

- **Interpolated tracks** (CameraPath, AnimOverride, TweenTrack): For each
  frame, find the two keyframes surrounding the current time and interpolate
  using the leading keyframe's easing function. CameraPath uses
  `CatmullRomSpline` for smooth curves through the control points.

- **Event tracks** (Dialogue, Sound, EntitySpawn): One-shot events fire when
  playback time crosses their timestamp. The `fired_events` set prevents
  double-firing on seek or replay. Seeking backward clears events after the
  new time from the fired set.

- **Blocking dialogue**: When a `DialogueTrigger` with `blocking: true` fires,
  the player enters `WaitingForDialogue` state. Playback resumes when
  `on_dialogue_complete()` is called.

- **Skip**: `skip()` jumps to `duration`, applies all un-fired spawn events
  (so the final world state is correct), sets camera to the last CameraPath
  keyframe, fires all remaining sound cues, and enters `Finished`. Dialogue
  triggers during skip are suppressed.

- **RON format**: Timelines are loaded from `.ron` files using serde.
  Example structure is documented in the `Timeline` struct.

## Internal Design

- Keyframe lookup uses a binary search on the sorted `time` values within each
  track's keyframe list. Typical tracks have 5-50 keyframes, so binary search
  is efficient.
- CameraPath interpolation constructs a `CatmullRomSpline` from the keyframe
  positions at load time. The spline `t` parameter is mapped from the timeline
  time range to [0, 1].
- `fired_events` is a `FxHashSet<(track_index, event_index)>` -- compact and
  O(1) lookup per event.
- `entity_tags` maps string tags from SpawnEvents to EntityIds so that
  subsequent AnimOverride or Despawn events can reference spawned entities.
- The `TimelineContext` uses trait objects (`dyn FnMut`) for callbacks to avoid
  hard-coupling the timeline system to specific dialogue or tween implementations.

## Non-Goals

- **Visual timeline editor.** The RON format is hand-authored or generated by
  external tools. An in-engine visual editor is a separate project.
- **Branching / conditional timelines.** Timelines are linear sequences. For
  branching narratives, use the dialogue tree system. Timelines can trigger
  dialogue nodes but do not contain branching logic themselves.
- **Looping timelines.** Timelines play once and finish. For looping ambient
  sequences, use the animation or tween systems directly.
- **Nested timelines.** A timeline cannot contain sub-timelines. Keep them
  flat for simplicity. Chain timelines in game code if needed.

## Open Questions

- Should CameraPath tracks support both CatmullRom and CubicBezier, selectable
  per track? Or default to CatmullRom only?
- How should timeline-spawned entities be cleaned up if the timeline is stopped
  mid-playback? Auto-despawn, or leave them for game code to handle?
- Should there be a `WaitForSeconds` event type that pauses the timeline clock
  without requiring a dialogue trigger?
- Is RON the right format, or should timelines support a more compact binary
  format for shipping?
- Should the `TimelineContext` include access to the `LocaleManager` so that
  dialogue triggers can resolve localized text inline?

## Referenzen

- [engine/tween](tween.md) -- EasingFn, Tweenable trait, TweenSequence
- [engine/spline](spline.md) -- CatmullRomSpline for camera path interpolation
- [engine/camera](camera.md) -- Camera struct, CameraMode
- [engine/audio](audio.md) -- SfxManager, AdaptiveMusicEngine
- [engine/dialogue](dialogue.md) -- DialogTree for triggered conversations
- Unity Timeline / Godot AnimationPlayer as design references
