//! Deterministic lockstep for two players.
//!
//! Both machines run the same simulation, so they only have to agree on the
//! input. Each side schedules its local input `input_delay` ticks ahead,
//! sends it to the other (repeating every input the peer has not
//! acknowledged yet, so a lost packet costs nothing as long as a later one
//! arrives), and runs a tick only once it has both players' input for it.
//! After each tick both sides exchange a hash of their state; the first tick
//! whose hashes differ is reported as a desync.
//!
//! [`LockstepSession`] is the bookkeeping only: it never touches a socket
//! or a clock. Feed it bytes from a [`Link`](crate::peer::Link) and the
//! current time in milliseconds; [`peer`](crate::peer) has a UDP link and an
//! in-memory one for tests.
//!
//! Ticks travel as `u32`, which lasts 2.2 years at 60 ticks a second.

use crate::PlayerId;
use crate::wire::{ByteReader, ByteWriter, WireError};
use amigo_core::{Fix, SimVec2};
use std::collections::BTreeMap;

/// Most inputs one message repeats. 24 inputs of at most 33 bytes plus the
/// checksums stay under [`MAX_PAYLOAD_SIZE`](crate::protocol::MAX_PAYLOAD_SIZE).
pub const MAX_REDUNDANCY: usize = 24;

/// The most state hashes one message carries. Hashes are sent from the
/// oldest one the peer has not acknowledged, so none is ever skipped.
const CHECKSUM_WINDOW: usize = 16;

/// The largest input delay a session accepts: one second.
pub const MAX_INPUT_DELAY: u32 = 60;

/// Inputs further ahead of what we have than this are refused: no honest
/// peer runs that far ahead, and storing them would let one fill memory.
const MAX_TICKS_AHEAD: u64 = 4096;

/// Compared or stale hashes older than this many ticks are dropped.
const CHECKSUM_HISTORY: u64 = 256;

/// One player's input for one tick: bound actions as bit sets (bit `i` is
/// the `i`-th action of a table both sides share), and optionally a cursor
/// in world coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetInput {
    pub held: u64,
    pub pressed: u64,
    pub released: u64,
    pub cursor: Option<SimVec2>,
}

impl NetInput {
    fn write(&self, w: &mut ByteWriter) {
        w.u64(self.held).u64(self.pressed).u64(self.released);
        match self.cursor {
            None => {
                w.u8(0);
            }
            Some(c) => {
                w.u8(1).i32(c.x.to_bits()).i32(c.y.to_bits());
            }
        }
    }

    fn read(r: &mut ByteReader<'_>) -> Result<Self, WireError> {
        let held = r.u64()?;
        let pressed = r.u64()?;
        let released = r.u64()?;
        let cursor = match r.u8()? {
            0 => None,
            1 => Some(SimVec2::new(
                Fix::from_bits(r.i32()?),
                Fix::from_bits(r.i32()?),
            )),
            _ => return Err(WireError::Invalid("cursor flag")),
        };
        Ok(Self {
            held,
            pressed,
            released,
            cursor,
        })
    }
}

/// Settings for one side of a session.
#[derive(Clone, Debug)]
pub struct LockstepConfig {
    /// This side's player: 0 (the host) or 1.
    pub local: PlayerId,
    /// Ticks between sampling an input and running it. More hides more
    /// latency; 3 ticks (50 ms) suits a LAN or a good connection. At most
    /// [`MAX_INPUT_DELAY`].
    pub input_delay: u32,
    /// Unacknowledged inputs each message repeats, up to
    /// [`MAX_REDUNDANCY`].
    pub redundancy: usize,
    /// Milliseconds without a message before the peer counts as gone.
    pub timeout_ms: u64,
}

impl Default for LockstepConfig {
    fn default() -> Self {
        Self {
            local: PlayerId(0),
            input_delay: 3,
            redundancy: 16,
            timeout_ms: 5000,
        }
    }
}

/// What a session knows about the connection, from
/// [`LockstepSession::status`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LockstepStatus {
    pub local: u32,
    pub input_delay: u32,
    /// Round trip in milliseconds, once measured.
    pub rtt_ms: Option<u32>,
    /// Every tick below this has the peer's input.
    pub remote_tick: u64,
    /// Ticks that had to wait for the peer's input at least once.
    pub stalled_ticks: u64,
    /// The first tick whose state hash differed between the two sides.
    pub desync_tick: Option<u64>,
    /// Milliseconds since the last message from the peer.
    pub silent_ms: Option<u64>,
    /// No message for longer than the timeout.
    pub timed_out: bool,
}

/// One side of a two-player lockstep session. See the [module docs](self).
#[derive(Debug)]
pub struct LockstepSession {
    config: LockstepConfig,
    /// First tick with real input; earlier ticks run on empty input.
    first_input_tick: u64,
    local_inputs: BTreeMap<u64, NetInput>,
    /// Next tick a local input will be scheduled for.
    local_next: u64,
    remote_inputs: BTreeMap<u64, NetInput>,
    /// Every remote input below this has arrived.
    remote_next: u64,
    /// The peer has every local input below this.
    peer_ack: u64,
    /// The peer has every one of our state hashes below this.
    peer_hash_ack: u64,
    /// We have every one of the peer's state hashes below this.
    hash_next: u64,
    /// Every tick below this has been simulated here.
    simulated: u64,
    checksums: BTreeMap<u64, u64>,
    peer_checksums: BTreeMap<u64, u64>,
    desync_tick: Option<u64>,
    last_ping: Option<u32>,
    rtt_ms: Option<u32>,
    last_heard_ms: Option<u64>,
    started_ms: Option<u64>,
    stalled_ticks: u64,
    last_stalled: Option<u64>,
}

impl LockstepSession {
    /// A session whose first tick is `start_tick`. Both sides must start at
    /// the same tick with the same `input_delay`.
    pub fn new(mut config: LockstepConfig, start_tick: u64) -> Self {
        config.redundancy = config.redundancy.clamp(1, MAX_REDUNDANCY);
        config.input_delay = config.input_delay.min(MAX_INPUT_DELAY);
        let first_input_tick = start_tick + u64::from(config.input_delay);
        Self {
            config,
            first_input_tick,
            local_inputs: BTreeMap::new(),
            local_next: first_input_tick,
            remote_inputs: BTreeMap::new(),
            remote_next: first_input_tick,
            peer_ack: first_input_tick,
            peer_hash_ack: start_tick,
            hash_next: start_tick,
            simulated: start_tick,
            checksums: BTreeMap::new(),
            peer_checksums: BTreeMap::new(),
            desync_tick: None,
            last_ping: None,
            rtt_ms: None,
            last_heard_ms: None,
            started_ms: None,
            stalled_ticks: 0,
            last_stalled: None,
        }
    }

    pub fn config(&self) -> &LockstepConfig {
        &self.config
    }

    /// The other player.
    pub fn remote(&self) -> PlayerId {
        PlayerId(1 - self.config.local.0.min(1))
    }

    /// Schedule the local input sampled while about to run `tick`; it runs
    /// at `tick + input_delay`. Calling again for the same tick (say, while
    /// stalled) keeps the first input. Returns whether it was stored.
    pub fn add_local_input(&mut self, tick: u64, input: NetInput) -> bool {
        let target = tick + u64::from(self.config.input_delay);
        if target < self.local_next {
            return false;
        }
        // Ticks skipped by the caller repeat this input, so both sides
        // still see an unbroken sequence.
        while self.local_next <= target {
            self.local_inputs.insert(self.local_next, input);
            self.local_next += 1;
        }
        true
    }

    /// Both players' inputs for `tick`, indexed by player id, or `None`
    /// while one is missing: the caller must not run the tick yet.
    pub fn inputs_for(&mut self, tick: u64) -> Option<[NetInput; 2]> {
        let local = self.input_at(&self.local_inputs, tick);
        let remote = self.input_at(&self.remote_inputs, tick);
        match (local, remote) {
            (Some(local), Some(remote)) => Some(if self.config.local.0 == 0 {
                [local, remote]
            } else {
                [remote, local]
            }),
            _ => {
                if self.last_stalled != Some(tick) {
                    self.last_stalled = Some(tick);
                    self.stalled_ticks += 1;
                }
                None
            }
        }
    }

    fn input_at(&self, inputs: &BTreeMap<u64, NetInput>, tick: u64) -> Option<NetInput> {
        if tick < self.first_input_tick {
            return Some(NetInput::default());
        }
        inputs.get(&tick).copied()
    }

    /// Report the state hash after running `tick`. Marks the tick as done,
    /// so inputs nobody needs any more are dropped.
    pub fn record_checksum(&mut self, tick: u64, hash: u64) {
        self.simulated = self.simulated.max(tick + 1);
        self.checksums.insert(tick, hash);
        self.compare(tick);
        self.prune();
    }

    fn compare(&mut self, tick: u64) {
        if let (Some(a), Some(b)) = (self.checksums.get(&tick), self.peer_checksums.get(&tick))
            && a != b
        {
            self.desync_tick = Some(self.desync_tick.map_or(tick, |d| d.min(tick)));
        }
    }

    fn prune(&mut self) {
        // Local inputs are kept until both the peer has them and this side
        // has run them.
        let keep_local = self.peer_ack.min(self.simulated);
        self.local_inputs = self.local_inputs.split_off(&keep_local);
        self.remote_inputs = self.remote_inputs.split_off(&self.simulated);
        // Our hashes stay until the peer has them, however long that takes.
        let keep_hashes = self.simulated.saturating_sub(CHECKSUM_HISTORY);
        self.checksums = self
            .checksums
            .split_off(&keep_hashes.min(self.peer_hash_ack));
        self.peer_checksums = self.peer_checksums.split_off(&keep_hashes);
        self.hash_next = self.hash_next.max(keep_hashes);
    }

    /// The message to send now: every local input the peer has not
    /// acknowledged (up to `redundancy`), what we have of theirs, a ping,
    /// and the state hashes the peer has not acknowledged, oldest first.
    pub fn outgoing(&mut self, now_ms: u64) -> Vec<u8> {
        self.started_ms.get_or_insert(now_ms);
        let first = self.peer_ack.max(self.first_input_tick);
        let inputs: Vec<NetInput> = self
            .local_inputs
            .range(first..self.local_next)
            .take(self.config.redundancy)
            .map(|(_, input)| *input)
            .collect();
        let hashes: Vec<(u64, u64)> = self
            .checksums
            .range(self.peer_hash_ack..)
            .take(CHECKSUM_WINDOW)
            .map(|(&t, &h)| (t, h))
            .collect();

        let mut w = ByteWriter::with_capacity(64 + inputs.len() * 33);
        w.u32(first as u32).u8(inputs.len() as u8);
        for input in &inputs {
            input.write(&mut w);
        }
        w.u32(self.remote_next as u32)
            .u32(self.hash_next as u32)
            .u32(now_ms as u32);
        match self.last_ping {
            Some(ping) => w.u8(1).u32(ping),
            None => w.u8(0),
        };
        w.u8(hashes.len() as u8);
        for (tick, hash) in hashes {
            w.u32(tick as u32).u64(hash);
        }
        w.finish()
    }

    /// Take a message from the peer. Duplicates, old and reordered messages
    /// are fine; malformed ones are refused whole.
    pub fn receive(&mut self, data: &[u8], now_ms: u64) -> Result<(), WireError> {
        let mut r = ByteReader::new(data);
        let first = u64::from(r.u32()?);
        let count = usize::from(r.u8()?);
        if count > MAX_REDUNDANCY {
            return Err(WireError::Invalid("input count"));
        }
        if first + count as u64 > self.remote_next + MAX_TICKS_AHEAD {
            return Err(WireError::Invalid("input tick"));
        }
        let mut inputs = Vec::with_capacity(count);
        for _ in 0..count {
            inputs.push(NetInput::read(&mut r)?);
        }
        let ack = u64::from(r.u32()?);
        let hash_ack = u64::from(r.u32()?);
        let ping = r.u32()?;
        let pong = match r.u8()? {
            0 => None,
            1 => Some(r.u32()?),
            _ => return Err(WireError::Invalid("pong flag")),
        };
        let hash_count = usize::from(r.u8()?);
        if hash_count > CHECKSUM_WINDOW {
            return Err(WireError::Invalid("checksum count"));
        }
        let mut hashes = Vec::with_capacity(hash_count);
        for _ in 0..hash_count {
            hashes.push((u64::from(r.u32()?), r.u64()?));
        }
        r.finish()?;
        if ack > self.local_next || hash_ack > self.simulated {
            return Err(WireError::Invalid("ack"));
        }

        // Valid: apply.
        self.last_heard_ms = Some(now_ms);
        for (i, input) in inputs.into_iter().enumerate() {
            let tick = first + i as u64;
            if tick >= self.remote_next {
                self.remote_inputs.entry(tick).or_insert(input);
            }
        }
        while self.remote_inputs.contains_key(&self.remote_next) {
            self.remote_next += 1;
        }
        self.peer_ack = self.peer_ack.max(ack);
        self.peer_hash_ack = self.peer_hash_ack.max(hash_ack);
        self.last_ping = Some(ping);
        if let Some(pong) = pong {
            self.rtt_ms = Some((now_ms as u32).wrapping_sub(pong));
        }
        let oldest = self.simulated.saturating_sub(CHECKSUM_HISTORY);
        for (tick, hash) in hashes {
            if tick >= oldest {
                self.peer_checksums.insert(tick, hash);
                self.compare(tick);
            }
        }
        while self.peer_checksums.contains_key(&self.hash_next) {
            self.hash_next += 1;
        }
        self.prune();
        Ok(())
    }

    /// Whether the peer has been silent for longer than the timeout.
    pub fn is_timed_out(&self, now_ms: u64) -> bool {
        self.silent_ms(now_ms)
            .is_some_and(|silent| silent > self.config.timeout_ms)
    }

    fn silent_ms(&self, now_ms: u64) -> Option<u64> {
        self.last_heard_ms
            .or(self.started_ms)
            .map(|since| now_ms.saturating_sub(since))
    }

    pub fn desync_tick(&self) -> Option<u64> {
        self.desync_tick
    }

    pub fn status(&self, now_ms: u64) -> LockstepStatus {
        LockstepStatus {
            local: self.config.local.0,
            input_delay: self.config.input_delay,
            rtt_ms: self.rtt_ms,
            remote_tick: self.remote_next,
            stalled_ticks: self.stalled_ticks,
            desync_tick: self.desync_tick,
            silent_ms: self.last_heard_ms.map(|h| now_ms.saturating_sub(h)),
            timed_out: self.is_timed_out(now_ms),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::{Link, LinkConditions, LoopbackEnd, loopback_pair};

    /// A toy game: each player walks one unit per tick in the direction of
    /// the bits they hold, and a click teleports them to the cursor.
    #[derive(Clone, Default)]
    struct Toy {
        pos: [SimVec2; 2],
    }

    impl Toy {
        fn step(&mut self, inputs: &[NetInput; 2]) {
            for (pos, input) in self.pos.iter_mut().zip(inputs) {
                if input.held & 1 != 0 {
                    pos.x += Fix::ONE;
                }
                if input.held & 2 != 0 {
                    pos.y -= Fix::ONE;
                }
                if input.pressed & 4 != 0
                    && let Some(cursor) = input.cursor
                {
                    *pos = cursor;
                }
            }
        }

        fn hash(&self) -> u64 {
            let mut h = 0xcbf2_9ce4_8422_2325u64;
            for p in &self.pos {
                for v in [p.x.to_bits(), p.y.to_bits()] {
                    h = (h ^ v as u32 as u64).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
            h
        }
    }

    /// What player `p` presses on `tick`.
    fn script(p: u32, tick: u64) -> NetInput {
        let phase = (tick + u64::from(p) * 17) % 90;
        let held = match phase {
            0..30 => 1,
            30..50 => 2,
            50..60 => 3,
            _ => 0,
        };
        let click = (tick + u64::from(p)).is_multiple_of(41);
        NetInput {
            held,
            pressed: if click { 4 } else { 0 },
            released: 0,
            cursor: click.then(|| SimVec2::from_num(tick as i32 % 50, p as i32 * 7)),
        }
    }

    struct Side {
        session: LockstepSession,
        link: LoopbackEnd,
        toy: Toy,
        tick: u64,
        hashes: Vec<u64>,
        /// Add one unit to player 0 after this tick, to force a desync.
        diverge_at: Option<u64>,
    }

    impl Side {
        fn new(player: u32, link: LoopbackEnd) -> Self {
            Self::with_delay(player, link, 3)
        }

        fn with_delay(player: u32, link: LoopbackEnd, input_delay: u32) -> Self {
            let config = LockstepConfig {
                local: PlayerId(player),
                input_delay,
                ..Default::default()
            };
            Self {
                session: LockstepSession::new(config, 0),
                link,
                toy: Toy::default(),
                tick: 0,
                hashes: Vec::new(),
                diverge_at: None,
            }
        }

        /// One frame: read, try to run a tick, send.
        fn frame(&mut self, now_ms: u64, target: u64) {
            for msg in self.link.recv() {
                self.session.receive(&msg, now_ms).unwrap();
            }
            if self.tick < target {
                let p = self.session.config().local.0;
                self.session
                    .add_local_input(self.tick, script(p, self.tick));
                if let Some(inputs) = self.session.inputs_for(self.tick) {
                    self.toy.step(&inputs);
                    if self.diverge_at == Some(self.tick) {
                        self.toy.pos[0].x += Fix::ONE;
                    }
                    self.session.record_checksum(self.tick, self.toy.hash());
                    self.hashes.push(self.toy.hash());
                    self.tick += 1;
                }
            }
            let msg = self.session.outgoing(now_ms);
            self.link.send(&msg);
        }
    }

    /// Run both sides until each has run `ticks` ticks (and a few frames
    /// more, so the last hashes get across). Returns the frames it took.
    fn run(a: &mut Side, b: &mut Side, ticks: u64, now: &mut u64) -> u64 {
        let mut frames = 0;
        while (a.tick < ticks || b.tick < ticks) && frames < 100 * ticks {
            a.frame(*now, ticks);
            b.frame(*now, ticks);
            a.link.step();
            *now += 16;
            frames += 1;
        }
        for _ in 0..20 {
            a.frame(*now, ticks);
            b.frame(*now, ticks);
            a.link.step();
            *now += 16;
        }
        frames
    }

    fn sides(conditions: LinkConditions) -> (Side, Side) {
        let (ea, eb) = loopback_pair(conditions);
        (Side::new(0, ea), Side::new(1, eb))
    }

    #[test]
    fn a_perfect_link_stays_in_sync() {
        let (mut a, mut b) = sides(LinkConditions::default());
        let mut now = 0;
        run(&mut a, &mut b, 1000, &mut now);
        assert_eq!(a.tick, 1000);
        assert_eq!(a.hashes, b.hashes);
        assert_eq!(a.session.desync_tick(), None);
        assert_eq!(b.session.desync_tick(), None);
    }

    #[test]
    fn a_bad_link_stays_in_sync() {
        let (mut a, mut b) = sides(LinkConditions {
            loss_percent: 20,
            duplicate_percent: 10,
            min_latency: 0,
            max_latency: 6,
            seed: 99,
        });
        let mut now = 0;
        let frames = run(&mut a, &mut b, 1000, &mut now);
        assert_eq!(
            (a.tick, b.tick),
            (1000, 1000),
            "stuck after {frames} frames"
        );
        assert_eq!(a.hashes, b.hashes);
        assert_eq!(a.session.desync_tick(), None);
        assert_eq!(b.session.desync_tick(), None);
        // Latency and loss made it wait, but it never gave up.
        assert!(a.session.status(now).stalled_ticks > 0);
        assert!(a.session.status(now).rtt_ms.is_some());
    }

    #[test]
    fn a_divergent_side_is_reported_at_the_tick_it_diverged() {
        let (mut a, mut b) = sides(LinkConditions::default());
        b.diverge_at = Some(300);
        let mut now = 0;
        run(&mut a, &mut b, 400, &mut now);
        assert_eq!(a.session.desync_tick(), Some(300));
        assert_eq!(b.session.desync_tick(), Some(300));
    }

    #[test]
    fn hashes_lost_in_one_direction_are_resent_until_acknowledged() {
        // With a long input delay one side can run far ahead while its
        // messages are lost. Sending only the latest hashes then skipped
        // the tick where the two sides parted, and the desync was reported
        // later than it happened, or not at all.
        let (ea, eb) = loopback_pair(LinkConditions::default());
        let mut a = Side::with_delay(0, ea, 30);
        let mut b = Side::with_delay(1, eb, 30);
        b.diverge_at = Some(100);
        let mut now = 0;
        run(&mut a, &mut b, 95, &mut now);

        b.link.set_send_loss_percent(Some(100));
        for _ in 0..150 {
            a.frame(now, 1000);
            b.frame(now, 1000);
            a.link.step();
            now += 16;
        }
        assert!(b.tick > 100 + 16 + 1, "b only reached {}", b.tick);

        b.link.set_send_loss_percent(None);
        run(&mut a, &mut b, 400, &mut now);
        assert_eq!(a.session.desync_tick(), Some(100));
        assert_eq!(b.session.desync_tick(), Some(100));
    }

    #[test]
    fn the_input_delay_is_capped() {
        let s = LockstepSession::new(
            LockstepConfig {
                input_delay: 1000,
                ..Default::default()
            },
            0,
        );
        assert_eq!(s.config().input_delay, MAX_INPUT_DELAY);
    }

    #[test]
    fn a_silent_peer_stalls_then_times_out() {
        let (mut a, mut b) = sides(LinkConditions::default());
        let mut now = 0;
        run(&mut a, &mut b, 100, &mut now);
        assert_eq!(a.tick, 100);

        // The peer goes quiet: nothing more runs.
        a.link.set_loss_percent(100);
        for _ in 0..30 {
            a.frame(now, 1000);
            b.frame(now, 1000);
            a.link.step();
            now += 16;
        }
        let stuck = a.tick;
        assert!(stuck < 100 + 2 * u64::from(a.session.config().input_delay) + 2);
        assert!(!a.session.is_timed_out(now));

        // Back again: both catch up and agree.
        a.link.set_loss_percent(0);
        run(&mut a, &mut b, 300, &mut now);
        assert_eq!((a.tick, b.tick), (300, 300));
        assert_eq!(a.hashes, b.hashes);

        a.link.set_loss_percent(100);
        for _ in 0..400 {
            a.frame(now, 1000);
            a.link.step();
            now += 16;
        }
        assert!(a.session.is_timed_out(now));
        assert!(a.session.status(now).timed_out);
    }

    #[test]
    fn malformed_messages_are_refused_whole() {
        let mut s = LockstepSession::new(LockstepConfig::default(), 0);
        let mut peer = LockstepSession::new(
            LockstepConfig {
                local: PlayerId(1),
                ..Default::default()
            },
            0,
        );
        peer.add_local_input(
            0,
            NetInput {
                held: 1,
                ..Default::default()
            },
        );
        let good = peer.outgoing(0);
        assert_eq!(
            s.receive(&good[..good.len() - 1], 0),
            Err(WireError::Truncated)
        );
        let mut long = good.clone();
        long.push(0);
        assert!(s.receive(&long, 0).is_err());
        // Nothing from the refused messages was applied.
        assert_eq!(s.status(0).remote_tick, 3);

        let mut far = ByteWriter::new();
        far.u32(1_000_000).u8(0).u32(0).u32(0).u32(0).u8(0).u8(0);
        assert_eq!(
            s.receive(&far.finish(), 0),
            Err(WireError::Invalid("input tick"))
        );

        s.receive(&good, 0).unwrap();
        assert_eq!(s.status(0).remote_tick, 4);
        s.add_local_input(0, NetInput::default());
        assert_eq!(s.inputs_for(3).unwrap()[1].held, 1);
    }

    #[test]
    fn inputs_wait_for_the_delay_and_never_change() {
        let mut s = LockstepSession::new(LockstepConfig::default(), 0);
        // The first `input_delay` ticks run on empty input...
        assert!(s.inputs_for(0).is_some());
        assert!(s.inputs_for(2).is_some());
        // ...then local input lands three ticks later.
        let jump = NetInput {
            pressed: 1,
            ..Default::default()
        };
        assert!(s.add_local_input(0, jump));
        assert!(!s.add_local_input(0, NetInput::default()), "kept the first");
        assert_eq!(s.inputs_for(3), None, "the peer's input is missing");
        assert_eq!(s.status(0).stalled_ticks, 1);
        assert_eq!(s.inputs_for(3), None);
        assert_eq!(s.status(0).stalled_ticks, 1, "one stall per tick");
    }
}
