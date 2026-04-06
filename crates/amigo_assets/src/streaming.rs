//! Asset streaming pipeline – texture registry for demand-driven loading.
//!
//! Gated behind the `asset_streaming` feature.

use rustc_hash::FxHashMap;

/// UV rectangle for a texture region.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// State of a texture slot in the streaming registry.
#[derive(Debug)]
pub enum TextureSlot {
    NotLoaded,
    Loading {
        task_id: u64,
    },
    Resident {
        atlas_page: u32,
        uv: Rect,
        last_used_tick: u64,
    },
    Failed,
}

/// Registry tracking the streaming state of all textures.
pub struct TextureRegistry {
    slots: FxHashMap<String, TextureSlot>,
    current_tick: u64,
    budget_bytes: u64,
}

impl TextureRegistry {
    /// Create a new registry with the given memory budget (in bytes).
    pub fn new(budget_bytes: u64) -> Self {
        Self {
            slots: FxHashMap::default(),
            current_tick: 0,
            budget_bytes,
        }
    }

    /// Request a texture by name. Returns the current slot state, inserting
    /// `NotLoaded` if the name has not been seen before.
    pub fn request(&mut self, name: &str) -> &TextureSlot {
        if !self.slots.contains_key(name) {
            self.slots.insert(name.to_string(), TextureSlot::NotLoaded);
        }
        // Touch resident entries so they stay alive.
        if let Some(TextureSlot::Resident { last_used_tick, .. }) = self.slots.get_mut(name) {
            *last_used_tick = self.current_tick;
        }
        self.slots.get(name).unwrap()
    }

    /// Mark a texture as loading with the given task id.
    pub fn mark_loading(&mut self, name: &str, task_id: u64) {
        self.slots
            .insert(name.to_string(), TextureSlot::Loading { task_id });
    }

    /// Mark a texture as resident in the atlas.
    pub fn mark_resident(&mut self, name: &str, atlas_page: u32, uv: Rect) {
        self.slots.insert(
            name.to_string(),
            TextureSlot::Resident {
                atlas_page,
                uv,
                last_used_tick: self.current_tick,
            },
        );
    }

    /// Mark a texture as failed.
    pub fn mark_failed(&mut self, name: &str) {
        self.slots.insert(name.to_string(), TextureSlot::Failed);
    }

    /// Advance the internal tick counter.
    pub fn tick(&mut self) {
        self.current_tick += 1;
    }

    /// Evict the least-recently-used resident entries until at most
    /// `max_resident` remain. Returns the names of evicted entries.
    pub fn evict_lru(&mut self, max_resident: usize) -> Vec<String> {
        let mut evicted = Vec::new();

        let resident_count = self.resident_count();
        if resident_count <= max_resident {
            return evicted;
        }
        let to_evict = resident_count - max_resident;

        // Collect resident entries with their last-used tick.
        let mut residents: Vec<(String, u64)> = self
            .slots
            .iter()
            .filter_map(|(name, slot)| {
                if let TextureSlot::Resident { last_used_tick, .. } = slot {
                    Some((name.clone(), *last_used_tick))
                } else {
                    None
                }
            })
            .collect();

        // Sort by last_used_tick ascending (oldest first).
        residents.sort_by_key(|&(_, tick)| tick);

        for (name, _) in residents.into_iter().take(to_evict) {
            self.slots.insert(name.clone(), TextureSlot::NotLoaded);
            evicted.push(name);
        }

        evicted
    }

    /// Count the number of resident textures.
    pub fn resident_count(&self) -> usize {
        self.slots
            .values()
            .filter(|s| matches!(s, TextureSlot::Resident { .. }))
            .count()
    }

    /// Check whether a texture is currently resident.
    pub fn is_loaded(&self, name: &str) -> bool {
        matches!(self.slots.get(name), Some(TextureSlot::Resident { .. }))
    }

    /// Return the memory budget in bytes.
    pub fn budget_bytes(&self) -> u64 {
        self.budget_bytes
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_creates_not_loaded() {
        let mut reg = TextureRegistry::new(1024);
        let slot = reg.request("hero");
        assert!(matches!(slot, TextureSlot::NotLoaded));
    }

    #[test]
    fn mark_loading_transitions() {
        let mut reg = TextureRegistry::new(1024);
        reg.request("hero");
        reg.mark_loading("hero", 42);
        let slot = reg.request("hero");
        assert!(matches!(slot, TextureSlot::Loading { task_id: 42 }));
    }

    #[test]
    fn mark_resident_transitions() {
        let mut reg = TextureRegistry::new(1024);
        reg.request("hero");
        reg.mark_loading("hero", 1);
        let uv = Rect {
            x: 0.0,
            y: 0.0,
            w: 0.5,
            h: 0.5,
        };
        reg.mark_resident("hero", 0, uv);
        assert!(reg.is_loaded("hero"));
        assert!(matches!(
            reg.request("hero"),
            TextureSlot::Resident { atlas_page: 0, .. }
        ));
    }

    #[test]
    fn mark_failed_transitions() {
        let mut reg = TextureRegistry::new(1024);
        reg.request("bad_texture");
        reg.mark_failed("bad_texture");
        assert!(matches!(
            reg.slots.get("bad_texture"),
            Some(TextureSlot::Failed)
        ));
        assert!(!reg.is_loaded("bad_texture"));
    }

    #[test]
    fn evict_lru_evicts_oldest() {
        let mut reg = TextureRegistry::new(1024);
        let uv = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        };

        // Insert three textures at different ticks.
        reg.mark_resident("old", 0, uv);
        reg.tick(); // tick = 1
        reg.mark_resident("mid", 0, uv);
        reg.tick(); // tick = 2
        reg.mark_resident("new", 0, uv);

        assert_eq!(reg.resident_count(), 3);

        // Evict down to 1 resident.
        let evicted = reg.evict_lru(1);
        assert_eq!(evicted.len(), 2);
        assert!(evicted.contains(&"old".to_string()));
        assert!(evicted.contains(&"mid".to_string()));
        assert_eq!(reg.resident_count(), 1);
        assert!(reg.is_loaded("new"));
        assert!(!reg.is_loaded("old"));
        assert!(!reg.is_loaded("mid"));
    }

    #[test]
    fn evict_lru_no_eviction_needed() {
        let mut reg = TextureRegistry::new(1024);
        let uv = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        };
        reg.mark_resident("a", 0, uv);
        let evicted = reg.evict_lru(10);
        assert!(evicted.is_empty());
        assert_eq!(reg.resident_count(), 1);
    }

    #[test]
    fn tick_increments_counter() {
        let mut reg = TextureRegistry::new(1024);
        assert_eq!(reg.current_tick, 0);
        reg.tick();
        assert_eq!(reg.current_tick, 1);
        reg.tick();
        reg.tick();
        assert_eq!(reg.current_tick, 3);
    }

    #[test]
    fn is_loaded_returns_correct_values() {
        let mut reg = TextureRegistry::new(1024);
        assert!(!reg.is_loaded("missing"));

        reg.request("tex");
        assert!(!reg.is_loaded("tex"));

        reg.mark_loading("tex", 1);
        assert!(!reg.is_loaded("tex"));

        let uv = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        };
        reg.mark_resident("tex", 0, uv);
        assert!(reg.is_loaded("tex"));

        reg.mark_failed("tex");
        assert!(!reg.is_loaded("tex"));
    }

    #[test]
    fn request_touches_resident_entry() {
        let mut reg = TextureRegistry::new(1024);
        let uv = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        };

        reg.mark_resident("a", 0, uv);
        // tick = 0 for "a"
        reg.tick(); // tick = 1
        reg.mark_resident("b", 0, uv);
        // "b" at tick 1

        reg.tick(); // tick = 2
                    // Touch "a" so it becomes more recent than "b"
        reg.request("a");

        // Now evict down to 1: "b" should be evicted (tick 1), "a" was touched at tick 2
        let evicted = reg.evict_lru(1);
        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0], "b");
        assert!(reg.is_loaded("a"));
    }
}
