//! Collision layer editor — provides tools and data structures for editing
//! collision maps within the level editor.

use amigo_tilemap::CollisionType;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Collision brush
// ---------------------------------------------------------------------------

/// The currently selected collision type for painting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollisionBrush {
    #[default]
    Empty,
    Solid,
    OneWay,
    Slope(SlopeDirection),
    Trigger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlopeDirection {
    UpRight,
    UpLeft,
}

impl CollisionBrush {
    /// Convert brush to the tilemap's collision tile value.
    pub fn to_collision_tile(self) -> CollisionType {
        match self {
            CollisionBrush::Empty => CollisionType::Empty,
            CollisionBrush::Solid => CollisionType::Solid,
            CollisionBrush::OneWay => CollisionType::OneWay,
            CollisionBrush::Slope(SlopeDirection::UpRight) => CollisionType::Slope {
                left_height: 0,
                right_height: 16,
            },
            CollisionBrush::Slope(SlopeDirection::UpLeft) => CollisionType::Slope {
                left_height: 16,
                right_height: 0,
            },
            CollisionBrush::Trigger => CollisionType::Trigger { id: 0 },
        }
    }

    /// All available brushes for the palette.
    pub fn all() -> &'static [CollisionBrush] {
        &[
            CollisionBrush::Empty,
            CollisionBrush::Solid,
            CollisionBrush::OneWay,
            CollisionBrush::Slope(SlopeDirection::UpRight),
            CollisionBrush::Slope(SlopeDirection::UpLeft),
            CollisionBrush::Trigger,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            CollisionBrush::Empty => "Empty",
            CollisionBrush::Solid => "Solid",
            CollisionBrush::OneWay => "OneWay",
            CollisionBrush::Slope(SlopeDirection::UpRight) => "Slope UR",
            CollisionBrush::Slope(SlopeDirection::UpLeft) => "Slope UL",
            CollisionBrush::Trigger => "Trigger",
        }
    }
}

// ---------------------------------------------------------------------------
// Collision editor state
// ---------------------------------------------------------------------------

/// State for the collision editing sub-mode of the level editor.
pub struct CollisionEditorState {
    /// Currently selected brush.
    pub brush: CollisionBrush,
    /// Whether collision overlay is visible.
    pub visible: bool,
    /// Opacity of the collision overlay (0.0 – 1.0).
    pub overlay_alpha: f32,
    /// The collision grid being edited (width × height).
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<CollisionType>,
    /// Undo stack for collision changes.
    pub undo_stack: Vec<CollisionEdit>,
    pub redo_stack: Vec<CollisionEdit>,
}

/// A single collision tile edit (for undo/redo).
#[derive(Clone, Debug)]
pub struct CollisionEdit {
    pub x: u32,
    pub y: u32,
    pub old: CollisionType,
    pub new: CollisionType,
}

impl CollisionEditorState {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            brush: CollisionBrush::default(),
            visible: true,
            overlay_alpha: 0.4,
            width,
            height,
            tiles: vec![CollisionType::Empty; (width * height) as usize],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Load collision data from an existing layer.
    pub fn load(&mut self, tiles: &[CollisionType], width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.tiles = tiles.to_vec();
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Get the collision tile at (x, y).
    pub fn get(&self, x: u32, y: u32) -> CollisionType {
        if x < self.width && y < self.height {
            self.tiles[(y * self.width + x) as usize]
        } else {
            CollisionType::Empty
        }
    }

    /// Paint a collision tile, recording the edit for undo.
    pub fn paint(&mut self, x: u32, y: u32) {
        if x >= self.width || y >= self.height {
            return;
        }

        let idx = (y * self.width + x) as usize;
        let old = self.tiles[idx];
        let new = self.brush.to_collision_tile();

        if old == new {
            return;
        }

        self.tiles[idx] = new;
        self.redo_stack.clear();
        self.undo_stack.push(CollisionEdit { x, y, old, new });
    }

    /// Fill a rectangular region with the current brush.
    pub fn fill_rect(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) {
        let min_x = x1.min(x2);
        let max_x = x1.max(x2).min(self.width.saturating_sub(1));
        let min_y = y1.min(y2);
        let max_y = y1.max(y2).min(self.height.saturating_sub(1));

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                self.paint(x, y);
            }
        }
    }

    /// Undo the last edit.
    pub fn undo(&mut self) -> bool {
        if let Some(edit) = self.undo_stack.pop() {
            let idx = (edit.y * self.width + edit.x) as usize;
            self.tiles[idx] = edit.old;
            self.redo_stack.push(edit);
            true
        } else {
            false
        }
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) -> bool {
        if let Some(edit) = self.redo_stack.pop() {
            let idx = (edit.y * self.width + edit.x) as usize;
            self.tiles[idx] = edit.new;
            self.undo_stack.push(edit);
            true
        } else {
            false
        }
    }

    /// Count how many tiles of each collision type exist.
    pub fn stats(&self) -> CollisionStats {
        let mut stats = CollisionStats::default();
        for tile in &self.tiles {
            match tile {
                CollisionType::Empty => stats.empty += 1,
                CollisionType::Solid => stats.solid += 1,
                CollisionType::OneWay => stats.one_way += 1,
                CollisionType::Slope { .. } => stats.slope += 1,
                CollisionType::Trigger { .. } => stats.trigger += 1,
            }
        }
        stats
    }

    /// Export the collision data as a flat vector (for saving).
    pub fn export(&self) -> Vec<CollisionType> {
        self.tiles.clone()
    }
}

/// Statistics about collision tile usage.
#[derive(Debug, Default)]
pub struct CollisionStats {
    pub empty: u32,
    pub solid: u32,
    pub one_way: u32,
    pub slope: u32,
    pub trigger: u32,
}

// ---------------------------------------------------------------------------
// Asset browser (lightweight file listing)
// ---------------------------------------------------------------------------

/// Categories shown in the asset browser panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetCategory {
    Sprites,
    Tilesets,
    Levels,
    Audio,
    Fonts,
    Scripts,
    All,
}

impl AssetCategory {
    pub fn label(self) -> &'static str {
        match self {
            AssetCategory::Sprites => "Sprites",
            AssetCategory::Tilesets => "Tilesets",
            AssetCategory::Levels => "Levels",
            AssetCategory::Audio => "Audio",
            AssetCategory::Fonts => "Fonts",
            AssetCategory::Scripts => "Scripts",
            AssetCategory::All => "All",
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            AssetCategory::Sprites => &["png", "aseprite", "ase"],
            AssetCategory::Tilesets => &["png", "tsx"],
            AssetCategory::Levels => &["amigo", "ron"],
            AssetCategory::Audio => &["ogg", "wav", "mp3"],
            AssetCategory::Fonts => &["ttf", "otf"],
            AssetCategory::Scripts => &["json", "ron"],
            AssetCategory::All => &[],
        }
    }
}

/// An entry in the asset browser.
#[derive(Clone, Debug)]
pub struct AssetEntry {
    pub name: String,
    pub path: String,
    pub category: AssetCategory,
    pub size_bytes: u64,
}

/// Asset browser state.
pub struct AssetBrowser {
    pub category: AssetCategory,
    pub entries: Vec<AssetEntry>,
    pub selected: Option<usize>,
    pub search_query: String,
}

impl AssetBrowser {
    pub fn new() -> Self {
        Self {
            category: AssetCategory::All,
            entries: Vec::new(),
            selected: None,
            search_query: String::new(),
        }
    }

    /// Scan a directory and populate entries.
    pub fn scan(&mut self, base_path: &std::path::Path) {
        self.entries.clear();
        self.selected = None;

        let subdirs = [
            ("assets/sprites", AssetCategory::Sprites),
            ("assets/tilesets", AssetCategory::Tilesets),
            ("assets/levels", AssetCategory::Levels),
            ("assets/audio", AssetCategory::Audio),
            ("assets/fonts", AssetCategory::Fonts),
        ];

        for (subdir, category) in &subdirs {
            let dir = base_path.join(subdir);
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        self.entries.push(AssetEntry {
                            name,
                            path: path.to_string_lossy().to_string(),
                            category: *category,
                            size_bytes: size,
                        });
                    }
                }
            }
        }

        // Sort by name
        self.entries.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Filter entries by current category and search query.
    pub fn filtered(&self) -> Vec<&AssetEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if self.category != AssetCategory::All && e.category != self.category {
                    return false;
                }
                if !self.search_query.is_empty() {
                    return e
                        .name
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase());
                }
                true
            })
            .collect()
    }

    /// Select an entry by index.
    pub fn select(&mut self, index: usize) {
        if index < self.entries.len() {
            self.selected = Some(index);
        }
    }

    /// Get the currently selected entry.
    pub fn selected_entry(&self) -> Option<&AssetEntry> {
        self.selected.and_then(|i| self.entries.get(i))
    }
}

impl Default for AssetBrowser {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_brush_to_tile() {
        assert_eq!(
            CollisionBrush::Solid.to_collision_tile(),
            CollisionType::Solid
        );
        assert_eq!(
            CollisionBrush::Empty.to_collision_tile(),
            CollisionType::Empty
        );
        assert_eq!(
            CollisionBrush::OneWay.to_collision_tile(),
            CollisionType::OneWay
        );
    }

    #[test]
    fn collision_editor_paint_and_undo() {
        let mut editor = CollisionEditorState::new(10, 10);
        assert_eq!(editor.get(0, 0), CollisionType::Empty);

        editor.brush = CollisionBrush::Solid;
        editor.paint(0, 0);
        assert_eq!(editor.get(0, 0), CollisionType::Solid);

        // Undo should revert
        assert!(editor.undo());
        assert_eq!(editor.get(0, 0), CollisionType::Empty);

        // Redo should re-apply
        assert!(editor.redo());
        assert_eq!(editor.get(0, 0), CollisionType::Solid);
    }

    #[test]
    fn collision_editor_fill_rect() {
        let mut editor = CollisionEditorState::new(10, 10);
        editor.brush = CollisionBrush::Solid;
        editor.fill_rect(2, 2, 4, 4);

        for y in 2..=4 {
            for x in 2..=4 {
                assert_eq!(editor.get(x, y), CollisionType::Solid);
            }
        }
        // Outside the rect should still be empty
        assert_eq!(editor.get(1, 1), CollisionType::Empty);
        assert_eq!(editor.get(5, 5), CollisionType::Empty);
    }

    #[test]
    fn collision_editor_stats() {
        let mut editor = CollisionEditorState::new(4, 4);
        editor.brush = CollisionBrush::Solid;
        editor.paint(0, 0);
        editor.paint(1, 0);
        editor.brush = CollisionBrush::OneWay;
        editor.paint(2, 0);

        let stats = editor.stats();
        assert_eq!(stats.solid, 2);
        assert_eq!(stats.one_way, 1);
        assert_eq!(stats.empty, 13);
    }

    #[test]
    fn collision_paint_same_is_noop() {
        let mut editor = CollisionEditorState::new(5, 5);
        editor.brush = CollisionBrush::Empty; // same as default
        editor.paint(0, 0);
        assert!(editor.undo_stack.is_empty()); // no edit recorded
    }

    #[test]
    fn collision_editor_export() {
        let mut editor = CollisionEditorState::new(3, 3);
        editor.brush = CollisionBrush::Solid;
        editor.paint(1, 1);
        let data = editor.export();
        assert_eq!(data.len(), 9);
        assert_eq!(data[4], CollisionType::Solid); // (1,1) in 3-wide grid
    }

    #[test]
    fn asset_browser_filter() {
        let mut browser = AssetBrowser::new();
        browser.entries.push(AssetEntry {
            name: "player.png".to_string(),
            path: "assets/sprites/player.png".to_string(),
            category: AssetCategory::Sprites,
            size_bytes: 1024,
        });
        browser.entries.push(AssetEntry {
            name: "level_01.amigo".to_string(),
            path: "assets/levels/level_01.amigo".to_string(),
            category: AssetCategory::Levels,
            size_bytes: 2048,
        });

        // All
        browser.category = AssetCategory::All;
        assert_eq!(browser.filtered().len(), 2);

        // Filter by sprites
        browser.category = AssetCategory::Sprites;
        assert_eq!(browser.filtered().len(), 1);
        assert_eq!(browser.filtered()[0].name, "player.png");

        // Search
        browser.category = AssetCategory::All;
        browser.search_query = "level".to_string();
        assert_eq!(browser.filtered().len(), 1);
    }

    #[test]
    fn asset_browser_selection() {
        let mut browser = AssetBrowser::new();
        browser.entries.push(AssetEntry {
            name: "test.png".to_string(),
            path: "test.png".to_string(),
            category: AssetCategory::Sprites,
            size_bytes: 100,
        });

        assert!(browser.selected_entry().is_none());
        browser.select(0);
        assert_eq!(browser.selected_entry().unwrap().name, "test.png");
    }
}
