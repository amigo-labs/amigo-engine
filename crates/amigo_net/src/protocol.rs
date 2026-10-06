//! The packet format shared by every UDP transport in this crate.
//!
//! A packet is a fixed 14-byte binary header followed by the payload bytes:
//!
//! | bytes | field |
//! |---|---|
//! | 4 | magic `AMGO` |
//! | 1 | [`PROTOCOL_VERSION`] |
//! | 1 | [`PacketKind`] |
//! | 2 | sequence (LE) |
//! | 2 | ack (LE) |
//! | 4 | player id (LE) |
//! | rest | payload |
//!
//! It used to be the whole packet as JSON, payload included, so every
//! payload byte became a decimal number and a comma: about 300 bytes of
//! commands already overflowed the 1200-byte receive buffer, and the packet
//! vanished without a word.

use crate::wire::{ByteReader, ByteWriter, WireError};
use serde::{Deserialize, Serialize};

/// Maximum UDP packet size we'll send. Stays well under typical MTU.
pub const MAX_PACKET_SIZE: usize = 1200;

/// Bytes in front of every payload.
pub const HEADER_SIZE: usize = 14;

/// The largest payload that fits in one packet.
pub const MAX_PAYLOAD_SIZE: usize = MAX_PACKET_SIZE - HEADER_SIZE;

/// Bumped whenever the wire format changes; peers on another version are
/// refused.
pub const PROTOCOL_VERSION: u8 = 1;

const MAGIC: [u8; 4] = *b"AMGO";

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
    /// Lockstep guest → host: request to join a session.
    Hello = 6,
    /// Lockstep host → guest: session accepted, with its settings.
    Welcome = 7,
    /// Lockstep host → guest: session refused, with the reason.
    Reject = 8,
    /// Lockstep, either direction: inputs, acks and checksums.
    Lockstep = 9,
}

impl PacketKind {
    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Connect,
            1 => Self::Accept,
            2 => Self::Disconnect,
            3 => Self::Heartbeat,
            4 => Self::Commands,
            5 => Self::Broadcast,
            6 => Self::Hello,
            7 => Self::Welcome,
            8 => Self::Reject,
            9 => Self::Lockstep,
            _ => return None,
        })
    }
}

/// Wire header prepended to every packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacketHeader {
    pub kind: PacketKind,
    pub sequence: u16,
    pub ack: u16,
    pub player_id: u32,
}

/// A complete packet on the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub header: PacketHeader,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(
        kind: PacketKind,
        sequence: u16,
        ack: u16,
        player_id: u32,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            header: PacketHeader {
                kind,
                sequence,
                ack,
                player_id,
            },
            payload,
        }
    }

    /// The packet's bytes, or [`WireError::TooLarge`] when it would not fit
    /// in [`MAX_PACKET_SIZE`].
    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        let size = HEADER_SIZE + self.payload.len();
        if size > MAX_PACKET_SIZE {
            return Err(WireError::TooLarge {
                size,
                max: MAX_PACKET_SIZE,
            });
        }
        let mut w = ByteWriter::with_capacity(size);
        w.bytes(&MAGIC)
            .u8(PROTOCOL_VERSION)
            .u8(self.header.kind as u8)
            .u16(self.header.sequence)
            .u16(self.header.ack)
            .u32(self.header.player_id)
            .bytes(&self.payload);
        Ok(w.finish())
    }

    /// Parse a datagram. Anything that is not a packet of this protocol
    /// version is an error.
    pub fn decode(data: &[u8]) -> Result<Self, WireError> {
        if data.len() > MAX_PACKET_SIZE {
            return Err(WireError::TooLarge {
                size: data.len(),
                max: MAX_PACKET_SIZE,
            });
        }
        let mut r = ByteReader::new(data);
        if r.bytes(4)? != MAGIC {
            return Err(WireError::Invalid("magic"));
        }
        if r.u8()? != PROTOCOL_VERSION {
            return Err(WireError::Invalid("protocol version"));
        }
        let kind = PacketKind::from_u8(r.u8()?).ok_or(WireError::Invalid("packet kind"))?;
        let sequence = r.u16()?;
        let ack = r.u16()?;
        let player_id = r.u32()?;
        Ok(Self::new(kind, sequence, ack, player_id, r.rest().to_vec()))
    }
}

/// Receive buffer size: larger than any valid packet, so an oversized
/// datagram arrives whole and is rejected by [`Packet::decode`] instead of
/// being cut off (Linux) or failing the read (Windows).
pub(crate) const RECV_BUFFER_SIZE: usize = 2 * MAX_PACKET_SIZE;

/// Encode `packet` and send it to `addr`. A packet too large to send is
/// logged rather than dropped silently; returns whether it went out.
pub(crate) fn send_packet(
    socket: &std::net::UdpSocket,
    addr: std::net::SocketAddr,
    packet: &Packet,
) -> bool {
    match packet.encode() {
        Ok(data) => socket.send_to(&data, addr).is_ok(),
        Err(e) => {
            tracing::warn!("Not sending {:?} packet to {addr}: {e}", packet.header.kind);
            false
        }
    }
}

/// A `recv_from` error to skip rather than stop polling on: an interrupted
/// call, or Windows reporting that an earlier datagram to a closed port
/// bounced (UDP has no connection to reset; the next datagram is fine).
pub(crate) fn is_ignorable_recv_error(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::Interrupted | std::io::ErrorKind::ConnectionReset
    )
}

/// Sequence number wrapper with wrapping arithmetic and comparison.
#[derive(Clone, Copy, Debug, Default)]
pub struct SeqNum(pub u16);

impl SeqNum {
    #[expect(
        clippy::should_implement_trait,
        reason = "`next` never ends, so this is not an Iterator"
    )]
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

    // ── Packet Encoding ────────────────────────────────────────

    #[test]
    fn packet_roundtrip() {
        let pkt = Packet::new(PacketKind::Commands, 42, 41, 1, b"hello".to_vec());
        let encoded = pkt.encode().unwrap();
        assert_eq!(encoded.len(), HEADER_SIZE + 5);
        assert_eq!(Packet::decode(&encoded), Ok(pkt));
    }

    #[test]
    fn a_full_payload_fits_and_one_byte_more_does_not() {
        // About 300 bytes of commands used to vanish: JSON in JSON spent
        // up to four bytes per payload byte.
        let pkt = Packet::new(PacketKind::Commands, 1, 0, 1, vec![0xab; MAX_PAYLOAD_SIZE]);
        let encoded = pkt.encode().unwrap();
        assert_eq!(encoded.len(), MAX_PACKET_SIZE);
        assert_eq!(
            Packet::decode(&encoded).unwrap().payload.len(),
            MAX_PAYLOAD_SIZE
        );

        let too_big = Packet::new(PacketKind::Commands, 1, 0, 1, vec![0; MAX_PAYLOAD_SIZE + 1]);
        assert_eq!(
            too_big.encode(),
            Err(WireError::TooLarge {
                size: MAX_PACKET_SIZE + 1,
                max: MAX_PACKET_SIZE
            })
        );
    }

    #[test]
    fn decode_rejects_malformed_input() {
        let good = Packet::new(PacketKind::Heartbeat, 3, 2, 1, Vec::new())
            .encode()
            .unwrap();
        assert_eq!(Packet::decode(b""), Err(WireError::Truncated));
        assert_eq!(
            Packet::decode(&good[..HEADER_SIZE - 1]),
            Err(WireError::Truncated)
        );
        assert!(Packet::decode(b"{\"header\":{},\"payload\":null}").is_err());

        let mut wrong_magic = good.clone();
        wrong_magic[0] = b'X';
        assert_eq!(
            Packet::decode(&wrong_magic),
            Err(WireError::Invalid("magic"))
        );
        let mut wrong_version = good.clone();
        wrong_version[4] = PROTOCOL_VERSION + 1;
        assert_eq!(
            Packet::decode(&wrong_version),
            Err(WireError::Invalid("protocol version"))
        );
        let mut wrong_kind = good;
        wrong_kind[5] = 200;
        assert_eq!(
            Packet::decode(&wrong_kind),
            Err(WireError::Invalid("packet kind"))
        );
        assert!(Packet::decode(&[0; MAX_PACKET_SIZE + 1]).is_err());
    }

    // ── Sequence Numbers ────────────────────────────────────────

    #[test]
    fn sequence_wrapping() {
        assert!(SeqNum::is_newer(1, 0));
        assert!(SeqNum::is_newer(100, 50));
        assert!(!SeqNum::is_newer(50, 100));
        // Wrapping: 0 is newer than 65530
        assert!(SeqNum::is_newer(
            0_u16.wrapping_sub(1),
            0_u16.wrapping_sub(10)
        ));
    }

    #[test]
    fn seqnum_increments() {
        let mut seq = SeqNum(0);
        assert_eq!(seq.next(), 0);
        assert_eq!(seq.next(), 1);
        assert_eq!(seq.next(), 2);
    }
}
