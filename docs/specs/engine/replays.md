---
status: done
crate: amigo_engine
depends_on: ["engine/input", "engine/simulation", "ai-pipelines/agent-api"]
last_updated: 2026-10-06
---

# Replays

## Purpose

Record a session's input and play it back later to get the same session again,
tick for tick. The uses:

- **Bug reports.** A player sends the replay and the developer watches the bug
  happen.
- **Regression tests.** A replay played headless must still end in the same state
  after a code change.
- **Determinism checks.** Every tick carries a state hash, so a replay played on
  another machine or build names the first tick where the two runs part ways.

The simulation is deterministic (ADR-0001): fixed-point math, a fixed timestep,
fixed iteration order and a seeded RNG. Under those conditions the input and the
starting RNG state are enough to reproduce a run, and that is all a replay stores.

## Public API

### Starting

| Way | Record | Play |
|---|---|---|
| CLI | `amigo run --record replays/run.json` | `amigo run --replay replays/run.json` |
| Environment | `AMIGO_RECORD=<file>` | `AMIGO_REPLAY=<file>` |
| Builder | `EngineBuilder::record_replay(path)` | `EngineBuilder::play_replay(path)` |
| API (mid-session) | `replay.record_start`, then `replay.record_stop {path}` | `replay.play {path}`, `replay.stop` |

- **Recording from launch** starts before the root game's `init`. The file is
  written when the engine shuts down: window closed, `quit`, or the game returns
  `SceneAction::Quit`.
- **Playing from launch** sets the RNG state before `init`. The first tick then
  runs on exactly the recorded input.
- **Mid-session recording** (API) also stores `Game::on_dev_snapshot`. Playback
  restores it through `Game::on_dev_restore`, and moves `ctx.time.tick` and
  `ctx.rng` to where the recording started. A game that keeps state the snapshot
  does not cover is not reproduced exactly; the desync check then reports it.
- **Status.** `GameContext::replay_status()`, or over the API
  `engine.get_property {"key": "replay"}`:

```json
{"mode": "finished", "path": "replays/run.json", "start_tick": 0,
 "ticks": 600, "total_ticks": 600, "desync_tick": null}
```

`mode` is `off`, `recording`, `playing`, or `finished`. `desync_tick` is the first
tick whose state hash differed from the recording.

### Game hooks

```rust
pub trait Game {
    // ...
    /// A hash of the simulation state after a tick. Default: None.
    fn state_hash(&self, ctx: &GameContext) -> Option<u64>;
}

pub struct GameContext {
    // ...
    /// Seeded RNG; replays restore its state.
    pub rng: SimRng,
}
impl GameContext {
    pub fn seed(&self) -> u64;
    pub fn reseed(&mut self, seed: u64);
    pub fn replay_status(&self) -> ReplayStatus;
}
```

- **Seed.** `[dev] seed` in `amigo.toml`, `AMIGO_SEED`, `amigo run --seed N`, or
  `EngineBuilder::seed`. With none of these set, the engine takes a seed from the
  clock and logs it (`Simulation seed …`).
- **Hash.** The engine mixes the tick, the RNG state and the entity count into
  every hash itself. Even without `state_hash`, a game that draws a different
  number of random values, or spawns a different number of entities, shows up.
  Hashing the game's own state catches a desync on the tick it happens.

### Input

`amigo_input::InputSnapshot` holds one tick's keyboard and mouse state:
- keys held, pressed and released, by their `key_name`;
- mouse buttons held, pressed and released, as numbers;
- the mouse position in window, UI and world coordinates;
- scroll and typed text.

`InputState::snapshot()` and `restore()` convert to and from it.
`ActionSnapshot` (`ActionState::snapshot`/`restore`) does the same for the bound
actions. `key_name(KeyCode)` is the inverse of `key_from_name`.

## Behavior

Each tick (the engine-internal `tick::run_tick`, shared by the windowed and the
headless loop) runs:

1. `ctx.time.dt = TICK_DURATION`, `ui.begin()`.
2. **Recording or live play:** actions are refreshed from live input. A recording
   then stores the input and actions, but only when they differ from the
   previous tick (delta encoding).
   **Playback:** the replay's input and actions replace the live ones. The live
   gamepads are set aside and the game sees none.
3. `Game::update`, `tick += 1`, scene-stack change, `Plugin::update`, ECS and
   event flush, particles.
4. **Recording:** the state hash is stored. **Playback:** it is compared with the
   recorded one. After the last recorded tick, playback ends, live input takes
   over again, and the result is logged.
5. One-tick input (presses, releases, scroll, text) is cleared.

## File format

JSON, version 1 (`REPLAY_VERSION`):

```json
{
  "version": 1, "scene": "my_game::Game", "from_start": true,
  "start_tick": 0, "ticks": 600, "seed": 42, "rng_state": 42,
  "inputs": [
    {"tick": 0,  "input": {"mouse": [0,0], "mouse_ui": [0,0], "mouse_world": [0,0]}, "actions": {}},
    {"tick": 12, "input": {"held": ["KeyD"], "pressed": ["KeyD"], "...": "..."},
                 "actions": {"pressed": ["right"], "held": ["right"]}}
  ],
  "hashes": [1234567890123, "... one per tick ..."],
  "metadata": {"engine_version": "0.1.0"}
}
```

Mouse coordinates are `f32` and survive the JSON round trip bit for bit. Empty
lists are left out.

## Limits

- **Raw gamepad state is not recorded.** `GamepadState` cannot be fed from data.
  Gamepad input that goes through bound actions is replayed; a game that reads
  `ctx.gamepad` directly is not.
- **Wall-clock state.** `time.elapsed`, `time.frame_dt` and anything else
  wall-clock is not part of the replay. Simulation code counts `time.tick`.
- **No seeking.** `replay.play` starts where the recording started.
  `from_tick` is rejected, because a replay holds input, not the state at an
  arbitrary tick.
- **Modules still in `f32`.** ADR-0001 lists the modules that are not yet
  deterministic across platforms (`combat`, `ai`, `platformer`, …). A replay of a
  game built on them reproduces on the machine that recorded it, but not
  necessarily on another one.
- **Assets and code must match.** A replay assumes the same game build and the
  same data. `metadata.engine_version` records the engine version only.

## Tests

- `amigo_engine/src/replay.rs`:
  - 600 ticks recorded from launch, then played in a fresh engine with another
    seed and other live input: same end state, no desync.
  - A changed input is reported at its tick; a wrong RNG state at tick 0.
  - A mid-session recording restores the snapshot.
  - Delta encoding; ending playback hands input back; launch recording and
    playback through `AMIGO_RECORD`/`AMIGO_REPLAY`; the fixed `dt`.
  - The `replay.*` API from start to finish.
- `amigo_input`:
  - `key_name` and `key_from_name` round-trip for every key.
  - `InputSnapshot` and `ActionSnapshot` round-trip, through JSON included.
- `amigo_core::rng`: golden values (SplitMix64's reference output), bounds,
  and uniformity.
