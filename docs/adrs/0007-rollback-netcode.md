---
number: "0007"
title: Rollback Netcode
status: done
date: 2026-03-20
---

# ADR-0007: Rollback Netcode

## Status

proposed

## Context

The networking stack in `crates/amigo_net/` provides the building blocks for multiplayer but does not implement rollback:

- **`Transport<C>` trait** (`lib.rs`, line 18-21): abstracts `send(&[C])` and `receive() -> Vec<(PlayerId, Vec<C>)>`. Two implementations exist: `LocalTransport` (line 24-50, zero-overhead singleplayer stub) and `NetworkClient` (`client.rs`, which uses UDP via `UdpSocket`).

- **Protocol** (`protocol.rs`): defines `PacketKind` (Connect, Accept, Disconnect, Heartbeat, Commands, Broadcast), `PacketHeader` with sequence/ack numbers, and `SeqNum` wrapping arithmetic (line 69-85). The wire format is JSON-serialized via serde.

- **`StateHasher`** (`checksum.rs`): accumulates game state into a CRC-32 checksum for desync detection. The game feeds per-tick data (positions, health, etc.) via `write_u32`/`write_i32`/`write_hash`, then calls `finish_crc()`.

- **`DesyncDetector`** (`replay.rs`, line 179-212): stores per-tick checksums in a `FxHashMap<u64, u64>` and compares two detectors to find the first divergent tick.

- **`LobbyManager`** (`lobby.rs`): manages pre-game rooms, player slots, ready state, and match lifecycle. Supports password protection, host migration, and phase transitions (Waiting -> Countdown -> InGame -> Results).

- **`RewindBuffer`** (`crates/amigo_core/src/state_rewind.rs`): a ring buffer of game state snapshots with delta compression (keyframe every N frames, byte-level diff patches in between). Supports `record()`, `rewind_to()`, `step_back()`, `step_forward()`, and `truncate_after_current()` for timeline forking.

- **`NetworkServer`** (`server.rs`): accepts UDP connections, assigns player IDs, and broadcasts commands.

What is missing:

1. **Client-side prediction.** The engine does not run the game locally ahead of confirmed inputs. When remote inputs arrive late, the game stutters.
2. **Rollback.** There is no mechanism to rewind the game state to the last confirmed tick, re-apply corrected inputs, and fast-forward back to the present.
3. **Input delay management.** The protocol sends commands per-tick but has no concept of input delay frames or speculative execution.
4. **State snapshot for rollback.** `RewindBuffer` exists but is designed for the "Braid-style rewind" use case, not for netcode rollback. It serializes via `serde_json` (line 117, 122), which is too slow for per-tick snapshots in a rollback context.

## Decision

Implement **GGPO-style rollback netcode** behind the feature flag `rollback_net`. This depends on AP-06 (cross-platform determinism) for bit-identical simulation.

The architecture:

1. **`RollbackSession`** -- The central coordinator. Manages a ring buffer of N confirmed + M predicted game state snapshots (using a fast binary serializer, not serde_json). Each tick:
   - Receive remote inputs from the transport.
   - If remote inputs arrive for a past tick that differs from the predicted inputs, **rollback**: restore the snapshot at that tick, re-simulate forward with corrected inputs, updating the prediction buffer.
   - If no correction needed, advance normally.
   - Record the current state snapshot for future rollback.

2. **Input prediction.** For each remote player, if their input for the current tick has not arrived, predict it by repeating their last known input. Track prediction accuracy; if predictions are frequently wrong, increase the input delay.

3. **Snapshot strategy.** Instead of serde_json, use `memcpy`-style snapshots of the game state. The game provides a `RollbackState` trait with `snapshot(&self) -> Vec<u8>` and `restore(&mut self, data: &[u8])`. For the ECS, this means snapshotting the relevant SparseSet dense arrays (or archetype columns from AP-01). The existing `RewindBuffer` delta compression (byte-level diffs from `state_rewind.rs`, lines 358-397) can be reused for reducing memory usage of the snapshot ring buffer.

4. **Desync detection.** Each confirmed tick, compute `StateHasher::finish_crc()` and exchange checksums with peers. Compare using `DesyncDetector::compare()`. If a desync is detected, log the first divergent tick and pause the session for debugging.

5. **Integration with Lobby.** `RollbackSession` starts when `Room::start_game()` transitions to `RoomPhase::InGame`. The lobby provides the player list, and the session creates a `Transport` per peer.

### Alternatives Considered

1. **Lockstep networking.** All clients wait for all inputs before advancing. Simpler but causes the game to run at the speed of the slowest connection. Rejected because it produces unacceptable latency for action games.

2. **Server-authoritative with client-side interpolation.** The server runs the simulation; clients only render interpolated state. Eliminates desync entirely but adds a full RTT of input latency and requires a dedicated server. Rejected for a peer-to-peer-first engine.

## Migration Path

1. **Define `RollbackState` trait and `RollbackConfig`** -- Create `crates/amigo_net/src/rollback.rs` with the `RollbackState` trait (`snapshot`, `restore`, `simulate_tick`), `RollbackConfig` (max rollback frames, input delay, snapshot ring buffer size), and `RollbackSession` struct. Gate behind `#[cfg(feature = "rollback_net")]`. Verify: unit test with two `LocalTransport` instances simulating a 2-player session over 100 ticks with zero latency; confirm checksums match.

2. **Implement snapshot ring buffer with fast serialization** -- Instead of `serde_json`, implement a binary snapshot format. For the MVP, use `Vec<u8>` memcpy of serializable game state. Reuse `compute_delta`/`apply_delta` from `state_rewind.rs` for delta-compressed storage. Verify: snapshot + restore roundtrip of a 10k-entity world takes <1ms.

3. **Implement rollback + resimulation** -- When remote inputs arrive that disagree with predictions, restore the snapshot at the last confirmed tick, re-apply corrected inputs, and fast-forward. Track the number of resimulated frames per rollback. Verify: integration test with artificial 50ms latency (injected via `thread::sleep` in a test transport) confirms the game advances smoothly with rollbacks occurring and resolving correctly.

4. (rough) Add input prediction (repeat last input) and adaptive input delay.
5. (rough) Integrate checksum exchange with `StateHasher` and `DesyncDetector` for per-tick desync detection.
6. (rough) Add spectator mode: a read-only `RollbackSession` that receives all inputs but does not send any.
7. (rough) Add rollback debug overlay (F8 toggle, extending the existing `show_network_debug` in `engine.rs` line 590) showing rollback depth, prediction accuracy, and desync status.

## Abort Criteria

- If rollback of more than **3 frames** at 100ms network latency causes visible stutter (frame time spike >16ms), the snapshot/restore is too slow.
- If the snapshot ring buffer exceeds **50MB of memory** for a typical 10k-entity game with 10 frames of rollback history, the memory cost is too high.
- If AP-06 (cross-platform determinism) is not completed, rollback netcode cannot ship, as simulations will desync across platforms.

## Consequences

### Positive
- Responsive multiplayer: local inputs are processed immediately with zero perceived latency.
- Graceful degradation under packet loss: predicted inputs fill the gap, rollback corrects when real inputs arrive.
- Leverages existing infrastructure: `StateHasher`, `DesyncDetector`, `RewindBuffer` delta compression, `LobbyManager`, `Transport` trait.
- Spectator and replay support come naturally from the input-recording architecture.

### Negative / Trade-offs
- CPU cost: rollback requires re-simulating up to N frames per tick, multiplying the per-tick compute cost.
- Memory cost: storing N snapshots of the full game state (mitigated by delta compression).
- Complexity: rollback netcode is notoriously difficult to debug; desync issues can be subtle.
- Hard dependency on AP-06 (determinism): any non-deterministic code path causes multiplayer desync.
- JSON wire format in `protocol.rs` (`Packet::encode`/`decode`, lines 59-65) may need to be replaced with a binary format for bandwidth efficiency.

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
