use amigo_core::Rect;
use rustc_hash::FxHashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Position and size of a sprite within the atlas texture (in pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// A single entry in a packed atlas.
#[derive(Debug, Clone)]
pub struct AtlasEntry {
    pub name: String,
    pub rect: AtlasRect,
    /// Normalised UV coordinates (0.0 .. 1.0) suitable for rendering.
    pub uv: Rect,
}

/// Errors that can occur during atlas packing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtlasError {
    /// The sprites do not fit within `max_size`.
    TooLarge,
    /// No sprites were added before calling `pack()`.
    Empty,
}

impl std::fmt::Display for AtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtlasError::TooLarge => write!(f, "sprites exceed maximum atlas size"),
            AtlasError::Empty => write!(f, "no sprites to pack"),
        }
    }
}

impl std::error::Error for AtlasError {}

// ---------------------------------------------------------------------------
// AtlasPack – the result of a successful pack operation
// ---------------------------------------------------------------------------

/// The output of [`AtlasBuilder::pack`].
#[derive(Debug, Clone)]
pub struct AtlasPack {
    /// Width of the atlas texture (power-of-2).
    pub width: u32,
    /// Height of the atlas texture (power-of-2).
    pub height: u32,
    /// Lookup table keyed by sprite name.
    pub entries: FxHashMap<String, AtlasEntry>,
}

impl AtlasPack {
    /// Look up an entry by name.
    pub fn get(&self, name: &str) -> Option<&AtlasEntry> {
        self.entries.get(name)
    }

    /// Convenience: return only the UV [`Rect`] for a named sprite.
    pub fn uv_rect(&self, name: &str) -> Option<Rect> {
        self.entries.get(name).map(|e| e.uv)
    }
}

// ---------------------------------------------------------------------------
// AtlasBuilder
// ---------------------------------------------------------------------------

/// Pending sprite to be packed.
struct Pending {
    name: String,
    width: u32,
    height: u32,
}

/// Builds a texture atlas using a shelf (row) packing algorithm.
pub struct AtlasBuilder {
    max_size: u32,
    padding: u32,
    pending: Vec<Pending>,
}

impl AtlasBuilder {
    /// Create a new builder.
    ///
    /// * `max_size` – maximum atlas dimension (both width and height).
    /// * `padding`  – gap inserted between sprites (in pixels).
    pub fn new(max_size: u32, padding: u32) -> Self {
        Self {
            max_size,
            padding,
            pending: Vec::new(),
        }
    }

    /// Register a sprite to be packed.
    pub fn add(&mut self, name: String, width: u32, height: u32) {
        self.pending.push(Pending {
            name,
            width,
            height,
        });
    }

    /// Run the shelf-packing algorithm and return an [`AtlasPack`].
    ///
    /// Sprites are sorted by descending height, then placed left-to-right in
    /// rows (shelves).  A new row is started whenever the next sprite would
    /// exceed `max_size` horizontally.
    pub fn pack(mut self) -> Result<AtlasPack, AtlasError> {
        if self.pending.is_empty() {
            return Err(AtlasError::Empty);
        }

        // Sort by height descending (stable sort keeps insertion order for
        // sprites of equal height).
        self.pending.sort_by(|a, b| b.height.cmp(&a.height));

        let mut entries = FxHashMap::default();

        // Current cursor inside the atlas.
        let mut cursor_x: u32 = 0;
        let mut cursor_y: u32 = 0;
        // Height of the tallest sprite on the current shelf.
        let mut shelf_height: u32 = 0;
        // Required atlas dimensions (before power-of-2 rounding).
        let mut required_width: u32 = 0;

        for sprite in &self.pending {
            let padded_w = sprite.width + self.padding;

            // If the sprite does not fit on the current shelf, start a new one.
            if cursor_x + sprite.width > self.max_size {
                cursor_y += shelf_height + self.padding;
                cursor_x = 0;
                shelf_height = 0;
            }

            // Check that the sprite fits vertically.
            if cursor_y + sprite.height > self.max_size {
                return Err(AtlasError::TooLarge);
            }

            // Also check that a single sprite is not wider than max_size.
            if sprite.width > self.max_size || sprite.height > self.max_size {
                return Err(AtlasError::TooLarge);
            }

            let rect = AtlasRect {
                x: cursor_x,
                y: cursor_y,
                w: sprite.width,
                h: sprite.height,
            };

            entries.insert(
                sprite.name.clone(),
                AtlasEntry {
                    name: sprite.name.clone(),
                    rect,
                    // UV will be filled in once we know the final atlas size.
                    uv: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 0.0,
                        h: 0.0,
                    },
                },
            );

            // Advance cursor.
            cursor_x += padded_w;
            if sprite.height > shelf_height {
                shelf_height = sprite.height;
            }
            if cursor_x - self.padding > required_width {
                required_width = cursor_x - self.padding;
            }
        }

        // Final required height includes the last shelf.
        let required_height = cursor_y + shelf_height;

        // Round up to power-of-2.
        let atlas_w = next_power_of_two(required_width);
        let atlas_h = next_power_of_two(required_height);

        if atlas_w > self.max_size || atlas_h > self.max_size {
            return Err(AtlasError::TooLarge);
        }

        // Fill in normalised UV coordinates.
        let fw = atlas_w as f32;
        let fh = atlas_h as f32;
        for entry in entries.values_mut() {
            let r = &entry.rect;
            entry.uv = Rect {
                x: r.x as f32 / fw,
                y: r.y as f32 / fh,
                w: r.w as f32 / fw,
                h: r.h as f32 / fh,
            };
        }

        Ok(AtlasPack {
            width: atlas_w,
            height: atlas_h,
            entries,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Round `n` up to the next power of two. Returns 1 for `n == 0`.
pub fn next_power_of_two(n: u32) -> u32 {
    if n == 0 {
        return 1;
    }
    n.next_power_of_two()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Packing ──────────────────────────────────────────────────

    #[test]
    fn pack_single_sprite() {
        let mut builder = AtlasBuilder::new(1024, 0);
        builder.add("hero".into(), 64, 64);
        let pack = builder.pack().unwrap();

        assert_eq!(pack.width, 64);
        assert_eq!(pack.height, 64);

        let entry = pack.get("hero").unwrap();
        assert_eq!(
            entry.rect,
            AtlasRect {
                x: 0,
                y: 0,
                w: 64,
                h: 64
            }
        );
    }

    #[test]
    fn pack_multiple_no_overlap() {
        let mut builder = AtlasBuilder::new(1024, 0);
        builder.add("a".into(), 32, 32);
        builder.add("b".into(), 64, 64);
        builder.add("c".into(), 48, 48);
        builder.add("d".into(), 16, 16);

        let pack = builder.pack().unwrap();

        let rects: Vec<AtlasRect> = pack.entries.values().map(|e| e.rect).collect();

        // Brute-force overlap check.
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                let overlap_x = a.x < b.x + b.w && a.x + a.w > b.x;
                let overlap_y = a.y < b.y + b.h && a.y + a.h > b.y;
                assert!(
                    !(overlap_x && overlap_y),
                    "overlap detected: {:?} vs {:?}",
                    a,
                    b
                );
            }
        }
    }

    #[test]
    fn power_of_two_sizing() {
        let mut builder = AtlasBuilder::new(1024, 0);
        builder.add("a".into(), 33, 17);
        let pack = builder.pack().unwrap();

        assert!(
            pack.width.is_power_of_two(),
            "width {} not pow2",
            pack.width
        );
        assert!(
            pack.height.is_power_of_two(),
            "height {} not pow2",
            pack.height
        );
        assert!(pack.width >= 33);
        assert!(pack.height >= 17);
    }

    // ── Error cases ───────────────────────────────────────────────

    #[test]
    fn overflow_too_large() {
        let mut builder = AtlasBuilder::new(64, 0);
        builder.add("huge".into(), 128, 128);
        assert_eq!(builder.pack().unwrap_err(), AtlasError::TooLarge);
    }

    #[test]
    fn overflow_cumulative() {
        let mut builder = AtlasBuilder::new(64, 0);
        // 5 sprites of 32x32 need at least 160 pixels wide or multiple rows.
        // With max_size=64 we can fit 2 per row, so 3 rows = 96 > 64.
        for i in 0..5 {
            builder.add(format!("s{}", i), 32, 32);
        }
        assert_eq!(builder.pack().unwrap_err(), AtlasError::TooLarge);
    }

    #[test]
    fn empty_returns_error() {
        let builder = AtlasBuilder::new(1024, 0);
        assert_eq!(builder.pack().unwrap_err(), AtlasError::Empty);
    }

    // ── UV coordinates & padding ────────────────────────────────

    #[test]
    fn uv_coordinates_normalised() {
        let mut builder = AtlasBuilder::new(1024, 0);
        builder.add("a".into(), 64, 64);
        builder.add("b".into(), 64, 64);
        let pack = builder.pack().unwrap();

        for entry in pack.entries.values() {
            assert!(entry.uv.x >= 0.0 && entry.uv.x <= 1.0, "uv.x out of range");
            assert!(entry.uv.y >= 0.0 && entry.uv.y <= 1.0, "uv.y out of range");
            assert!(entry.uv.w > 0.0, "uv.w must be positive");
            assert!(entry.uv.h > 0.0, "uv.h must be positive");
            assert!(
                entry.uv.x + entry.uv.w <= 1.0 + f32::EPSILON,
                "uv exceeds atlas width"
            );
            assert!(
                entry.uv.y + entry.uv.h <= 1.0 + f32::EPSILON,
                "uv exceeds atlas height"
            );
        }

        // Verify UV matches pixel rect / atlas size.
        let entry_a = pack.get("a").unwrap();
        let expected_u = entry_a.rect.x as f32 / pack.width as f32;
        let expected_v = entry_a.rect.y as f32 / pack.height as f32;
        assert!((entry_a.uv.x - expected_u).abs() < f32::EPSILON);
        assert!((entry_a.uv.y - expected_v).abs() < f32::EPSILON);
    }

    #[test]
    fn padding_between_sprites() {
        let padding = 2;
        let mut builder = AtlasBuilder::new(1024, padding);
        builder.add("a".into(), 32, 32);
        builder.add("b".into(), 32, 32);
        builder.add("c".into(), 32, 32);
        let pack = builder.pack().unwrap();

        let mut rects: Vec<AtlasRect> = pack.entries.values().map(|e| e.rect).collect();
        // Sort by x so we can check adjacency.
        rects.sort_by_key(|r| (r.y, r.x));

        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                // On the same row, there must be at least `padding` pixels gap.
                if a.y == b.y {
                    let gap = if b.x >= a.x + a.w {
                        b.x - (a.x + a.w)
                    } else {
                        a.x - (b.x + b.w)
                    };
                    assert!(
                        gap >= padding,
                        "horizontal gap {} < padding {} between {:?} and {:?}",
                        gap,
                        padding,
                        a,
                        b,
                    );
                }
            }
        }
    }

    // ── Utility ──────────────────────────────────────────────────

    #[test]
    fn next_power_of_two_cases() {
        assert_eq!(next_power_of_two(0), 1);
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(2), 2);
        assert_eq!(next_power_of_two(3), 4);
        assert_eq!(next_power_of_two(5), 8);
        assert_eq!(next_power_of_two(64), 64);
        assert_eq!(next_power_of_two(65), 128);
    }
}
