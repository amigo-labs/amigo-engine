//! Links between the two sides of a [`LockstepSession`]: [`UdpPeer`] over
//! the network, [`loopback_pair`] in memory for tests.
//!
//! [`LockstepSession`]: crate::lockstep::LockstepSession

use crate::PlayerId;
use crate::protocol::{
    Packet, PacketKind, RECV_BUFFER_SIZE, SeqNum, is_ignorable_recv_error, send_packet,
};
use crate::wire::{ByteReader, ByteWriter, WireError};
use amigo_core::SimRng;
use std::cell::RefCell;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::rc::Rc;
use tracing::{debug, info, warn};

/// A datagram link to the other player: unreliable and unordered, like UDP.
pub trait Link {
    /// Send one message. It may be lost, duplicated or reordered.
    fn send(&mut self, payload: &[u8]);
    /// Every message that arrived since the last call.
    fn recv(&mut self) -> Vec<Vec<u8>>;
}

// ---------------------------------------------------------------------------
// In-memory link
// ---------------------------------------------------------------------------

/// How badly a [`loopback_pair`] treats its messages. Latency is in steps
/// ([`LoopbackEnd::step`]); the dice come from `seed`, so a run is
/// repeatable.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinkConditions {
    pub loss_percent: u32,
    pub duplicate_percent: u32,
    pub min_latency: u32,
    pub max_latency: u32,
    pub seed: u64,
}

#[derive(Debug)]
struct Shared {
    now: u64,
    order: u64,
    rng: SimRng,
    conditions: LinkConditions,
    /// Messages on their way to side 0 and side 1:
    /// `(deliver_at, order, payload)`.
    queues: [Vec<(u64, u64, Vec<u8>)>; 2],
}

/// One end of an in-memory link, from [`loopback_pair`].
#[derive(Clone, Debug)]
pub struct LoopbackEnd {
    shared: Rc<RefCell<Shared>>,
    side: usize,
}

/// Two connected in-memory link ends, losing, duplicating and delaying
/// messages as `conditions` say.
pub fn loopback_pair(conditions: LinkConditions) -> (LoopbackEnd, LoopbackEnd) {
    let shared = Rc::new(RefCell::new(Shared {
        now: 0,
        order: 0,
        rng: SimRng::new(conditions.seed),
        conditions,
        queues: [Vec::new(), Vec::new()],
    }));
    (
        LoopbackEnd {
            shared: shared.clone(),
            side: 0,
        },
        LoopbackEnd { shared, side: 1 },
    )
}

impl LoopbackEnd {
    /// Advance the link's clock by one step (for both ends).
    pub fn step(&self) {
        self.shared.borrow_mut().now += 1;
    }

    /// Change the loss rate, e.g. to 100 to cut the link.
    pub fn set_loss_percent(&self, percent: u32) {
        self.shared.borrow_mut().conditions.loss_percent = percent;
    }
}

impl Link for LoopbackEnd {
    fn send(&mut self, payload: &[u8]) {
        let mut s = self.shared.borrow_mut();
        let c = s.conditions;
        if s.rng.below(100) < c.loss_percent {
            return;
        }
        let copies = if s.rng.below(100) < c.duplicate_percent {
            2
        } else {
            1
        };
        for _ in 0..copies {
            let spread = c.max_latency.saturating_sub(c.min_latency);
            let latency = c.min_latency + s.rng.below(spread + 1);
            let deliver_at = s.now + u64::from(latency);
            let order = s.order;
            s.order += 1;
            s.queues[1 - self.side].push((deliver_at, order, payload.to_vec()));
        }
    }

    fn recv(&mut self) -> Vec<Vec<u8>> {
        let mut s = self.shared.borrow_mut();
        let now = s.now;
        let queue = &mut s.queues[self.side];
        let mut due: Vec<(u64, u64, Vec<u8>)> = Vec::new();
        queue.retain(|m| {
            if m.0 <= now {
                due.push(m.clone());
                false
            } else {
                true
            }
        });
        due.sort_by_key(|m| (m.0, m.1));
        due.into_iter().map(|m| m.2).collect()
    }
}

// ---------------------------------------------------------------------------
// UDP peer
// ---------------------------------------------------------------------------

/// What both sides of a session must agree on, set by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionSettings {
    /// Seed for the simulation's RNG.
    pub seed: u64,
    /// Ticks between sampling and running an input.
    pub input_delay: u32,
    /// Hash of the action table: both sides must bind the same actions in
    /// the same order, or bit `i` of an input means different things.
    pub actions_hash: u64,
}

/// Where a [`UdpPeer`] stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeerState {
    /// Host: waiting for a guest. Guest: waiting for the host's answer.
    Waiting,
    Connected,
    /// The host refused us, with its reason.
    Rejected(String),
    /// The other side said goodbye.
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Host,
    Guest,
}

/// How often a guest repeats its Hello until the host answers.
const HELLO_INTERVAL_MS: u64 = 200;

/// A UDP link to exactly one other player, with a handshake.
///
/// The host binds a port and waits; the guest sends `Hello` with a nonce
/// and its action-table hash until the host answers `Welcome` (with the
/// session settings) or `Reject`. From then on only the other side's
/// address is listened to: anyone else gets a `Reject` (to a `Hello`) or
/// nothing at all.
pub struct UdpPeer {
    socket: UdpSocket,
    role: Role,
    state: PeerState,
    peer: Option<SocketAddr>,
    nonce: u64,
    actions_hash: u64,
    settings: Option<SessionSettings>,
    seq: SeqNum,
    last_hello_ms: Option<u64>,
    inbound: Vec<Vec<u8>>,
}

impl UdpPeer {
    /// Host a session on `bind_addr` (e.g. `"0.0.0.0:7777"`) with
    /// `settings`.
    pub fn host(bind_addr: &str, settings: SessionSettings) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_nonblocking(true)?;
        info!("Hosting a session on {}", socket.local_addr()?);
        Ok(Self {
            socket,
            role: Role::Host,
            state: PeerState::Waiting,
            peer: None,
            nonce: 0,
            actions_hash: settings.actions_hash,
            settings: Some(settings),
            seq: SeqNum::default(),
            last_hello_ms: None,
            inbound: Vec::new(),
        })
    }

    /// Join the session hosted at `host_addr` (e.g. `"192.168.1.5:7777"`).
    /// `nonce` tells this attempt apart from an earlier one from the same
    /// address; any value that differs between attempts will do.
    pub fn join(host_addr: &str, actions_hash: u64, nonce: u64) -> std::io::Result<Self> {
        let peer = host_addr.to_socket_addrs()?.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "no address to join")
        })?;
        let bind = if peer.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket = UdpSocket::bind(bind)?;
        socket.set_nonblocking(true)?;
        info!("Joining the session at {peer}");
        Ok(Self {
            socket,
            role: Role::Guest,
            state: PeerState::Waiting,
            peer: Some(peer),
            nonce,
            actions_hash,
            settings: None,
            seq: SeqNum::default(),
            last_hello_ms: None,
            inbound: Vec::new(),
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn state(&self) -> &PeerState {
        &self.state
    }

    /// The other side's address: the host's for a guest from the start, the
    /// guest's for a host once one connected.
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.peer
    }

    /// The session settings: the host's own, or what its `Welcome` said.
    pub fn settings(&self) -> Option<SessionSettings> {
        self.settings
    }

    /// The host is player 0, the guest player 1.
    pub fn local_player(&self) -> PlayerId {
        match self.role {
            Role::Host => PlayerId(0),
            Role::Guest => PlayerId(1),
        }
    }

    /// Read the socket and move the handshake along. Call every frame.
    pub fn poll(&mut self, now_ms: u64) {
        let mut buf = vec![0u8; RECV_BUFFER_SIZE];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((len, src)) => match Packet::decode(&buf[..len]) {
                    Ok(packet) => self.handle(packet, src),
                    Err(e) => debug!("Dropping datagram from {src}: {e}"),
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(ref e) if is_ignorable_recv_error(e) => continue,
                Err(e) => {
                    warn!("UDP recv error: {e}");
                    break;
                }
            }
        }
        if self.role == Role::Guest
            && self.state == PeerState::Waiting
            && self
                .last_hello_ms
                .is_none_or(|t| now_ms.saturating_sub(t) >= HELLO_INTERVAL_MS)
        {
            self.last_hello_ms = Some(now_ms);
            let mut w = ByteWriter::new();
            w.u64(self.nonce).u64(self.actions_hash);
            self.send_kind(PacketKind::Hello, w.finish());
        }
    }

    /// Tell the other side we are leaving.
    pub fn close(&mut self) {
        if self.state == PeerState::Connected {
            self.send_kind(PacketKind::Disconnect, Vec::new());
        }
        self.state = PeerState::Closed;
    }

    fn send_kind(&mut self, kind: PacketKind, payload: Vec<u8>) {
        if let Some(peer) = self.peer {
            self.send_to(peer, kind, payload);
        }
    }

    fn send_to(&mut self, addr: SocketAddr, kind: PacketKind, payload: Vec<u8>) {
        let seq = self.seq.next();
        let packet = Packet::new(kind, seq, 0, self.local_player().0, payload);
        send_packet(&self.socket, addr, &packet);
    }

    fn handle(&mut self, packet: Packet, src: SocketAddr) {
        let from_peer = self.peer == Some(src);
        let result = match (self.role, packet.header.kind) {
            (Role::Host, PacketKind::Hello) => self.on_hello(&packet.payload, src),
            (Role::Guest, PacketKind::Welcome) if from_peer => self.on_welcome(&packet.payload),
            (Role::Guest, PacketKind::Reject) if from_peer => self.on_reject(&packet.payload),
            (_, PacketKind::Lockstep) if from_peer && self.state == PeerState::Connected => {
                self.inbound.push(packet.payload);
                Ok(())
            }
            (_, PacketKind::Disconnect) if from_peer && self.state == PeerState::Connected => {
                info!("The other player left");
                self.state = PeerState::Closed;
                Ok(())
            }
            _ => {
                debug!("Ignoring {:?} from {src}", packet.header.kind);
                Ok(())
            }
        };
        if let Err(e) = result {
            debug!("Malformed {:?} from {src}: {e}", packet.header.kind);
        }
    }

    fn on_hello(&mut self, payload: &[u8], src: SocketAddr) -> Result<(), WireError> {
        let mut r = ByteReader::new(payload);
        let nonce = r.u64()?;
        let actions_hash = r.u64()?;
        r.finish()?;
        let refuse = |peer: &mut Self, reason: &str| {
            let mut w = ByteWriter::new();
            w.u64(nonce).bytes(reason.as_bytes());
            peer.send_to(src, PacketKind::Reject, w.finish());
        };
        match self.state {
            PeerState::Waiting => {}
            // Our Welcome got lost and the guest asks again.
            PeerState::Connected if self.peer == Some(src) && self.nonce == nonce => {}
            _ => {
                refuse(self, "the session is full");
                return Ok(());
            }
        }
        if actions_hash != self.actions_hash {
            warn!("Refusing {src}: its input bindings differ from ours");
            refuse(self, "the input bindings differ from the host's");
            return Ok(());
        }
        let Some(settings) = self.settings else {
            return Ok(());
        };
        if self.state == PeerState::Waiting {
            info!("Player 2 joined from {src}");
        }
        self.peer = Some(src);
        self.nonce = nonce;
        self.state = PeerState::Connected;
        let mut w = ByteWriter::new();
        w.u64(nonce)
            .u64(settings.seed)
            .u32(settings.input_delay)
            .u64(settings.actions_hash);
        self.send_to(src, PacketKind::Welcome, w.finish());
        Ok(())
    }

    fn on_welcome(&mut self, payload: &[u8]) -> Result<(), WireError> {
        let mut r = ByteReader::new(payload);
        let nonce = r.u64()?;
        let seed = r.u64()?;
        let input_delay = r.u32()?;
        let actions_hash = r.u64()?;
        r.finish()?;
        if nonce != self.nonce || self.state != PeerState::Waiting {
            return Ok(());
        }
        if input_delay > 60 {
            return Err(WireError::Invalid("input delay"));
        }
        info!("Joined the session, input delay {input_delay} ticks");
        self.settings = Some(SessionSettings {
            seed,
            input_delay,
            actions_hash,
        });
        self.state = PeerState::Connected;
        Ok(())
    }

    fn on_reject(&mut self, payload: &[u8]) -> Result<(), WireError> {
        let mut r = ByteReader::new(payload);
        let nonce = r.u64()?;
        let reason = String::from_utf8_lossy(r.rest()).into_owned();
        if nonce == self.nonce && self.state == PeerState::Waiting {
            warn!("The host refused us: {reason}");
            self.state = PeerState::Rejected(reason);
        }
        Ok(())
    }
}

impl Link for UdpPeer {
    fn send(&mut self, payload: &[u8]) {
        if self.state == PeerState::Connected {
            self.send_kind(PacketKind::Lockstep, payload.to_vec());
        }
    }

    fn recv(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.inbound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockstep::{LockstepConfig, LockstepSession, NetInput};

    const SETTINGS: SessionSettings = SessionSettings {
        seed: 42,
        input_delay: 2,
        actions_hash: 0xabc,
    };

    fn loopback(addr: SocketAddr) -> String {
        format!("127.0.0.1:{}", addr.port())
    }

    /// Poll both until `done`, without sleeping; panics after `limit`
    /// rounds.
    fn pump(
        host: &mut UdpPeer,
        guest: &mut UdpPeer,
        now: &mut u64,
        done: impl Fn(&UdpPeer, &UdpPeer) -> bool,
    ) {
        for _ in 0..200_000 {
            host.poll(*now);
            guest.poll(*now);
            if done(host, guest) {
                return;
            }
            *now += 1;
            std::thread::yield_now();
        }
        panic!(
            "no progress: host {:?}, guest {:?}",
            host.state(),
            guest.state()
        );
    }

    #[test]
    fn handshake_shares_the_settings() {
        let mut host = UdpPeer::host("127.0.0.1:0", SETTINGS).unwrap();
        let mut guest = UdpPeer::join(
            &loopback(host.local_addr().unwrap()),
            SETTINGS.actions_hash,
            7,
        )
        .unwrap();
        let mut now = 0;
        pump(&mut host, &mut guest, &mut now, |h, g| {
            *h.state() == PeerState::Connected && *g.state() == PeerState::Connected
        });
        assert_eq!(guest.settings(), Some(SETTINGS));
        assert_eq!(
            (host.local_player(), guest.local_player()),
            (PlayerId(0), PlayerId(1))
        );
    }

    #[test]
    fn different_bindings_are_refused_and_a_third_player_too() {
        let mut host = UdpPeer::host("127.0.0.1:0", SETTINGS).unwrap();
        let addr = loopback(host.local_addr().unwrap());
        let mut wrong = UdpPeer::join(&addr, 0xdef, 1).unwrap();
        let mut now = 0;
        pump(&mut host, &mut wrong, &mut now, |_, g| {
            matches!(g.state(), PeerState::Rejected(_))
        });
        assert_eq!(*host.state(), PeerState::Waiting);

        let mut guest = UdpPeer::join(&addr, SETTINGS.actions_hash, 2).unwrap();
        pump(&mut host, &mut guest, &mut now, |h, g| {
            *h.state() == PeerState::Connected && *g.state() == PeerState::Connected
        });
        let mut third = UdpPeer::join(&addr, SETTINGS.actions_hash, 3).unwrap();
        pump(&mut host, &mut third, &mut now, |_, t| {
            *t.state() == PeerState::Rejected("the session is full".into())
        });
        assert_eq!(
            host.peer_addr().map(|a| a.port()),
            guest.local_addr().ok().map(|a| a.port())
        );
    }

    #[test]
    fn two_udp_peers_stay_in_lockstep_for_1000_ticks() {
        let mut host = UdpPeer::host("127.0.0.1:0", SETTINGS).unwrap();
        let mut guest = UdpPeer::join(
            &loopback(host.local_addr().unwrap()),
            SETTINGS.actions_hash,
            9,
        )
        .unwrap();
        let mut now = 0;
        pump(&mut host, &mut guest, &mut now, |h, g| {
            *h.state() == PeerState::Connected && *g.state() == PeerState::Connected
        });

        let config = |p| LockstepConfig {
            local: PlayerId(p),
            input_delay: SETTINGS.input_delay,
            ..Default::default()
        };
        let mut sessions = [
            LockstepSession::new(config(0), 0),
            LockstepSession::new(config(1), 0),
        ];
        let mut ticks = [0u64; 2];
        let mut sums = [0u64; 2];
        let mut hashes: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
        let mut peers = [host, guest];
        for _ in 0..500_000 {
            for i in 0..2 {
                let peer = &mut peers[i];
                peer.poll(now);
                for msg in peer.recv() {
                    sessions[i].receive(&msg, now).unwrap();
                }
                if ticks[i] < 1000 {
                    let t = ticks[i];
                    let input = NetInput {
                        held: (t * (i as u64 + 3)) % 7,
                        ..Default::default()
                    };
                    sessions[i].add_local_input(t, input);
                    if let Some(inputs) = sessions[i].inputs_for(t) {
                        sums[i] = sums[i]
                            .wrapping_mul(31)
                            .wrapping_add(inputs[0].held * 5 + inputs[1].held);
                        sessions[i].record_checksum(t, sums[i]);
                        hashes[i].push(sums[i]);
                        ticks[i] += 1;
                    }
                }
                let msg = sessions[i].outgoing(now);
                peers[i].send(&msg);
            }
            now += 1;
            if ticks == [1000, 1000] {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(ticks, [1000, 1000]);
        assert_eq!(hashes[0], hashes[1]);
        assert_eq!(sessions[0].desync_tick(), None);
        assert_eq!(sessions[1].desync_tick(), None);

        peers[1].close();
        pump_one(&mut peers[0], now);
        assert_eq!(*peers[0].state(), PeerState::Closed);
    }

    fn pump_one(peer: &mut UdpPeer, now: u64) {
        for _ in 0..200_000 {
            peer.poll(now);
            if *peer.state() == PeerState::Closed {
                return;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn the_loopback_link_loses_and_delays_repeatably() {
        let conditions = LinkConditions {
            loss_percent: 30,
            duplicate_percent: 20,
            min_latency: 1,
            max_latency: 4,
            seed: 5,
        };
        let deliveries = || {
            let (mut a, mut b) = loopback_pair(conditions);
            let mut got = Vec::new();
            for i in 0..200u8 {
                a.send(&[i]);
                a.step();
                got.extend(b.recv().into_iter().map(|m| m[0]));
            }
            for _ in 0..5 {
                a.step();
                got.extend(b.recv().into_iter().map(|m| m[0]));
            }
            got
        };
        let got = deliveries();
        assert_eq!(got, deliveries(), "same seed, same deliveries");
        assert!(got.len() < 200, "some were lost");
        assert!(got.windows(2).any(|w| w[0] > w[1]), "some were reordered");
        let mut seen = std::collections::HashSet::new();
        assert!(!got.iter().all(|m| seen.insert(*m)), "some were duplicated");
    }
}
