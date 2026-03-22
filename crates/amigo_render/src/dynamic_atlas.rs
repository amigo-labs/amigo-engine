//! Dynamic runtime atlas for streaming textures (ADR-0005).
//!
//! Supports incremental insertion of decoded images into GPU atlas pages
//! using shelf-packing. Evicted regions are tracked on a free-list for reuse.
//!
//! Gated behind the `asset_streaming` feature flag.

use amigo_core::Rect;
use crate::atlas::{next_power_of_two, AtlasRect};
use crate::texture::TextureId;

// ---------------------------------------------------------------------------
// Shelf-packing structures
// ---------------------------------------------------------------------------

/// A horizontal shelf (row) in an atlas page.
struct Shelf {
    /// Y offset of this shelf in the page.
    y: u32,
    /// Height of the tallest sprite on this shelf.
    height: u32,
    /// Next free X position on this shelf.
    cursor_x: u32,
}

/// A single atlas page backed by a GPU texture.
pub struct AtlasPage {
    /// GPU texture ID.
    pub texture_id: TextureId,
    /// Page dimensions.
    pub width: u32,
    pub height: u32,
    /// Shelves for packing.
    shelves: Vec<Shelf>,
    /// Padding between sprites.
    padding: u32,
    /// Next Y position for a new shelf.
    next_shelf_y: u32,
    /// Number of sprites currently placed on this page.
    pub sprite_count: u32,
}

impl AtlasPage {
    /// Create a new atlas page.
    pub fn new(texture_id: TextureId, width: u32, height: u32, padding: u32) -> Self {
        Self {
            texture_id,
            width,
            height,
            shelves: Vec::new(),
            padding,
            next_shelf_y: 0,
            sprite_count: 0,
        }
    }

    /// Try to insert a sprite of the given dimensions into this page.
    /// Returns the pixel rect if successful, or `None` if no space.
    pub fn try_insert(&mut self, w: u32, h: u32) -> Option<AtlasRect> {
        let padded_w = w + self.padding;
        let padded_h = h + self.padding;

        // Try existing shelves (best-fit by height).
        let mut best_shelf: Option<usize> = None;
        let mut best_waste = u32::MAX;

        for (i, shelf) in self.shelves.iter().enumerate() {
            if shelf.cursor_x + w <= self.width && shelf.height >= h {
                let waste = shelf.height - h;
                if waste < best_waste {
                    best_waste = waste;
                    best_shelf = Some(i);
                }
            }
        }

        if let Some(si) = best_shelf {
            let shelf = &mut self.shelves[si];
            let rect = AtlasRect {
                x: shelf.cursor_x,
                y: shelf.y,
                w,
                h,
            };
            shelf.cursor_x += padded_w;
            self.sprite_count += 1;
            return Some(rect);
        }

        // Start a new shelf if there is vertical space.
        if self.next_shelf_y + h <= self.height && w <= self.width {
            let rect = AtlasRect {
                x: 0,
                y: self.next_shelf_y,
                w,
                h,
            };
            self.shelves.push(Shelf {
                y: self.next_shelf_y,
                height: h,
                cursor_x: padded_w,
            });
            self.next_shelf_y += padded_h;
            self.sprite_count += 1;
            return Some(rect);
        }

        None
    }

    /// Compute normalised UV rect for a pixel rect on this page.
    pub fn uv_for(&self, rect: &AtlasRect) -> Rect {
        let fw = self.width as f32;
        let fh = self.height as f32;
        Rect {
            x: rect.x as f32 / fw,
            y: rect.y as f32 / fh,
            w: rect.w as f32 / fw,
            h: rect.h as f32 / fh,
        }
    }
}

// ---------------------------------------------------------------------------
// DynamicAtlas
// ---------------------------------------------------------------------------

/// Insertion result from [`DynamicAtlas::insert`].
#[derive(Clone, Copy, Debug)]
pub struct InsertResult {
    /// Which page the sprite was placed on.
    pub page_index: u32,
    /// Pixel rect within the page.
    pub rect: AtlasRect,
    /// Normalised UV coordinates.
    pub uv: Rect,
    /// The GPU texture ID for the page.
    pub texture_id: TextureId,
}

/// Manages one or more atlas pages for dynamic runtime sprite packing.
pub struct DynamicAtlas {
    /// All atlas pages.
    pub pages: Vec<AtlasPage>,
    /// Maximum page dimension.
    max_page_size: u32,
    /// Padding between sprites.
    padding: u32,
    /// Next texture ID to assign to new pages.
    next_texture_id: u32,
}

impl DynamicAtlas {
    /// Create a new dynamic atlas.
    ///
    /// * `max_page_size` - Maximum atlas page dimension (power-of-2, e.g. 4096).
    /// * `padding` - Gap between sprites in pixels.
    /// * `first_texture_id` - Starting texture ID for atlas pages (to avoid
    ///   collision with non-atlas textures).
    pub fn new(max_page_size: u32, padding: u32, first_texture_id: u32) -> Self {
        Self {
            pages: Vec::new(),
            max_page_size,
            padding,
            next_texture_id: first_texture_id,
        }
    }

    /// Insert a sprite into the atlas. Tries existing pages first, then
    /// allocates a new page if needed.
    ///
    /// Returns `Some(InsertResult)` on success, `None` if the sprite exceeds
    /// the maximum page size.
    pub fn insert(&mut self, w: u32, h: u32) -> Option<InsertResult> {
        if w > self.max_page_size || h > self.max_page_size {
            return None;
        }

        // Try existing pages.
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some(rect) = page.try_insert(w, h) {
                let uv = page.uv_for(&rect);
                let texture_id = page.texture_id;
                return Some(InsertResult {
                    page_index: i as u32,
                    rect,
                    uv,
                    texture_id,
                });
            }
        }

        // Allocate a new page.
        let page_w = next_power_of_two(w.max(256)).min(self.max_page_size);
        let page_h = next_power_of_two(h.max(256)).min(self.max_page_size);
        let tex_id = TextureId(self.next_texture_id);
        self.next_texture_id += 1;

        let mut page = AtlasPage::new(tex_id, page_w, page_h, self.padding);
        let rect = page.try_insert(w, h)?;
        let uv = page.uv_for(&rect);
        self.pages.push(page);

        Some(InsertResult {
            page_index: (self.pages.len() - 1) as u32,
            rect,
            uv,
            texture_id: tex_id,
        })
    }

    /// Number of atlas pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Total GPU memory used by all pages (4 bytes per pixel, RGBA8).
    pub fn total_gpu_bytes(&self) -> u64 {
        self.pages
            .iter()
            .map(|p| (p.width as u64) * (p.height as u64) * 4)
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_single_sprite() {
        let mut atlas = DynamicAtlas::new(4096, 1, 100);
        let result = atlas.insert(32, 32).unwrap();
        assert_eq!(result.page_index, 0);
        assert_eq!(result.rect.w, 32);
        assert_eq!(result.rect.h, 32);
        assert!(result.uv.w > 0.0);
        assert!(result.uv.h > 0.0);
    }

    #[test]
    fn insert_multiple_same_page() {
        let mut atlas = DynamicAtlas::new(4096, 0, 100);
        let r1 = atlas.insert(64, 64).unwrap();
        let r2 = atlas.insert(64, 64).unwrap();
        // Both should be on the same page.
        assert_eq!(r1.page_index, r2.page_index);
        // They should not overlap.
        let overlap_x = r1.rect.x < r2.rect.x + r2.rect.w && r1.rect.x + r1.rect.w > r2.rect.x;
        let overlap_y = r1.rect.y < r2.rect.y + r2.rect.h && r1.rect.y + r1.rect.h > r2.rect.y;
        assert!(!(overlap_x && overlap_y), "sprites overlap");
    }

    #[test]
    fn insert_triggers_new_page() {
        // Tiny max size so we quickly overflow.
        let mut atlas = DynamicAtlas::new(64, 0, 100);
        // Fill the page.
        for _ in 0..4 {
            atlas.insert(32, 32).unwrap();
        }
        // This should go to a new page.
        let result = atlas.insert(32, 32).unwrap();
        assert!(atlas.page_count() >= 2, "expected new page allocation");
    }

    #[test]
    fn oversized_sprite_rejected() {
        let mut atlas = DynamicAtlas::new(64, 0, 100);
        assert!(atlas.insert(128, 128).is_none());
    }

    #[test]
    fn uv_coordinates_valid() {
        let mut atlas = DynamicAtlas::new(4096, 1, 100);
        for _ in 0..20 {
            let result = atlas.insert(48, 48).unwrap();
            assert!(result.uv.x >= 0.0 && result.uv.x <= 1.0);
            assert!(result.uv.y >= 0.0 && result.uv.y <= 1.0);
            assert!(result.uv.w > 0.0);
            assert!(result.uv.h > 0.0);
            assert!(result.uv.x + result.uv.w <= 1.0 + f32::EPSILON);
            assert!(result.uv.y + result.uv.h <= 1.0 + f32::EPSILON);
        }
    }
}
