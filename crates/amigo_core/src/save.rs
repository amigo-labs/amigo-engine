use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fs;
use std::path::{Path, PathBuf};
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
    /// Overrides the platform save directory (tests, portable installs).
    base_dir: Option<PathBuf>,
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
            base_dir: None,
            next_autosave_index: 0,
            time_since_autosave: 0.0,
        }
    }

    /// A `SaveManager` that keeps its slots directly in `dir` instead of the
    /// platform save directory: for tests, and for portable installs that
    /// save next to the executable.
    pub fn with_base_dir(config: SaveConfig, dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: Some(dir.into()),
            ..Self::new(config)
        }
    }

    /// The configuration this manager was created with.
    pub fn config(&self) -> &SaveConfig {
        &self.config
    }

    /// Returns the platform-aware save directory for this application.
    ///
    /// - Linux: `$XDG_DATA_HOME/{app}/saves`, by default
    ///   `~/.local/share/{app}/saves`
    /// - macOS: `~/Library/Application Support/{app}/saves`
    /// - Windows: `%APPDATA%\{app}\saves`
    /// - Fallback (variable unset, other platforms): `./{app}/saves`
    ///
    /// `{app}` is `app_name` reduced to characters that are safe in a
    /// directory name, so a title like "Space Race: Remix" cannot escape the
    /// data directory or fail to create. With
    /// [`with_base_dir`](Self::with_base_dir), slots go straight into that
    /// directory.
    pub fn save_dir(&self) -> PathBuf {
        if let Some(dir) = &self.base_dir {
            return dir.clone();
        }
        let os = std::env::consts::OS;
        let data =
            data_dir_for(os, |key| std::env::var(key).ok()).unwrap_or_else(|| PathBuf::from("."));
        data.join(sanitize_app_name(&self.config.app_name))
            .join("saves")
    }

    /// Returns the directory path for a specific slot.
    fn slot_dir(&self, slot: u32) -> PathBuf {
        self.save_dir().join(format!("slot_{slot}"))
    }

    /// Where the previous contents of a slot wait while a save replaces it.
    fn previous_slot_dir(&self, slot: u32) -> PathBuf {
        self.save_dir().join(format!("slot_{slot}.old"))
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
    ///
    /// When the slot is missing or corrupt but the previous version of it
    /// survived an interrupted save (see [`save`](Self::save)), that version
    /// is returned instead.
    pub fn load<T: DeserializeOwned>(&self, slot: u32) -> Result<T, SaveError> {
        match load_slot_dir(&self.slot_dir(slot), slot) {
            Err(e @ (SaveError::SlotNotFound(_) | SaveError::CorruptedSave(_))) => {
                let previous = self.previous_slot_dir(slot);
                if previous.exists() {
                    load_slot_dir(&previous, slot)
                } else {
                    Err(e)
                }
            }
            other => other,
        }
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
            // Only `slot_<n>`: the `.tmp` and `.old` directories of a save in
            // progress hold copies of the same slot.
            let is_slot = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix("slot_"))
                .is_some_and(|n| n.parse::<u32>().is_ok());
            if !path.is_dir() || !is_slot {
                continue;
            }
            let meta_path = path.join("meta.json");
            if !meta_path.exists() {
                continue;
            }
            if let Ok(bytes) = fs::read(&meta_path)
                && let Ok(info) = serde_json::from_slice::<SlotInfo>(&bytes)
            {
                slots.push(info);
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
        // A leftover from an interrupted save would otherwise come back as
        // the slot's contents on the next load.
        let _ = fs::remove_dir_all(self.previous_slot_dir(slot));
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
    ///
    /// With `autosave_slots == 0` (autosave disabled) there is no slot to
    /// write, so this returns [`SaveError::SlotNotFound`] instead of saving.
    pub fn autosave<T: Serialize>(&mut self, data: &T, play_time: f64) -> Result<u32, SaveError> {
        if self.config.autosave_slots == 0 {
            return Err(SaveError::SlotNotFound(self.config.max_slots + 1));
        }
        let index = self.next_autosave_index % self.config.autosave_slots;
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
    ///
    /// Always `false` when autosave is disabled (`autosave_slots == 0`).
    pub fn should_autosave(&mut self, elapsed: f64) -> bool {
        if self.config.autosave_slots == 0 {
            return false;
        }
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

        // Replace the slot as a whole. Writing data.json and meta.json in
        // place meant a crash between or during the two writes left a new
        // data.json beside the old meta.json (a checksum mismatch, so the
        // slot read as corrupt) or a truncated data.json: either way the
        // previous save was gone too.
        //
        // Instead: write both files into `slot_N.tmp` and flush them, move
        // the current slot aside to `slot_N.old`, move the new one into
        // place, then delete the old one. A crash at any point leaves either
        // a complete `slot_N` or a complete `slot_N.old`, and `load` falls
        // back to the latter.
        let dir = self.slot_dir(slot);
        let tmp = self.save_dir().join(format!("slot_{slot}.tmp"));
        let previous = self.previous_slot_dir(slot);

        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp)?;
        write_synced(&tmp.join("data.json"), &data_bytes)?;
        write_synced(&tmp.join("meta.json"), &meta_bytes)?;

        if dir.exists() {
            let _ = fs::remove_dir_all(&previous);
            fs::rename(&dir, &previous)?;
        }
        fs::rename(&tmp, &dir)?;
        let _ = fs::remove_dir_all(&previous);

        Ok(())
    }
}

/// Read and verify one slot directory.
fn load_slot_dir<T: DeserializeOwned>(dir: &Path, slot: u32) -> Result<T, SaveError> {
    let meta_path = dir.join("meta.json");
    let data_path = dir.join("data.json");

    if !meta_path.exists() || !data_path.exists() {
        return Err(SaveError::SlotNotFound(slot));
    }

    let meta_bytes = fs::read(&meta_path)?;
    let info: SlotInfo = serde_json::from_slice(&meta_bytes)
        .map_err(|e| SaveError::DeserializeError(e.to_string()))?;

    let data_bytes = fs::read(&data_path)?;
    if crc32(&data_bytes) != info.checksum {
        return Err(SaveError::CorruptedSave(slot));
    }

    serde_json::from_slice(&data_bytes).map_err(|e| SaveError::DeserializeError(e.to_string()))
}

/// Write `bytes` to `path` and flush it to disk before returning, so a
/// rename that follows cannot publish a file the OS has not written yet.
fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The per-user data directory on `os`, from the environment `env` reads.
/// `None` when the variable it needs is unset or not an absolute path.
fn data_dir_for(os: &str, env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let absolute = |key: &str| env(key).map(PathBuf::from).filter(|p| p.is_absolute());
    match os {
        "windows" => absolute("APPDATA"),
        "macos" => absolute("HOME").map(|home| home.join("Library").join("Application Support")),
        // Linux and the BSDs follow the XDG base directory spec.
        _ => absolute("XDG_DATA_HOME")
            .or_else(|| absolute("HOME").map(|home| home.join(".local").join("share"))),
    }
}

/// `name` reduced to a single safe path component: letters, digits, `-`,
/// `_` and `.`, everything else as `_`, no leading dots, never empty.
fn sanitize_app_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_start_matches('.');
    if cleaned.is_empty() {
        "amigo_game".to_string()
    } else {
        cleaned.to_string()
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
        };
        let mut mgr = SaveManager::new(config);

        assert!(!mgr.should_autosave(5.0));
        assert!(!mgr.should_autosave(4.0));
        // 5 + 4 + 2 = 11 >= 10
        assert!(mgr.should_autosave(2.0));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn zero_autosave_slots_disables_autosave_instead_of_panicking() {
        let config = SaveConfig {
            max_slots: 5,
            autosave_slots: 0,
            autosave_interval_secs: 1.0,
            app_name: "test_autosave_disabled".to_string(),
        };
        let mut mgr = SaveManager::new(config);

        assert!(!mgr.should_autosave(100.0));
        // Used to compute `(index + 1) % 0` and panic before touching disk.
        assert!(matches!(
            mgr.autosave(&42u32, 0.0),
            Err(SaveError::SlotNotFound(6))
        ));
    }

    fn manager_in(dir: &Path) -> SaveManager {
        SaveManager::with_base_dir(
            SaveConfig {
                max_slots: 5,
                autosave_slots: 2,
                autosave_interval_secs: 60.0,
                app_name: "unused".into(),
            },
            dir,
        )
    }

    #[test]
    fn save_and_load_through_the_manager() {
        let tmp = temp_test_dir();
        let mgr = manager_in(&tmp);
        let first = FakeState {
            level: 1,
            health: 10.0,
            name: "a".into(),
        };
        let second = FakeState {
            level: 2,
            health: 20.0,
            name: "b".into(),
        };

        mgr.save(1, "one", &first, 1.0).unwrap();
        assert_eq!(mgr.load::<FakeState>(1).unwrap(), first);
        // Overwriting replaces the slot and leaves no .tmp/.old behind.
        mgr.save(1, "two", &second, 2.0).unwrap();
        assert_eq!(mgr.load::<FakeState>(1).unwrap(), second);
        assert!(!tmp.join("slot_1.tmp").exists());
        assert!(!tmp.join("slot_1.old").exists());
        assert_eq!(mgr.list_slots().len(), 1);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn an_interrupted_save_keeps_the_previous_one_loadable() {
        let tmp = temp_test_dir();
        let mgr = manager_in(&tmp);
        let old = FakeState {
            level: 7,
            health: 1.0,
            name: "old".into(),
        };
        mgr.save(2, "old", &old, 0.0).unwrap();

        // A crash after the current slot was moved aside but before the new
        // one moved into place: only slot_2.old and a half-written .tmp.
        fs::rename(tmp.join("slot_2"), tmp.join("slot_2.old")).unwrap();
        fs::create_dir_all(tmp.join("slot_2.tmp")).unwrap();
        fs::write(tmp.join("slot_2.tmp/data.json"), b"{\"lev").unwrap();
        assert_eq!(mgr.load::<FakeState>(2).unwrap(), old);
        // The leftovers are not listed as extra slots.
        assert!(mgr.list_slots().is_empty());

        // A crash halfway through writing data.json in place, the old way:
        // the slot is corrupt, and the surviving .old copy is used.
        mgr.save(2, "old", &old, 0.0).unwrap();
        fs::rename(tmp.join("slot_2"), tmp.join("slot_2.old")).unwrap();
        fs::create_dir_all(tmp.join("slot_2")).unwrap();
        fs::copy(
            tmp.join("slot_2.old/meta.json"),
            tmp.join("slot_2/meta.json"),
        )
        .unwrap();
        fs::write(tmp.join("slot_2/data.json"), b"{\"level\": 99").unwrap();
        assert_eq!(mgr.load::<FakeState>(2).unwrap(), old);

        // Deleting the slot also deletes the fallback.
        mgr.delete_slot(2).unwrap();
        assert!(matches!(
            mgr.load::<FakeState>(2),
            Err(SaveError::SlotNotFound(2))
        ));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn platform_data_dirs() {
        // `data_dir_for` only accepts absolute paths, and what counts as
        // absolute depends on the host running the test: `/home/a` is not on
        // Windows, which needs a drive. Build the fake environment from paths
        // that are absolute here.
        let root = if cfg!(windows) { "C:/" } else { "/" };
        let abs = |p: &str| format!("{root}{p}");
        let env = |pairs: Vec<(&'static str, String)>| {
            move |key: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| v.clone())
            }
        };
        let home = PathBuf::from(abs("home/a"));

        assert_eq!(
            data_dir_for("linux", env(vec![("HOME", abs("home/a"))])),
            Some(home.join(".local").join("share"))
        );
        assert_eq!(
            data_dir_for(
                "linux",
                env(vec![
                    ("HOME", abs("home/a")),
                    ("XDG_DATA_HOME", abs("data"))
                ])
            ),
            Some(PathBuf::from(abs("data")))
        );
        // The XDG spec says to ignore a relative XDG_DATA_HOME.
        assert_eq!(
            data_dir_for(
                "linux",
                env(vec![
                    ("HOME", abs("home/a")),
                    ("XDG_DATA_HOME", "rel".into())
                ])
            ),
            Some(home.join(".local").join("share"))
        );
        // macOS used to save into the working directory.
        assert_eq!(
            data_dir_for("macos", env(vec![("HOME", abs("home/a"))])),
            Some(home.join("Library").join("Application Support"))
        );
        assert_eq!(
            data_dir_for("windows", env(vec![("APPDATA", abs("Users/a/AppData"))])),
            Some(PathBuf::from(abs("Users/a/AppData")))
        );
        assert_eq!(data_dir_for("linux", env(vec![])), None);
    }

    #[test]
    fn app_names_become_safe_directory_names() {
        assert_eq!(sanitize_app_name("Space Race: Remix"), "Space_Race__Remix");
        assert_eq!(sanitize_app_name("../../etc"), "_.._etc");
        assert_eq!(sanitize_app_name("my-game_2"), "my-game_2");
        assert_eq!(sanitize_app_name("  "), "amigo_game");
        assert_eq!(sanitize_app_name("..."), "amigo_game");
    }
}
