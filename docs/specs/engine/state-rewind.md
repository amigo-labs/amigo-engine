---
status: spec
crate: amigo_core
depends_on: ["engine/save-load", "engine/simulation"]
last_updated: 2026-03-18
---

# State Rewind

## Purpose

Frame-by-frame rewind system that records game state snapshots into a ring buffer
and allows rewinding to any recorded frame. Enables Braid-style time-rewind
mechanics, debugging tools (step back through simulation), and level design
testing. Integrates with the fixed-timestep simulation system so that rewound
state is deterministic. Uses delta compression to keep memory usage practical
at 60fps recording rates.

## Public API

### RewindBuffer

```rust
use serde::{Serialize, de::DeserializeOwned};

/// Ring buffer that stores a sliding window of game state snapshots.
/// `T` is the game state type (must be serializable for delta compression).
///
/// Capacity is measured in frames. At 60fps with capacity 300, the buffer
/// stores 5 seconds of rewind history.
pub struct RewindBuffer<T: Clone> {
    /// Stored snapshots (ring buffer).
    frames: Vec<RewindFrame<T>>,
    /// Maximum number of frames to retain.
    capacity: usize,
    /// Index of the oldest valid frame in the ring buffer.
    head: usize,
    /// Total number of valid frames currently stored.
    len: usize,
    /// Global frame counter (monotonically increasing simulation tick).
    current_tick: u64,
    /// Whether recording is active.
    recording: bool,
    /// Compression mode.
    compression: CompressionMode,
    /// Performance stats for the last record/rewind operation.
    last_stats: RewindStats,
}

/// A single frame in the rewind buffer.
#[derive(Clone)]
struct RewindFrame<T: Clone> {
    /// Simulation tick this frame was recorded at.
    tick: u64,
    /// State storage (full snapshot or delta).
    data: FrameData<T>,
}

/// How a frame's state is stored.
#[derive(Clone)]
enum FrameData<T: Clone> {
    /// Full state snapshot (used for keyframes).
    Full(T),
    /// Delta from the previous frame (compressed).
    Delta(Vec<u8>),
}

/// Compression strategy for the rewind buffer.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CompressionMode {
    /// Store every frame as a full snapshot. Simple but memory-heavy.
    None,
    /// Store a full keyframe every N frames, deltas in between.
    /// `keyframe_interval` is the number of frames between full snapshots.
    Delta { keyframe_interval: u32 },
}

impl Default for CompressionMode {
    fn default() -> Self {
        CompressionMode::Delta { keyframe_interval: 30 }
    }
}

impl<T: Clone + Serialize + DeserializeOwned + PartialEq> RewindBuffer<T> {
    /// Create a new rewind buffer with the given frame capacity.
    pub fn new(capacity: usize) -> Self;

    /// Create with explicit compression mode.
    pub fn with_compression(capacity: usize, mode: CompressionMode) -> Self;

    /// Record the current state as the next frame.
    /// Call once per simulation tick while recording is active.
    /// Returns the tick number assigned to this frame.
    pub fn record(&mut self, state: &T) -> u64;

    /// Rewind to a specific simulation tick.
    /// Returns the reconstructed state at that tick, or None if the tick
    /// is not in the buffer's range.
    pub fn rewind_to(&self, tick: u64) -> Option<T>;

    /// Step one frame backward from the current position.
    /// Returns the state at (current_tick - 1), or None if at the oldest frame.
    pub fn step_back(&mut self) -> Option<T>;

    /// Step one frame forward after a rewind.
    /// Returns the state at (current_tick + 1), or None if at the newest frame.
    pub fn step_forward(&mut self) -> Option<T>;

    /// Get the state at the current tick without changing position.
    pub fn current_state(&self) -> Option<T>;

    /// Start recording (enabled by default on construction).
    pub fn start_recording(&mut self);

    /// Stop recording. The buffer retains existing frames but does not
    /// accept new ones. Useful during rewind playback.
    pub fn stop_recording(&mut self);

    /// Whether recording is currently active.
    pub fn is_recording(&self) -> bool;

    /// Clear all stored frames and reset the tick counter.
    pub fn clear(&mut self);

    /// The simulation tick of the oldest frame in the buffer.
    pub fn oldest_tick(&self) -> Option<u64>;

    /// The simulation tick of the newest frame in the buffer.
    pub fn newest_tick(&self) -> Option<u64>;

    /// The current tick position (may be between oldest and newest during rewind).
    pub fn current_tick(&self) -> u64;

    /// Number of frames currently stored.
    pub fn len(&self) -> usize;

    /// Maximum number of frames the buffer can hold.
    pub fn capacity(&self) -> usize;

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool;

    /// Performance statistics from the last record or rewind operation.
    pub fn last_stats(&self) -> &RewindStats;
}
```

### RewindStats

```rust
/// Performance metrics for the last rewind operation.
#[derive(Clone, Debug, Default)]
pub struct RewindStats {
    /// Time spent on the last record() call in microseconds.
    pub last_record_us: u64,
    /// Time spent on the last rewind/step call in microseconds.
    pub last_rewind_us: u64,
    /// Approximate memory usage of the entire buffer in bytes.
    pub memory_bytes: usize,
    /// Number of full keyframes in the buffer.
    pub keyframe_count: usize,
    /// Number of delta frames in the buffer.
    pub delta_count: usize,
    /// Average delta size in bytes (0 if no deltas).
    pub avg_delta_bytes: usize,
}
```

### RewindController

```rust
/// High-level controller that integrates rewind with the simulation loop.
/// Manages the transition between normal play and rewind mode.
pub struct RewindController<T: Clone + Serialize + DeserializeOwned + PartialEq> {
    buffer: RewindBuffer<T>,
    mode: RewindMode,
    /// Visual playback speed during rewind (negative = backward).
    rewind_speed: f32,
    /// Accumulator for fractional frame steps during rewind.
    step_accumulator: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewindMode {
    /// Normal gameplay. Buffer records each tick.
    Playing,
    /// Rewinding backward. Simulation is frozen; buffer is read.
    Rewinding,
    /// Paused at a specific frame during rewind.
    Paused,
    /// Fast-forwarding through recorded frames after a rewind.
    FastForward,
}

impl<T: Clone + Serialize + DeserializeOwned + PartialEq> RewindController<T> {
    /// Create a controller with the given buffer capacity.
    pub fn new(capacity: usize) -> Self;

    /// Record a frame during normal play. No-op if not in Playing mode.
    pub fn record(&mut self, state: &T);

    /// Enter rewind mode. Stops recording, begins stepping backward.
    pub fn begin_rewind(&mut self);

    /// Exit rewind mode and resume normal play from the current frame.
    /// Frames after the current position are discarded (timeline is forked).
    pub fn resume_play(&mut self);

    /// Update the rewind controller. During Rewinding mode, steps backward
    /// at `rewind_speed` frames per second. Returns the state to display,
    /// or None if in Playing mode (game uses live state).
    pub fn update(&mut self, dt: f32) -> Option<T>;

    /// Set rewind playback speed (frames per second, default: 60.0).
    pub fn set_rewind_speed(&mut self, fps: f32);

    /// Pause during rewind.
    pub fn pause(&mut self);

    /// Resume rewind from pause.
    pub fn resume_rewind(&mut self);

    /// Step exactly one frame backward (for frame-by-frame debugging).
    pub fn step_back(&mut self) -> Option<T>;

    /// Step exactly one frame forward.
    pub fn step_forward(&mut self) -> Option<T>;

    /// Current mode.
    pub fn mode(&self) -> RewindMode;

    /// Progress through the rewind buffer as a fraction [0.0, 1.0] where
    /// 0.0 is the oldest frame and 1.0 is the newest.
    pub fn progress(&self) -> f32;

    /// Access the underlying buffer for stats or direct queries.
    pub fn buffer(&self) -> &RewindBuffer<T>;
}
```

### Rewind UI Overlay

```rust
/// Renders a rewind progress bar and frame counter.
/// Uses the existing UiContext immediate-mode API.
pub fn draw_rewind_overlay(
    ui: &mut UiContext,
    controller: &RewindController<impl Clone + Serialize + DeserializeOwned + PartialEq>,
    screen_width: f32,
    screen_height: f32,
);
```

## Behavior

- **Recording**: During normal gameplay, `record()` is called once per
  simulation tick (fixed timestep). The buffer stores frames in a ring: when
  full, the oldest frame is overwritten.

- **Delta compression**: With `CompressionMode::Delta`, every Nth frame is a
  full keyframe (serialized with `serde_json::to_vec`). Intermediate frames
  store only a binary diff against the previous frame. The diff is computed
  by serializing both states to bytes and storing only the changed byte ranges.

- **Rewind reconstruction**: To reconstruct a frame that was stored as a delta,
  find the nearest preceding keyframe, deserialize it, then apply each delta
  in sequence up to the target tick. With `keyframe_interval: 30`, worst case
  is 29 delta applications.

- **Timeline forking**: When the player resumes from a rewound position, all
  frames after the current tick are discarded. The game continues from the
  restored state, creating a new timeline branch. The CommandLog is truncated
  to match.

- **Simulation integration**: During rewind, the simulation loop is frozen
  (no `SimSystem::update` calls). The rewind controller provides the historical
  state for rendering only. On resume, the simulation restarts from the
  restored state.

- **Performance budget**: Target is < 0.5ms per `record()` call at 60fps.
  Delta compression reduces memory from ~300 full snapshots to ~10 keyframes
  + 290 small deltas. For a typical game state of 50KB, this means ~500KB
  for keyframes + ~100KB for deltas = ~600KB total for 5 seconds of history.

- **UI overlay**: The `draw_rewind_overlay` function renders a horizontal
  progress bar at the bottom of the screen showing the rewind position,
  plus a frame counter and "REWIND" text indicator. Uses `UiContext` methods
  (`filled_rect`, `pixel_text`, `progress_bar`).

## Internal Design

- Ring buffer is a `Vec<RewindFrame<T>>` with pre-allocated capacity. Head
  index wraps around via modulo arithmetic. No dynamic allocation during
  steady-state recording.
- Delta computation: serialize current and previous state to `Vec<u8>`, then
  produce a compact diff (list of `(offset, length, new_bytes)` patches).
  Simple byte-level diffing, not structural. This works well because JSON
  serialization of similar states produces mostly identical byte sequences.
- Keyframe detection: `tick % keyframe_interval == 0`.
- `RewindStats` is updated on each `record()` and `rewind_to()` call using
  `std::time::Instant` measurements. Exposed for profiling overlays.
- The `step_accumulator` in `RewindController` allows smooth rewind at
  arbitrary speeds by accumulating fractional frame steps from `dt`.

## Non-Goals

- **Networked rewind / rollback netcode.** This is a single-player rewind
  system. Rollback for multiplayer uses different architecture (see
  networking spec).
- **Partial state rewind.** The entire game state is rewound atomically.
  Rewinding only specific entities (e.g., player position but not enemies)
  requires a different design.
- **Persistent rewind history.** The buffer is in-memory only and is cleared
  on save/load. It does not persist across sessions.
- **Audio rewind.** Sound effects are not reversed during rewind. Only visual
  state is replayed. A "rewind sound effect" plays as feedback.
- **Compression beyond delta.** LZ4 or similar compression of deltas is not
  in scope. If delta sizes are too large, increase keyframe interval or
  reduce state size.

## Open Questions

- Should the delta format be JSON byte-diff or structural serde diff?
  Structural would be more compact but significantly more complex to implement.
- What happens to particles, tweens, and other ephemeral visual state during
  rewind? Suppress them, or record them too?
- Should rewind support variable-speed playback (hold button = rewind faster)?
  Current design supports this via `set_rewind_speed()`.
- Is 300 frames (5 seconds) enough for gameplay mechanics, or should capacity
  be configurable per game? Current design allows any capacity.
- Should `resume_play()` fork the timeline (discard future frames) or allow
  "redo" (keep future frames accessible)? Current design forks.
- How does rewind interact with the autosave system? Should autosave be
  suppressed during rewind mode?

## Referenzen

- [engine/save-load](save-load.md) -- SaveManager, serialization patterns
- [engine/simulation](simulation.md) -- Fixed timestep, SimSpeed, tick counter
- [engine/command](core.md) -- CommandLog for replay (truncated on rewind fork)
- [engine/ui](ui.md) -- UiContext for rewind overlay rendering
- Braid (Jonathan Blow) -- Reference game for time-rewind mechanics
- Forza Motorsport rewind system -- GDC talk on snapshot ring buffers
