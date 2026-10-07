//! Two players over the network, in lockstep.
//!
//! `amigo run --host 7777` on one machine and `amigo run --join
//! <host>:7777` on the other start the same game on both. The host picks
//! the seed and input delay; the root game's `init` runs on both once they
//! are connected. From then on every tick runs on both players' input:
//! - each side sends its bound actions (and cursor) `input_delay` ticks
//!   ahead;
//! - a tick waits until both inputs are there;
//! - the state hashes are compared after every tick.
//!
//! Game code reads the players through [`GameContext::players`],
//! [`GameContext::player_actions`] and [`GameContext::player_cursor`]. The
//! same code runs alone, as player 0 with the local input. Raw `ctx.input`
//! is never sent: keys only count through bound actions.
//!
//! The protocol lives in [`amigo_net::lockstep`] and [`amigo_net::peer`];
//! `docs/specs/engine/networking.md` describes it.

use crate::stack::GameStack;
use crate::{GameContext, replay};
use amigo_core::{Fix, SimVec2};
use amigo_input::{ActionSnapshot, ActionState};
use amigo_net::PlayerId;
use amigo_net::lockstep::{LockstepConfig, LockstepSession, NetInput};
use amigo_net::peer::{Link, LoopbackEnd, PeerState, SessionSettings, UdpPeer};
use serde::Serialize;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Bound actions a network game can have: one bit each in a `u64`.
pub const MAX_ACTIONS: usize = 64;

/// Something happened to the network session. Read it like any event:
/// `ctx.events.read::<NetEvent>()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetEvent {
    /// The other player is there; the game started as `local`.
    Connected { local: PlayerId },
    /// The session ended. The game keeps running with the local player
    /// alone, so `update` can show a message or go back to a menu.
    Disconnected { reason: String },
    /// The two machines computed different states after `tick`. The game
    /// keeps running, but the two sides no longer see the same thing.
    Desync { tick: u64 },
}

/// The network session, from [`GameContext::net_status`] or
/// `engine.get_property {"key": "net"}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct NetStatus {
    pub mode: NetMode,
    /// `"host"` or `"guest"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub local_player: u32,
    /// The other player's address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    pub input_delay: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u32>,
    /// Every tick below this has the other player's input.
    pub remote_tick: u64,
    /// Ticks that had to wait for the other player.
    pub stalled_ticks: u64,
    pub desync_tick: Option<u64>,
    /// Why the session ended.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetMode {
    /// Single player.
    #[default]
    Off,
    /// Waiting for the other player.
    Connecting,
    Playing,
    /// The session ended; see `reason`.
    Ended,
}

/// Network play requested at launch, by `AMIGO_NET_HOST` /
/// `AMIGO_NET_JOIN` or the builder.
#[derive(Clone, Debug, Default)]
pub(crate) struct LaunchNet {
    /// Address to host on; a bare port means every interface.
    pub(crate) host: Option<String>,
    /// The host's `address:port`.
    pub(crate) join: Option<String>,
}

impl LaunchNet {
    pub(crate) fn from_env() -> Self {
        let var = |key| std::env::var(key).ok().filter(|v| !v.is_empty());
        Self {
            host: var("AMIGO_NET_HOST"),
            join: var("AMIGO_NET_JOIN"),
        }
    }

    pub(crate) fn requested(&self) -> bool {
        self.host.is_some() || self.join.is_some()
    }
}

/// A link the engine can drive: polled once per frame, may close.
trait EngineLink: Link {
    fn poll(&mut self, _now_ms: u64) {}
    fn closed(&self) -> bool {
        false
    }
    fn peer_label(&self) -> Option<String> {
        None
    }
    /// Say goodbye, so the other side does not wait for the timeout.
    fn close(&mut self) {}
}

impl EngineLink for UdpPeer {
    fn poll(&mut self, now_ms: u64) {
        UdpPeer::poll(self, now_ms);
    }
    fn closed(&self) -> bool {
        *self.state() == PeerState::Closed
    }
    fn peer_label(&self) -> Option<String> {
        self.peer_addr().map(|a| a.to_string())
    }
    fn close(&mut self) {
        UdpPeer::close(self);
    }
}

impl EngineLink for LoopbackEnd {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Host,
    Guest,
}

impl Role {
    fn player(self) -> PlayerId {
        match self {
            Role::Host => PlayerId(0),
            Role::Guest => PlayerId(1),
        }
    }

    fn label(self) -> String {
        match self {
            Role::Host => "host".into(),
            Role::Guest => "guest".into(),
        }
    }
}

/// The engine's network state, kept on the [`GameContext`].
pub(crate) struct NetDriver {
    state: State,
    clock: Clock,
    /// Bound action names, sorted: bit `i` of a [`NetInput`] is `table[i]`.
    table: Vec<String>,
    /// Both players' actions and cursors for the current tick.
    actions: [ActionState; 2],
    cursors: [Option<SimVec2>; 2],
    ended: Option<(Role, String)>,
}

enum State {
    Off,
    Connecting { peer: Box<UdpPeer>, role: Role },
    Playing(Box<Playing>),
}

struct Playing {
    link: Box<dyn EngineLink>,
    session: LockstepSession,
    role: Role,
    desync_reported: bool,
}

enum Clock {
    Real(Instant),
    #[cfg(test)]
    Manual(std::rc::Rc<std::cell::Cell<u64>>),
}

impl Clock {
    fn now_ms(&self) -> u64 {
        match self {
            Clock::Real(epoch) => epoch.elapsed().as_millis() as u64,
            #[cfg(test)]
            Clock::Manual(t) => t.get(),
        }
    }
}

impl Default for NetDriver {
    fn default() -> Self {
        Self {
            state: State::Off,
            clock: Clock::Real(Instant::now()),
            table: Vec::new(),
            actions: Default::default(),
            cursors: [None, None],
            ended: None,
        }
    }
}

impl NetDriver {
    /// A session is connecting or running.
    pub(crate) fn is_active(&self) -> bool {
        !matches!(self.state, State::Off)
    }

    pub(crate) fn is_playing(&self) -> bool {
        matches!(self.state, State::Playing(_))
    }

    pub(crate) fn local_player(&self) -> PlayerId {
        match &self.state {
            State::Playing(p) => p.session.config().local,
            _ => PlayerId(0),
        }
    }

    pub(crate) fn player_actions(&self, player: PlayerId) -> Option<&ActionState> {
        match self.state {
            State::Playing(_) => self.actions.get(player.0 as usize),
            _ => None,
        }
    }

    pub(crate) fn player_cursor(&self, player: PlayerId) -> Option<SimVec2> {
        self.cursors.get(player.0 as usize).copied().flatten()
    }

    pub(crate) fn status(&self) -> NetStatus {
        let now = self.clock.now_ms();
        match &self.state {
            State::Off => match &self.ended {
                Some((role, reason)) => NetStatus {
                    mode: NetMode::Ended,
                    role: Some(role.label()),
                    local_player: role.player().0,
                    reason: Some(reason.clone()),
                    ..Default::default()
                },
                None => NetStatus::default(),
            },
            State::Connecting { peer, role } => NetStatus {
                mode: NetMode::Connecting,
                role: Some(role.label()),
                local_player: peer.local_player().0,
                peer: peer.peer_addr().map(|a| a.to_string()),
                ..Default::default()
            },
            State::Playing(p) => {
                let s = p.session.status(now);
                NetStatus {
                    mode: NetMode::Playing,
                    role: Some(p.role.label()),
                    local_player: s.local,
                    peer: p.link.peer_label(),
                    input_delay: s.input_delay,
                    rtt_ms: s.rtt_ms,
                    remote_tick: s.remote_tick,
                    stalled_ticks: s.stalled_ticks,
                    desync_tick: s.desync_tick,
                    reason: None,
                }
            }
        }
    }

    /// What to show while waiting for the other player, if waiting.
    pub(crate) fn waiting_message(&self) -> Option<String> {
        let State::Connecting { peer, role } = &self.state else {
            return None;
        };
        Some(match role {
            Role::Host => match peer.local_addr() {
                Ok(addr) => format!("Waiting for player 2 (port {})...", addr.port()),
                Err(_) => "Waiting for player 2...".into(),
            },
            Role::Guest => match peer.peer_addr() {
                Some(addr) => format!("Joining {addr}..."),
                None => "Joining...".into(),
            },
        })
    }
}

/// The sorted action names of `ctx.bindings`, and their hash, which both
/// sides must agree on.
fn action_table(ctx: &GameContext) -> Result<(Vec<String>, u64), String> {
    let mut table: Vec<String> = ctx.bindings.bindings.keys().cloned().collect();
    table.sort_unstable();
    if table.len() > MAX_ACTIONS {
        return Err(format!(
            "{} bound actions; network play supports at most {MAX_ACTIONS}",
            table.len()
        ));
    }
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for name in &table {
        for byte in name.bytes().chain([0]) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    Ok((table, hash))
}

/// Start hosting or joining as `launch` asks. The root game's `init` waits
/// for [`poll_connect`] to report the other player.
pub(crate) fn launch(ctx: &mut GameContext, launch: &LaunchNet, input_delay: u32) {
    let result = action_table(ctx).and_then(|(table, actions_hash)| {
        let (peer, role) = if let Some(host) = &launch.host {
            if launch.join.is_some() {
                warn!("Both --host and --join were given; hosting");
            }
            let addr = if host.bytes().all(|b| b.is_ascii_digit()) {
                format!("0.0.0.0:{host}")
            } else {
                host.clone()
            };
            let settings = SessionSettings {
                seed: ctx.seed(),
                input_delay,
                actions_hash,
            };
            let peer = UdpPeer::host(&addr, settings).map_err(|e| format!("{addr}: {e}"))?;
            (peer, Role::Host)
        } else if let Some(join) = &launch.join {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1);
            let peer =
                UdpPeer::join(join, actions_hash, nonce).map_err(|e| format!("{join}: {e}"))?;
            (peer, Role::Guest)
        } else {
            return Ok(());
        };
        ctx.net.table = table;
        ctx.net.state = State::Connecting {
            peer: Box::new(peer),
            role,
        };
        Ok(())
    });
    if let Err(e) = result {
        warn!("Network play could not start ({e}); playing alone");
        ctx.net.ended = Some((
            if launch.host.is_some() {
                Role::Host
            } else {
                Role::Guest
            },
            e,
        ));
    }
}

/// Move the handshake along. Returns `true` once the root game may start:
/// connected, refused (then alone), or no network play at all.
pub(crate) fn poll_connect(ctx: &mut GameContext) -> bool {
    let now = ctx.net.clock.now_ms();
    let State::Connecting { peer, role } = &mut ctx.net.state else {
        return true;
    };
    peer.poll(now);
    let role = *role;
    match peer.state().clone() {
        PeerState::Waiting => false,
        PeerState::Connected => {
            let Some(settings) = peer.settings() else {
                return false;
            };
            let local = peer.local_player();
            let State::Connecting { peer, .. } = std::mem::replace(&mut ctx.net.state, State::Off)
            else {
                unreachable!()
            };
            if role == Role::Guest {
                ctx.reseed(settings.seed);
            }
            info!(
                "Network play as player {} (seed {}, input delay {} ticks)",
                local.0, settings.seed, settings.input_delay
            );
            start(ctx, peer, local, settings.input_delay, role);
            true
        }
        PeerState::Rejected(reason) => {
            end(ctx, role, format!("the host refused: {reason}"));
            true
        }
        PeerState::Closed => {
            end(ctx, role, "the other player left".into());
            true
        }
    }
}

/// Run a session over `link` from the current tick.
fn start(
    ctx: &mut GameContext,
    link: Box<dyn EngineLink>,
    local: PlayerId,
    input_delay: u32,
    role: Role,
) {
    let config = LockstepConfig {
        local,
        input_delay,
        ..Default::default()
    };
    ctx.net.state = State::Playing(Box::new(Playing {
        link,
        session: LockstepSession::new(config, ctx.time.tick),
        role,
        desync_reported: false,
    }));
    ctx.events.emit(NetEvent::Connected { local });
}

/// End the session and carry on alone.
fn end(ctx: &mut GameContext, role: Role, reason: String) {
    warn!("Network play ended: {reason}; playing alone");
    ctx.net.state = State::Off;
    ctx.net.ended = Some((role, reason.clone()));
    ctx.events.emit(NetEvent::Disconnected { reason });
}

/// Once per frame: read and send, and notice a peer that left. Keeps the
/// session alive while no tick runs (paused, stalled, a slow frame).
pub(crate) fn frame(ctx: &mut GameContext) {
    if matches!(ctx.net.state, State::Connecting { .. }) {
        poll_connect(ctx);
        return;
    }
    exchange(ctx);
}

/// Receive, check for a timeout, send. Returns whether still playing.
fn exchange(ctx: &mut GameContext) -> bool {
    let now = ctx.net.clock.now_ms();
    let State::Playing(p) = &mut ctx.net.state else {
        return false;
    };
    p.link.poll(now);
    for msg in p.link.recv() {
        if let Err(e) = p.session.receive(&msg, now) {
            debug!("Dropping a malformed lockstep message: {e}");
        }
    }
    let gone = if p.link.closed() {
        Some("the other player left")
    } else if p.session.is_timed_out(now) {
        Some("the other player stopped answering")
    } else {
        None
    };
    if let Some(reason) = gone {
        let role = p.role;
        end(ctx, role, reason.into());
        return false;
    }
    let msg = p.session.outgoing(now);
    p.link.send(&msg);
    true
}

/// On shutdown: tell the other player, so their game goes on alone at once.
pub(crate) fn shutdown(ctx: &mut GameContext) {
    match &mut ctx.net.state {
        State::Playing(p) => p.link.close(),
        State::Connecting { peer, .. } => peer.close(),
        State::Off => {}
    }
}

/// Whether the tick about to run may run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prepared {
    Ready,
    /// The other player's input for this tick has not arrived.
    Stall,
}

/// Before `Game::update` in a network game: schedule the local input,
/// exchange, and set every player's input for this tick, or report that it
/// must wait. `ctx.actions` must hold the live local actions.
pub(crate) fn before_update(ctx: &mut GameContext) -> Prepared {
    let tick = ctx.time.tick;
    let local_input = encode(&ctx.net.table, &ctx.actions, ctx.input.mouse_world_pos());
    if let State::Playing(p) = &mut ctx.net.state {
        p.session.add_local_input(tick, local_input);
    }
    if !exchange(ctx) {
        // Ended just now: this tick runs alone, on the live input.
        return Prepared::Ready;
    }
    let State::Playing(p) = &mut ctx.net.state else {
        return Prepared::Ready;
    };
    let Some(inputs) = p.session.inputs_for(tick) else {
        return Prepared::Stall;
    };
    let local = p.session.config().local.0 as usize;
    for (i, input) in inputs.iter().enumerate() {
        ctx.net.actions[i].restore(&decode(&ctx.net.table, input));
        ctx.net.cursors[i] = input.cursor;
    }
    // Single-player code reads `ctx.actions`: the local player's input, as
    // delayed as everyone else's.
    ctx.actions.restore(&decode(&ctx.net.table, &inputs[local]));
    Prepared::Ready
}

/// After a tick: report its state hash, and the first desync.
pub(crate) fn after_tick(ctx: &mut GameContext, stack: &GameStack) {
    if !ctx.net.is_playing() {
        return;
    }
    let hash = replay::state_hash(ctx, stack.top());
    let tick = ctx.time.tick.saturating_sub(1);
    let now = ctx.net.clock.now_ms();
    let State::Playing(p) = &mut ctx.net.state else {
        return;
    };
    p.session.record_checksum(tick, hash);
    let msg = p.session.outgoing(now);
    p.link.send(&msg);
    if let Some(tick) = p.session.desync_tick()
        && !p.desync_reported
    {
        p.desync_reported = true;
        warn!("Network desync: the two machines differ after tick {tick}");
        ctx.events.emit(NetEvent::Desync { tick });
    }
}

fn encode(table: &[String], actions: &ActionState, cursor: amigo_core::RenderVec2) -> NetInput {
    let mut input = NetInput {
        cursor: Some(SimVec2::new(
            Fix::saturating_from_num(cursor.x),
            Fix::saturating_from_num(cursor.y),
        )),
        ..Default::default()
    };
    for (i, name) in table.iter().enumerate() {
        let bit = 1u64 << i;
        if actions.held(name) {
            input.held |= bit;
        }
        if actions.pressed(name) {
            input.pressed |= bit;
        }
        if actions.released(name) {
            input.released |= bit;
        }
    }
    input
}

fn decode(table: &[String], input: &NetInput) -> ActionSnapshot {
    let names = |bits: u64| {
        table
            .iter()
            .enumerate()
            .filter(|(i, _)| bits & (1u64 << i) != 0)
            .map(|(_, n)| n.clone())
            .collect()
    };
    ActionSnapshot {
        pressed: names(input.pressed),
        held: names(input.held),
        released: names(input.released),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::DrawContext;
    use crate::tick::{TickOutcome, run_tick};
    use crate::{Game, SceneAction};
    use amigo_net::peer::{LinkConditions, loopback_pair};
    use std::cell::Cell;
    use std::rc::Rc;
    use winit::event::ElementState;
    use winit::keyboard::{KeyCode, PhysicalKey};

    /// Each player walks with their own actions and the RNG nudges both.
    #[derive(Default)]
    struct Duel {
        pos: [SimVec2; 2],
        rolls: u64,
    }

    impl Game for Duel {
        fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
            let players: Vec<PlayerId> = ctx.players().collect();
            for p in players {
                let a = ctx.player_actions(p);
                let (right, up, dash) = (a.held("right"), a.held("up"), a.pressed("dash"));
                let pos = &mut self.pos[p.0 as usize];
                if right {
                    pos.x += Fix::ONE;
                }
                if up {
                    pos.y -= Fix::ONE;
                }
                if dash {
                    pos.x += Fix::from_num(ctx.rng.range(1, 6));
                    self.rolls += 1;
                }
            }
            SceneAction::Continue
        }

        fn draw(&self, _ctx: &mut DrawContext) {}

        fn state_hash(&self, _ctx: &GameContext) -> Option<u64> {
            let mut h = self.rolls;
            for p in &self.pos {
                h = h.wrapping_mul(31) ^ (p.x.to_bits() as u32 as u64);
                h = h.wrapping_mul(31) ^ (p.y.to_bits() as u32 as u64);
            }
            Some(h)
        }
    }

    struct Side {
        ctx: GameContext,
        stack: GameStack,
        clock: Rc<Cell<u64>>,
    }

    fn side(link: LoopbackEnd, player: u32, seed: u64) -> Side {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        for (action, key) in [("right", "D"), ("up", "W"), ("dash", "Space")] {
            ctx.bindings.bind_key(action, key);
        }
        ctx.reseed(seed);
        let clock = Rc::new(Cell::new(0));
        ctx.net.clock = Clock::Manual(clock.clone());
        ctx.net.table = action_table(&ctx).unwrap().0;
        let role = if player == 0 { Role::Host } else { Role::Guest };
        start(&mut ctx, Box::new(link), PlayerId(player), 3, role);
        let mut stack = GameStack::new(Box::new(Duel::default()));
        stack.enter_root(&mut ctx);
        Side { ctx, stack, clock }
    }

    fn key(ctx: &mut GameContext, code: KeyCode, down: bool) {
        let state = if down {
            ElementState::Pressed
        } else {
            ElementState::Released
        };
        ctx.input.handle_key_event(PhysicalKey::Code(code), state);
    }

    /// What player `p` does on their `n`-th tick.
    fn play(ctx: &mut GameContext, p: u64, n: u64) {
        match (n + p * 13) % 60 {
            0 => key(ctx, KeyCode::KeyD, true),
            25 => key(ctx, KeyCode::KeyD, false),
            30 => key(ctx, KeyCode::KeyW, true),
            50 => key(ctx, KeyCode::KeyW, false),
            _ => {}
        }
        match (n + p * 7) % 23 {
            3 => key(ctx, KeyCode::Space, true),
            4 => key(ctx, KeyCode::Space, false),
            _ => {}
        }
    }

    /// Frames until both ran `ticks`; each frame tries one tick per side.
    fn run(a: &mut Side, b: &mut Side, ticks: u64, link: &LoopbackEnd) -> Vec<(u64, u64)> {
        let mut hashes = Vec::new();
        let mut sampled = [u64::MAX, u64::MAX];
        for _ in 0..ticks * 50 {
            for (i, s) in [&mut *a, &mut *b].into_iter().enumerate() {
                let t = s.ctx.time.tick;
                if t >= ticks {
                    frame(&mut s.ctx);
                    continue;
                }
                // Feed the scripted input once per tick, not per attempt.
                if sampled[i] != t {
                    play(&mut s.ctx, i as u64, t);
                    sampled[i] = t;
                }
                match run_tick(&mut s.ctx, &mut s.stack, &mut Default::default()) {
                    TickOutcome::Ran => {}
                    TickOutcome::Stalled => {}
                    TickOutcome::Quit => panic!("quit"),
                }
                s.clock.set(s.clock.get() + 16);
            }
            link.step();
            if a.ctx.time.tick >= ticks && b.ctx.time.tick >= ticks {
                break;
            }
        }
        for _ in 0..20 {
            frame(&mut a.ctx);
            frame(&mut b.ctx);
            link.step();
        }
        hashes.push((hash(a), hash(b)));
        hashes
    }

    fn hash(s: &Side) -> u64 {
        replay::state_hash(&s.ctx, s.stack.top())
    }

    #[test]
    fn two_engines_stay_in_lockstep_over_a_bad_link() {
        let (la, lb) = loopback_pair(LinkConditions {
            loss_percent: 15,
            duplicate_percent: 5,
            min_latency: 0,
            max_latency: 4,
            seed: 3,
        });
        let link = la.clone();
        // Different seeds: the guest takes the host's in a real handshake;
        // here both start from the same one.
        let mut a = side(la, 0, 77);
        let mut b = side(lb, 1, 77);
        let end = run(&mut a, &mut b, 1000, &link);
        assert_eq!(a.ctx.time.tick, 1000);
        assert_eq!(b.ctx.time.tick, 1000);
        assert_eq!(end[0].0, end[0].1, "the two sides disagree");
        assert_eq!(a.ctx.net_status().desync_tick, None);
        assert_eq!(b.ctx.net_status().desync_tick, None);
        assert!(
            a.ctx.net_status().stalled_ticks > 0,
            "the link never made it wait"
        );
        assert_eq!(a.ctx.players().count(), 2);
        assert_eq!(b.ctx.local_player(), PlayerId(1));
    }

    #[test]
    fn different_seeds_are_a_desync() {
        let (la, lb) = loopback_pair(LinkConditions::default());
        let link = la.clone();
        let mut a = side(la, 0, 1);
        let mut b = side(lb, 1, 2);
        run(&mut a, &mut b, 200, &link);
        // The engine's own hash includes the RNG state, so a different seed
        // shows up on the very first tick, before anyone rolls a die.
        assert_eq!(a.ctx.net_status().desync_tick, Some(0));
        assert_eq!(b.ctx.net_status().desync_tick, Some(0));
    }

    #[test]
    fn a_silent_peer_stalls_then_the_game_goes_on_alone() {
        let (la, lb) = loopback_pair(LinkConditions::default());
        let link = la.clone();
        let mut a = side(la, 0, 5);
        let mut b = side(lb, 1, 5);
        run(&mut a, &mut b, 100, &link);

        // The guest vanishes: the host stalls...
        drop(b);
        link.set_loss_percent(100);
        let tick = a.ctx.time.tick;
        for _ in 0..20 {
            let _ = run_tick(&mut a.ctx, &mut a.stack, &mut Default::default());
            a.clock.set(a.clock.get() + 16);
        }
        assert!(a.ctx.time.tick <= tick + 4, "ran without the guest's input");
        assert_eq!(a.ctx.net_status().mode, NetMode::Playing);

        // ...until the timeout, then plays alone.
        a.clock.set(a.clock.get() + 10_000);
        let outcome = run_tick(&mut a.ctx, &mut a.stack, &mut Default::default());
        assert_eq!(outcome, TickOutcome::Ran);
        let status = a.ctx.net_status();
        assert_eq!(status.mode, NetMode::Ended);
        assert_eq!(
            status.reason.as_deref(),
            Some("the other player stopped answering")
        );
        assert_eq!(a.ctx.players().count(), 1);
        // The tick's flush has made the event readable.
        let events: Vec<NetEvent> = a.ctx.events.read::<NetEvent>().to_vec();
        assert!(matches!(events.last(), Some(NetEvent::Disconnected { .. })));
    }

    #[test]
    fn alone_there_is_one_player_with_the_local_input() {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        ctx.bindings.bind_key("right", "D");
        key(&mut ctx, KeyCode::KeyD, true);
        ctx.update_actions();
        assert_eq!(ctx.players().collect::<Vec<_>>(), [PlayerId(0)]);
        assert_eq!(ctx.local_player(), PlayerId(0));
        assert!(ctx.player_actions(PlayerId(0)).held("right"));
        assert!(!ctx.player_actions(PlayerId(1)).held("right"));
        assert_eq!(ctx.net_status().mode, NetMode::Off);
    }

    #[test]
    fn actions_survive_the_wire() {
        let table: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        let mut actions = ActionState::new();
        actions.restore(&ActionSnapshot {
            pressed: vec!["c".into()],
            held: vec!["a".into(), "c".into()],
            released: vec!["b".into()],
        });
        let input = encode(&table, &actions, amigo_core::RenderVec2::new(1.5, -2.0));
        assert_eq!(
            (input.held, input.pressed, input.released),
            (0b101, 0b100, 0b010)
        );
        assert_eq!(input.cursor, Some(SimVec2::from_num(1.5f32, -2.0f32)));
        assert_eq!(decode(&table, &input), actions.snapshot());
    }

    #[test]
    fn hosts_and_guests_with_different_bindings_play_alone() {
        let mut host = GameContext::new(320.0, 180.0, "assets");
        host.bindings.bind_key("jump", "Space");
        launch(
            &mut host,
            &LaunchNet {
                host: Some("127.0.0.1:0".into()),
                join: None,
            },
            3,
        );
        let State::Connecting { peer, .. } = &host.net.state else {
            panic!("not hosting");
        };
        let port = peer.local_addr().unwrap().port();

        let mut guest = GameContext::new(320.0, 180.0, "assets");
        guest.bindings.bind_key("fire", "Space");
        launch(
            &mut guest,
            &LaunchNet {
                host: None,
                join: Some(format!("127.0.0.1:{port}")),
            },
            3,
        );
        for _ in 0..200_000 {
            poll_connect(&mut host);
            if poll_connect(&mut guest) {
                break;
            }
            std::thread::yield_now();
        }
        let status = guest.net_status();
        assert_eq!(status.mode, NetMode::Ended);
        assert!(status.reason.unwrap().contains("bindings differ"));
        assert_eq!(host.net_status().mode, NetMode::Connecting);
    }

    #[test]
    fn a_handshake_over_udp_shares_the_seed() {
        let mut host = GameContext::new(320.0, 180.0, "assets");
        host.bindings.bind_key("jump", "Space");
        host.reseed(4242);
        launch(
            &mut host,
            &LaunchNet {
                host: Some("127.0.0.1:0".into()),
                join: None,
            },
            2,
        );
        let State::Connecting { peer, .. } = &host.net.state else {
            panic!("not hosting");
        };
        let port = peer.local_addr().unwrap().port();
        let mut guest = GameContext::new(320.0, 180.0, "assets");
        guest.bindings.bind_key("jump", "Space");
        launch(
            &mut guest,
            &LaunchNet {
                host: None,
                join: Some(format!("127.0.0.1:{port}")),
            },
            9,
        );
        let mut ready = (false, false);
        for _ in 0..200_000 {
            ready.0 |= poll_connect(&mut host);
            ready.1 |= poll_connect(&mut guest);
            if ready == (true, true) {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(ready, (true, true));
        assert_eq!(guest.seed(), 4242);
        let status = guest.net_status();
        assert_eq!((status.mode, status.local_player), (NetMode::Playing, 1));
        // The host's input delay wins.
        assert_eq!(status.input_delay, 2);
        assert_eq!(host.net_status().role.as_deref(), Some("host"));
    }
}
