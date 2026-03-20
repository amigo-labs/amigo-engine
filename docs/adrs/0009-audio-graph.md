---
number: "0009"
title: Node-Based Audio Graph Replacing Linear Playback
status: proposed
date: 2026-03-20
---

# ADR-0009: Node-Based Audio Graph Replacing Linear Playback

## Status

proposed

## Context

The engine's audio system lives in `crates/amigo_audio/` and wraps Kira 0.9 (`Cargo.toml` line 65). Two subsystems provide all audio functionality today:

- **`AudioManager`** (lib.rs lines 216-386): Owns a `KiraManager<DefaultBackend>`, stores `StaticSoundData` per SFX name, and plays sounds via `kira::manager::play()`. Volume is applied per-play via `StaticSoundSettings` (line 330-333). There is no persistent signal chain -- each `play()` call creates an independent voice in Kira's internal mixer. Volume channels (`VolumeChannels`, lines 41-57) track master/music/sfx/ambient levels as floats but are **not wired to Kira sub-mixes** -- they are advisory values the game reads but never apply to playing sounds after the initial `play()` call.

- **`AdaptiveMusicEngine`** (lib.rs lines 819-1343): A sophisticated system with `BarClock`, `MusicParameters`, `LayerRule`-driven vertical layering, `MusicTransition` horizontal re-sequencing (CrossfadeOnBar, FadeOutThenPlay, CutOnBar, StingerThen, LayerSwap), and stinger quantization. Each `MusicLayer` holds an `Option<StaticSoundHandle>` and a `current_volume` / `target_volume` pair that is smoothed per frame (line 648-658). However, volume changes are computed but **never pushed back to the Kira handle** -- the `update_volume` method adjusts `current_volume` on the struct but there is no `handle.set_volume()` call. This means adaptive volume changes are computed but currently silent.

- **Spatial audio** (`crates/amigo_audio/src/spatial.rs`): Pure-math functions (`compute_volume`, `compute_pan`) for distance attenuation and stereo panning using `SpatialEmitter` / `SpatialListener` components. These produce volume/pan floats but do not feed into a persistent signal chain.

- **SfxManager** (lib.rs lines 90-209): Per-sound cooldowns, concurrency limits, variant randomization. Plays through the shared `KiraManager` but has no access to sub-mixes or filters.

- **Tidal integration** (`crates/amigo_tidal_parser/src/lib.rs`): Parses TidalCycles mini-notation into `Composition` / `Stem` / `NoteEvent` ASTs. The parser produces note events with timing data but has no way to route them through filters, effects, or dynamic mixing.

The `docs/specs/engine/audio.md` spec (line 157-163) describes a desired "menu muffle" low-pass filter (`master_filter.lerp_to(lowpass_800hz, dt)`) that does not exist in the implementation. The spec's volume channel hierarchy (master -> music/sfx/ambient, lines 174-179) is also not enforced at the Kira level.

### Problems

1. **No signal routing**: Every `play()` goes straight to Kira's master output. There is no way to apply a filter to all music, duck SFX during dialogue, or route spatial audio through a reverb bus.
2. **Disconnected volume control**: `VolumeChannels` stores values but they never reach the audio backend. Changing `volumes.music` has no audible effect on already-playing music.
3. **Adaptive music volume is silent**: `MusicLayer::update_volume` computes `current_volume` but never calls `handle.set_volume()`.
4. **No effects**: The spec calls for low-pass filter, crossfade, and potentially reverb. None of these exist.
5. **Tidal events have no sink**: `NoteEvent`s from the parser need to trigger sounds through the mixing graph, but there is no graph to connect them to.

### Kira 0.9 Capabilities

Kira 0.9 provides `Track` (sub-mix buses), `TrackBuilder` with effect slots, built-in `Filter`, `Reverb`, `Delay`, and `Compressor` effects, and `TrackRoutes` for routing tracks to other tracks or the main output. Sounds can be played onto specific tracks via `StaticSoundSettings::output_destination`. This means an audio graph can be built on top of Kira's existing track system without replacing the backend.

## Decision

Introduce an `audio_graph` feature flag that adds a node-based audio graph layer on top of Kira's track system. The graph manages persistent sub-mix buses (tracks), per-bus effects, and volume routing. The `AdaptiveMusicEngine` and `SfxManager` play sounds onto named buses rather than the master output.

### Architecture

1. **`AudioGraph` struct**: Owns a set of named `AudioNode`s, each backed by a Kira `TrackHandle`. Nodes are connected in a DAG (directed acyclic graph) with the master output as the sink. Default topology:

   ```
   [sfx_bus] ──────────────────────┐
   [music_bus] ── [music_filter] ──┤
   [ambient_bus] ──────────────────┼── [master_bus] ── Kira main output
   [stinger_bus] ──────────────────┘
   ```

2. **`AudioNode` types**:
   - `MixerNode`: A Kira `Track` with volume and mute. Maps to `VolumeChannels`.
   - `FilterNode`: A Kira `Track` with a `Filter` effect (low-pass, high-pass, band-pass). Enables the "menu muffle" feature from the spec.
   - `CrossfadeNode`: Two input slots with a blend parameter. Used for smooth transitions between music sections without managing per-layer volumes manually.
   - `DuckingNode`: Side-chain compressor. Lowers one bus when another is active (e.g. duck music during dialogue).

3. **Volume channel enforcement**: `AudioGraph::set_channel_volume("music", 0.6)` calls `music_bus_handle.set_volume(Volume::Amplitude(0.6))` on the Kira track. This replaces the disconnected `VolumeChannels` struct. Changes propagate immediately to all sounds playing on that bus.

4. **Adaptive music fix**: `AdaptiveMusicEngine` gains an `output_track: Option<TrackId>` field. When the audio graph is active, `play_section` routes layer handles to the `music_bus` track via `StaticSoundSettings::output_destination`. `MusicLayer::update_volume` calls `handle.set_volume()` on each tick, fixing the silent volume issue.

5. **Tidal routing**: `NoteEvent`s from `amigo_tidal_parser` trigger sounds via `SfxManager::play()` with an explicit bus target, allowing Tidal-generated patterns to flow through the graph's effects chain.

### Alternatives Considered

1. **Replace Kira with a custom mixer**: Build a raw `cpal`-based audio graph from scratch. This gives full control but requires implementing sample-accurate mixing, resampling, and effect DSP. Rejected because Kira already provides high-quality mixing and effects, and the adaptive music system is deeply integrated with Kira's `StaticSoundHandle` API.

2. **Use Kira tracks directly without an abstraction layer**: Expose Kira `TrackHandle`s directly to game code. This is simpler but tightly couples game systems to Kira's API, making it impossible to swap backends or add custom nodes. Rejected because the abstraction layer is necessary for the crossfade and ducking nodes, which do not exist in Kira natively.

## Migration Path

1. **Fix `MusicLayer::update_volume` to push to Kira** -- In `crates/amigo_audio/src/lib.rs`, after `layer.update_volume(dt)` at line 648-658, add a call to `handle.set_volume(Volume::Amplitude(layer.effective_volume() as f64))` when `handle.is_some()`. This is a bugfix that should land before the graph, unblocking adaptive music audibly. Verify: play the TD sample game, change `tension` parameter, and confirm layer volumes change audibly in real time.

2. **Implement `AudioGraph` with default bus topology** -- Create `crates/amigo_audio/src/graph.rs` behind `cfg(feature = "audio_graph")`. Define `AudioGraph`, `AudioNode`, `MixerNode`, `FilterNode`. On construction, create four Kira tracks (sfx, music, ambient, stinger) routed to the main output. Expose `AudioGraph::bus_id(name) -> Option<TrackId>` for routing sounds. Verify: write an integration test that creates an `AudioGraph`, plays a sound onto the `sfx_bus`, and confirms it is audible through the main output. Measure end-to-end latency from `play()` to audio callback; must be under 20 ms.

3. (rough) Wire `AudioManager::play_sfx` and `play_music` to route through the graph's buses when the feature is active. `VolumeChannels` setters delegate to `AudioGraph::set_channel_volume`.

4. (rough) Implement `FilterNode` wrapping Kira's built-in `Filter` effect. Add `AudioGraph::set_filter(bus, FilterParams)` API. Implement "menu muffle" by calling `set_filter("music", lowpass(800))` when `MusicParameters::get_bool("menu_open")` is true.

5. (rough) Implement `CrossfadeNode` for music section transitions. Replace the manual volume management in `TransitionState::Crossfading` (lib.rs lines 1106-1132) with a `CrossfadeNode::set_blend(progress)` call.

6. (rough) Implement `DuckingNode` for dialogue ducking. Side-chain input monitors the dialogue bus volume; when above threshold, attenuate the music bus.

7. (rough) Route Tidal `NoteEvent` playback through the graph by having the Tidal playback system specify a target bus when calling `SfxManager::play`.

## Abort Criteria

- If end-to-end audio latency (from `play()` call to sound reaching the OS audio callback) exceeds **20 ms** with the graph layer enabled, the abstraction overhead is too high. Measure with `kira`'s internal timestamp logging on the audio thread. Abandon the graph and fix only the volume push bug (step 1).
- If Kira 0.9's `Track` API does not support dynamic effect insertion/removal at runtime (needed for `FilterNode` toggling), the filter architecture must be redesigned. Check `TrackHandle::add_effect` availability. If absent, evaluate Kira 0.10 or abandon runtime effect switching in favor of pre-built filter tracks.
- If adding four Kira tracks to the default bus topology causes audible artifacts (clicks, pops) on any platform during crossfade transitions, simplify to two tracks (music + everything else) before proceeding with the full graph.

## Consequences

### Positive
- Volume channels actually work: `set_volume("music", 0.3)` immediately affects all playing music.
- Adaptive music becomes audible: layer volume changes propagate to Kira handles.
- The "menu muffle" low-pass filter from the audio spec (audio.md line 157-163) becomes implementable.
- Signal routing enables future features: reverb sends, dialogue ducking, spatial audio buses with shared effects.
- Tidal-generated audio can flow through effects and mixing just like hand-authored SFX.

### Negative / Trade-offs
- Additional abstraction layer over Kira adds complexity and a small amount of latency (expected < 1 ms per bus hop).
- The `audio_graph` feature is off by default. Games not using it still get the legacy linear playback path, which means two code paths to maintain.
- Kira's track-based routing has a fixed topology per track once created. Dynamic graph rewiring (adding/removing nodes at runtime) requires destroying and recreating tracks, which may cause brief audio gaps.
- Testing audio graphs programmatically is difficult -- most verification requires listening. CI tests can only check that the graph builds and routes produce no panics, not that the audio sounds correct.

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
