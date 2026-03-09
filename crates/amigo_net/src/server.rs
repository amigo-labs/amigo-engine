use crate::protocol::{Packet, PacketKind, SeqNum, MAX_PACKET_SIZE};
use crate::{PlayerId, Transport};
use rustc_hash::FxHashMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::net::{SocketAddr, UdpSocket};
use std::time::Instant;
use tracing::{debug, warn};

/// Tracks a connected client on the server side.
struct ClientSlot {
    addr: SocketAddr,
    player_id: PlayerId,
    last_seen: Instant,
    last_ack: u16,
}

/// Authoritative game server that receives commands from clients and
/// broadcasts the combined commands each tick.
pub struct NetworkServer<C> {
    socket: UdpSocket,
    clients: FxHashMap<SocketAddr, ClientSlot>,
    next_player_id: u32,
    local_seq: SeqNum,
    /// Commands buffered from clients this tick, ready for receive().
    inbound: Vec<(PlayerId, Vec<C>)>,
    /// How many seconds of silence before we consider a client timed out.
    pub timeout_secs: f32,
    recv_buf: Vec<u8>,
}

impl<C: Clone + Serialize + DeserializeOwned> NetworkServer<C> {
    /// Bind to the given address (e.g. "0.0.0.0:7777").
    pub fn bind(addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        debug!("Server listening on {}", socket.local_addr()?);
        Ok(Self {
            socket,
            clients: FxHashMap::default(),
            next_player_id: 1,
            local_seq: SeqNum::default(),
            inbound: Vec::new(),
            timeout_secs: 10.0,
            recv_buf: vec![0u8; MAX_PACKET_SIZE],
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Poll the socket for incoming packets. Call once per tick before receive().
    pub fn poll(&mut self) {
        loop {
            match self.socket.recv_from(&mut self.recv_buf) {
                Ok((len, src)) => {
                    if let Some(packet) = Packet::decode(&self.recv_buf[..len]) {
                        self.handle_packet(src, packet);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    warn!("Server recv error: {}", e);
                    break;
                }
            }
        }

        // Timeout stale clients
        let now = Instant::now();
        let timeout = self.timeout_secs;
        self.clients.retain(|addr, slot| {
            let alive = now.duration_since(slot.last_seen).as_secs_f32() < timeout;
            if !alive {
                debug!("Client {} ({}) timed out", slot.player_id.0, addr);
            }
            alive
        });
    }

    fn handle_packet(&mut self, src: SocketAddr, packet: Packet) {
        match packet.header.kind {
            PacketKind::Connect => {
                if self.clients.contains_key(&src) {
                    // Already connected, resend accept
                    let pid = self.clients[&src].player_id;
                    self.send_accept(src, pid);
                    return;
                }
                let pid = PlayerId(self.next_player_id);
                self.next_player_id += 1;
                debug!("Client connected from {}: assigned {:?}", src, pid);
                self.clients.insert(src, ClientSlot {
                    addr: src,
                    player_id: pid,
                    last_seen: Instant::now(),
                    last_ack: 0,
                });
                self.send_accept(src, pid);
            }
            PacketKind::Disconnect => {
                if let Some(slot) = self.clients.remove(&src) {
                    debug!("Client {} disconnected", slot.player_id.0);
                }
            }
            PacketKind::Heartbeat => {
                if let Some(slot) = self.clients.get_mut(&src) {
                    slot.last_seen = Instant::now();
                }
            }
            PacketKind::Commands => {
                if let Some(slot) = self.clients.get_mut(&src) {
                    slot.last_seen = Instant::now();
                    slot.last_ack = packet.header.sequence;
                    if let Ok(commands) = serde_json::from_slice::<Vec<C>>(&packet.payload) {
                        self.inbound.push((slot.player_id, commands));
                    }
                }
            }
            _ => {} // Clients shouldn't send Accept/Broadcast
        }
    }

    fn send_accept(&self, addr: SocketAddr, pid: PlayerId) {
        let payload = pid.0.to_le_bytes().to_vec();
        let pkt = Packet::new(PacketKind::Accept, 0, 0, pid.0, payload);
        if let Some(data) = pkt.encode() {
            let _ = self.socket.send_to(&data, addr);
        }
    }

    /// Broadcast commands from all players to all connected clients.
    pub fn broadcast(&mut self, all_commands: &[(PlayerId, Vec<C>)]) {
        let payload = match serde_json::to_vec(all_commands) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to serialize broadcast: {}", e);
                return;
            }
        };
        let seq = self.local_seq.next();
        let pkt = Packet::new(PacketKind::Broadcast, seq, 0, 0, payload);
        let data = match pkt.encode() {
            Some(d) => d,
            None => return,
        };
        for slot in self.clients.values() {
            let _ = self.socket.send_to(&data, slot.addr);
        }
    }
}

impl<C: Clone + Serialize + DeserializeOwned> Transport<C> for NetworkServer<C> {
    fn send(&mut self, _commands: &[C]) {
        // Server doesn't send commands itself; use broadcast() instead.
    }

    fn receive(&mut self) -> Vec<(PlayerId, Vec<C>)> {
        std::mem::take(&mut self.inbound)
    }
}
