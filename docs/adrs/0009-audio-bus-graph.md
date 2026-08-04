# ADR-0009: Node-Based Audio Bus Graph on Kira Tracks

- Status: accepted
- Date: 2026-08-03

## Context

`AudioManager` plays sounds through Kira's main output. That is enough to make
noise, but not to mix: lowering music without touching SFX, ducking ambience
under a stinger, or muffling everything except the UI while a menu is open all
need persistent sub-mixes with their own effects. Applying an effect per sound
instead of per group means re-applying it on every play call and gives no way to
crossfade a whole category.

Kira 0.9 already has sub-mix tracks with routing and per-track effects, so the
question is whether to build a mixer or to expose Kira's.

## Decision

Expose Kira's tracks as a fixed bus topology behind the `audio_graph` feature,
in `crates/amigo_audio/src/graph.rs`:

```text
[sfx_bus] ──────────────────────┐
[music_bus] ── [music_filter] ──┤
[ambient_bus] ──────────────────┼── [master_bus] ── Kira main output
[stinger_bus] ──────────────────┘
```

Buses are created once and identified by an opaque `BusId`. Volume changes go
through Kira tweens, so a change is a ramp rather than a click. A low/high-pass
`FilterNode` sits on the music bus, which is the one effect adaptive music
actually needs (muffling music behind a menu or underwater).

Feature-gated because a game that only plays a few sounds should not pay for the
extra tracks, and because the topology is opinionated.

## Consequences

- Category volume is one call, and applies to sounds already playing.
- The topology is fixed. A game needing an arbitrary graph has to go to Kira
  directly; that is a deliberate trade against an API nobody can predict.
- The bus set (`sfx`, `music`, `ambient`, `stinger`) mirrors the adaptive-music
  design in `docs/specs/ai-pipelines/audiogen.md`. Adding a bus is cheap;
  removing one is a breaking change.
