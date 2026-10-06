---
status: done
crate: amigo_net, amigo_engine
depends_on: ["engine/core", "engine/replays"]
last_updated: 2026-10-06
---

# Networking

## Purpose

Let two players share one game over the network. The simulation is
deterministic (ADR-0001: fixed point, fixed timestep, fixed iteration order, a
seeded RNG), so the two machines only need to agree on each tick's input; then
both compute the same states. That is **lockstep**, and it is the supported
model.

`done` for two-player lockstep, which the engine drives end to end:
- `amigo run --host` / `--join` starts a network game;
- `GameContext` hands each player's input to the game;
- desyncs are reported.

The client/server relay and rollback (below) are not wired into the engine.

## In a game (`amigo_engine::net`)

```sh
amigo run --host 7777                 # player 1 (the host), waits for player 2
amigo run --join 192.168.1.5:7777     # player 2
```

The same is possible through `AMIGO_NET_HOST` / `AMIGO_NET_JOIN`,
`EngineBuilder::host` / `join`, and `[net] input_delay` in `amigo.toml`
(default 3 ticks; the host's value counts).

```rust
impl GameContext {
    pub fn players(&self) -> impl Iterator<Item = PlayerId>;  // [0] alone, [0, 1] online
    pub fn local_player(&self) -> PlayerId;                   // 0 alone or host, 1 guest
    pub fn player_actions(&self, p: PlayerId) -> &ActionState;
    pub fn player_cursor(&self, p: PlayerId) -> Option<SimVec2>;
    pub fn net_status(&self) -> NetStatus;
}
pub enum NetEvent { Connected { local }, Disconnected { reason }, Desync { tick } }
```

- **Start.** The root game's `init` waits until both players are connected; the
  window shows "Waiting for player 2". The guest takes over the host's seed.
- **Input.** Only bound actions and the world cursor travel; raw `ctx.input`
  stays local. `ctx.actions` is the local player's input, delayed exactly like
  the other player's, so single-player code keeps working.
- **Stall.** A tick whose remote input is missing does not run: no `update`,
  no UI rebuild, and the press stays pending until the retry. Ticks requested
  over the API are kept for later.
- **Desync.** After every tick the engine's state hash (tick, `ctx.rng`, entity
  count, `Game::state_hash`) is exchanged. The first difference raises a
  `NetEvent::Desync`.
- **End.** The session ends when the other player closes the game (they send
  a goodbye on shutdown), stays silent for 5 s, or the host refuses the join
  (bindings differ, session full). Then `NetEvent::Disconnected` arrives and
  the game continues with the local player alone.
- **Status.** `engine.get_property {"key": "net"}` reports mode, role, peer,
  RTT, input delay, `remote_tick`, `stalled_ticks`, `desync_tick` and
  `reason`. The F8 debug overlay shows the same.
- **Not during network play:** replay recording and playback, and restoring a
  dev snapshot (both are refused with a warning).

`examples/lockstep_demo` is a minimal two-player game.

## Public API

### Packets (`amigo_net::protocol`, `amigo_net::wire`)

```rust
pub const MAX_PACKET_SIZE: usize = 1200;     // one datagram, under the MTU
pub const MAX_PAYLOAD_SIZE: usize = 1186;
pub const PROTOCOL_VERSION: u8 = 1;

impl Packet {
    pub fn encode(&self) -> Result<Vec<u8>, WireError>;   // TooLarge over 1200 bytes
    pub fn decode(data: &[u8]) -> Result<Packet, WireError>;
}
```

A packet is a 14-byte binary header followed by the payload: magic `AMGO`,
version, kind, sequence, ack and player id. `decode` refuses anything else:
wrong magic, another version, an unknown kind, a short header or more than
1200 bytes. `ByteWriter` and `ByteReader` (little endian, every read
bounds-checked) do the encoding for everything in the crate.

### Lockstep (`amigo_net::lockstep`)

```rust
pub struct NetInput { pub held: u64, pub pressed: u64, pub released: u64, pub cursor: Option<SimVec2> }

pub struct LockstepConfig { pub local: PlayerId, pub input_delay: u32, pub redundancy: usize, pub timeout_ms: u64 }

impl LockstepSession {
    pub fn new(config: LockstepConfig, start_tick: u64) -> Self;
    pub fn add_local_input(&mut self, tick: u64, input: NetInput) -> bool; // runs at tick + input_delay
    pub fn inputs_for(&mut self, tick: u64) -> Option<[NetInput; 2]>;       // None: wait (stall)
    pub fn record_checksum(&mut self, tick: u64, hash: u64);
    pub fn outgoing(&mut self, now_ms: u64) -> Vec<u8>;
    pub fn receive(&mut self, data: &[u8], now_ms: u64) -> Result<(), WireError>;
    pub fn desync_tick(&self) -> Option<u64>;
    pub fn is_timed_out(&self, now_ms: u64) -> bool;
    pub fn status(&self, now_ms: u64) -> LockstepStatus;
}
```

### Links (`amigo_net::peer`)

```rust
pub trait Link { fn send(&mut self, payload: &[u8]); fn recv(&mut self) -> Vec<Vec<u8>>; }

impl UdpPeer {
    pub fn host(bind_addr: &str, settings: SessionSettings) -> io::Result<Self>;
    pub fn join(host_addr: &str, actions_hash: u64, nonce: u64) -> io::Result<Self>;
    pub fn poll(&mut self, now_ms: u64);
    pub fn state(&self) -> &PeerState;            // Waiting, Connected, Rejected(reason), Closed
    pub fn settings(&self) -> Option<SessionSettings>;
    pub fn local_player(&self) -> PlayerId;       // host 0, guest 1
}

pub fn loopback_pair(conditions: LinkConditions) -> (LoopbackEnd, LoopbackEnd);
```

## Behavior

### Lockstep

- **Input delay.** The input sampled before tick `t` runs at `t + input_delay`
  (default 3 ticks, 50 ms; at most `MAX_INPUT_DELAY`, 60). A host with more is
  refused when it starts, since its guest would refuse the session. The first `input_delay` ticks run on empty input on
  both sides.
- **Redundancy.** Every message repeats all local inputs the peer has not
  acknowledged, up to `redundancy` (default 16, at most 24). A lost packet
  costs nothing as long as a later one arrives. Each message also carries:
  - an ack: every input of the peer below this tick has arrived;
  - a ping and the echo of the peer's last ping, for the round trip;
  - the sender's state hashes from the oldest one the peer has not
    acknowledged (up to 16 per message), with an ack for the peer's hashes.
    A hash is resent until it arrives, so no tick goes unchecked, however
    long a one-way loss lasts.
- **Stall.** `inputs_for(t)` returns `None` while either input for `t` is
  missing, and the caller does not run the tick. Nothing is predicted, so
  nothing is ever rolled back.
- **Desync.** After running a tick, the caller reports its state hash with
  `record_checksum`. When both sides have a hash for the same tick and the two
  differ, that tick becomes `desync_tick`.
- **Robustness.**
  - A malformed message is refused whole.
  - Duplicates and old messages are harmless.
  - An input more than 4096 ticks ahead is refused.
  - A peer silent for `timeout_ms` (default 5 s) counts as gone.

### Handshake (`UdpPeer`)

1. The guest sends `Hello { nonce, actions_hash }` every 200 ms.
2. The host answers with one of:
   - `Welcome { nonce, seed, input_delay, actions_hash }`, and both sides start
     at tick 0 with the host's seed;
   - `Reject { reason }`, when the action tables differ or the session is
     full.
3. From then on each side listens only to the other's address. A third party
   gets `Reject` to a `Hello`, and nothing at all otherwise.

### Client/server relay (`udp`, `server`, `client`)

Command relay for a server with up to `max_clients` clients. Each command
batch is JSON inside one binary packet; a batch too large for one packet is
logged and not sent.
- Clients that stay silent past the timeout lose their slot.
- A client accepts packets only from its server.
- Commands arrive tagged with the player id from the packet header.

There is no reliability, ordering or tick alignment, so for gameplay use
lockstep.

### Rollback (`rollback`, feature `rollback_net`, experimental)

GGPO-style prediction with snapshot and resimulation, not driven by anything
in the engine.
- It never predicts more than `max_rollback_frames` past the last fully
  confirmed tick (`RollbackError::Stalled`).
- A correction it cannot apply is an error rather than a silent desync
  (`TooLate`, `SnapshotMissing`).
- Inputs that contradict a confirmed one, or come from unknown players, are
  rejected.
- Checksums are refreshed by a rollback and readable via `checksum(tick)`.
- Prediction uses the player's latest confirmed input at or before the
  predicted tick, so an early or late packet does not skew it.

## Limits

- Exactly two players in lockstep.
- No NAT traversal or relay server: the host's port must be reachable. On a
  LAN, or with a forwarded port, it is.
- No host migration: if one side leaves, the session ends.
- Open from the audit:
  - eng-10: the lobby state machine;
  - eng-15: `PositionDeltaEncoder` assumes lossless delivery.

## Tests

- **`protocol` and `wire`:**
  - round trip;
  - a 1186-byte payload fits, one byte more is refused;
  - malformed, foreign and oversized datagrams are refused.
- **`udp` and `client`:**
  - a client ignores packets from anyone but its server;
  - silent clients lose their slot;
  - a 1 KB command batch arrives (about 300 bytes used to be lost silently).
- **`lockstep` over `loopback_pair`:**
  - 1000 ticks in sync on a perfect link, and with 20 % loss, 10 % duplicates
    and 0–6 steps of latency;
  - a divergence is reported at its exact tick, also after a long one-way loss
    with a 30-tick input delay;
  - a silent peer stalls the game, it resumes afterwards, and the timeout
    fires;
  - malformed messages are refused.
- **`peer`:**
  - the handshake shares the settings;
  - different bindings and a third player are refused;
  - two `UdpPeer`s on loopback stay in lockstep for 1000 ticks;
  - the in-memory link is lossy and repeatable.
- **`rollback`:** stalls at the window, rejects contradicting and foreign
  inputs, and refreshes checksums.
- **`amigo_engine::net`:**
  - two engines over a link with 15 % loss stay in lockstep for 1000 ticks
    (same hash, no desync);
  - different seeds desync at tick 0;
  - a silent peer stalls the host until the timeout, after which it plays
    alone and gets `Disconnected`;
  - alone, there is exactly player 0;
  - actions survive the wire;
  - over UDP, different bindings are refused and the handshake hands over the
    host's seed and input delay.
- **Headless end to end:** two processes of `lockstep_demo` (`--host` and
  `--join`) each run 600 ticks over JSON-RPC and report the same tick,
  `desync_tick: null`.
