//! State Rewind: frame-by-frame game state recording and time-rewind.
//!
//! Records game state snapshots into a ring buffer and allows rewinding
//! to any recorded frame. Enables Braid-style time-rewind mechanics and
//! debugging tools.

use serde::{de::DeserializeOwned, Serialize};

// ---------------------------------------------------------------------------
// Compression Mode
// ---------------------------------------------------------------------------

/// Compression strategy for the rewind buffer.
#[derive(Clone, Copy, Debug)]
pub enum CompressionMode {
    /// Store every frame as a full snapshot. Simple but memory-heavy.
    None,
    /// Store a full keyframe every N frames, deltas in between.
    Delta { keyframe_interval: u32 },
}

impl Default for CompressionMode {
    fn default() -> Self {
        CompressionMode::Delta {
            keyframe_interval: 30,
        }
    }
}

// ---------------------------------------------------------------------------
// RewindStats
// ---------------------------------------------------------------------------

/// Performance metrics for the last rewind operation.
#[derive(Clone, Debug, Default)]
pub struct RewindStats {
    pub last_record_us: u64,
    pub last_rewind_us: u64,
    pub memory_bytes: usize,
    pub keyframe_count: usize,
    pub delta_count: usize,
    pub avg_delta_bytes: usize,
}

// ---------------------------------------------------------------------------
// Internal Frame Storage
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum FrameData<T: Clone> {
    Full(T),
    Delta(Vec<u8>), // Byte-level diff patches
}

#[derive(Clone)]
struct RewindFrame<T: Clone> {
    tick: u64,
    data: FrameData<T>,
}

// ---------------------------------------------------------------------------
// RewindBuffer
// ---------------------------------------------------------------------------

/// Ring buffer that stores a sliding window of game state snapshots.
pub struct RewindBuffer<T: Clone> {
    frames: Vec<Option<RewindFrame<T>>>,
    capacity: usize,
    head: usize,
    len: usize,
    current_tick: u64,
    recording: bool,
    compression: CompressionMode,
    last_stats: RewindStats,
    /// Cache of the last full state for delta computation.
    last_full_bytes: Option<Vec<u8>>,
}

impl<T: Clone + Serialize + DeserializeOwned + PartialEq> RewindBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self::with_compression(capacity, CompressionMode::default())
    }

    pub fn with_compression(capacity: usize, mode: CompressionMode) -> Self {
        let cap = capacity.max(1);
        Self {
            frames: (0..cap).map(|_| None).collect(),
            capacity: cap,
            head: 0,
            len: 0,
            current_tick: 0,
            recording: true,
            compression: mode,
            last_stats: RewindStats::default(),
            last_full_bytes: None,
        }
    }

    /// Record the current state as the next frame.
    pub fn record(&mut self, state: &T) -> u64 {
        if !self.recording {
            return self.current_tick;
        }

        let start = std::time::Instant::now();
        self.current_tick += 1;
        let tick = self.current_tick;

        let is_keyframe = match self.compression {
            CompressionMode::None => true,
            CompressionMode::Delta { keyframe_interval } => {
                tick % keyframe_interval as u64 == 0 || self.len == 0
            }
        };

        let frame_data = if is_keyframe {
            let bytes = serde_json::to_vec(state).unwrap_or_default();
            self.last_full_bytes = Some(bytes);
            FrameData::Full(state.clone())
        } else {
            // Delta: serialize current, diff against last
            let current_bytes = serde_json::to_vec(state).unwrap_or_default();
            let delta = if let Some(ref prev_bytes) = self.last_full_bytes {
                compute_delta(prev_bytes, &current_bytes)
            } else {
                current_bytes.clone()
            };
            self.last_full_bytes = Some(current_bytes);
            FrameData::Delta(delta)
        };

        let write_idx = (self.head + self.len) % self.capacity;
        self.frames[write_idx] = Some(RewindFrame {
            tick,
            data: frame_data,
        });

        if self.len < self.capacity {
            self.len += 1;
        } else {
            self.head = (self.head + 1) % self.capacity;
        }

        self.update_stats();
        self.last_stats.last_record_us = start.elapsed().as_micros() as u64;
        tick
    }

    /// Rewind to a specific tick. Returns the reconstructed state.
    pub fn rewind_to(&self, tick: u64) -> Option<T> {
        let start = std::time::Instant::now();
        let result = self.reconstruct(tick);
        // Can't update stats in &self, but that's fine for const access
        let _ = start;
        result
    }

    /// Step one frame backward.
    pub fn step_back(&mut self) -> Option<T> {
        if self.current_tick <= self.oldest_tick().unwrap_or(0) {
            return None;
        }
        self.current_tick -= 1;
        self.reconstruct(self.current_tick)
    }

    /// Step one frame forward.
    pub fn step_forward(&mut self) -> Option<T> {
        if self.current_tick >= self.newest_tick().unwrap_or(0) {
            return None;
        }
        self.current_tick += 1;
        self.reconstruct(self.current_tick)
    }

    /// Get the state at the current tick.
    pub fn current_state(&self) -> Option<T> {
        self.reconstruct(self.current_tick)
    }

    pub fn start_recording(&mut self) {
        self.recording = true;
    }

    pub fn stop_recording(&mut self) {
        self.recording = false;
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn clear(&mut self) {
        self.frames.iter_mut().for_each(|f| *f = None);
        self.head = 0;
        self.len = 0;
        self.current_tick = 0;
        self.last_full_bytes = None;
        self.last_stats = RewindStats::default();
    }

    pub fn oldest_tick(&self) -> Option<u64> {
        if self.len == 0 {
            return None;
        }
        self.frames[self.head].as_ref().map(|f| f.tick)
    }

    pub fn newest_tick(&self) -> Option<u64> {
        if self.len == 0 {
            return None;
        }
        let idx = (self.head + self.len - 1) % self.capacity;
        self.frames[idx].as_ref().map(|f| f.tick)
    }

    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn last_stats(&self) -> &RewindStats {
        &self.last_stats
    }

    // ── Internal ────────────────────────────────────────

    fn frame_at_index(&self, buffer_idx: usize) -> Option<&RewindFrame<T>> {
        self.frames[buffer_idx].as_ref()
    }

    fn find_frame_index(&self, tick: u64) -> Option<usize> {
        for i in 0..self.len {
            let idx = (self.head + i) % self.capacity;
            if let Some(frame) = &self.frames[idx] {
                if frame.tick == tick {
                    return Some(idx);
                }
            }
        }
        None
    }

    fn reconstruct(&self, tick: u64) -> Option<T> {
        // Find the target frame
        let target_idx = self.find_frame_index(tick)?;
        let target_frame = self.frame_at_index(target_idx)?;

        match &target_frame.data {
            FrameData::Full(state) => Some(state.clone()),
            FrameData::Delta(_) => {
                // Walk backward to find the nearest keyframe
                let mut chain: Vec<usize> = vec![target_idx];
                let mut search_tick = tick;
                loop {
                    if search_tick == 0 {
                        return None;
                    }
                    search_tick -= 1;
                    let prev_idx = self.find_frame_index(search_tick)?;
                    let prev_frame = self.frame_at_index(prev_idx)?;
                    chain.push(prev_idx);
                    if matches!(prev_frame.data, FrameData::Full(_)) {
                        break;
                    }
                }

                // Reconstruct from keyframe forward
                chain.reverse();
                let keyframe_idx = chain[0];
                let keyframe = self.frame_at_index(keyframe_idx)?;
                let base_state = match &keyframe.data {
                    FrameData::Full(s) => s.clone(),
                    _ => return None,
                };
                let mut bytes = serde_json::to_vec(&base_state).ok()?;

                for &idx in &chain[1..] {
                    let frame = self.frame_at_index(idx)?;
                    if let FrameData::Delta(delta) = &frame.data {
                        bytes = apply_delta(&bytes, delta);
                    }
                }

                serde_json::from_slice(&bytes).ok()
            }
        }
    }

    fn update_stats(&mut self) {
        let mut keyframes = 0;
        let mut deltas = 0;
        let mut total_delta_bytes = 0usize;
        let mut total_memory = 0usize;

        for i in 0..self.len {
            let idx = (self.head + i) % self.capacity;
            if let Some(frame) = &self.frames[idx] {
                match &frame.data {
                    FrameData::Full(state) => {
                        keyframes += 1;
                        total_memory += std::mem::size_of_val(state);
                    }
                    FrameData::Delta(d) => {
                        deltas += 1;
                        total_delta_bytes += d.len();
                        total_memory += d.len();
                    }
                }
            }
        }

        self.last_stats.keyframe_count = keyframes;
        self.last_stats.delta_count = deltas;
        self.last_stats.avg_delta_bytes = if deltas > 0 {
            total_delta_bytes / deltas
        } else {
            0
        };
        self.last_stats.memory_bytes = total_memory;
    }

    /// Discard all frames after the current tick (timeline fork).
    pub fn truncate_after_current(&mut self) {
        let current = self.current_tick;
        let mut new_len = 0;
        for i in 0..self.len {
            let idx = (self.head + i) % self.capacity;
            if let Some(frame) = &self.frames[idx] {
                if frame.tick <= current {
                    new_len = i + 1;
                } else {
                    self.frames[idx] = None;
                }
            }
        }
        self.len = new_len;
    }
}

// ---------------------------------------------------------------------------
// Delta compression (simple byte-level diff)
// ---------------------------------------------------------------------------

/// Compute a compact delta between two byte slices.
/// Format: sequence of (offset: u32, length: u16, bytes: [u8; length]) patches.
fn compute_delta(old: &[u8], new: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let max_len = old.len().max(new.len());
    let mut i = 0;
    while i < max_len {
        // Find start of a differing region
        let old_byte = old.get(i).copied().unwrap_or(0);
        let new_byte = new.get(i).copied().unwrap_or(0);
        if old_byte != new_byte {
            let patch_start = i;
            // Find end of differing region (max 65535 bytes per patch)
            while i < max_len && i - patch_start < 65535 {
                let ob = old.get(i).copied().unwrap_or(0);
                let nb = new.get(i).copied().unwrap_or(0);
                if ob == nb {
                    // Allow up to 4 matching bytes within a patch to avoid tiny patches
                    let lookahead = (i..max_len.min(i + 4))
                        .all(|j| old.get(j).copied().unwrap_or(0) == new.get(j).copied().unwrap_or(0));
                    if lookahead {
                        break;
                    }
                }
                i += 1;
            }
            let patch_len = i - patch_start;
            // Write patch: offset(4 bytes LE) + length(2 bytes LE) + bytes
            result.extend_from_slice(&(patch_start as u32).to_le_bytes());
            result.extend_from_slice(&(patch_len as u16).to_le_bytes());
            for j in patch_start..patch_start + patch_len {
                result.push(new.get(j).copied().unwrap_or(0));
            }
        } else {
            i += 1;
        }
    }
    // Store the total new length at the end (4 bytes LE) so we can resize on apply
    result.extend_from_slice(&(new.len() as u32).to_le_bytes());
    result
}

/// Apply a delta to reconstruct the new byte slice.
fn apply_delta(old: &[u8], delta: &[u8]) -> Vec<u8> {
    if delta.len() < 4 {
        return old.to_vec();
    }
    // Last 4 bytes are the target length
    let target_len =
        u32::from_le_bytes([delta[delta.len() - 4], delta[delta.len() - 3], delta[delta.len() - 2], delta[delta.len() - 1]])
            as usize;
    let mut result = old.to_vec();
    result.resize(target_len, 0);

    let mut pos = 0;
    let patches_end = delta.len() - 4;
    while pos + 6 <= patches_end {
        let offset = u32::from_le_bytes([delta[pos], delta[pos + 1], delta[pos + 2], delta[pos + 3]]) as usize;
        let length = u16::from_le_bytes([delta[pos + 4], delta[pos + 5]]) as usize;
        pos += 6;
        if pos + length > patches_end {
            break;
        }
        for i in 0..length {
            if offset + i < result.len() {
                result[offset + i] = delta[pos + i];
            }
        }
        pos += length;
    }
    result
}

// ---------------------------------------------------------------------------
// RewindController
// ---------------------------------------------------------------------------

/// High-level rewind mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewindMode {
    Playing,
    Rewinding,
    Paused,
    FastForward,
}

/// High-level controller integrating rewind with the simulation loop.
pub struct RewindController<T: Clone + Serialize + DeserializeOwned + PartialEq> {
    buffer: RewindBuffer<T>,
    mode: RewindMode,
    rewind_speed: f32,
    step_accumulator: f32,
}

impl<T: Clone + Serialize + DeserializeOwned + PartialEq> RewindController<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: RewindBuffer::new(capacity),
            mode: RewindMode::Playing,
            rewind_speed: 60.0,
            step_accumulator: 0.0,
        }
    }

    pub fn record(&mut self, state: &T) {
        if self.mode == RewindMode::Playing {
            self.buffer.record(state);
        }
    }

    pub fn begin_rewind(&mut self) {
        self.buffer.stop_recording();
        self.mode = RewindMode::Rewinding;
        self.step_accumulator = 0.0;
    }

    pub fn resume_play(&mut self) {
        self.buffer.truncate_after_current();
        self.buffer.start_recording();
        self.mode = RewindMode::Playing;
    }

    pub fn update(&mut self, dt: f32) -> Option<T> {
        match self.mode {
            RewindMode::Playing => None,
            RewindMode::Rewinding => {
                self.step_accumulator += self.rewind_speed * dt;
                while self.step_accumulator >= 1.0 {
                    self.step_accumulator -= 1.0;
                    if self.buffer.step_back().is_none() {
                        self.mode = RewindMode::Paused;
                        break;
                    }
                }
                self.buffer.current_state()
            }
            RewindMode::Paused => self.buffer.current_state(),
            RewindMode::FastForward => {
                self.step_accumulator += self.rewind_speed * dt;
                while self.step_accumulator >= 1.0 {
                    self.step_accumulator -= 1.0;
                    if self.buffer.step_forward().is_none() {
                        self.resume_play();
                        return None;
                    }
                }
                self.buffer.current_state()
            }
        }
    }

    pub fn set_rewind_speed(&mut self, fps: f32) {
        self.rewind_speed = fps.max(1.0);
    }

    pub fn pause(&mut self) {
        if self.mode == RewindMode::Rewinding || self.mode == RewindMode::FastForward {
            self.mode = RewindMode::Paused;
        }
    }

    pub fn resume_rewind(&mut self) {
        if self.mode == RewindMode::Paused {
            self.mode = RewindMode::Rewinding;
        }
    }

    pub fn step_back(&mut self) -> Option<T> {
        self.buffer.step_back()
    }

    pub fn step_forward(&mut self) -> Option<T> {
        self.buffer.step_forward()
    }

    pub fn mode(&self) -> RewindMode {
        self.mode
    }

    pub fn progress(&self) -> f32 {
        let oldest = self.buffer.oldest_tick().unwrap_or(0) as f32;
        let newest = self.buffer.newest_tick().unwrap_or(0) as f32;
        let current = self.buffer.current_tick() as f32;
        if (newest - oldest).abs() < 0.001 {
            return 1.0;
        }
        ((current - oldest) / (newest - oldest)).clamp(0.0, 1.0)
    }

    pub fn buffer(&self) -> &RewindBuffer<T> {
        &self.buffer
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct GameState {
        x: f32,
        y: f32,
        score: u32,
    }

    #[test]
    fn record_and_rewind() {
        let mut buf = RewindBuffer::with_compression(100, CompressionMode::None);
        for i in 0..10 {
            buf.record(&GameState {
                x: i as f32,
                y: 0.0,
                score: i,
            });
        }
        assert_eq!(buf.len(), 10);

        let state = buf.rewind_to(5).unwrap();
        assert_eq!(state.score, 4); // tick 5 = 5th record (0-indexed score = 4)
    }

    #[test]
    fn step_back_and_forward() {
        let mut buf = RewindBuffer::with_compression(100, CompressionMode::None);
        for i in 0..5 {
            buf.record(&GameState {
                x: i as f32,
                y: 0.0,
                score: i,
            });
        }
        // current_tick = 5
        let s = buf.step_back().unwrap();
        assert_eq!(s.score, 3); // tick 4

        let s = buf.step_forward().unwrap();
        assert_eq!(s.score, 4); // tick 5
    }

    #[test]
    fn ring_buffer_overflow() {
        let mut buf = RewindBuffer::with_compression(5, CompressionMode::None);
        for i in 0..10 {
            buf.record(&GameState {
                x: i as f32,
                y: 0.0,
                score: i,
            });
        }
        assert_eq!(buf.len(), 5);
        assert!(buf.rewind_to(1).is_none()); // Oldest frames evicted
        assert!(buf.rewind_to(6).is_some()); // Recent frames available
    }

    #[test]
    fn delta_compression() {
        let mut buf = RewindBuffer::with_compression(
            100,
            CompressionMode::Delta {
                keyframe_interval: 5,
            },
        );
        for i in 0..10 {
            buf.record(&GameState {
                x: i as f32,
                y: 0.0,
                score: i,
            });
        }
        assert!(buf.last_stats().keyframe_count > 0);
        assert!(buf.last_stats().delta_count > 0);

        // Reconstruct a delta frame
        let state = buf.rewind_to(3).unwrap();
        assert_eq!(state.score, 2);
    }

    #[test]
    fn timeline_fork() {
        let mut buf = RewindBuffer::with_compression(100, CompressionMode::None);
        for i in 0..10 {
            buf.record(&GameState {
                x: i as f32,
                y: 0.0,
                score: i,
            });
        }
        // Rewind to tick 5
        buf.current_tick = 5;
        buf.truncate_after_current();
        assert_eq!(buf.newest_tick(), Some(5));
        assert!(buf.len() <= 5);
    }

    #[test]
    fn controller_lifecycle() {
        let mut ctrl: RewindController<GameState> = RewindController::new(100);
        // Record 10 frames
        for i in 0..10 {
            ctrl.record(&GameState {
                x: i as f32,
                y: 0.0,
                score: i,
            });
        }
        assert_eq!(ctrl.mode(), RewindMode::Playing);

        // Begin rewind
        ctrl.begin_rewind();
        assert_eq!(ctrl.mode(), RewindMode::Rewinding);

        // Step back
        let s = ctrl.step_back().unwrap();
        assert_eq!(s.score, 8); // One step back from tick 10

        // Resume play (forks timeline)
        ctrl.resume_play();
        assert_eq!(ctrl.mode(), RewindMode::Playing);
    }

    #[test]
    fn delta_roundtrip() {
        let old = b"hello world, this is a test string";
        let new = b"hello WORLD, this is a TEST string";
        let delta = compute_delta(old, new);
        let reconstructed = apply_delta(old, &delta);
        assert_eq!(&reconstructed, new);
    }

    #[test]
    fn clear_resets_buffer() {
        let mut buf = RewindBuffer::with_compression(100, CompressionMode::None);
        for i in 0..5 {
            buf.record(&GameState {
                x: i as f32,
                y: 0.0,
                score: i,
            });
        }
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.current_tick(), 0);
    }
}
