use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// CRC32 (simple table-based implementation)
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

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[index];
    }
    crc ^ 0xFFFF_FFFF
}

// ---------------------------------------------------------------------------
// SaveError
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializeError(String),

    #[error("Deserialization error: {0}")]
    DeserializeError(String),

    #[error("Corrupted save in slot {0}")]
    CorruptedSave(u32),

    #[error("Slot {0} not found")]
    SlotNotFound(u32),
}

// ---------------------------------------------------------------------------
// SaveConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveConfig {
    #[serde(default = "default_max_slots")]
    pub max_slots: u32,

    #[serde(default = "default_autosave_slots")]
    pub autosave_slots: u32,

    #[serde(default = "default_autosave_interval_secs")]
    pub autosave_interval_secs: f64,

    pub app_name: String,

    #[serde(default = "default_compression")]
    pub compression: bool,
}

fn default_max_slots() -> u32 {
    10
}
fn default_autosave_slots() -> u32 {
    3
}
fn default_autosave_interval_secs() -> f64 {
    300.0
}
fn default_compression() -> bool {
    true
}

// ---------------------------------------------------------------------------
// SlotInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotInfo {
    pub slot_id: u32,
    pub label: String,
    pub timestamp: u64,
    pub play_time_secs: f64,
    pub checksum: u32,
    pub is_autosave: bool,
}

// ---------------------------------------------------------------------------
// SaveManager
// ---------------------------------------------------------------------------

pub struct SaveManager {
    config: SaveConfig,
    /// Index that rotates through autosave slots (1-based slot ids).
    next_autosave_index: u32,
    /// Accumulated elapsed time since last autosave (seconds).
    time_since_autosave: f64,
}

impl SaveManager {
    /// Create a new `SaveManager` with the given configuration.
    pub fn new(config: SaveConfig) -> Self {
        Self {
            config,
            next_autosave_index: 0,
            time_since_autosave: 0.0,
        }
    }

    /// Returns the platform-aware save directory for this application.
    ///
    /// - Linux: `~/.local/share/{app_name}/saves`
    /// - Windows: `%APPDATA%/{app_name}/saves`
    /// - Fallback: `./saves`
    pub fn save_dir(&self) -> PathBuf {
        let base = if cfg!(target_os = "linux") {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join(&self.config.app_name)
            } else {
                PathBuf::from(".")
            }
        } else if cfg!(target_os = "windows") {
            if let Ok(appdata) = std::env::var("APPDATA") {
                PathBuf::from(appdata).join(&self.config.app_name)
            } else {
                PathBuf::from(".")
            }
        } else {
            // macOS / other: fallback
            PathBuf::from(".")
        };

        base.join("saves")
    }

    /// Returns the directory path for a specific slot.
    fn slot_dir(&self, slot: u32) -> PathBuf {
        self.save_dir().join(format!("slot_{slot}"))
    }

    /// Save game data into a numbered slot.
    pub fn save<T: Serialize>(
        &self,
        slot: u32,
        label: &str,
        data: &T,
        play_time: f64,
    ) -> Result<(), SaveError> {
        self.save_internal(slot, label, data, play_time, false)
    }

    /// Load game data from a numbered slot, verifying the CRC checksum.
    pub fn load<T: DeserializeOwned>(&self, slot: u32) -> Result<T, SaveError> {
        let dir = self.slot_dir(slot);
        let meta_path = dir.join("meta.json");
        let data_path = dir.join("data.json");

        if !meta_path.exists() || !data_path.exists() {
            return Err(SaveError::SlotNotFound(slot));
        }

        let meta_bytes = fs::read(&meta_path)?;
        let info: SlotInfo = serde_json::from_slice(&meta_bytes)
            .map_err(|e| SaveError::DeserializeError(e.to_string()))?;

        let data_bytes = fs::read(&data_path)?;
        let checksum = crc32(&data_bytes);

        if checksum != info.checksum {
            return Err(SaveError::CorruptedSave(slot));
        }

        let value: T = serde_json::from_slice(&data_bytes)
            .map_err(|e| SaveError::DeserializeError(e.to_string()))?;

        Ok(value)
    }

    /// List metadata for all occupied save slots without loading full save data.
    pub fn list_slots(&self) -> Vec<SlotInfo> {
        let save_dir = self.save_dir();
        let mut slots = Vec::new();

        if !save_dir.exists() {
            return slots;
        }

        let entries = match fs::read_dir(&save_dir) {
            Ok(e) => e,
            Err(_) => return slots,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let meta_path = path.join("meta.json");
            if !meta_path.exists() {
                continue;
            }
            if let Ok(bytes) = fs::read(&meta_path) {
                if let Ok(info) = serde_json::from_slice::<SlotInfo>(&bytes) {
                    slots.push(info);
                }
            }
        }

        slots.sort_by_key(|s| s.slot_id);
        slots
    }

    /// Delete a save slot and its directory.
    pub fn delete_slot(&self, slot: u32) -> Result<(), SaveError> {
        let dir = self.slot_dir(slot);
        if !dir.exists() {
            return Err(SaveError::SlotNotFound(slot));
        }
        fs::remove_dir_all(&dir)?;
        Ok(())
    }

    /// Quicksave into slot 0.
    pub fn quicksave<T: Serialize>(&self, data: &T, play_time: f64) -> Result<(), SaveError> {
        self.save_internal(0, "Quicksave", data, play_time, false)
    }

    /// Quickload from slot 0.
    pub fn quickload<T: DeserializeOwned>(&self) -> Result<T, SaveError> {
        self.load(0)
    }

    /// Autosave with rotating slot ids.
    ///
    /// Autosave slots use ids starting from `max_slots + 1` up to
    /// `max_slots + autosave_slots`. Returns the slot id used.
    pub fn autosave<T: Serialize>(&mut self, data: &T, play_time: f64) -> Result<u32, SaveError> {
        let index = self.next_autosave_index;
        self.next_autosave_index = (index + 1) % self.config.autosave_slots;
        self.time_since_autosave = 0.0;

        // Autosave slots live beyond the normal slot range.
        let slot = self.config.max_slots + 1 + index;
        let label = format!("Autosave {}", index + 1);
        self.save_internal(slot, &label, data, play_time, true)?;
        Ok(slot)
    }

    /// Returns `true` when enough time has passed for an autosave.
    ///
    /// Call this every frame / tick with the frame delta. When it returns
    /// `true` you should call [`autosave`](Self::autosave).
    pub fn should_autosave(&mut self, elapsed: f64) -> bool {
        self.time_since_autosave += elapsed;
        self.time_since_autosave >= self.config.autosave_interval_secs
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn save_internal<T: Serialize>(
        &self,
        slot: u32,
        label: &str,
        data: &T,
        play_time: f64,
        is_autosave: bool,
    ) -> Result<(), SaveError> {
        let dir = self.slot_dir(slot);
        fs::create_dir_all(&dir)?;

        let data_bytes = serde_json::to_vec_pretty(data)
            .map_err(|e| SaveError::SerializeError(e.to_string()))?;

        let checksum = crc32(&data_bytes);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let info = SlotInfo {
            slot_id: slot,
            label: label.to_string(),
            timestamp,
            play_time_secs: play_time,
            checksum,
            is_autosave,
        };

        let meta_bytes = serde_json::to_vec_pretty(&info)
            .map_err(|e| SaveError::SerializeError(e.to_string()))?;

        // Write data first, then metadata – if we crash between the two writes
        // the slot will appear missing rather than corrupted.
        fs::write(dir.join("data.json"), &data_bytes)?;
        fs::write(dir.join("meta.json"), &meta_bytes)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_test_dir() -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("amigo_test_{}_{}", pid, id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct FakeState {
        level: u32,
        health: f64,
        name: String,
    }

    // ── CRC32 checksum ──────────────────────────────────────

    #[test]
    fn crc32_basic() {
        let data = b"hello world";
        let c = crc32(data);
        // Known CRC32 for "hello world"
        assert_eq!(c, 0x0D4A_1185);
    }

    // ── Save / load round trip ──────────────────────────────

    #[test]
    fn save_and_load_round_trip() {
        let tmp = temp_test_dir();

        let state = FakeState {
            level: 5,
            health: 87.5,
            name: "Hero".to_string(),
        };

        let dir = tmp.join("slot_1");
        fs::create_dir_all(&dir).unwrap();

        let data_bytes = serde_json::to_vec_pretty(&state).unwrap();
        let checksum = crc32(&data_bytes);

        let info = SlotInfo {
            slot_id: 1,
            label: "Test".into(),
            timestamp: 0,
            play_time_secs: 42.0,
            checksum,
            is_autosave: false,
        };

        fs::write(dir.join("data.json"), &data_bytes).unwrap();
        fs::write(
            dir.join("meta.json"),
            serde_json::to_vec_pretty(&info).unwrap(),
        )
        .unwrap();

        // Read back the data and verify checksum.
        let meta: SlotInfo =
            serde_json::from_slice(&fs::read(dir.join("meta.json")).unwrap()).unwrap();
        let raw = fs::read(dir.join("data.json")).unwrap();
        assert_eq!(crc32(&raw), meta.checksum);

        let loaded: FakeState = serde_json::from_slice(&raw).unwrap();
        assert_eq!(loaded, state);

        let _ = fs::remove_dir_all(&tmp);
    }

    // ── Autosave timing ─────────────────────────────────────

    #[test]
    fn should_autosave_timing() {
        let tmp = temp_test_dir();
        let config = SaveConfig {
            max_slots: 5,
            autosave_slots: 2,
            autosave_interval_secs: 10.0,
            app_name: "test_autosave".to_string(),
            compression: false,
        };
        let mut mgr = SaveManager::new(config);

        assert!(!mgr.should_autosave(5.0));
        assert!(!mgr.should_autosave(4.0));
        // 5 + 4 + 2 = 11 >= 10
        assert!(mgr.should_autosave(2.0));

        let _ = fs::remove_dir_all(&tmp);
    }
}
