---
status: draft
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Save / Load

## Purpose

Persistence system for game state across sessions. Provides numbered save slots with metadata, CRC32 integrity verification, configurable autosave with rotating slots, quicksave/quickload, platform-aware save directories, and slot management (list, delete). Designed to serialize any `serde::Serialize` game state without coupling to a specific game's data structures.

## Public API

Existing implementation in `crates/amigo_core/src/save.rs`.

### SaveError

```rust
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
```

### SaveConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveConfig {
    pub max_slots: u32,             // default: 10
    pub autosave_slots: u32,        // default: 3
    pub autosave_interval_secs: f64, // default: 300.0
    pub app_name: String,
    pub compression: bool,          // default: true
}
```

### SlotInfo

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotInfo {
    pub slot_id: u32,
    pub label: String,
    pub timestamp: u64,
    pub play_time_secs: f64,
    pub checksum: u32,
    pub is_autosave: bool,
}
```

### SaveManager

```rust
pub struct SaveManager {
    config: SaveConfig,
    next_autosave_index: u32,
    time_since_autosave: f64,
}

impl SaveManager {
    pub fn new(config: SaveConfig) -> Self;
    pub fn save_dir(&self) -> PathBuf;
    pub fn save<T: Serialize>(
        &self, slot: u32, label: &str, data: &T, play_time: f64,
    ) -> Result<(), SaveError>;
    pub fn load<T: DeserializeOwned>(&self, slot: u32) -> Result<T, SaveError>;
    pub fn list_slots(&self) -> Vec<SlotInfo>;
    pub fn delete_slot(&self, slot: u32) -> Result<(), SaveError>;
    pub fn quicksave<T: Serialize>(&self, data: &T, play_time: f64) -> Result<(), SaveError>;
    pub fn quickload<T: DeserializeOwned>(&self) -> Result<T, SaveError>;
    pub fn autosave<T: Serialize>(&mut self, data: &T, play_time: f64) -> Result<u32, SaveError>;
    pub fn should_autosave(&mut self, elapsed: f64) -> bool;
}
```

## Behavior

- **Save format:** Each slot is a directory (`slot_{id}/`) containing `meta.json` (SlotInfo) and `data.json` (game state). Data is written first, then metadata, so a crash mid-write results in a missing slot rather than a corrupted one.
- **CRC32 integrity:** A CRC32 checksum is computed over the serialized data bytes and stored in `SlotInfo.checksum`. On load, the checksum is re-computed and compared; mismatch returns `SaveError::CorruptedSave`.
- **Platform directories:**
  - Linux: `~/.local/share/{app_name}/saves`
  - Windows: `%APPDATA%/{app_name}/saves`
  - Fallback: `./saves`
- **Quicksave:** Always uses slot 0 with the label "Quicksave".
- **Autosave:** Rotating slots starting at `max_slots + 1`. With `autosave_slots: 3` and `max_slots: 10`, autosaves use slots 11, 12, 13 in rotation. `should_autosave()` accumulates elapsed time and returns `true` when the interval is reached; the caller then calls `autosave()`.
- **Slot listing:** `list_slots()` scans the save directory for valid slot directories with parseable `meta.json`, returning sorted metadata without loading game data.
- **Deletion:** `delete_slot()` removes the entire slot directory. Returns `SlotNotFound` if the directory does not exist.

## Internal Design

- Uses `serde_json` for both metadata and game data serialization (human-readable, debuggable).
- CRC32 uses a compile-time-generated 256-entry lookup table for fast computation.
- Autosave index rotates via modular arithmetic: `next_index = (current + 1) % autosave_slots`.

## Non-Goals

- **Incremental saves.** The system serializes the entire game state each time. Dirty-chunk-only saving is a future optimization that would integrate with [engine/chunks](chunks.md).
- **Binary format.** Currently JSON-only. Bincode or MessagePack support can be layered on top.
- **Save migration.** Schema versioning and forward/backward compatibility are not yet implemented.
- **Async I/O.** All save/load operations are synchronous. Background saving should be wrapped in a thread by the caller.
- **Compression.** The `compression` config field is defined but not yet implemented.

## Open Questions

- Should the save system support binary formats (bincode) for smaller file sizes and faster serialization?
- How should save migration work when game data structures change between versions?
- Should autosave run on a background thread to avoid frame hitches on large worlds?
- Should save slots support screenshot thumbnails for the load screen UI?
