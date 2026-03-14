//! Deterministic state hashing for desync detection.
//!
//! Provides a simple CRC-based hasher that game code feeds per-tick data into.
//! The resulting checksum is compared across clients/server to detect desyncs.

use std::hash::{Hash, Hasher};

// ---------------------------------------------------------------------------
// CRC-32 lookup table (IEEE polynomial 0xEDB88320, reflected)
// ---------------------------------------------------------------------------

const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

/// Compute CRC-32 of a byte slice.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
    }
    crc ^ 0xFFFF_FFFF
}

// ---------------------------------------------------------------------------
// StateHasher — accumulates game state into a single checksum
// ---------------------------------------------------------------------------

/// Accumulates hashable game state into a CRC-32 checksum.
///
/// Usage each tick:
/// ```
/// use amigo_net::checksum::StateHasher;
///
/// let mut hasher = StateHasher::new();
/// hasher.write_u32(100); // player_x
/// hasher.write_u32(200); // player_y
/// hasher.write_u32(75);  // health
/// let checksum = hasher.finish_crc();
/// assert_ne!(checksum, 0);
/// ```
pub struct StateHasher {
    buffer: Vec<u8>,
}

impl StateHasher {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(1024),
        }
    }

    /// Reset for a new tick.
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Write raw bytes.
    pub fn write_bytes(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Write a u32 value (little-endian).
    pub fn write_u32(&mut self, v: u32) {
        self.buffer.extend_from_slice(&v.to_le_bytes());
    }

    /// Write an i32 value (little-endian).
    pub fn write_i32(&mut self, v: i32) {
        self.buffer.extend_from_slice(&v.to_le_bytes());
    }

    /// Write a u64 value (little-endian).
    pub fn write_u64(&mut self, v: u64) {
        self.buffer.extend_from_slice(&v.to_le_bytes());
    }

    /// Write an f32 value as its bit representation.
    pub fn write_f32(&mut self, v: f32) {
        self.buffer.extend_from_slice(&v.to_bits().to_le_bytes());
    }

    /// Hash any type implementing `Hash` via the standard library.
    pub fn write_hash<T: Hash>(&mut self, value: &T) {
        let mut h = SimpleHasher(0);
        value.hash(&mut h);
        self.write_u64(h.0);
    }

    /// Compute CRC-32 of all accumulated data.
    pub fn finish_crc(&self) -> u32 {
        crc32(&self.buffer)
    }

    /// Compute CRC-32 as u64 (for use with DesyncDetector).
    pub fn finish_crc64(&self) -> u64 {
        crc32(&self.buffer) as u64
    }
}

impl Default for StateHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal hasher for converting `Hash` trait data to u64.
struct SimpleHasher(u64);

impl Hasher for SimpleHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(b as u64);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_empty() {
        assert_eq!(crc32(b""), 0x0000_0000);
    }

    #[test]
    fn crc32_known_values() {
        // "123456789" should produce 0xCBF43926 per IEEE CRC-32
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_hello() {
        let c = crc32(b"hello");
        assert_ne!(c, 0);
        // Same input → same output
        assert_eq!(crc32(b"hello"), c);
        // Different input → different output
        assert_ne!(crc32(b"world"), c);
    }

    #[test]
    fn state_hasher_deterministic() {
        let mut h1 = StateHasher::new();
        h1.write_u32(100);
        h1.write_u32(200);
        h1.write_i32(-50);

        let mut h2 = StateHasher::new();
        h2.write_u32(100);
        h2.write_u32(200);
        h2.write_i32(-50);

        assert_eq!(h1.finish_crc(), h2.finish_crc());
    }

    #[test]
    fn state_hasher_different_data() {
        let mut h1 = StateHasher::new();
        h1.write_u32(100);

        let mut h2 = StateHasher::new();
        h2.write_u32(101);

        assert_ne!(h1.finish_crc(), h2.finish_crc());
    }

    #[test]
    fn state_hasher_order_matters() {
        let mut h1 = StateHasher::new();
        h1.write_u32(1);
        h1.write_u32(2);

        let mut h2 = StateHasher::new();
        h2.write_u32(2);
        h2.write_u32(1);

        assert_ne!(h1.finish_crc(), h2.finish_crc());
    }

    #[test]
    fn state_hasher_reset() {
        let mut h = StateHasher::new();
        h.write_u32(42);
        let c1 = h.finish_crc();

        h.reset();
        h.write_u32(42);
        assert_eq!(h.finish_crc(), c1);
    }

    #[test]
    fn state_hasher_f32() {
        let mut h = StateHasher::new();
        h.write_f32(1.5);
        let c = h.finish_crc();
        assert_ne!(c, 0);

        let mut h2 = StateHasher::new();
        h2.write_f32(1.5);
        assert_eq!(h2.finish_crc(), c);
    }

    #[test]
    fn state_hasher_hash_trait() {
        let mut h = StateHasher::new();
        h.write_hash(&"hello");
        let c = h.finish_crc();

        let mut h2 = StateHasher::new();
        h2.write_hash(&"hello");
        assert_eq!(h2.finish_crc(), c);

        let mut h3 = StateHasher::new();
        h3.write_hash(&"world");
        assert_ne!(h3.finish_crc(), c);
    }
}
