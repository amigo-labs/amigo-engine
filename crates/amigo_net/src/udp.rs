//! UDP transport implementation (RS-05).
//!
//! Provides `UdpTransport` which implements the `Transport` trait using
//! standard library UDP sockets with the engine's packet protocol.

use crate::protocol::{
    Packet, PacketKind, RECV_BUFFER_SIZE, SeqNum, is_ignorable_recv_error, send_packet,
};
use crate::{PlayerId, Transport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Configuration for the UDP transport.
#[derive(Clone, Debug)]
pub struct UdpConfig {
    /// For server mode: address to bind to.
    pub bind_addr: String,
    /// Maximum number of clients (server mode).
    pub max_clients: usize,
    /// Client mode: seconds between heartbeats, and between connect
    /// attempts until the server answers.
    pub heartbeat_interval: f64,
    /// Server mode: seconds of silence after which a client's slot is freed.
    pub timeout: f64,
}

impl Default for UdpConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:7777".into(),
            max_clients: 8,
            heartbeat_interval: 1.0,
            timeout: 10.0,
        }
    }
}

/// A connected client on the server side.
#[derive(Debug)]
struct ClientSlot {
    player_id: PlayerId,
    addr: SocketAddr,
    last_seen: Instant,
    remote_seq: u16,
}

/// UDP transport for real network multiplayer.
///
/// Can operate in **server** or **client** mode:
/// - Server: binds to a port, accepts connections, broadcasts commands.
/// - Client: connects to a server address, sends commands, receives broadcasts.
pub struct UdpTransport<C: Clone + Serialize + for<'de> Deserialize<'de>> {
    socket: UdpSocket,
    mode: UdpMode,
    local_seq: SeqNum,
    config: UdpConfig,
    _marker: std::marker::PhantomData<C>,
}

enum UdpMode {
    Server {
        clients: HashMap<SocketAddr, ClientSlot>,
        next_player_id: u32,
        /// Inbound commands from all clients this frame.
        inbound: Vec<(PlayerId, Vec<u8>)>,
    },
    Client {
        server_addr: SocketAddr,
        player_id: Option<PlayerId>,
        connected: bool,
        /// Inbound broadcasts from the server this frame, with the player
        /// the header names.
        inbound: Vec<(PlayerId, Vec<u8>)>,
        /// When the client last sent anything, for heartbeats and connect
        /// retries.
        last_sent: Instant,
    },
}

impl<C: Clone + Serialize + for<'de> Deserialize<'de>> UdpTransport<C> {
    /// Create a server transport bound to the configured address.
    pub fn bind_server(config: UdpConfig) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(&config.bind_addr)?;
        socket.set_nonblocking(true)?;
        info!("UDP server bound to {}", config.bind_addr);

        Ok(Self {
            socket,
            mode: UdpMode::Server {
                clients: HashMap::new(),
                next_player_id: 1,
                inbound: Vec::new(),
            },
            local_seq: SeqNum(0),
            config,
            _marker: std::marker::PhantomData,
        })
    }

    /// Create a client transport that will connect to the given server.
    pub fn connect_client(server_addr: &str) -> std::io::Result<Self> {
        Self::connect_client_with(server_addr, UdpConfig::default())
    }

    /// [`connect_client`](Self::connect_client) with explicit timing
    /// (`heartbeat_interval` paces heartbeats and connect retries).
    pub fn connect_client_with(server_addr: &str, config: UdpConfig) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        let server_addr: SocketAddr = server_addr
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        info!("UDP client connecting to {}", server_addr);

        send_packet(
            &socket,
            server_addr,
            &Packet::new(PacketKind::Connect, 0, 0, 0, Vec::new()),
        );

        Ok(Self {
            socket,
            mode: UdpMode::Client {
                server_addr,
                player_id: None,
                connected: false,
                inbound: Vec::new(),
                last_sent: Instant::now(),
            },
            local_seq: SeqNum(0),
            config,
            _marker: std::marker::PhantomData,
        })
    }

    /// Poll for incoming packets (non-blocking). Call once per tick.
    ///
    /// Also frees the slots of clients silent for longer than
    /// `config.timeout` (server), and sends heartbeats or connect retries
    /// every `config.heartbeat_interval` (client).
    pub fn poll(&mut self) {
        let mut buf = vec![0u8; RECV_BUFFER_SIZE];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((len, addr)) => match Packet::decode(&buf[..len]) {
                    Ok(packet) => self.handle_packet(packet, addr),
                    Err(e) => tracing::debug!("Dropping datagram from {addr}: {e}"),
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(ref e) if is_ignorable_recv_error(e) => continue,
                Err(e) => {
                    warn!("UDP recv error: {}", e);
                    break;
                }
            }
        }

        let timeout = Duration::from_secs_f64(self.config.timeout.max(0.0));
        let heartbeat = Duration::from_secs_f64(self.config.heartbeat_interval.max(0.0));
        match &mut self.mode {
            UdpMode::Server { clients, .. } => clients.retain(|addr, slot| {
                let alive = slot.last_seen.elapsed() < timeout;
                if !alive {
                    info!("Client {} ({addr}) timed out", slot.player_id.0);
                }
                alive
            }),
            UdpMode::Client {
                server_addr,
                player_id,
                connected,
                last_sent,
                ..
            } => {
                if last_sent.elapsed() >= heartbeat {
                    let (kind, pid) = if *connected {
                        (PacketKind::Heartbeat, player_id.map_or(0, |p| p.0))
                    } else {
                        (PacketKind::Connect, 0)
                    };
                    let seq = self.local_seq.next();
                    send_packet(
                        &self.socket,
                        *server_addr,
                        &Packet::new(kind, seq, 0, pid, Vec::new()),
                    );
                    *last_sent = Instant::now();
                }
            }
        }
    }

    fn handle_packet(&mut self, packet: Packet, addr: SocketAddr) {
        match &mut self.mode {
            UdpMode::Server {
                clients,
                next_player_id,
                inbound,
            } => match packet.header.kind {
                PacketKind::Connect => {
                    if !clients.contains_key(&addr) && clients.len() < self.config.max_clients {
                        let pid = PlayerId(*next_player_id);
                        // Wrap instead of overflowing after u32::MAX joins
                        // over a very long server lifetime.
                        *next_player_id = next_player_id.wrapping_add(1);
                        clients.insert(
                            addr,
                            ClientSlot {
                                player_id: pid,
                                addr,
                                last_seen: Instant::now(),
                                remote_seq: 0,
                            },
                        );
                        info!("Client connected from {} as player {}", addr, pid.0);

                        let seq = self.local_seq.next();
                        let accept = Packet::new(PacketKind::Accept, seq, 0, pid.0, Vec::new());
                        send_packet(&self.socket, addr, &accept);
                    } else if let Some(client) = clients.get_mut(&addr) {
                        // A retry whose Accept was lost: answer again.
                        client.last_seen = Instant::now();
                        let seq = self.local_seq.next();
                        let accept =
                            Packet::new(PacketKind::Accept, seq, 0, client.player_id.0, Vec::new());
                        send_packet(&self.socket, addr, &accept);
                    }
                }
                PacketKind::Commands => {
                    if let Some(client) = clients.get_mut(&addr) {
                        client.last_seen = Instant::now();
                        client.remote_seq = packet.header.sequence;
                        inbound.push((client.player_id, packet.payload));
                    }
                }
                PacketKind::Disconnect => {
                    if let Some(client) = clients.remove(&addr) {
                        info!("Client {} disconnected", client.player_id.0);
                    }
                }
                PacketKind::Heartbeat => {
                    if let Some(client) = clients.get_mut(&addr) {
                        client.last_seen = Instant::now();
                    }
                }
                _ => {}
            },
            UdpMode::Client {
                server_addr,
                player_id,
                connected,
                inbound,
                ..
            } => match packet.header.kind {
                // Only the server speaks to a client; anyone else could
                // forge an Accept or inject commands.
                _ if addr != *server_addr => {
                    tracing::debug!("Client: dropping a packet from {addr}, not the server");
                }
                PacketKind::Accept => {
                    *player_id = Some(PlayerId(packet.header.player_id));
                    *connected = true;
                    info!("Connected as player {}", packet.header.player_id);
                }
                PacketKind::Broadcast => {
                    inbound.push((PlayerId(packet.header.player_id), packet.payload));
                }
                PacketKind::Disconnect => {
                    *connected = false;
                    warn!("Disconnected by server");
                }
                _ => {}
            },
        }
    }

    /// Send a disconnect packet.
    pub fn disconnect(&mut self) {
        let seq = self.local_seq.next();
        let pkt = Packet::new(PacketKind::Disconnect, seq, 0, 0, Vec::new());
        match &self.mode {
            UdpMode::Server { clients, .. } => {
                for client in clients.values() {
                    send_packet(&self.socket, client.addr, &pkt);
                }
            }
            UdpMode::Client { server_addr, .. } => {
                send_packet(&self.socket, *server_addr, &pkt);
            }
        }
    }

    /// Whether the client is connected (client mode only).
    pub fn is_connected(&self) -> bool {
        match &self.mode {
            UdpMode::Client { connected, .. } => *connected,
            UdpMode::Server { .. } => true,
        }
    }

    /// Get the local player ID (client mode only).
    pub fn local_player_id(&self) -> Option<PlayerId> {
        match &self.mode {
            UdpMode::Client { player_id, .. } => *player_id,
            UdpMode::Server { .. } => Some(PlayerId(0)),
        }
    }

    /// Number of connected clients (server mode only).
    pub fn client_count(&self) -> usize {
        match &self.mode {
            UdpMode::Server { clients, .. } => clients.len(),
            UdpMode::Client { .. } => 0,
        }
    }

    /// The local address this transport's socket is bound to.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

impl<C: Clone + Serialize + for<'de> Deserialize<'de>> Transport<C> for UdpTransport<C> {
    fn send(&mut self, commands: &[C]) {
        let payload = match serde_json::to_vec(commands) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to serialize commands: {}", e);
                return;
            }
        };

        let seq = self.local_seq.next();
        match &mut self.mode {
            UdpMode::Server { clients, .. } => {
                // Broadcast to all clients
                let pkt = Packet::new(PacketKind::Broadcast, seq, 0, 0, payload);
                for client in clients.values() {
                    send_packet(&self.socket, client.addr, &pkt);
                }
            }
            UdpMode::Client {
                server_addr,
                player_id,
                last_sent,
                ..
            } => {
                let pid = player_id.map_or(0, |p| p.0);
                let pkt = Packet::new(PacketKind::Commands, seq, 0, pid, payload);
                if send_packet(&self.socket, *server_addr, &pkt) {
                    *last_sent = Instant::now();
                }
            }
        }
    }

    fn receive(&mut self) -> Vec<(PlayerId, Vec<C>)> {
        self.poll();

        let mut result = Vec::new();

        match &mut self.mode {
            UdpMode::Server { inbound, .. } => {
                for (pid, data) in inbound.drain(..) {
                    if let Ok(cmds) = serde_json::from_slice::<Vec<C>>(&data) {
                        result.push((pid, cmds));
                    }
                }
            }
            UdpMode::Client { inbound, .. } => {
                for (pid, data) in inbound.drain(..) {
                    if let Ok(cmds) = serde_json::from_slice::<Vec<C>>(&data) {
                        result.push((pid, cmds));
                    }
                }
            }
        }

        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_bind_and_config() {
        let config = UdpConfig {
            bind_addr: "127.0.0.1:0".into(),
            ..Default::default()
        };
        let transport: UdpTransport<String> = UdpTransport::bind_server(config).unwrap();
        assert!(transport.is_connected());
        assert_eq!(transport.client_count(), 0);
    }

    #[test]
    fn client_initial_state() {
        // Use a dummy address (won't actually connect)
        let transport: UdpTransport<String> =
            UdpTransport::connect_client("127.0.0.1:19999").unwrap();
        assert!(!transport.is_connected());
        assert_eq!(transport.local_player_id(), None);
    }

    #[test]
    fn default_config() {
        let cfg = UdpConfig::default();
        assert_eq!(cfg.max_clients, 8);
        assert_eq!(cfg.bind_addr, "0.0.0.0:7777");
    }

    /// Malformed datagrams from untrusted peers must be dropped, never panic.
    #[test]
    fn server_survives_malformed_packets() {
        let config = UdpConfig {
            bind_addr: "127.0.0.1:0".into(),
            ..Default::default()
        };
        let mut server: UdpTransport<String> = UdpTransport::bind_server(config).unwrap();
        let server_addr = server.local_addr().unwrap();

        let attacker = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let payloads: &[&[u8]] = &[
            b"",                                             // empty datagram
            b"\x00\x01\x02\x03",                             // binary garbage
            b"{\"header\"",                                  // truncated JSON
            b"{\"header\":{},\"payload\":null}",             // wrong schema
            &[0xffu8; crate::protocol::MAX_PACKET_SIZE],     // max-size garbage
            &[0xffu8; 3 * crate::protocol::MAX_PACKET_SIZE], // oversized
        ];
        for payload in payloads {
            attacker.send_to(payload, server_addr).unwrap();
        }
        // Give the datagrams a moment to arrive.
        std::thread::sleep(std::time::Duration::from_millis(50));

        server.poll();
        assert_eq!(server.client_count(), 0, "garbage must not create clients");
    }

    /// A connected client whose Commands payload is not valid JSON must be
    /// ignored by receive_commands rather than crash the server.
    #[test]
    fn server_ignores_malformed_command_payload() {
        let config = UdpConfig {
            bind_addr: "127.0.0.1:0".into(),
            ..Default::default()
        };
        let mut server: UdpTransport<String> = UdpTransport::bind_server(config).unwrap();
        let server_addr = server.local_addr().unwrap();

        let peer = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let connect = Packet::new(PacketKind::Connect, 0, 0, 0, Vec::new());
        peer.send_to(&connect.encode().unwrap(), server_addr)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        server.poll();
        assert_eq!(server.client_count(), 1);

        let bad_commands = Packet::new(PacketKind::Commands, 1, 0, 1, b"not json".to_vec());
        peer.send_to(&bad_commands.encode().unwrap(), server_addr)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        let commands: Vec<(PlayerId, Vec<String>)> = server.receive();
        assert!(commands.is_empty(), "malformed payloads must be dropped");
    }

    fn wait() {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    fn loopback(addr: SocketAddr) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], addr.port()))
    }

    fn server(timeout: f64) -> UdpTransport<String> {
        UdpTransport::bind_server(UdpConfig {
            bind_addr: "127.0.0.1:0".into(),
            timeout,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn a_client_only_listens_to_its_server() {
        let fake_server = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut client: UdpTransport<String> =
            UdpTransport::connect_client(&fake_server.local_addr().unwrap().to_string()).unwrap();
        let client_addr = loopback(client.local_addr().unwrap());
        let accept = Packet::new(PacketKind::Accept, 0, 0, 5, Vec::new())
            .encode()
            .unwrap();

        let stranger = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        stranger.send_to(&accept, client_addr).unwrap();
        wait();
        client.poll();
        assert!(
            !client.is_connected(),
            "an Accept from a stranger was taken"
        );

        fake_server.send_to(&accept, client_addr).unwrap();
        wait();
        client.poll();
        assert_eq!(client.local_player_id(), Some(PlayerId(5)));
    }

    #[test]
    fn silent_clients_lose_their_slot() {
        let mut server = server(0.05);
        let peer = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let connect = Packet::new(PacketKind::Connect, 0, 0, 0, Vec::new());
        peer.send_to(&connect.encode().unwrap(), server.local_addr().unwrap())
            .unwrap();
        wait();
        server.poll();
        assert_eq!(server.client_count(), 1);
        std::thread::sleep(std::time::Duration::from_millis(100));
        server.poll();
        assert_eq!(server.client_count(), 0);
    }

    #[test]
    fn command_batches_of_a_kilobyte_arrive() {
        // Around 300 bytes used to be the limit, silently.
        let mut server = server(10.0);
        let mut client: UdpTransport<String> =
            UdpTransport::connect_client(&loopback(server.local_addr().unwrap()).to_string())
                .unwrap();
        wait();
        server.poll();
        wait();
        client.poll();
        assert!(client.is_connected());

        let commands: Vec<String> = (0..50).map(|i| format!("move_unit_{i:02}_to_x")).collect();
        assert!(serde_json::to_vec(&commands).unwrap().len() > 900);
        client.send(&commands);
        wait();
        let received = server.receive();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].1, commands);
    }
}
