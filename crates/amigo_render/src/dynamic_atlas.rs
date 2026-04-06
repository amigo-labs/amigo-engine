//! Dynamically growing texture atlas with incremental sprite insertion.
//!
//! Gated behind the `asset_streaming` feature.

use rustc_hash::FxHashMap;

/// Rectangle in the atlas (pixel coordinates).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Errors that can occur during dynamic atlas insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicAtlasError {
    /// The atlas cannot grow any further (all pages full and max size reached).
    TooLarge,
    /// A single sprite exceeds the maximum page dimensions.
    SpriteTooLarge,
}

impl std::fmt::Display for DynamicAtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DynamicAtlasError::TooLarge => write!(f, "dynamic atlas cannot grow further"),
            DynamicAtlasError::SpriteTooLarge => {
                write!(f, "sprite exceeds maximum page dimensions")
            }
        }
    }
}

impl std::error::Error for DynamicAtlasError {}

/// A shelf (horizontal row) inside an atlas page.
struct Shelf {
    y: u32,
    height: u32,
    cursor_x: u32,
}

/// A single page in the dynamic atlas.
struct AtlasPage {
    width: u32,
    height: u32,
    entries: FxHashMap<String, AtlasRect>,
    shelves: Vec<Shelf>,
}

impl AtlasPage {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            entries: FxHashMap::default(),
            shelves: Vec::new(),
        }
    }

    /// Try to insert a sprite onto an existing shelf.
    fn try_insert_on_shelf(
        &mut self,
        name: &str,
        w: u32,
        h: u32,
        padding: u32,
    ) -> Option<AtlasRect> {
        for shelf in &mut self.shelves {
            // Sprite must fit in the shelf height and remaining width.
            if h <= shelf.height && shelf.cursor_x + w <= self.width {
                let rect = AtlasRect {
                    x: shelf.cursor_x,
                    y: shelf.y,
                    w,
                    h,
                };
                shelf.cursor_x += w + padding;
                self.entries.insert(name.to_string(), rect);
                return Some(rect);
            }
        }
        None
    }

    /// Try to create a new shelf and insert the sprite there.
    fn try_insert_new_shelf(
        &mut self,
        name: &str,
        w: u32,
        h: u32,
        padding: u32,
    ) -> Option<AtlasRect> {
        let next_y = if let Some(last) = self.shelves.last() {
            last.y + last.height + padding
        } else {
            0
        };

        // Check that the new shelf fits vertically and the sprite fits horizontally.
        if next_y + h > self.height || w > self.width {
            return None;
        }

        let rect = AtlasRect {
            x: 0,
            y: next_y,
            w,
            h,
        };

        self.shelves.push(Shelf {
            y: next_y,
            height: h,
            cursor_x: w + padding,
        });

        self.entries.insert(name.to_string(), rect);
        Some(rect)
    }

    /// Remove an entry by name. Returns true if found.
    fn remove(&mut self, name: &str) -> bool {
        self.entries.remove(name).is_some()
    }
}

/// A dynamically growing atlas that supports incremental sprite insertion.
pub struct DynamicAtlas {
    pages: Vec<AtlasPage>,
    max_page_size: u32,
    padding: u32,
}

impl DynamicAtlas {
    /// Create a new dynamic atlas.
    ///
    /// * `max_page_size` – maximum width and height for each atlas page.
    /// * `padding` – gap inserted between sprites (in pixels).
    pub fn new(max_page_size: u32, padding: u32) -> Self {
        Self {
            pages: Vec::new(),
            max_page_size,
            padding,
        }
    }

    /// Insert a sprite into the atlas.
    ///
    /// If a sprite with the same name already exists, returns its existing rect
    /// without allocating new space. Returns `(page_index, rect)` on success.
    pub fn insert(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
    ) -> Result<(u32, AtlasRect), DynamicAtlasError> {
        // Return existing entry if already inserted (idempotent).
        if let Some((page_idx, rect)) = self.lookup(name) {
            return Ok((page_idx, *rect));
        }

        if width > self.max_page_size || height > self.max_page_size {
            return Err(DynamicAtlasError::SpriteTooLarge);
        }

        // Try existing pages: first try existing shelves, then new shelves.
        for (idx, page) in self.pages.iter_mut().enumerate() {
            if let Some(rect) = page.try_insert_on_shelf(name, width, height, self.padding) {
                return Ok((idx as u32, rect));
            }
        }
        for (idx, page) in self.pages.iter_mut().enumerate() {
            if let Some(rect) = page.try_insert_new_shelf(name, width, height, self.padding) {
                return Ok((idx as u32, rect));
            }
        }

        // No existing page can fit the sprite – create a new page.
        let mut page = AtlasPage::new(self.max_page_size, self.max_page_size);
        let rect = page
            .try_insert_new_shelf(name, width, height, self.padding)
            .ok_or(DynamicAtlasError::TooLarge)?;
        self.pages.push(page);
        Ok(((self.pages.len() - 1) as u32, rect))
    }

    /// Remove a sprite by name from all pages. Returns true if found.
    pub fn remove(&mut self, name: &str) -> bool {
        for page in &mut self.pages {
            if page.remove(name) {
                return true;
            }
        }
        false
    }

    /// Look up a sprite by name. Returns `(page_index, &AtlasRect)` if found.
    pub fn lookup(&self, name: &str) -> Option<(u32, &AtlasRect)> {
        for (idx, page) in self.pages.iter().enumerate() {
            if let Some(rect) = page.entries.get(name) {
                return Some((idx as u32, rect));
            }
        }
        None
    }

    /// Number of atlas pages currently allocated.
    pub fn page_count(&self) -> usize {
        self.pages.len()
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
        let mut atlas = DynamicAtlas::new(256, 0);
        let (page, rect) = atlas.insert("hero", 64, 64).unwrap();
        assert_eq!(page, 0);
        assert_eq!(
            rect,
            AtlasRect {
                x: 0,
                y: 0,
                w: 64,
                h: 64
            }
        );
        assert_eq!(atlas.page_count(), 1);
    }

    #[test]
    fn insert_many_fills_shelves() {
        let mut atlas = DynamicAtlas::new(128, 0);
        // Insert four 64x64 sprites. Should fit on one page (128x128):
        // shelf 0: sprite0 at (0,0), sprite1 at (64,0)
        // shelf 1: sprite2 at (0,64), sprite3 at (64,64)
        for i in 0..4 {
            let (page, _rect) = atlas.insert(&format!("s{}", i), 64, 64).unwrap();
            assert_eq!(page, 0);
        }
        assert_eq!(atlas.page_count(), 1);
    }

    #[test]
    fn page_overflow_creates_new_page() {
        let mut atlas = DynamicAtlas::new(128, 0);
        // Fill one page: four 64x64 sprites fill 128x128
        for i in 0..4 {
            atlas.insert(&format!("s{}", i), 64, 64).unwrap();
        }
        // Fifth sprite should go to page 1
        let (page, _rect) = atlas.insert("overflow", 64, 64).unwrap();
        assert_eq!(page, 1);
        assert_eq!(atlas.page_count(), 2);
    }

    #[test]
    fn lookup_finds_inserted_sprite() {
        let mut atlas = DynamicAtlas::new(256, 0);
        atlas.insert("hero", 32, 48).unwrap();
        atlas.insert("enemy", 16, 16).unwrap();

        let (page, rect) = atlas.lookup("hero").unwrap();
        assert_eq!(page, 0);
        assert_eq!(rect.w, 32);
        assert_eq!(rect.h, 48);

        let (page, rect) = atlas.lookup("enemy").unwrap();
        assert_eq!(page, 0);
        assert_eq!(rect.w, 16);
        assert_eq!(rect.h, 16);

        assert!(atlas.lookup("missing").is_none());
    }

    #[test]
    fn remove_allows_reinsert() {
        let mut atlas = DynamicAtlas::new(256, 0);
        atlas.insert("hero", 64, 64).unwrap();
        assert!(atlas.lookup("hero").is_some());

        let removed = atlas.remove("hero");
        assert!(removed);
        assert!(atlas.lookup("hero").is_none());

        // Re-insert should succeed.
        let result = atlas.insert("hero", 64, 64);
        assert!(result.is_ok());
        assert!(atlas.lookup("hero").is_some());
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut atlas = DynamicAtlas::new(256, 0);
        assert!(!atlas.remove("ghost"));
    }

    #[test]
    fn too_large_error_for_oversized_sprite() {
        let mut atlas = DynamicAtlas::new(128, 0);
        let err = atlas.insert("huge", 256, 256).unwrap_err();
        assert_eq!(err, DynamicAtlasError::SpriteTooLarge);
    }

    #[test]
    fn too_large_width_only() {
        let mut atlas = DynamicAtlas::new(128, 0);
        let err = atlas.insert("wide", 256, 32).unwrap_err();
        assert_eq!(err, DynamicAtlasError::SpriteTooLarge);
    }

    #[test]
    fn padding_between_sprites() {
        let mut atlas = DynamicAtlas::new(256, 2);
        atlas.insert("a", 32, 32).unwrap();
        let (_, rect_b) = atlas.insert("b", 32, 32).unwrap();
        // "b" should start at x = 32 + 2 = 34 (after padding)
        assert_eq!(rect_b.x, 34);
    }
}
