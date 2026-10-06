---
status: spec
crate: amigo_audio, amigo_engine
depends_on: ["engine/audio"]
last_updated: 2026-10-06
---

# Audio Playback Control

## Purpose

`AudioManager` (`crates/amigo_audio/src/lib.rs:318`) is what a game reaches as
`ctx.audio`. Today it can start a sound and nothing more:

- `play_sfx` is fire-and-forget (`let _ = manager.play(data)`, `lib.rs:396`), so a
  sound cannot be stopped, faded or re-levelled once started.
- There is no delayed start, no way to play part of a file, and no loop region.
- The volume channels exist but never reach playback. `sfx` and `master` do not
  affect sound effects, and the `[audio]` volumes in `amigo.toml` are ignored
  (backlog sub-33).
- Music does not loop, and is re-read from disk on every `play_music`.
- Sounds can only be loaded from a file path.

kira, underneath, supports every one of these. This spec puts them behind
`AudioManager`, without exposing kira types:

- handles to playing sounds;
- start control (delay, start point, length, loop region, repeat count);
- real buses for music, sound effects and ambience under a master;
- pause and resume of everything;
- loading from bytes.

The reference case is [HamsterFlight](https://github.com/daniel-rck/HamsterFlight),
a browser port of a Flash game whose audio player (`src/audio/AudioPlayer.ts`)
reproduces Flash `Sound` semantics on Web Audio. Each of its needs maps to a section:

| Need | Where in HamsterFlight | Engine today | Section |
|---|---|---|---|
| `stop()` ends every instance of a sound; `setVolume` changes every instance, including ones already playing | `src/audio/AudioPlayer.ts` (class comment) | no handles, no per-sound volume | A1 |
| Timeline sounds start `delayFrames / 19` s after their cue | `AudioPlayer.ts:259` | no delayed start | A2 |
| Skip each MP3's encoder delay and play exactly the recorded sample count; loop within that range | `src/assets/sounds.generated.ts` (`seek`, `samples`), `AudioPlayer.ts:261-284` | whole file only | A2 |
| A sound that plays twice back to back | `AudioPlayer.ts:44` (`LoopCount`, total plays) | — | A2 |
| Music fades out 3 % per 50 ms; music and effects mute separately | `AudioPlayer.ts` (`FADE_STEP`, `FADE_MS`) | `sfx` volume never applied | A1, A3 |
| Audio suspends with the game's pause | `AudioPlayer.ts:157` | — | A4 |
| Sounds are fetched over HTTP and decoded from bytes | `src/assets/SoundUrls.ts` | file paths only | A5 |
| Looping theme, switched between prelude, theme and ending | `src/sim/events.ts` (music cues) | `play_music` does not loop | A6 |

## Public API

All new types live in `amigo_audio` and are re-exported from its root. `SoundHandle`,
`PlaySettings`, `Bus`, `Fade`, `LoopMode`, `Position` and `SoundState` are added to
`amigo_engine::prelude` next to `AudioManager`. No kira type appears in the new API.

### Common types

```rust
/// Refers to one started sound. Copy, never dangling: once the sound ends,
/// every operation on it is a no-op and `state` reports `Stopped`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SoundHandle {
    index: u32,
    generation: u32,
}

impl SoundHandle {
    /// A handle that never referred to a sound.
    pub const NONE: SoundHandle;
    pub fn is_none(self) -> bool;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Bus {
    Music,
    #[default]
    Sfx,
    Ambient,
}

/// A point in a sound file. `Samples` counts sample frames at the file's own
/// sample rate, so a value read from the file's metadata can be used as is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Position {
    Seconds(f64),
    Samples(u64),
}

impl Default for Position {
    /// `Seconds(0.0)`.
    fn default() -> Self;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    /// Play the range once.
    #[default]
    Once,
    /// Play `n` times in total, back to back, without a gap. `Count(0)` and
    /// `Count(1)` are the same as `Once`.
    Count(u32),
    /// Until stopped.
    Forever,
}

/// A linear volume ramp. `Fade::NONE` changes immediately.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fade {
    pub seconds: f32,
}

impl Fade {
    pub const NONE: Fade;
    pub fn secs(seconds: f32) -> Fade;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundState {
    /// Started with a delay that has not elapsed yet.
    Scheduled,
    Playing,
    /// Paused by `pause_all`.
    Paused,
    /// Finished, stopped, never started, or the handle is `NONE`.
    Stopped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaySettings {
    pub bus: Bus,
    /// Linear amplitude; 1.0 plays the file as recorded. Clamped to 0.0..=4.0.
    pub volume: f32,
    /// Seconds from the `play` call until the sound starts. `<= 0` is now.
    pub delay: f64,
    /// Where in the file playback starts.
    pub start: Position,
    /// How much to play from `start`. `None` plays to the end of the file.
    pub length: Option<Position>,
    /// The part that `loops` repeats, in file positions. `None` repeats the
    /// whole played range `start..start + length`.
    pub loop_region: Option<(Position, Position)>,
    pub loops: LoopMode,
    pub fade_in: Fade,
    /// 1.0 is normal speed and pitch. Clamped to 0.01..=8.0.
    pub playback_rate: f64,
    /// -1.0 is left, 0.0 centre, 1.0 right. Clamped.
    pub panning: f32,
}

impl Default for PlaySettings {
    /// Sfx bus, volume 1.0, no delay, from the start to the end, once,
    /// no fade, rate 1.0, centred.
    fn default() -> Self;
}

impl PlaySettings {
    /// `PlaySettings { bus, ..Default::default() }`.
    pub fn on(bus: Bus) -> Self;
}
```

### A1: Handles and per-sound volume

```rust
impl AudioManager {
    /// Start a loaded sound. Always returns a handle. When the sound cannot
    /// start (no audio device, unknown name, empty range, kira's sound limit),
    /// the handle is already `Stopped`.
    pub fn play(&mut self, name: &str, settings: &PlaySettings) -> SoundHandle;

    /// Stop one sound, fading it out over `fade`.
    pub fn stop(&mut self, handle: SoundHandle, fade: Fade);

    /// Stop every playing or scheduled instance of `name`.
    pub fn stop_all(&mut self, name: &str, fade: Fade);

    /// Change one sound's volume (the `PlaySettings::volume` it started with).
    pub fn set_sound_volume(&mut self, handle: SoundHandle, volume: f32, fade: Fade);

    /// Per-name volume, multiplied into every current and every future
    /// instance of `name`. Default 1.0. This is Flash's `Sound.setVolume`.
    pub fn set_name_volume(&mut self, name: &str, volume: f32, fade: Fade);

    pub fn name_volume(&self, name: &str) -> f32;

    pub fn set_playback_rate(&mut self, handle: SoundHandle, rate: f64, fade: Fade);

    pub fn state(&self, handle: SoundHandle) -> SoundState;

    /// `state` is `Scheduled`, `Playing` or `Paused`.
    pub fn is_playing(&self, handle: SoundHandle) -> bool;

    /// Live instances of `name` (not `Stopped`).
    pub fn playing_count(&self, name: &str) -> usize;

    /// Free the slots of sounds that have ended. The engine calls this once
    /// per frame; a game never has to.
    pub fn maintain(&mut self);
}
```

### A3: Buses and master

```rust
impl AudioManager {
    pub fn set_master_volume(&mut self, volume: f32, fade: Fade);
    pub fn master_volume(&self) -> f32;

    pub fn set_bus_volume(&mut self, bus: Bus, volume: f32, fade: Fade);
    pub fn bus_volume(&self, bus: Bus) -> f32;

    /// Mute without losing the volume setting.
    pub fn set_bus_muted(&mut self, bus: Bus, muted: bool);
    pub fn is_bus_muted(&self, bus: Bus) -> bool;
}
```

The existing `set_volume(&mut self, channel: &str, volume: f32)` stays as a wrapper:
`"master"` → `set_master_volume`, `"music"`/`"sfx"`/`"ambient"` → `set_bus_volume`,
always with `Fade::NONE`.

### A4: Pause

```rust
impl AudioManager {
    /// Pause every sound on every bus, fading out over `fade`.
    pub fn pause_all(&mut self, fade: Fade);

    /// Resume everything `pause_all` paused, fading in over `fade`.
    pub fn resume_all(&mut self, fade: Fade);

    pub fn is_paused(&self) -> bool;
}
```

### A5: Loading

```rust
impl AudioManager {
    /// Decode a file (wav, ogg, mp3, flac) and register it under `name`.
    /// Loading a second file under the same name adds a variant; `play`
    /// picks one variant at random.
    pub fn load_sound(&mut self, name: &str, path: &Path) -> Result<(), AudioError>;

    /// Same, from encoded bytes already in memory (a pak entry, a download).
    pub fn load_sound_from_bytes(&mut self, name: &str, bytes: Arc<[u8]>) -> Result<(), AudioError>;

    /// Forget `name` and all its variants. Playing instances stop.
    pub fn unload(&mut self, name: &str);

    pub fn is_loaded(&self, name: &str) -> bool;

    /// Length of the first variant in seconds.
    pub fn sound_duration(&self, name: &str) -> Option<f64>;

    /// Sample rate of the first variant, the unit of `Position::Samples`.
    pub fn sound_sample_rate(&self, name: &str) -> Option<u32>;

    /// A manager on kira's mock backend: every call works, nothing is heard.
    /// For tests and for headless mode.
    pub fn new_silent(base_path: impl Into<PathBuf>) -> Self;
}

// `AudioError` gains:
//     #[error("Failed to load sound '{name}': {message}")]
//     Load { name: String, message: String },
```

The existing `load_sfx(name, path)` becomes a wrapper around `load_sound` that logs
the error instead of returning it, as it does today.

### A6: Music

```rust
impl AudioManager {
    /// Play the loaded sound `name` on the music bus, looping forever, and
    /// crossfade from whatever music is playing over `fade`. Returns the
    /// existing handle when `name` is already the current music.
    pub fn start_music(&mut self, name: &str, fade: Fade) -> SoundHandle;

    /// Fade out and stop all music.
    pub fn stop_music_with(&mut self, fade: Fade);

    /// The handle `start_music` last returned, while it is still live.
    pub fn current_music(&self) -> Option<SoundHandle>;
}
```

The existing `play_music(name, path)` loads `path` under `name` if it is not loaded
yet, then calls `start_music(name, Fade::NONE)`; it therefore now loops. The existing
`stop_music()` is `stop_music_with(Fade::NONE)`.

## Behavior

### Signal chain

```
kira main track          <- master volume
├── music sub-track      <- Music bus volume, mute
├── sfx sub-track        <- Sfx bus volume, mute
└── ambient sub-track    <- Ambient bus volume, mute
      └── each sound     <- PlaySettings::volume × name volume
```

- The effective amplitude of a sound is the product of master, bus (0 while muted),
  name volume and its own volume.
- Changing any factor reaches sounds that are already playing.
- Volume setters clamp to `0.0..=4.0`, and a non-finite value is ignored with a
  warning.
- Amplitude converts to kira's decibels with the existing `amplitude_to_decibels`;
  zero is silence.
- `play_sfx` and `play_sfx_at` now go through the sfx bus. This is the fix for backlog
  sub-33.
- `SfxManager`, `AdaptiveMusicEngine` and sounds played through `kira_manager_mut()`
  still play on kira's main track. They get the master volume but no bus until they
  are migrated (see Non-Goals).

### Configuration

- At startup the engine applies `[audio] master_volume`, `music_volume` and
  `sfx_volume` from `amigo.toml` to `set_master_volume` and `set_bus_volume`. Ambient
  starts at 1.0.
- The public `volumes: VolumeChannels` field stays.
  - It mirrors what the setters set.
  - A game that writes to it directly still gets its change applied: `maintain()`
    compares the field against the last applied values and applies any difference
    with `Fade::NONE`.

### A1: Handles

- **Handle slots.** Handles index a slot table with a generation counter. `maintain`
  frees the slots of sounds kira reports as stopped and bumps their generation, so a
  stale handle can never reach a newer sound in the same slot.
- **Dead handles.** A dead or `NONE` handle is accepted by every method and does
  nothing. `state` returns `Stopped`.
- **`play` that cannot start.** These cases return a `Stopped` handle:
  - an unknown name;
  - an empty range after clamping (A2);
  - kira's sound limit reached.

  Each logs one `warn!` per name, not per call, because they happen in the frame
  path. Without an audio device, `play` returns a `Stopped` handle silently, as
  sounds are silently skipped today.
- **Settings without a device.** Getters return what was set even without a device, so
  a settings screen works on a machine with no sound card.
- **Fades.**
  - `stop` with a fade lets the sound fade out and then ends it. The handle reads
    `Playing` until the fade completes.
  - A `stop` or `stop_all` before a scheduled sound starts cancels it at once.
- **Per-name volume.**
  - `set_name_volume` applies to every live instance with the given fade.
  - Instances started later start at the new value.
  - The value persists across `unload` and reload of the same name.

### A2: Start control

- **Delay.** `delay` is measured from the `play` call and timed on the audio thread
  (kira `StartTime::Delayed`), not by the frame loop.
  - Several ticks processed in one frame call `play` at nearly the same moment. A game
    that schedules sounds tick by tick should add the tick offset to `delay` itself.
- **Positions.** `start`, `length` and `loop_region` resolve to sample frames of the
  chosen variant: `Seconds(s)` becomes `round(s · sample_rate)`. Then they are
  clamped:
  - `start` is clamped to the file length;
  - `start + length` is clamped to the file length;
  - an empty range (`start >= end`) does not play (see A1).
- **Loop region.**
  - It only matters with `LoopMode::Count` or `LoopMode::Forever`. With `Once` the
    range plays straight through.
  - The region is clamped to the played range.
  - If it is empty after clamping, the sound plays once and one `warn!` is logged per
    name.
  - Playback runs from `start` into the region, repeats the region, and the last
    repetition plays on to the end of the played range.
- **`LoopMode::Count(n)`.** With `n >= 2` the region is played `n` times in total,
  and the sound then continues to the end of the range and stops. Without a
  `loop_region` the whole played range repeats `n` times, so the sound is heard `n`
  times back to back with no gap.
  - The finite repetition is rendered into its own sample buffer when `play` is
    called: the lead-in `start..region_start`, `n` copies of the region, then the tail
    `region_end..end`. The result is gapless and sample-exact, and the tail always
    plays.
  - Rendered buffers are cached per name, variant, resolved range and `n`.
  - A rendered buffer longer than 120 s of audio is not built. Such a `play` returns a
    `Stopped` handle and logs one `warn!` per name. For long repeats, use `Forever` and
    `stop`.
- **`LoopMode::Forever`.** Repeats the region until the sound is stopped.
- **Other settings.** `fade_in` ramps from silence to the sound's volume after the
  delay. `playback_rate` scales speed and pitch together.

### A3: Buses

- `set_bus_muted(bus, true)` silences the bus at once without changing
  `bus_volume`. Unmuting restores the bus volume.
- A sound started on a muted bus plays silently and is heard when the bus is
  unmuted. This is how a "music off" toggle keeps the music in time.

### A4: Pause

- `pause_all` pauses the three bus tracks.
  - Paused sounds keep their position, and scheduled sounds keep their remaining
    delay.
  - Sounds started while paused start paused and are heard after `resume_all`.
  - `state` reports `Paused` for every sound on a paused bus.
- `pause_all` while paused and `resume_all` while not paused are no-ops.
- The engine does not pause audio by itself: not on focus loss and not when a scene is
  pushed. The game decides, usually in `Game::on_pause`.

### A5: Loading

- `load_sound` and `load_sound_from_bytes` decode eagerly. The data is shared between
  all instances (kira's `StaticSoundData` is reference-counted), so playing does not
  copy samples.
- A decode failure returns `AudioError::Load` with the decoder's message. The name
  then keeps the variants it had before the failed call.
- Loading does not need an audio device, so a game can load at startup on a machine
  without one.
- `play` picks a variant with presentation-only randomness (an internal xorshift
  seeded at construction), not from the clock as today. It never touches simulation
  RNG.

### A6: Music

- `start_music(name, fade)`:
  - fades out and stops every sound on the music bus over `fade`;
  - starts `name` on the music bus with `LoopMode::Forever` and `fade_in: fade`;
  - records the new handle as current music.
- A call for the name that is already the current music, while it is live, changes
  nothing and returns that handle. A game can therefore call it every frame from
  state.

### Determinism

Audio is presentation. `state`, `is_playing`, `playing_count` and `current_music`
depend on the audio device and on wall-clock time, so simulation code must not
branch on them; replays and lockstep rely on that (ADR-0001). Nothing in this spec
reads or writes simulation state.

### Headless

Headless mode (`AMIGO_HEADLESS`) constructs its `AudioManager` with `new_silent`, so
it no longer opens a real output device (part of backlog eng-5). Every call behaves
the same there except that nothing is heard and sounds do not advance.

## Internal Design

These are suggestions, not part of the contract.

- **Buses.** `AudioManager` holds three `kira::track::TrackHandle`s created with
  `add_sub_track` when the device opens. Master volume is
  `manager.main_track().set_volume`.
- **Handle slots.** Each slot holds `Option<StaticSoundHandle>`, the sound's name, and
  its own volume. kira handles are not `Clone`, which is why the manager owns them and
  games get indices.
- **Count loops.** A delayed `stop` cannot implement `Count(n)`: while kira's
  `loop_region` is set, the sound keeps repeating the region and never reaches the
  tail. kira's `set_loop_region` takes effect at once and cannot be scheduled for the
  final traversal either.
  - Instead, a pure function `render_finite_loop(frames: &[Frame], range: ResolvedRange,
    n: u32) -> Arc<[Frame]>` concatenates the lead-in, `n` copies of the region and the
    tail from the decoded frames.
  - kira's `StaticSoundData` exposes `frames: Arc<[Frame]>` and `sample_rate`, so the
    result plays as an ordinary one-shot.
  - `Forever` keeps using kira's `loop_region`.
- **Region resolution.** Resolving and clamping regions is a pure function
  `resolve_range(&PlaySettings, sample_rate, frames) -> Option<ResolvedRange>`. All of
  A2's clamping rules are tested there without a device.
- **Pausing.** `pause_all` maps to `TrackHandle::pause` on all three buses. Sounds
  started while paused are added to the paused track, which holds them; no per-sound
  bookkeeping is needed.
- **Muting.** Mute sets the track volume to silence with a zero-length tween and
  remembers the bus volume.
- **Testing.** `new_silent` uses `kira::backend::mock::MockBackend`. `AudioManager`
  keeps a private enum over the two backends, so its type stays non-generic.

## Non-Goals

- **Migrating `SfxManager` (cooldowns, pitch variance), `AdaptiveMusicEngine` and
  `SpatialAudioSystem` onto the buses.** They keep taking `&mut KiraManager`. A
  follow-up should hand them the bus tracks.
- **DSP effects on buses** (filters, ducking). Backlog sub-36 stays open.
- **Streaming playback** of long files. Everything is decoded into memory; a 4-minute
  stereo track at 44.1 kHz is about 85 MB of f32 frames. Streaming is a later
  addition alongside the static path.
- **A web audio backend.** `load_sound_from_bytes` is what such a backend needs from
  this layer, but the target itself is out of scope.
- **Sample-exact sync across sounds.** kira clocks (`StartTime::ClockTime`) would give
  it; `delay` is relative to the call.
- **Engine-level automatic pause** on focus loss or scene push.
- **HamsterFlight itself:** porting it, or shipping its sounds. They are extracted
  from the original SWF and belong to their owners.

## Open Questions

These do not change the public API.

- Whether `maintain()` also runs inside `play` when the slot table is full, or the
  table simply grows.
- Whether the mock backend in headless mode should advance in step with simulated
  time, so `state` changes there as it would with a device.

## Acceptance Criteria

Audio output cannot be heard in CI. Criteria are checked against `new_silent`
(kira's mock backend) and against the pure range resolution.

### API completeness
- [ ] `SoundHandle` (with `NONE`, `is_none`), `Bus`, `Position`, `LoopMode`, `Fade` (with `NONE`, `secs`), `SoundState`, `PlaySettings` (with `Default`, `on`) exist with the fields and derives above and are in `amigo_engine::prelude`
- [ ] `AudioManager::{play, stop, stop_all, set_sound_volume, set_name_volume, name_volume, set_playback_rate, state, is_playing, playing_count, maintain}` exist with the signatures above
- [ ] `AudioManager::{set_master_volume, master_volume, set_bus_volume, bus_volume, set_bus_muted, is_bus_muted}` exist
- [ ] `AudioManager::{pause_all, resume_all, is_paused}` exist
- [ ] `AudioManager::{load_sound, load_sound_from_bytes, unload, is_loaded, sound_duration, sound_sample_rate, new_silent}` exist; `AudioError::Load { name, message }` exists
- [ ] `AudioManager::{start_music, stop_music_with, current_music}` exist
- [ ] No kira type appears in a public signature added by this spec

### Behavior
- [ ] Test: `play` of an unknown name returns a handle whose `state` is `Stopped`; a second call logs no second warning
- [ ] Test: every method accepts `SoundHandle::NONE` and a handle whose slot was freed and reused, without effect on the slot's new sound
- [ ] Test: `maintain` frees slots of stopped sounds and bumps their generation
- [ ] Test: `set_name_volume("x", 0.5, ..)` then `play("x", ..)` starts the instance at 0.5 × its own volume; `name_volume` survives `unload` + reload
- [ ] Test (`resolve_range`): `Samples` and `Seconds` resolve with the variant's sample rate; `start` past the end yields `None`; `length` past the end is clamped; an empty `loop_region` resolves to "no loop"
- [ ] Test (`resolve_range`): `Count(0)` and `Count(1)` resolve like `Once`
- [ ] Test (`render_finite_loop`): played range 0–6 s, region 2–4 s, `Count(2)` yields an 8 s buffer equal, frame for frame, to source 0–4 s, then 2–4 s, then 4–6 s (the tail is present)
- [ ] Test (`render_finite_loop`): `Count(3)` without a region yields three back-to-back copies of the played range
- [ ] Test: a `Count(n)` whose rendered length exceeds 120 s returns a `Stopped` handle and warns once per name
- [ ] Test: bus and master getters return set values without an audio device; setters clamp to `0.0..=4.0` and ignore non-finite input
- [ ] Test: `set_bus_muted(Sfx, true)` leaves `bus_volume(Sfx)` unchanged; unmuting restores it
- [ ] Test: writing `audio.volumes.sfx` directly is applied by the next `maintain`
- [ ] Test: `pause_all` then `play` returns a handle in state `Paused`; `pause_all` twice and `resume_all` while running are no-ops
- [ ] Test: `load_sound_from_bytes` with the bytes of a test WAV registers the name and reports its duration and sample rate; garbage bytes return `AudioError::Load` and leave earlier variants intact
- [ ] Test: `start_music("a", ..)` twice returns the same handle; `start_music("b", ..)` makes `current_music` the new handle
- [ ] `play_sfx`, `play_sfx_at` and `play_music` route through the new path (sfx and music buses); `set_volume(channel, v)` maps to the bus and master setters
- [ ] The engine applies `[audio]` volumes from `amigo.toml` at startup (test with a config whose `sfx_volume = 0.3`)
- [ ] The engine calls `audio.maintain()` once per frame in the windowed loop and in the headless loop
- [ ] Headless mode constructs its `AudioManager` with `new_silent`
- [ ] `examples/audio_demo` uses `start_music` with a crossfade, a delayed `play`, `stop` on a handle and the bus mutes, and ships (or generates) the audio files it plays

### Quality gates
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`, and the rest of `just ci` (feature matrix with and without `audio`, doc tests, rustdoc)
- [ ] No `unwrap()` in library code; no `Result` on the per-frame playback calls

### Conventions
- [ ] No simulation code reads `state`, `is_playing`, `playing_count` or `current_music` (ADR-0001)
- [ ] `CHANGELOG.md` `[Unreleased]` lists the behaviour changes: `play_music` now loops; `sfx`/`master` volumes and `[audio]` config now apply
