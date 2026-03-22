//! Streaming asset pipeline (ADR-0005).
//!
//! Provides [`TextureRegistry`] which manages on-demand texture loading with
//! background I/O and LRU eviction. Textures transition through the states
//! `NotLoaded -> Loading -> Resident -> (Evicted -> NotLoaded)`.
//!
//! Gated behind the `asset_streaming` feature flag.

use amigo_core::Rect;
use rustc_hash::FxHashMap;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Texture slot states
// ---------------------------------------------------------------------------

/// Region within a dynamic atlas page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtlasRegion {
    /// Which atlas page this region belongs to.
    pub page: u32,
    /// Pixel rect within the page.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Normalised UV coordinates for rendering.
    pub uv: Rect,
}

/// The load state of a texture slot.
pub enum TextureSlotState {
    /// Not yet loaded; no CPU or GPU memory used.
    NotLoaded,
    /// A background load is in progress.
    Loading,
    /// Decoded on CPU, ready for GPU upload.
    Decoded(image::RgbaImage),
    /// Uploaded to a GPU atlas page and ready to render.
    Resident(AtlasRegion),
    /// Evicted from GPU memory (can be reloaded on demand).
    Evicted,
}

impl std::fmt::Debug for TextureSlotState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLoaded => write!(f, "NotLoaded"),
            Self::Loading => write!(f, "Loading"),
            Self::Decoded(_) => write!(f, "Decoded"),
            Self::Resident(r) => write!(f, "Resident({r:?})"),
            Self::Evicted => write!(f, "Evicted"),
        }
    }
}

/// A single texture managed by the registry.
pub struct TextureSlot {
    pub name: String,
    pub state: TextureSlotState,
    /// Frame tick when this slot was last accessed via `request()`.
    pub last_used_tick: u64,
    /// Pixel dimensions (known after decoding).
    pub width: u32,
    pub height: u32,
}

// ---------------------------------------------------------------------------
// Background load message
// ---------------------------------------------------------------------------

/// Result of a background image load.
pub(crate) struct LoadResult {
    pub name: String,
    pub result: Result<image::RgbaImage, String>,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the streaming texture registry.
#[derive(Clone, Debug)]
pub struct StreamingConfig {
    /// Maximum GPU memory budget in bytes (across all atlas pages).
    /// Default: 64 MiB.
    pub gpu_budget_bytes: u64,
    /// Maximum atlas page dimension (power-of-2). Default: 4096.
    pub max_atlas_size: u32,
    /// Padding between sprites in the atlas. Default: 1.
    pub atlas_padding: u32,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            gpu_budget_bytes: 64 * 1024 * 1024,
            max_atlas_size: 4096,
            atlas_padding: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// TextureRegistry
// ---------------------------------------------------------------------------

/// Manages on-demand texture loading with background I/O and LRU eviction.
///
/// # Usage
///
/// 1. Call [`TextureRegistry::register`] to declare known sprite names.
/// 2. Each frame, call [`TextureRegistry::request`] to get a sprite's state.
///    If `NotLoaded`, a background load is kicked off automatically.
/// 3. Call [`TextureRegistry::poll`] each frame to process completed loads.
/// 4. Call [`TextureRegistry::evict_lru`] at end-of-frame to stay within budget.
pub struct TextureRegistry {
    /// All known texture slots, keyed by sprite name.
    slots: FxHashMap<String, TextureSlot>,
    /// Base path for loading sprite PNGs (e.g. `assets/sprites/`).
    base_path: PathBuf,
    /// Sender for dispatching background load tasks.
    load_tx: mpsc::Sender<String>,
    /// Receiver for completed loads.
    result_rx: mpsc::Receiver<LoadResult>,
    /// Current frame tick (incremented by caller).
    current_tick: u64,
    /// Tracks total GPU bytes currently used (4 bytes per pixel).
    gpu_bytes_used: u64,
    /// Configuration.
    pub config: StreamingConfig,
    /// Ordered list of resident sprite names for LRU scanning.
    resident_names: Vec<String>,
}

impl TextureRegistry {
    /// Create a new registry. Spawns a background loader thread.
    pub fn new(base_path: impl Into<PathBuf>, config: StreamingConfig) -> Self {
        let base_path = base_path.into();
        let (load_tx, load_rx) = mpsc::channel::<String>();
        let (result_tx, result_rx) = mpsc::channel::<LoadResult>();

        let loader_base = base_path.clone();
        std::thread::Builder::new()
            .name("asset-loader".into())
            .spawn(move || {
                Self::loader_thread(loader_base, load_rx, result_tx);
            })
            .expect("failed to spawn asset loader thread");

        Self {
            slots: FxHashMap::default(),
            base_path,
            load_tx,
            result_rx,
            current_tick: 0,
            gpu_bytes_used: 0,
            config,
            resident_names: Vec::new(),
        }
    }

    /// Background loader thread: reads names from `load_rx`, decodes PNGs,
    /// sends results back via `result_tx`.
    fn loader_thread(
        base_path: PathBuf,
        load_rx: mpsc::Receiver<String>,
        result_tx: mpsc::Sender<LoadResult>,
    ) {
        while let Ok(name) = load_rx.recv() {
            // Convert sprite name (e.g. "player/idle") to a file path.
            let rel_path = format!("{}.png", name.replace('/', std::path::MAIN_SEPARATOR_STR));
            let full_path = base_path.join(&rel_path);
            let result = match image::open(&full_path) {
                Ok(img) => Ok(img.to_rgba8()),
                Err(e) => Err(format!("Failed to load {}: {}", full_path.display(), e)),
            };
            if result_tx.send(LoadResult { name, result }).is_err() {
                // Registry dropped, exit thread.
                break;
            }
        }
    }

    /// Register a sprite name so the registry knows about it.
    /// Does not trigger loading.
    pub fn register(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.slots.contains_key(&name) {
            self.slots.insert(
                name.clone(),
                TextureSlot {
                    name,
                    state: TextureSlotState::NotLoaded,
                    last_used_tick: 0,
                    width: 0,
                    height: 0,
                },
            );
        }
    }

    /// Request a texture, triggering a background load if needed.
    /// If the texture is `NotLoaded` or `Evicted`, a background load is
    /// kicked off and the state transitions to `Loading`.
    ///
    /// Use [`state()`] afterwards to inspect the current state.
    pub fn request(&mut self, name: &str) {
        if !self.slots.contains_key(name) {
            self.register(name);
        }
        let tick = self.current_tick;
        if let Some(slot) = self.slots.get_mut(name) {
            slot.last_used_tick = tick;
            match &slot.state {
                TextureSlotState::NotLoaded | TextureSlotState::Evicted => {
                    if self.load_tx.send(name.to_string()).is_ok() {
                        slot.state = TextureSlotState::Loading;
                        debug!("Streaming: kicked off load for '{}'", name);
                    }
                }
                _ => {}
            }
        }
    }

    /// Poll for completed background loads. Call once per frame.
    /// Returns the names of sprites that are now in the `Decoded` state
    /// and ready for GPU upload.
    pub fn poll(&mut self) -> Vec<String> {
        let mut newly_decoded = Vec::new();
        while let Ok(load_result) = self.result_rx.try_recv() {
            if let Some(slot) = self.slots.get_mut(&load_result.name) {
                match load_result.result {
                    Ok(image) => {
                        slot.width = image.width();
                        slot.height = image.height();
                        slot.state = TextureSlotState::Decoded(image);
                        newly_decoded.push(load_result.name);
                    }
                    Err(e) => {
                        warn!("Streaming load failed: {}", e);
                        slot.state = TextureSlotState::NotLoaded;
                    }
                }
            }
        }
        newly_decoded
    }

    /// Mark a slot as resident (GPU upload complete).
    pub fn mark_resident(&mut self, name: &str, region: AtlasRegion) {
        if let Some(slot) = self.slots.get_mut(name) {
            let bytes = (slot.width as u64) * (slot.height as u64) * 4;
            slot.state = TextureSlotState::Resident(region);
            self.gpu_bytes_used += bytes;
            self.resident_names.push(name.to_string());
            debug!(
                "Streaming: '{}' now resident ({} bytes, total GPU: {})",
                name, bytes, self.gpu_bytes_used
            );
        }
    }

    /// Take the decoded image out of a slot (for GPU upload).
    /// Returns `None` if the slot is not in the `Decoded` state.
    pub fn take_decoded(&mut self, name: &str) -> Option<image::RgbaImage> {
        if let Some(slot) = self.slots.get_mut(name) {
            if matches!(slot.state, TextureSlotState::Decoded(_)) {
                let old = std::mem::replace(&mut slot.state, TextureSlotState::Loading);
                if let TextureSlotState::Decoded(img) = old {
                    return Some(img);
                }
            }
        }
        None
    }

    /// Evict least-recently-used textures until GPU usage is within budget.
    /// Returns the names of evicted sprites.
    pub fn evict_lru(&mut self) -> Vec<String> {
        let mut evicted = Vec::new();
        if self.gpu_bytes_used <= self.config.gpu_budget_bytes {
            return evicted;
        }

        // Sort resident names by last_used_tick ascending (oldest first).
        let mut candidates: Vec<(String, u64)> = self
            .resident_names
            .iter()
            .filter_map(|name| {
                self.slots
                    .get(name)
                    .filter(|s| matches!(s.state, TextureSlotState::Resident(_)))
                    .map(|s| (name.clone(), s.last_used_tick))
            })
            .collect();
        candidates.sort_by_key(|(_, tick)| *tick);

        for (name, _) in candidates {
            if self.gpu_bytes_used <= self.config.gpu_budget_bytes {
                break;
            }
            if let Some(slot) = self.slots.get_mut(&name) {
                if matches!(slot.state, TextureSlotState::Resident(_)) {
                    let bytes = (slot.width as u64) * (slot.height as u64) * 4;
                    slot.state = TextureSlotState::Evicted;
                    self.gpu_bytes_used = self.gpu_bytes_used.saturating_sub(bytes);
                    info!("Streaming: evicted '{}' ({} bytes freed)", name, bytes);
                    evicted.push(name);
                }
            }
        }

        // Clean up resident_names list.
        let slots_ref = &self.slots;
        self.resident_names.retain(|name| {
            slots_ref
                .get(name)
                .is_some_and(|s| matches!(s.state, TextureSlotState::Resident(_)))
        });

        evicted
    }

    /// Advance the frame tick. Call at the start of each frame.
    pub fn tick(&mut self) {
        self.current_tick += 1;
    }

    /// Get the current state of a slot without triggering a load.
    pub fn state(&self, name: &str) -> Option<&TextureSlotState> {
        self.slots.get(name).map(|s| &s.state)
    }

    /// Check if a sprite is resident (uploaded to GPU and ready to render).
    pub fn is_resident(&self, name: &str) -> bool {
        self.slots
            .get(name)
            .is_some_and(|s| matches!(s.state, TextureSlotState::Resident(_)))
    }

    /// Get the atlas region for a resident sprite.
    pub fn atlas_region(&self, name: &str) -> Option<&AtlasRegion> {
        self.slots.get(name).and_then(|s| match &s.state {
            TextureSlotState::Resident(r) => Some(r),
            _ => None,
        })
    }

    /// Number of registered sprites.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Number of currently resident sprites.
    pub fn resident_count(&self) -> usize {
        self.resident_names.len()
    }

    /// Current GPU memory usage in bytes.
    pub fn gpu_bytes_used(&self) -> u64 {
        self.gpu_bytes_used
    }

    /// Current frame tick.
    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    /// Iterate over all slot names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.slots.keys().map(|s| s.as_str())
    }

    /// Pre-load a list of sprites (kick off background loads without waiting).
    pub fn preload(&mut self, names: &[&str]) {
        for name in names {
            self.request(name);
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
    fn register_starts_not_loaded() {
        let config = StreamingConfig::default();
        let mut registry = TextureRegistry::new("/nonexistent", config);
        registry.register("player");
        assert!(matches!(
            registry.state("player"),
            Some(TextureSlotState::NotLoaded)
        ));
    }

    #[test]
    fn request_triggers_loading() {
        let config = StreamingConfig::default();
        let mut registry = TextureRegistry::new("/nonexistent", config);
        registry.register("player");
        registry.request("player");
        assert!(matches!(
            registry.state("player"),
            Some(TextureSlotState::Loading)
        ));
    }

    #[test]
    fn auto_register_on_request() {
        let config = StreamingConfig::default();
        let mut registry = TextureRegistry::new("/nonexistent", config);
        registry.request("unknown_sprite");
        assert!(matches!(
            registry.state("unknown_sprite"),
            Some(TextureSlotState::Loading)
        ));
        assert_eq!(registry.slot_count(), 1);
    }

    #[test]
    fn evict_lru_frees_memory() {
        let config = StreamingConfig {
            gpu_budget_bytes: 1000,
            ..Default::default()
        };
        let mut registry = TextureRegistry::new("/nonexistent", config);

        // Manually set up two resident sprites.
        registry.register("old_sprite");
        registry.register("new_sprite");

        // Simulate: old_sprite used at tick 1, new_sprite at tick 5.
        registry.slots.get_mut("old_sprite").unwrap().width = 16;
        registry.slots.get_mut("old_sprite").unwrap().height = 16;
        registry.slots.get_mut("old_sprite").unwrap().last_used_tick = 1;
        registry.slots.get_mut("old_sprite").unwrap().state =
            TextureSlotState::Resident(AtlasRegion {
                page: 0,
                x: 0,
                y: 0,
                w: 16,
                h: 16,
                uv: Rect::new(0.0, 0.0, 1.0, 1.0),
            });
        registry.gpu_bytes_used = 16 * 16 * 4; // 1024 bytes
        registry.resident_names.push("old_sprite".to_string());

        registry.slots.get_mut("new_sprite").unwrap().width = 16;
        registry.slots.get_mut("new_sprite").unwrap().height = 16;
        registry.slots.get_mut("new_sprite").unwrap().last_used_tick = 5;
        registry.slots.get_mut("new_sprite").unwrap().state =
            TextureSlotState::Resident(AtlasRegion {
                page: 0,
                x: 16,
                y: 0,
                w: 16,
                h: 16,
                uv: Rect::new(0.0, 0.0, 1.0, 1.0),
            });
        registry.gpu_bytes_used += 16 * 16 * 4; // 2048 total
        registry.resident_names.push("new_sprite".to_string());

        // Budget is 1000, usage is 2048 => should evict old_sprite first.
        let evicted = registry.evict_lru();
        assert!(evicted.contains(&"old_sprite".to_string()));
        assert!(matches!(
            registry.state("old_sprite"),
            Some(TextureSlotState::Evicted)
        ));
        // new_sprite should also be evicted since 1024 > 1000.
        assert!(registry.gpu_bytes_used <= 1000 || evicted.len() == 2);
    }

    #[test]
    fn tick_advances() {
        let config = StreamingConfig::default();
        let mut registry = TextureRegistry::new("/nonexistent", config);
        assert_eq!(registry.current_tick(), 0);
        registry.tick();
        assert_eq!(registry.current_tick(), 1);
        registry.tick();
        assert_eq!(registry.current_tick(), 2);
    }
}
