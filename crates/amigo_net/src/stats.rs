//! Network statistics tracking.
//!
//! Collects RTT, packet loss, bandwidth, and jitter measurements
//! for the network debug overlay and connection quality monitoring.

use std::collections::VecDeque;

/// Rolling window size for statistics.
const WINDOW_SIZE: usize = 120;

/// Collected network statistics for a connection.
#[derive(Clone, Debug)]
pub struct NetStats {
    /// Round-trip time samples (milliseconds).
    rtt_samples: VecDeque<f32>,
    /// Packets sent in the current window.
    packets_sent: u32,
    /// Packets acknowledged in the current window.
    packets_acked: u32,
    /// Total packets lost (across all time).
    packets_lost_total: u64,
    /// Total packets sent (across all time).
    packets_sent_total: u64,
    /// Bytes sent in the current window.
    bytes_sent: VecDeque<u32>,
    /// Bytes received in the current window.
    bytes_recv: VecDeque<u32>,
    /// Per-tick byte counters for bandwidth calculation.
    current_bytes_sent: u32,
    current_bytes_recv: u32,
}

impl NetStats {
    pub fn new() -> Self {
        Self {
            rtt_samples: VecDeque::with_capacity(WINDOW_SIZE),
            packets_sent: 0,
            packets_acked: 0,
            packets_lost_total: 0,
            packets_sent_total: 0,
            bytes_sent: VecDeque::with_capacity(WINDOW_SIZE),
            bytes_recv: VecDeque::with_capacity(WINDOW_SIZE),
            current_bytes_sent: 0,
            current_bytes_recv: 0,
        }
    }

    /// Record a RTT sample (in milliseconds).
    pub fn record_rtt(&mut self, rtt_ms: f32) {
        if self.rtt_samples.len() >= WINDOW_SIZE {
            self.rtt_samples.pop_front();
        }
        self.rtt_samples.push_back(rtt_ms);
    }

    /// Record that a packet was sent.
    pub fn record_send(&mut self, bytes: u32) {
        self.packets_sent += 1;
        self.packets_sent_total += 1;
        self.current_bytes_sent += bytes;
    }

    /// Record that a packet was acknowledged.
    pub fn record_ack(&mut self) {
        self.packets_acked += 1;
    }

    /// Record packet loss (e.g., detected by sequence gap).
    pub fn record_loss(&mut self, count: u32) {
        self.packets_lost_total += count as u64;
    }

    /// Record received bytes.
    pub fn record_recv(&mut self, bytes: u32) {
        self.current_bytes_recv += bytes;
    }

    /// Call once per tick to finalize bandwidth counters.
    pub fn end_tick(&mut self) {
        if self.bytes_sent.len() >= WINDOW_SIZE {
            self.bytes_sent.pop_front();
        }
        if self.bytes_recv.len() >= WINDOW_SIZE {
            self.bytes_recv.pop_front();
        }
        self.bytes_sent.push_back(self.current_bytes_sent);
        self.bytes_recv.push_back(self.current_bytes_recv);
        self.current_bytes_sent = 0;
        self.current_bytes_recv = 0;
    }

    // --- Computed metrics ---

    /// Average RTT in milliseconds.
    pub fn rtt_avg(&self) -> f32 {
        if self.rtt_samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.rtt_samples.iter().sum();
        sum / self.rtt_samples.len() as f32
    }

    /// Minimum RTT in the window.
    pub fn rtt_min(&self) -> f32 {
        self.rtt_samples
            .iter()
            .copied()
            .reduce(f32::min)
            .unwrap_or(0.0)
    }

    /// Maximum RTT in the window.
    pub fn rtt_max(&self) -> f32 {
        self.rtt_samples
            .iter()
            .copied()
            .reduce(f32::max)
            .unwrap_or(0.0)
    }

    /// Jitter: standard deviation of RTT samples.
    pub fn jitter(&self) -> f32 {
        if self.rtt_samples.len() < 2 {
            return 0.0;
        }
        let avg = self.rtt_avg();
        let variance: f32 = self
            .rtt_samples
            .iter()
            .map(|&r| (r - avg) * (r - avg))
            .sum::<f32>()
            / self.rtt_samples.len() as f32;
        variance.sqrt()
    }

    /// Packet loss ratio in the current window (0.0 - 1.0).
    pub fn packet_loss(&self) -> f32 {
        if self.packets_sent == 0 {
            return 0.0;
        }
        let lost = self.packets_sent.saturating_sub(self.packets_acked);
        lost as f32 / self.packets_sent as f32
    }

    /// Total packet loss ratio across all time.
    pub fn packet_loss_total(&self) -> f32 {
        if self.packets_sent_total == 0 {
            return 0.0;
        }
        self.packets_lost_total as f32 / self.packets_sent_total as f32
    }

    /// Average bytes sent per tick over the window.
    pub fn bandwidth_send(&self) -> f32 {
        if self.bytes_sent.is_empty() {
            return 0.0;
        }
        let sum: u32 = self.bytes_sent.iter().sum();
        sum as f32 / self.bytes_sent.len() as f32
    }

    /// Average bytes received per tick over the window.
    pub fn bandwidth_recv(&self) -> f32 {
        if self.bytes_recv.is_empty() {
            return 0.0;
        }
        let sum: u32 = self.bytes_recv.iter().sum();
        sum as f32 / self.bytes_recv.len() as f32
    }

    /// Connection quality rating.
    pub fn quality(&self) -> ConnectionQuality {
        let rtt = self.rtt_avg();
        let loss = self.packet_loss();

        if rtt < 50.0 && loss < 0.01 {
            ConnectionQuality::Excellent
        } else if rtt < 100.0 && loss < 0.03 {
            ConnectionQuality::Good
        } else if rtt < 200.0 && loss < 0.10 {
            ConnectionQuality::Fair
        } else {
            ConnectionQuality::Poor
        }
    }

    /// Reset per-window counters (call between matches).
    pub fn reset_window(&mut self) {
        self.rtt_samples.clear();
        self.packets_sent = 0;
        self.packets_acked = 0;
        self.bytes_sent.clear();
        self.bytes_recv.clear();
        self.current_bytes_sent = 0;
        self.current_bytes_recv = 0;
    }
}

impl Default for NetStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Overall connection quality rating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── RTT statistics ──────────────────────────────────────────

    #[test]
    fn rtt_stats() {
        let mut stats = NetStats::new();
        stats.record_rtt(10.0);
        stats.record_rtt(20.0);
        stats.record_rtt(30.0);

        assert_eq!(stats.rtt_avg(), 20.0);
        assert_eq!(stats.rtt_min(), 10.0);
        assert_eq!(stats.rtt_max(), 30.0);
    }

    #[test]
    fn jitter_calculation() {
        let mut stats = NetStats::new();
        // All same → zero jitter
        for _ in 0..10 {
            stats.record_rtt(50.0);
        }
        assert!(stats.jitter() < 0.001);

        // Variable → non-zero jitter
        let mut stats2 = NetStats::new();
        stats2.record_rtt(10.0);
        stats2.record_rtt(90.0);
        assert!(stats2.jitter() > 30.0);
    }

    // ── Packet loss ─────────────────────────────────────────────

    #[test]
    fn packet_loss_ratio() {
        let mut stats = NetStats::new();
        for _ in 0..100 {
            stats.record_send(100);
        }
        for _ in 0..95 {
            stats.record_ack();
        }
        let loss = stats.packet_loss();
        assert!((loss - 0.05).abs() < 0.001);
    }

    // ── Bandwidth ───────────────────────────────────────────────

    #[test]
    fn bandwidth_tracking() {
        let mut stats = NetStats::new();
        stats.record_send(500);
        stats.record_recv(300);
        stats.end_tick();

        stats.record_send(600);
        stats.record_recv(400);
        stats.end_tick();

        assert_eq!(stats.bandwidth_send(), 550.0);
        assert_eq!(stats.bandwidth_recv(), 350.0);
    }

    // ── Connection quality ──────────────────────────────────────

    #[test]
    fn connection_quality() {
        let mut stats = NetStats::new();
        stats.record_rtt(20.0);
        assert_eq!(stats.quality(), ConnectionQuality::Excellent);

        let mut stats2 = NetStats::new();
        stats2.record_rtt(150.0);
        assert_eq!(stats2.quality(), ConnectionQuality::Fair);

        let mut stats3 = NetStats::new();
        stats3.record_rtt(300.0);
        assert_eq!(stats3.quality(), ConnectionQuality::Poor);
    }

    // ── Edge cases & reset ──────────────────────────────────────

    #[test]
    fn empty_stats() {
        let stats = NetStats::new();
        assert_eq!(stats.rtt_avg(), 0.0);
        assert_eq!(stats.jitter(), 0.0);
        assert_eq!(stats.packet_loss(), 0.0);
        assert_eq!(stats.bandwidth_send(), 0.0);
    }

    #[test]
    fn reset_window() {
        let mut stats = NetStats::new();
        stats.record_rtt(50.0);
        stats.record_send(100);
        stats.record_ack();
        stats.end_tick();

        stats.reset_window();
        assert_eq!(stats.rtt_avg(), 0.0);
        assert_eq!(stats.bandwidth_send(), 0.0);
    }
}
