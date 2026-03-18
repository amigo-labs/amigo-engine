//! UDP transport implementation (RS-05).
//!
//! Provides `UdpTransport` which implements the `Transport` trait using
//! standard library UDP sockets with the engine's packet protocol.

use crate::protocol::{Packet, PacketKind, SeqNum, MAX_PACKET_SIZE};
use crate::{PlayerId, Transport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use tracing::{info, warn};

/// Configuration for the UDP transport.
#[derive(Clone, Debug)]
pub struct UdpConfig {
    /// For server mode: address to bind to.
    pub bind_addr: String,
    /// Maximum number of clients (server mode).
    pub max_clients: usize,
    /// Heartbeat interval in seconds.
    pub heartbeat_interval: f64,
    /// Connection timeout in seconds.
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
    last_seen: std::time::Instant,
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
        /// Inbound broadcasts from the server this frame.
        inbound: Vec<Vec<u8>>,
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
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        let server_addr: SocketAddr = server_addr
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        info!("UDP client connecting to {}", server_addr);

        // Send connect packet
        let pkt = Packet::new(PacketKind::Connect, 0, 0, 0, Vec::new());
        if let Some(data) = pkt.encode() {
            let _ = socket.send_to(&data, server_addr);
        }

        Ok(Self {
            socket,
            mode: UdpMode::Client {
                server_addr,
                player_id: None,
                connected: false,
                inbound: Vec::new(),
            },
            local_seq: SeqNum(0),
            config: UdpConfig::default(),
            _marker: std::marker::PhantomData,
        })
    }

    /// Poll for incoming packets (non-blocking). Call once per tick.
    pub fn poll(&mut self) {
        let mut buf = [0u8; MAX_PACKET_SIZE];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((len, addr)) => {
                    if let Some(packet) = Packet::decode(&buf[..len]) {
                        self.handle_packet(packet, addr);
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::Interrupted =>
                {
                    break;
                }
                Err(e) => {
                    warn!("UDP recv error: {}", e);
                    break;
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
                    if !clients.contains_key(&addr)
                        && clients.len() < self.config.max_clients
                    {
                        let pid = PlayerId(*next_player_id);
                        *next_player_id += 1;
                        clients.insert(
                            addr,
                            ClientSlot {
                                player_id: pid,
                                addr,
                                last_seen: std::time::Instant::now(),
                                remote_seq: 0,
                            },
                        );
                        info!("Client connected from {} as player {}", addr, pid.0);

                        // Send accept
                        let seq = self.local_seq.next();
                        let accept = Packet::new(PacketKind::Accept, seq, 0, pid.0, Vec::new());
                        if let Some(data) = accept.encode() {
                            let _ = self.socket.send_to(&data, addr);
                        }
                    }
                }
                PacketKind::Commands => {
                    if let Some(client) = clients.get_mut(&addr) {
                        client.last_seen = std::time::Instant::now();
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
                        client.last_seen = std::time::Instant::now();
                    }
                }
                _ => {}
            },
            UdpMode::Client {
                player_id,
                connected,
                inbound,
                ..
            } => match packet.header.kind {
                PacketKind::Accept => {
                    *player_id = Some(PlayerId(packet.header.player_id));
                    *connected = true;
                    info!("Connected as player {}", packet.header.player_id);
                }
                PacketKind::Broadcast => {
                    inbound.push(packet.payload);
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
        if let Some(data) = pkt.encode() {
            match &self.mode {
                UdpMode::Server { clients, .. } => {
                    for client in clients.values() {
                        let _ = self.socket.send_to(&data, client.addr);
                    }
                }
                UdpMode::Client { server_addr, .. } => {
                    let _ = self.socket.send_to(&data, *server_addr);
                }
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
        match &self.mode {
            UdpMode::Server { clients, .. } => {
                // Broadcast to all clients
                let pkt = Packet::new(PacketKind::Broadcast, seq, 0, 0, payload);
                if let Some(data) = pkt.encode() {
                    for client in clients.values() {
                        let _ = self.socket.send_to(&data, client.addr);
                    }
                }
            }
            UdpMode::Client {
                server_addr,
                player_id,
                ..
            } => {
                let pid = player_id.map_or(0, |p| p.0);
                let pkt = Packet::new(PacketKind::Commands, seq, 0, pid, payload);
                if let Some(data) = pkt.encode() {
                    let _ = self.socket.send_to(&data, *server_addr);
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
                for data in inbound.drain(..) {
                    if let Ok(cmds) = serde_json::from_slice::<Vec<C>>(&data) {
                        // Server broadcasts come as player 0
                        result.push((PlayerId(0), cmds));
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
}
