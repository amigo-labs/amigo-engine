# ADR-0003: Rollback Netcode over Deterministic Lockstep State

- Status: accepted
- Date: 2026-06-09

## Context

Multiplayer for fast-paced pixel games needs low perceived latency. Classic
lockstep stalls every player until all inputs arrive; full state replication
(server-authoritative snapshots) costs bandwidth and still shows latency on
local actions. Because the simulation layer is already deterministic
(ADR-0001), sending only inputs and re-simulating is viable.

## Decision

`amigo_net` implements input-based rollback (GGPO-style):

- Each peer simulates ahead using predicted inputs for remote players
  (`RollbackSession::predict_input`, last-known-input prediction).
- Snapshots of game state are stored per tick in a ring buffer
  (`save_snapshot`/`restore_snapshot`, tick-tagged to detect overwrites).
- When a late input contradicts a prediction, the session restores the
  snapshot at the divergence tick and re-simulates to the present.
- CRC32 state checksums (`checksum::StateHasher`) are exchanged to detect
  desyncs early.
- Transport is connectionless UDP with a small JSON-encoded packet protocol
  (`protocol::Packet`, kinds Connect/Accept/Commands/Broadcast/...), with
  heartbeats and timeout-based disconnects. Malformed datagrams are dropped,
  never trusted (see udp.rs tests).

The game integrates by implementing `RollbackState` (snapshot / restore /
simulate_tick), keeping the engine agnostic of game state layout.

## Consequences

- Local input is applied immediately; remote corrections appear as small
  rollbacks instead of input delay.
- Re-simulation cost bounds the rollback window: `simulate_tick` must be fast
  enough to run several times per frame in the worst case.
- Snapshots must capture the full deterministic state; anything outside the
  snapshot (render-only state) must be derivable or cosmetic.
- JSON packet encoding is simple and debuggable but not bandwidth-optimal; a
  binary codec is a possible future optimization (delta encoding already
  exists for position sync in `sync.rs`).
