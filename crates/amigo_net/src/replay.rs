use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

/// A single tick's worth of recorded commands.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayFrame {
    /// The simulation tick this frame represents.
    pub tick: u64,
    /// Serialized commands for this tick (each inner Vec<u8> is one command batch).
    pub commands: Vec<Vec<u8>>,
}

/// Complete replay data that can be saved to and loaded from disk.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayData {
    /// Format version for forward compatibility.
    pub version: u32,
    /// The RNG seed used for this session.
    pub seed: u64,
    /// Total number of ticks in the replay.
    pub total_ticks: u64,
    /// All recorded frames.
    pub frames: Vec<ReplayFrame>,
    /// Arbitrary key-value metadata (map name, player names, etc.).
    pub metadata: FxHashMap<String, String>,
}

impl ReplayData {
    /// Save the replay to a file as JSON.
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let json = serde_json::to_vec(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// Load a replay from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let data = std::fs::read(path)?;
        serde_json::from_slice(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

/// Records replay frames as the game runs.
pub struct ReplayRecorder {
    frames: Vec<ReplayFrame>,
    metadata: FxHashMap<String, String>,
    seed: u64,
    start_time: Instant,
}

impl ReplayRecorder {
    /// Create a new recorder. Call this at the start of a session.
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            metadata: FxHashMap::default(),
            seed: 0,
            start_time: Instant::now(),
        }
    }

    /// Set the RNG seed for the replay.
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = seed;
    }

    /// Insert or update a metadata entry.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Record the commands for a given tick.
    pub fn record_tick(&mut self, tick: u64, commands: Vec<Vec<u8>>) {
        self.frames.push(ReplayFrame { tick, commands });
    }

    /// How long the recording has been running.
    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Finish recording and produce the final `ReplayData`.
    pub fn finish(mut self) -> ReplayData {
        let total_ticks = self.frames.last().map(|f| f.tick + 1).unwrap_or(0);
        self.metadata.insert(
            "duration_secs".into(),
            format!("{:.2}", self.elapsed_secs()),
        );
        ReplayData {
            version: 1,
            seed: self.seed,
            total_ticks,
            frames: self.frames,
            metadata: self.metadata,
        }
    }
}

impl Default for ReplayRecorder {
    fn default() -> Self {
        Self::new()
    }
}

/// Plays back a previously recorded replay frame-by-frame.
pub struct ReplayPlayer {
    data: ReplayData,
    /// Index into `data.frames` for the current playback position.
    cursor: usize,
    speed: f32,
}

impl ReplayPlayer {
    /// Create a new player from replay data.
    pub fn new(data: ReplayData) -> Self {
        Self {
            data,
            cursor: 0,
            speed: 1.0,
        }
    }

    /// Return the current frame, or `None` if playback is finished.
    pub fn tick(&self) -> Option<&ReplayFrame> {
        self.data.frames.get(self.cursor)
    }

    /// Advance to the next frame.
    pub fn advance(&mut self) {
        if self.cursor < self.data.frames.len() {
            self.cursor += 1;
        }
    }

    /// Seek to the frame whose tick equals `target_tick`.
    /// If no exact match, seeks to the first frame with tick >= target_tick.
    /// If target_tick is beyond all frames, seeks to the end (finished state).
    pub fn seek(&mut self, target_tick: u64) {
        match self
            .data
            .frames
            .binary_search_by_key(&target_tick, |f| f.tick)
        {
            Ok(idx) => self.cursor = idx,
            Err(idx) => self.cursor = idx,
        }
    }

    /// Returns `true` when all frames have been consumed.
    pub fn is_finished(&self) -> bool {
        self.cursor >= self.data.frames.len()
    }

    /// The tick number of the current frame, or `None` if finished.
    pub fn current_tick(&self) -> Option<u64> {
        self.data.frames.get(self.cursor).map(|f| f.tick)
    }

    /// Total number of ticks in the replay.
    pub fn total_ticks(&self) -> u64 {
        self.data.total_ticks
    }

    /// Set the playback speed multiplier (e.g. 2.0 for double speed).
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    /// Get the current playback speed multiplier.
    pub fn speed(&self) -> f32 {
        self.speed
    }
}

/// Detects simulation desyncs by comparing per-tick checksums between two
/// participants (e.g. client vs. server, or two clients in a replay).
pub struct DesyncDetector {
    checksums: FxHashMap<u64, u64>,
}

impl DesyncDetector {
    pub fn new() -> Self {
        Self {
            checksums: FxHashMap::default(),
        }
    }

    /// Record a checksum for a given tick.
    pub fn record_checksum(&mut self, tick: u64, checksum: u64) {
        self.checksums.insert(tick, checksum);
    }

    /// Compare against another detector. Returns the first tick (lowest)
    /// where the two detectors recorded different checksums, or `None` if
    /// all shared ticks match.
    pub fn compare(&self, other: &DesyncDetector) -> Option<u64> {
        let mut first_mismatch: Option<u64> = None;
        for (&tick, &checksum) in &self.checksums {
            if let Some(&other_checksum) = other.checksums.get(&tick) {
                if checksum != other_checksum {
                    first_mismatch = Some(match first_mismatch {
                        Some(prev) => prev.min(tick),
                        None => tick,
                    });
                }
            }
        }
        first_mismatch
    }
}

impl Default for DesyncDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_playback() {
        let mut recorder = ReplayRecorder::new();
        recorder.set_seed(42);
        recorder.set_metadata("map", "arena");

        recorder.record_tick(0, vec![b"move_left".to_vec()]);
        recorder.record_tick(1, vec![b"attack".to_vec(), b"jump".to_vec()]);
        recorder.record_tick(2, vec![]);

        let data = recorder.finish();
        assert_eq!(data.version, 1);
        assert_eq!(data.seed, 42);
        assert_eq!(data.total_ticks, 3);
        assert_eq!(data.frames.len(), 3);
        assert_eq!(data.metadata.get("map").unwrap(), "arena");

        let mut player = ReplayPlayer::new(data);
        assert!(!player.is_finished());
        assert_eq!(player.current_tick(), Some(0));
        assert_eq!(player.total_ticks(), 3);

        // Frame 0
        let frame = player.tick().unwrap();
        assert_eq!(frame.tick, 0);
        assert_eq!(frame.commands, vec![b"move_left".to_vec()]);
        player.advance();

        // Frame 1
        let frame = player.tick().unwrap();
        assert_eq!(frame.tick, 1);
        assert_eq!(frame.commands.len(), 2);
        player.advance();

        // Frame 2
        let frame = player.tick().unwrap();
        assert_eq!(frame.tick, 2);
        assert!(frame.commands.is_empty());
        player.advance();

        // Finished
        assert!(player.is_finished());
        assert!(player.tick().is_none());
        assert_eq!(player.current_tick(), None);
    }

    #[test]
    fn serialization_roundtrip() {
        let mut recorder = ReplayRecorder::new();
        recorder.set_seed(999);
        recorder.set_metadata("player", "alice");
        recorder.record_tick(0, vec![b"cmd1".to_vec()]);
        recorder.record_tick(1, vec![b"cmd2".to_vec()]);

        let data = recorder.finish();

        // Save to a temp file and load back
        let dir = std::env::temp_dir().join("amigo_replay_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_replay.json");

        data.save(&path).unwrap();
        let loaded = ReplayData::load(&path).unwrap();

        assert_eq!(loaded.version, data.version);
        assert_eq!(loaded.seed, data.seed);
        assert_eq!(loaded.total_ticks, data.total_ticks);
        assert_eq!(loaded.frames.len(), data.frames.len());
        assert_eq!(loaded.frames[0].tick, 0);
        assert_eq!(loaded.frames[0].commands, vec![b"cmd1".to_vec()]);
        assert_eq!(loaded.frames[1].tick, 1);
        assert_eq!(loaded.metadata.get("player").unwrap(), "alice");

        // Also test in-memory JSON roundtrip
        let json = serde_json::to_string(&data).unwrap();
        let from_json: ReplayData = serde_json::from_str(&json).unwrap();
        assert_eq!(from_json.seed, 999);
        assert_eq!(from_json.frames.len(), 2);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn seek_to_specific_tick() {
        let mut recorder = ReplayRecorder::new();
        for i in 0..10 {
            recorder.record_tick(i, vec![format!("tick_{}", i).into_bytes()]);
        }
        let data = recorder.finish();
        let mut player = ReplayPlayer::new(data);

        // Seek to tick 5
        player.seek(5);
        assert_eq!(player.current_tick(), Some(5));
        let frame = player.tick().unwrap();
        assert_eq!(frame.commands[0], b"tick_5");

        // Seek to tick 0
        player.seek(0);
        assert_eq!(player.current_tick(), Some(0));

        // Seek to tick 9 (last)
        player.seek(9);
        assert_eq!(player.current_tick(), Some(9));
        player.advance();
        assert!(player.is_finished());

        // Seek beyond the end
        player.seek(100);
        assert!(player.is_finished());

        // Seek back to 3
        player.seek(3);
        assert_eq!(player.current_tick(), Some(3));
        assert!(!player.is_finished());
    }

    #[test]
    fn desync_detector_finds_mismatch() {
        let mut detector_a = DesyncDetector::new();
        let mut detector_b = DesyncDetector::new();

        // Ticks 0-4 match
        for tick in 0..5 {
            let checksum = tick * 100 + 7;
            detector_a.record_checksum(tick, checksum);
            detector_b.record_checksum(tick, checksum);
        }

        // No desync yet
        assert_eq!(detector_a.compare(&detector_b), None);

        // Tick 5: desync!
        detector_a.record_checksum(5, 0xAAAA);
        detector_b.record_checksum(5, 0xBBBB);

        // Tick 6 also differs
        detector_a.record_checksum(6, 0x1111);
        detector_b.record_checksum(6, 0x2222);

        // Should report tick 5 as the first mismatch
        assert_eq!(detector_a.compare(&detector_b), Some(5));
        assert_eq!(detector_b.compare(&detector_a), Some(5));
    }

    #[test]
    fn desync_detector_no_overlap() {
        let mut detector_a = DesyncDetector::new();
        let mut detector_b = DesyncDetector::new();

        detector_a.record_checksum(0, 100);
        detector_b.record_checksum(1, 200);

        // No shared ticks, so no desync detected
        assert_eq!(detector_a.compare(&detector_b), None);
    }

    #[test]
    fn replay_player_speed() {
        let data = ReplayData {
            version: 1,
            seed: 0,
            total_ticks: 0,
            frames: vec![],
            metadata: FxHashMap::default(),
        };
        let mut player = ReplayPlayer::new(data);
        assert_eq!(player.speed(), 1.0);
        player.set_speed(2.0);
        assert_eq!(player.speed(), 2.0);
        player.set_speed(0.5);
        assert_eq!(player.speed(), 0.5);
    }
}
