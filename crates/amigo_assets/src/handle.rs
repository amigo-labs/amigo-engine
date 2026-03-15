use std::marker::PhantomData;

/// Typed handle to a loaded asset. The `T` parameter is a zero-cost type tag
/// that ensures you cannot accidentally use a sprite handle where a sound
/// handle is expected (and vice versa).
///
/// Handles are lightweight (Copy) and can be stored freely in components.
#[derive(Debug)]
pub struct AssetHandle<T> {
    /// Internal index into the asset storage.
    pub index: u32,
    /// Generation counter for dangling-handle detection.
    pub generation: u32,
    _marker: PhantomData<T>,
}

// Manual impls to avoid requiring T: Clone/Copy/etc.
impl<T> Clone for AssetHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for AssetHandle<T> {}
impl<T> PartialEq for AssetHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}
impl<T> Eq for AssetHandle<T> {}
impl<T> std::hash::Hash for AssetHandle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

/// The load state of an asset behind a handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetState {
    /// Not yet loaded.
    Pending,
    /// Successfully loaded and ready to use.
    Loaded,
    /// Failed to load.
    Failed,
    /// Slot was freed (dangling handle).
    Unloaded,
}

/// Slot in the handle allocator.
struct Slot {
    generation: u32,
    state: AssetState,
    /// Opaque name for debugging.
    name: String,
}

/// Allocates and tracks typed asset handles.
pub struct HandleAllocator {
    slots: Vec<Slot>,
    free_list: Vec<u32>,
}

impl HandleAllocator {
    pub fn new() -> Self {
        Self {
            slots: Vec::with_capacity(256),
            free_list: Vec::new(),
        }
    }

    /// Allocate a new handle for an asset with the given debug name.
    pub fn alloc<T>(&mut self, name: &str) -> AssetHandle<T> {
        if let Some(index) = self.free_list.pop() {
            let slot = &mut self.slots[index as usize];
            slot.generation += 1;
            slot.state = AssetState::Pending;
            slot.name = name.to_string();
            AssetHandle {
                index,
                generation: slot.generation,
                _marker: PhantomData,
            }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot {
                generation: 0,
                state: AssetState::Pending,
                name: name.to_string(),
            });
            AssetHandle {
                index,
                generation: 0,
                _marker: PhantomData,
            }
        }
    }

    /// Mark a handle's asset as loaded.
    pub fn mark_loaded<T>(&mut self, handle: AssetHandle<T>) {
        if let Some(slot) = self.slots.get_mut(handle.index as usize) {
            if slot.generation == handle.generation {
                slot.state = AssetState::Loaded;
            }
        }
    }

    /// Mark a handle's asset as failed.
    pub fn mark_failed<T>(&mut self, handle: AssetHandle<T>) {
        if let Some(slot) = self.slots.get_mut(handle.index as usize) {
            if slot.generation == handle.generation {
                slot.state = AssetState::Failed;
            }
        }
    }

    /// Free a handle's slot so it can be reused.
    pub fn free<T>(&mut self, handle: AssetHandle<T>) {
        if let Some(slot) = self.slots.get_mut(handle.index as usize) {
            if slot.generation == handle.generation {
                slot.state = AssetState::Unloaded;
                self.free_list.push(handle.index);
            }
        }
    }

    /// Query the load state of a handle. Returns `Unloaded` for stale/invalid handles.
    pub fn state<T>(&self, handle: AssetHandle<T>) -> AssetState {
        self.slots
            .get(handle.index as usize)
            .filter(|s| s.generation == handle.generation)
            .map(|s| s.state)
            .unwrap_or(AssetState::Unloaded)
    }

    /// Get the debug name for a handle. Returns `None` for stale handles.
    pub fn name<T>(&self, handle: AssetHandle<T>) -> Option<&str> {
        self.slots
            .get(handle.index as usize)
            .filter(|s| s.generation == handle.generation)
            .map(|s| s.name.as_str())
    }

    /// Number of currently allocated (non-free) slots.
    pub fn active_count(&self) -> usize {
        self.slots.len() - self.free_list.len()
    }
}

impl Default for HandleAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Type tags for common asset kinds
// ---------------------------------------------------------------------------

/// Type tag for sprite/texture assets.
#[derive(Debug)]
pub struct SpriteAsset;
/// Type tag for audio assets.
#[derive(Debug)]
pub struct AudioAsset;
/// Type tag for font assets.
#[derive(Debug)]
pub struct FontAsset;
/// Type tag for tileset assets.
#[derive(Debug)]
pub struct TilesetAsset;
/// Type tag for level/map assets.
#[derive(Debug)]
pub struct LevelAsset;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_and_state() {
        let mut alloc = HandleAllocator::new();
        let h: AssetHandle<SpriteAsset> = alloc.alloc("player");
        assert_eq!(alloc.state(h), AssetState::Pending);
        assert_eq!(alloc.name(h), Some("player"));

        alloc.mark_loaded(h);
        assert_eq!(alloc.state(h), AssetState::Loaded);
    }

    #[test]
    fn free_and_reuse() {
        let mut alloc = HandleAllocator::new();
        let h1: AssetHandle<SpriteAsset> = alloc.alloc("a");
        assert_eq!(h1.index, 0);
        assert_eq!(h1.generation, 0);

        alloc.free(h1);
        assert_eq!(alloc.state(h1), AssetState::Unloaded);

        // Reuse same slot, generation bumps.
        let h2: AssetHandle<SpriteAsset> = alloc.alloc("b");
        assert_eq!(h2.index, 0);
        assert_eq!(h2.generation, 1);

        // Old handle is stale.
        assert_eq!(alloc.state(h1), AssetState::Unloaded);
        assert_eq!(alloc.state(h2), AssetState::Pending);
    }

    #[test]
    fn type_safety() {
        let mut alloc = HandleAllocator::new();
        let _sprite: AssetHandle<SpriteAsset> = alloc.alloc("hero");
        let _audio: AssetHandle<AudioAsset> = alloc.alloc("music");
        // These are different types at compile time — you cannot accidentally
        // pass an AssetHandle<AudioAsset> where AssetHandle<SpriteAsset> is expected.
        assert_eq!(alloc.active_count(), 2);
    }

    #[test]
    fn mark_failed() {
        let mut alloc = HandleAllocator::new();
        let h: AssetHandle<SpriteAsset> = alloc.alloc("missing");
        alloc.mark_failed(h);
        assert_eq!(alloc.state(h), AssetState::Failed);
    }

    #[test]
    fn stale_handle_returns_unloaded() {
        let mut alloc = HandleAllocator::new();
        let h: AssetHandle<SpriteAsset> = alloc.alloc("old");
        alloc.free(h);
        let _h2: AssetHandle<SpriteAsset> = alloc.alloc("new");
        // h is stale (generation mismatch).
        assert_eq!(alloc.state(h), AssetState::Unloaded);
        assert_eq!(alloc.name(h), None);
    }

    #[test]
    fn active_count_tracks_correctly() {
        let mut alloc = HandleAllocator::new();
        let h1: AssetHandle<SpriteAsset> = alloc.alloc("a");
        let _h2: AssetHandle<SpriteAsset> = alloc.alloc("b");
        assert_eq!(alloc.active_count(), 2);

        alloc.free(h1);
        assert_eq!(alloc.active_count(), 1);
    }

    #[test]
    fn handles_are_copy() {
        let mut alloc = HandleAllocator::new();
        let h: AssetHandle<SpriteAsset> = alloc.alloc("test");
        let h2 = h; // Copy
        assert_eq!(h, h2);
        assert_eq!(alloc.state(h), alloc.state(h2));
    }
}
