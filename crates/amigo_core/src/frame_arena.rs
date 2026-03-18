//! Per-frame bump allocator backed by [`bumpalo`].
//!
//! `FrameArena` is reset every frame, giving O(1) allocation for
//! transient per-frame data (sprite batches, collision lists, AI scratch
//! buffers, etc.) without individual `Vec` heap churn.
//!
//! # Usage
//!
//! ```ignore
//! let arena = FrameArena::new();
//! // Each frame:
//! arena.reset();
//! let sprites = arena.alloc_slice_fill_default::<SpriteInstance>(256);
//! ```

use bumpalo::Bump;

/// A per-frame bump allocator.
///
/// Call [`reset()`](Self::reset) at the start of each frame to reclaim
/// all memory without freeing the backing pages.
pub struct FrameArena {
    bump: Bump,
}

impl FrameArena {
    /// Create a new arena with default capacity.
    pub fn new() -> Self {
        Self { bump: Bump::new() }
    }

    /// Create an arena with a pre-allocated capacity hint (bytes).
    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            bump: Bump::with_capacity(bytes),
        }
    }

    /// Reset the arena, reclaiming all allocations.
    /// Call this at the start of each frame.
    pub fn reset(&mut self) {
        self.bump.reset();
    }

    /// Allocate a single value in the arena.
    pub fn alloc<T>(&self, val: T) -> &mut T {
        self.bump.alloc(val)
    }

    /// Allocate a slice by cloning from a source slice.
    pub fn alloc_slice_clone<T: Clone>(&self, src: &[T]) -> &mut [T] {
        self.bump.alloc_slice_clone(src)
    }

    /// Allocate a slice by copying from a source slice.
    pub fn alloc_slice_copy<T: Copy>(&self, src: &[T]) -> &mut [T] {
        self.bump.alloc_slice_copy(src)
    }

    /// Allocate a slice of `len` elements, each initialized with `Default::default()`.
    pub fn alloc_slice_fill_default<T: Default>(&self, len: usize) -> &mut [T] {
        self.bump.alloc_slice_fill_with(len, |_| T::default())
    }

    /// Allocate a `String` in the arena.
    pub fn alloc_str(&self, s: &str) -> &mut str {
        self.bump.alloc_str(s)
    }

    /// Create a [`bumpalo::collections::Vec`] in this arena.
    pub fn vec<T>(&self) -> bumpalo::collections::Vec<'_, T> {
        bumpalo::collections::Vec::new_in(&self.bump)
    }

    /// How many bytes have been allocated (approximately).
    pub fn allocated_bytes(&self) -> usize {
        self.bump.allocated_bytes()
    }

    /// Access the underlying bumpalo `Bump` allocator.
    pub fn inner(&self) -> &Bump {
        &self.bump
    }
}

impl Default for FrameArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_alloc_and_reset() {
        let mut arena = FrameArena::new();
        let x = arena.alloc(42u32);
        assert_eq!(*x, 42);
        assert!(arena.allocated_bytes() > 0);

        arena.reset();
        // After reset, the old allocation is invalid (lifetime ended).
        // We can allocate again from the same memory.
        let y = arena.alloc(99u32);
        assert_eq!(*y, 99);
    }

    #[test]
    fn alloc_slice() {
        let arena = FrameArena::new();
        let src = [1, 2, 3, 4, 5];
        let slice = arena.alloc_slice_copy(&src);
        assert_eq!(slice, &[1, 2, 3, 4, 5]);
        slice[0] = 10;
        assert_eq!(slice[0], 10);
    }

    #[test]
    fn alloc_str() {
        let arena = FrameArena::new();
        let s = arena.alloc_str("hello");
        assert_eq!(s, "hello");
    }

    #[test]
    fn bump_vec() {
        let arena = FrameArena::new();
        let mut v = arena.vec::<i32>();
        v.push(1);
        v.push(2);
        v.push(3);
        assert_eq!(&v[..], &[1, 2, 3]);
    }

    #[test]
    fn with_capacity() {
        let arena = FrameArena::with_capacity(1024 * 64);
        let _slice = arena.alloc_slice_fill_default::<u8>(1000);
        assert!(arena.allocated_bytes() >= 1000);
    }
}
