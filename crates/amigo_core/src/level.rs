//! The `.amigo` level document: what the editor edits and saves, and what
//! [`crate::level_loader`] reads back for a game.
//!
//! These types used to live in `amigo_editor`, so a game could only read a
//! level through `level_loader::LoadedLevel`, whose stricter format (per-layer
//! sizes, required `zones`) rejected every file the editor and `amigo new`
//! wrote. Both now share this one format; `LoadedLevel` is the read-side view
//! with query helpers, built from an [`AmigoLevel`].

use crate::level_loader::ZoneDef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A single tile layer in a level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerData {
    pub name: String,
    /// `width * height` tile ids, row by row; 0 is empty.
    pub tiles: Vec<u16>,
    #[serde(default = "visible_by_default")]
    pub visible: bool,
}

fn visible_by_default() -> bool {
    true
}

/// A placed entity instance inside a level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityPlacement {
    pub entity_type: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

/// A named path (e.g. for AI movement or camera rails).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathData {
    pub name: String,
    pub points: Vec<(f32, f32)>,
    #[serde(default)]
    pub closed: bool,
}

/// The complete level document serialized as `.amigo` (RON format).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmigoLevel {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub layers: Vec<LayerData>,
    #[serde(default)]
    pub entities: Vec<EntityPlacement>,
    #[serde(default)]
    pub paths: Vec<PathData>,
    /// Trigger and spawn areas. The editor has no zone tool yet, but keeps
    /// zones written by hand when it saves.
    #[serde(default)]
    pub zones: Vec<ZoneDef>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Upper bound on tiles per layer, so a hostile or corrupt `.amigo` file cannot
/// make the editor allocate absurd amounts up front. 4096x4096 tiles is far past
/// any hand-authored pixel-art level.
pub const MAX_LEVEL_TILES: u64 = 4096 * 4096;

/// Number of cells in a `width * height` grid.
///
/// Computed in `u64`: `width * height` in `u32` wraps for large dimensions, and
/// a wrapped length produces a buffer far too small for the `y * width + x`
/// indexing that follows.
///
/// # Panics
/// If the grid exceeds [`MAX_LEVEL_TILES`] cells. Editor grids are sized from
/// level dimensions, so anything past this is a caller bug rather than
/// something to silently clamp.
pub fn grid_len(width: u32, height: u32) -> usize {
    let cells = u64::from(width) * u64::from(height);
    assert!(
        cells <= MAX_LEVEL_TILES,
        "grid of {}x{} = {} cells exceeds the {} cell limit",
        width,
        height,
        cells,
        MAX_LEVEL_TILES
    );
    cells as usize
}

impl AmigoLevel {
    /// An empty level with one visible layer called `ground`.
    ///
    /// # Panics
    /// If `width * height` exceeds [`MAX_LEVEL_TILES`].
    pub fn new(name: impl Into<String>, width: u32, height: u32, tile_size: u32) -> Self {
        Self {
            name: name.into(),
            width,
            height,
            tile_size,
            layers: vec![LayerData {
                name: "ground".to_string(),
                tiles: vec![0; grid_len(width, height)],
                visible: true,
            }],
            entities: Vec::new(),
            paths: Vec::new(),
            zones: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Index of the layer called `name`.
    pub fn layer_index(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.name == name)
    }

    /// The tile at `(x, y)` on `layer`, or `None` outside the level.
    pub fn tile(&self, layer: usize, x: i32, y: i32) -> Option<u16> {
        let index = self.cell_index(x, y)?;
        self.layers.get(layer)?.tiles.get(index).copied()
    }

    /// The `tiles` index of `(x, y)`, or `None` outside the level.
    pub fn cell_index(&self, x: i32, y: i32) -> Option<usize> {
        let (x, y) = (u32::try_from(x).ok()?, u32::try_from(y).ok()?);
        if x >= self.width || y >= self.height {
            return None;
        }
        usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()
    }

    /// Check that the level's declared dimensions match its data.
    ///
    /// Consumers index layer tiles as `y * width + x`, so a layer whose tile
    /// count disagrees with `width * height` means out-of-bounds reads or
    /// silently wrong tiles. `tile_size == 0` divides by zero when converting
    /// world positions to tile coordinates.
    pub fn validate(&self) -> Result<(), String> {
        if self.tile_size == 0 {
            return Err("tile_size must be greater than 0".to_string());
        }

        let expected = u64::from(self.width) * u64::from(self.height);
        if expected > MAX_LEVEL_TILES {
            return Err(format!(
                "level is {}x{} = {} tiles, which exceeds the {} tile limit",
                self.width, self.height, expected, MAX_LEVEL_TILES
            ));
        }

        for (i, layer) in self.layers.iter().enumerate() {
            if layer.tiles.len() as u64 != expected {
                return Err(format!(
                    "layer {} ('{}') has {} tiles but the level is {}x{} = {}",
                    i,
                    layer.name,
                    layer.tiles.len(),
                    self.width,
                    self.height,
                    expected
                ));
            }
        }

        for (i, path) in self.paths.iter().enumerate() {
            if path.closed && path.points.len() < 3 {
                return Err(format!(
                    "path {} ('{}') is closed but has only {} point(s)",
                    i,
                    path.name,
                    path.points.len()
                ));
            }
        }

        Ok(())
    }
}

/// Parse and validate a level from RON text.
pub fn level_from_str(ron_str: &str) -> Result<AmigoLevel, String> {
    let level: AmigoLevel = ron::from_str(ron_str).map_err(|e| e.to_string())?;
    level.validate()?;
    Ok(level)
}

/// Serialize a level to RON and write it to `path`.
///
/// The text goes to a temporary file next to `path`, is flushed to disk, and
/// then renamed over the old file, so a crash mid-save leaves the previous
/// level intact instead of a truncated one.
pub fn save_level(path: &Path, level: &AmigoLevel) -> Result<(), std::io::Error> {
    use std::io::Write;

    let ron_string = ron::ser::to_string_pretty(level, ron::ser::PrettyConfig::default())
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other(format!("{} has no file name", path.display())))?;
    let mut tmp_name = file_name.to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);

    let written = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(ron_string.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// Load a level from a RON file at the given path.
///
/// The file is validated before being handed back — see
/// [`AmigoLevel::validate`]. A `.amigo` file is external input like any other
/// asset, and consumers index layer tiles as `y * width + x`.
pub fn load_level(path: &Path) -> Result<AmigoLevel, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    level_from_str(&contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(width: u32, height: u32, tile_count: usize) -> AmigoLevel {
        let mut level = AmigoLevel::new("test", 1, 1, 16);
        level.width = width;
        level.height = height;
        level.layers[0].tiles = vec![0; tile_count];
        level
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("amigo_level_{}_{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn new_levels_are_valid_and_empty() {
        let level = AmigoLevel::new("Start", 6, 4, 16);
        assert!(level.validate().is_ok());
        assert_eq!(level.layers[0].tiles.len(), 24);
        assert_eq!(level.layer_index("ground"), Some(0));
        assert_eq!(level.tile(0, 5, 3), Some(0));
        assert_eq!(level.tile(0, 6, 0), None);
        assert_eq!(level.tile(0, -1, 0), None);
        assert_eq!(level.tile(1, 0, 0), None);
    }

    #[test]
    fn validate_accepts_a_consistent_level() {
        assert!(level(8, 8, 64).validate().is_ok());
    }

    #[test]
    fn validate_rejects_tile_count_mismatch() {
        let err = level(8, 8, 63).validate().expect_err("should be rejected");
        assert!(
            err.contains("63"),
            "error should name the actual count: {err}"
        );
    }

    #[test]
    fn validate_rejects_zero_tile_size() {
        let mut l = level(8, 8, 64);
        l.tile_size = 0;
        assert!(l.validate().is_err(), "tile_size 0 divides by zero later");
    }

    #[test]
    fn validate_rejects_absurd_dimensions() {
        // Declared dimensions that would overflow a u32 multiply.
        assert!(level(u32::MAX, u32::MAX, 0).validate().is_err());
    }

    #[test]
    fn validate_rejects_closed_path_without_enough_points() {
        let mut l = level(4, 4, 16);
        l.paths.push(PathData {
            name: "loop".into(),
            points: vec![(0.0, 0.0), (1.0, 1.0)],
            closed: true,
        });
        assert!(l.validate().is_err());
    }

    #[test]
    fn load_level_rejects_an_inconsistent_file() {
        // width*height says 64 tiles, the layer carries 4.
        let path = temp_path("bogus.amigo");
        save_level(&path, &level(8, 8, 4)).expect("write");
        let err = load_level(&path).expect_err("should refuse an inconsistent level");
        assert!(err.contains("tiles"), "unexpected error: {err}");
    }

    #[test]
    fn save_load_round_trip_keeps_zones_and_leaves_no_temp_file() {
        let mut original = AmigoLevel::new("zoned", 6, 5, 16);
        original.layers[0].tiles[7] = 3;
        original.zones.push(ZoneDef {
            name: "exit".into(),
            x: 0.0,
            y: 0.0,
            w: 16.0,
            h: 16.0,
            properties: HashMap::new(),
        });
        let path = temp_path("ok.amigo");
        save_level(&path, &original).expect("write");
        // Saving again replaces the file in place.
        save_level(&path, &original).expect("overwrite");

        let loaded = load_level(&path).expect("read back");
        assert_eq!((loaded.width, loaded.height), (6, 5));
        assert_eq!(loaded.layers[0].tiles[7], 3);
        assert_eq!(loaded.zones.len(), 1);
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, [std::ffi::OsString::from("ok.amigo")]);
    }

    /// Hand-written files may leave out everything but the grid.
    #[test]
    fn optional_sections_default() {
        let level = level_from_str(
            "(name: \"min\", width: 2, height: 1, tile_size: 8, layers: [(name: \"a\", tiles: [1, 0])])",
        )
        .expect("minimal level parses");
        assert!(level.layers[0].visible);
        assert!(level.entities.is_empty() && level.paths.is_empty() && level.zones.is_empty());
    }

    /// `width * height` in u32 wraps: 65536*65536 is 0, which used to produce an
    /// empty buffer that later got indexed as if it were full.
    #[test]
    fn grid_len_does_not_wrap() {
        assert_eq!(grid_len(4096, 4096), 4096 * 4096);
        assert_eq!(grid_len(0, 0), 0);
    }

    #[test]
    #[should_panic(expected = "exceeds")]
    fn grid_len_rejects_wrapping_dimensions() {
        // In u32 this multiplies to 0.
        grid_len(65536, 65536);
    }
}
