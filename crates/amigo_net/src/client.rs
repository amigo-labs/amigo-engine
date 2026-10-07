use crate::protocol::{
    Packet, PacketKind, RECV_BUFFER_SIZE, SeqNum, is_ignorable_recv_error, send_packet,
};
use crate::{PlayerId, Transport};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::net::{SocketAddr, UdpSocket};
use std::time::Instant;
use tracing::{debug, warn};

/// Connection state of the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

/// Game client that connects to a server, sends commands, and receives
/// broadcasted commands from all players.
pub struct NetworkClient<C> {
    socket: UdpSocket,
    server_addr: SocketAddr,
    pub state: ConnectionState,
    pub player_id: Option<PlayerId>,
    local_seq: SeqNum,
    remote_ack: u16,
    /// Buffered broadcast commands from the server.
    inbound: Vec<(PlayerId, Vec<C>)>,
    /// Outbound commands queued for the next send.
    outbound: Vec<C>,
    last_heartbeat: Instant,
    connect_time: Instant,
    recv_buf: Vec<u8>,
}

impl<C: Clone + Serialize + DeserializeOwned> NetworkClient<C> {
    /// Create a client and begin connecting to the server.
    pub fn connect(server_addr: &str) -> std::io::Result<Self> {
        let server: SocketAddr = server_addr
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        debug!("Client connecting to {}", server);

        let client = Self {
            socket,
            server_addr: server,
            state: ConnectionState::Connecting,
            player_id: None,
            local_seq: SeqNum::default(),
            remote_ack: 0,
            inbound: Vec::new(),
            outbound: Vec::new(),
            last_heartbeat: Instant::now(),
            connect_time: Instant::now(),
            recv_buf: vec![0u8; RECV_BUFFER_SIZE],
        };

        // Send initial connect packet
        client.send_connect();
        Ok(client)
    }

    /// Poll the socket for incoming packets. Call once per tick.
    pub fn poll(&mut self) {
        loop {
            match self.socket.recv_from(&mut self.recv_buf) {
                // Only the server speaks to us; anyone else could forge an
                // Accept or inject broadcasts.
                Ok((_, src)) if src != self.server_addr => {
                    debug!("Dropping a packet from {src}, not the server");
                }
                Ok((len, src)) => match Packet::decode(&self.recv_buf[..len]) {
                    Ok(packet) => self.handle_packet(packet),
                    Err(e) => debug!("Dropping datagram from {src}: {e}"),
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(ref e) if is_ignorable_recv_error(e) => continue,
                Err(e) => {
                    warn!("Client recv error: {}", e);
                    break;
                }
            }
        }

        // Retry connect if still connecting
        if self.state == ConnectionState::Connecting
            && self.connect_time.elapsed().as_millis() > 500
        {
            self.send_connect();
            self.connect_time = Instant::now();
        }

        // Send heartbeat every 2 seconds when connected
        if self.state == ConnectionState::Connected && self.last_heartbeat.elapsed().as_secs() >= 2
        {
            self.send_heartbeat();
            self.last_heartbeat = Instant::now();
        }
    }

    fn handle_packet(&mut self, packet: Packet) {
        match packet.header.kind {
            PacketKind::Accept => {
                let pid = if packet.payload.len() >= 4 {
                    let bytes: [u8; 4] = packet.payload[..4].try_into().unwrap();
                    u32::from_le_bytes(bytes)
                } else {
                    packet.header.player_id
                };
                self.player_id = Some(PlayerId(pid));
                self.state = ConnectionState::Connected;
                debug!("Connected as player {}", pid);
            }
            PacketKind::Broadcast => {
                if SeqNum::is_newer(packet.header.sequence, self.remote_ack) {
                    self.remote_ack = packet.header.sequence;
                }
                if let Ok(commands) =
                    serde_json::from_slice::<Vec<(PlayerId, Vec<C>)>>(&packet.payload)
                {
                    self.inbound.extend(commands);
                }
            }
            PacketKind::Disconnect => {
                debug!("Server disconnected us");
                self.state = ConnectionState::Disconnected;
            }
            _ => {}
        }
    }

    fn send_connect(&self) {
        let pkt = Packet::new(PacketKind::Connect, 0, 0, 0, Vec::new());
        send_packet(&self.socket, self.server_addr, &pkt);
    }

    fn send_heartbeat(&self) {
        let pid = self.player_id.map(|p| p.0).unwrap_or(0);
        let pkt = Packet::new(PacketKind::Heartbeat, 0, self.remote_ack, pid, Vec::new());
        send_packet(&self.socket, self.server_addr, &pkt);
    }

    /// Flush queued commands to the server.
    pub fn flush_commands(&mut self) {
        if self.state != ConnectionState::Connected || self.outbound.is_empty() {
            return;
        }
        let payload = match serde_json::to_vec(&self.outbound) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to serialize commands: {}", e);
                return;
            }
        };
        let pid = self.player_id.map(|p| p.0).unwrap_or(0);
        let seq = self.local_seq.next();
        let pkt = Packet::new(PacketKind::Commands, seq, self.remote_ack, pid, payload);
        send_packet(&self.socket, self.server_addr, &pkt);
        self.outbound.clear();
    }

    /// Send a disconnect packet and move to Disconnected state.
    pub fn disconnect(&mut self) {
        let pid = self.player_id.map(|p| p.0).unwrap_or(0);
        let pkt = Packet::new(PacketKind::Disconnect, 0, 0, pid, Vec::new());
        send_packet(&self.socket, self.server_addr, &pkt);
        self.state = ConnectionState::Disconnected;
    }
}

impl<C: Clone + Serialize + DeserializeOwned> Transport<C> for NetworkClient<C> {
    fn send(&mut self, commands: &[C]) {
        self.outbound.extend_from_slice(commands);
    }

    fn receive(&mut self) -> Vec<(PlayerId, Vec<C>)> {
        std::mem::take(&mut self.inbound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::NetworkServer;
    use serde::{Deserialize, Serialize};
    use std::thread;
    use std::time::Duration;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum TestCmd {
        Move(i32, i32),
        Attack,
    }

    #[test]
    fn client_server_connect_and_exchange() {
        // Start server
        let mut server: NetworkServer<TestCmd> =
            NetworkServer::bind("127.0.0.1:0").expect("bind server");
        let server_addr = server.local_addr().unwrap().to_string();

        // Connect client
        let mut client: NetworkClient<TestCmd> =
            NetworkClient::connect(&server_addr).expect("connect client");

        // Give the connect packet time to arrive
        thread::sleep(Duration::from_millis(50));
        server.poll();
        assert_eq!(server.client_count(), 1);

        // Client should receive the accept
        thread::sleep(Duration::from_millis(50));
        client.poll();
        assert_eq!(client.state, ConnectionState::Connected);
        assert!(client.player_id.is_some());

        // Client sends commands
        <NetworkClient<TestCmd> as Transport<TestCmd>>::send(
            &mut client,
            &[TestCmd::Move(10, 20), TestCmd::Attack],
        );
        client.flush_commands();

        thread::sleep(Duration::from_millis(50));
        server.poll();
        let received = server.receive();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].1, vec![TestCmd::Move(10, 20), TestCmd::Attack]);

        // Server broadcasts back
        server.broadcast(&received);

        thread::sleep(Duration::from_millis(50));
        client.poll();
        let broadcast = client.receive();
        assert_eq!(broadcast.len(), 1);
        assert_eq!(broadcast[0].1, vec![TestCmd::Move(10, 20), TestCmd::Attack]);

        // Clean disconnect
        client.disconnect();
        thread::sleep(Duration::from_millis(50));
        server.poll();
        assert_eq!(server.client_count(), 0);
    }

    #[test]
    fn a_client_ignores_packets_from_anyone_but_the_server() {
        let fake_server = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut client: NetworkClient<TestCmd> =
            NetworkClient::connect(&fake_server.local_addr().unwrap().to_string()).unwrap();
        let client_addr =
            SocketAddr::from(([127, 0, 0, 1], client.socket.local_addr().unwrap().port()));
        let accept = Packet::new(PacketKind::Accept, 0, 0, 3, Vec::new())
            .encode()
            .unwrap();

        let stranger = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        stranger.send_to(&accept, client_addr).unwrap();
        thread::sleep(Duration::from_millis(50));
        client.poll();
        assert_eq!(client.state, ConnectionState::Connecting);

        fake_server.send_to(&accept, client_addr).unwrap();
        thread::sleep(Duration::from_millis(50));
        client.poll();
        assert_eq!(client.state, ConnectionState::Connected);
        assert_eq!(client.player_id, Some(PlayerId(3)));
    }
}
