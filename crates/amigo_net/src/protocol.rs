use serde::{Deserialize, Serialize};

/// Maximum UDP packet size we'll send. Stays well under typical MTU.
pub const MAX_PACKET_SIZE: usize = 1200;

/// Packet types in the protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PacketKind {
    /// Client → Server: request to join.
    Connect = 0,
    /// Server → Client: connection accepted, assigned PlayerId.
    Accept = 1,
    /// Either direction: graceful disconnect.
    Disconnect = 2,
    /// Either direction: keep-alive.
    Heartbeat = 3,
    /// Client → Server: player commands for this tick.
    Commands = 4,
    /// Server → Client: all players' commands for this tick.
    Broadcast = 5,
}

/// Wire header prepended to every packet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacketHeader {
    pub kind: PacketKind,
    pub sequence: u16,
    pub ack: u16,
    pub player_id: u32,
}

/// A complete packet on the wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Packet {
    pub header: PacketHeader,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(kind: PacketKind, sequence: u16, ack: u16, player_id: u32, payload: Vec<u8>) -> Self {
        Self {
            header: PacketHeader { kind, sequence, ack, player_id },
            payload,
        }
    }

    pub fn encode(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(self).ok()
    }

    pub fn decode(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }
}

/// Sequence number wrapper with wrapping arithmetic and comparison.
#[derive(Clone, Copy, Debug, Default)]
pub struct SeqNum(pub u16);

impl SeqNum {
    pub fn next(&mut self) -> u16 {
        let val = self.0;
        self.0 = self.0.wrapping_add(1);
        val
    }

    /// Returns true if `a` is more recent than `b` (handles wrapping).
    pub fn is_newer(a: u16, b: u16) -> bool {
        let diff = a.wrapping_sub(b);
        diff > 0 && diff < 32768
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_roundtrip() {
        let pkt = Packet::new(PacketKind::Commands, 42, 41, 1, b"hello".to_vec());
        let encoded = pkt.encode().unwrap();
        let decoded = Packet::decode(&encoded).unwrap();
        assert_eq!(decoded.header.kind, PacketKind::Commands);
        assert_eq!(decoded.header.sequence, 42);
        assert_eq!(decoded.header.ack, 41);
        assert_eq!(decoded.header.player_id, 1);
        assert_eq!(decoded.payload, b"hello");
    }

    #[test]
    fn sequence_wrapping() {
        assert!(SeqNum::is_newer(1, 0));
        assert!(SeqNum::is_newer(100, 50));
        assert!(!SeqNum::is_newer(50, 100));
        // Wrapping: 0 is newer than 65530
        assert!(SeqNum::is_newer(0_u16.wrapping_sub(1), 0_u16.wrapping_sub(10)));
    }

    #[test]
    fn seqnum_increments() {
        let mut seq = SeqNum(0);
        assert_eq!(seq.next(), 0);
        assert_eq!(seq.next(), 1);
        assert_eq!(seq.next(), 2);
    }
}
