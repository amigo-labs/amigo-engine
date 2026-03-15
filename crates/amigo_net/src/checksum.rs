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

    // ── Determinism verification tests ──────────────────────────────────

    /// Simulate N ticks of a minimal game loop using fixed-point arithmetic.
    /// Returns the final state hash and the per-tick checksum trail.
    fn run_deterministic_sim(seed: u64, ticks: u64) -> (u64, Vec<u64>) {
        use amigo_core::math::{Fix, SimVec2};

        // Seed-derived initial state
        let mut pos = SimVec2::new(
            Fix::from_num((seed % 100) as i32),
            Fix::from_num((seed % 50) as i32),
        );
        let velocity = SimVec2::new(
            Fix::from_num(1),
            Fix::from_num(0),
        );
        let mut health: Fix = Fix::from_num(100);
        let damage_per_tick: Fix = Fix::from_bits(0x0000_4000); // ~0.25 in Q16.16
        let mut gold: u32 = 50;
        let mut tick_checksums = Vec::with_capacity(ticks as usize);

        for tick in 0..ticks {
            // Movement
            pos.x += velocity.x;
            pos.y += velocity.y;

            // Damage
            health -= damage_per_tick;

            // Gold logic: earn 1 gold every 10 ticks
            if tick % 10 == 0 {
                gold += 1;
            }

            // Boundary wrap (deterministic modular arithmetic on fixed-point)
            if pos.x > Fix::from_num(1000) {
                pos.x -= Fix::from_num(1000);
            }

            // Hash the state at this tick
            let mut hasher = StateHasher::new();
            hasher.write_u64(tick);
            hasher.write_i32(pos.x.to_bits());
            hasher.write_i32(pos.y.to_bits());
            hasher.write_i32(health.to_bits());
            hasher.write_u32(gold);
            tick_checksums.push(hasher.finish_crc64());
        }

        // Final state hash
        let mut final_hasher = StateHasher::new();
        final_hasher.write_i32(pos.x.to_bits());
        final_hasher.write_i32(pos.y.to_bits());
        final_hasher.write_i32(health.to_bits());
        final_hasher.write_u32(gold);
        (final_hasher.finish_crc64(), tick_checksums)
    }

    #[test]
    fn deterministic_sim_same_seed_same_result() {
        let (hash_a, trail_a) = run_deterministic_sim(42, 1000);
        let (hash_b, trail_b) = run_deterministic_sim(42, 1000);

        assert_eq!(hash_a, hash_b, "Same seed must produce identical final hash");
        assert_eq!(trail_a, trail_b, "Same seed must produce identical per-tick trail");
    }

    #[test]
    fn deterministic_sim_different_seed_different_result() {
        let (hash_a, _) = run_deterministic_sim(42, 1000);
        let (hash_b, _) = run_deterministic_sim(99, 1000);

        assert_ne!(hash_a, hash_b, "Different seeds must produce different hashes");
    }

    #[test]
    fn deterministic_sim_desync_detection() {
        let (_, trail_a) = run_deterministic_sim(42, 100);
        let (_, trail_b) = run_deterministic_sim(42, 100);
        let (_, trail_c) = run_deterministic_sim(99, 100);

        // Build desync detectors
        let mut det_a = super::super::replay::DesyncDetector::new();
        let mut det_b = super::super::replay::DesyncDetector::new();
        let mut det_c = super::super::replay::DesyncDetector::new();

        for (tick, (&ca, (&cb, &cc))) in trail_a.iter()
            .zip(trail_b.iter().zip(trail_c.iter()))
            .enumerate()
        {
            det_a.record_checksum(tick as u64, ca);
            det_b.record_checksum(tick as u64, cb);
            det_c.record_checksum(tick as u64, cc);
        }

        // Same sim → no desync
        assert_eq!(det_a.compare(&det_b), None, "Same sim should not desync");
        // Different seed → desync at tick 0
        assert_eq!(det_a.compare(&det_c), Some(0), "Different seeds should desync at tick 0");
    }

    #[test]
    fn deterministic_sim_replay_consistency() {
        // Run a sim, record it as a "replay", re-run with same params, verify match
        let (hash_1, trail_1) = run_deterministic_sim(123, 500);

        // "Replay" — run the exact same simulation again
        let (hash_2, trail_2) = run_deterministic_sim(123, 500);

        assert_eq!(hash_1, hash_2, "Replay must produce identical final hash");

        // Verify every single tick matches
        for (tick, (a, b)) in trail_1.iter().zip(trail_2.iter()).enumerate() {
            assert_eq!(a, b, "Tick {tick} mismatch in replay");
        }
    }

    #[test]
    fn fixed_point_arithmetic_is_deterministic() {
        use amigo_core::math::Fix;

        // Verify that repeated fixed-point operations produce identical results
        let a = Fix::from_num(7);
        let b = Fix::from_num(3);
        let result_1 = a * b + Fix::from_num(1);
        let result_2 = a * b + Fix::from_num(1);
        assert_eq!(result_1, result_2);

        // Division is also deterministic
        let div_1 = a / b;
        let div_2 = a / b;
        assert_eq!(div_1, div_2);

        // Hash the bit representation to verify
        let mut h1 = StateHasher::new();
        h1.write_i32(result_1.to_bits());
        h1.write_i32(div_1.to_bits());

        let mut h2 = StateHasher::new();
        h2.write_i32(result_2.to_bits());
        h2.write_i32(div_2.to_bits());

        assert_eq!(h1.finish_crc(), h2.finish_crc());
    }
}
