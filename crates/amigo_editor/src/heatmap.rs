//! Heatmap data collection and visualization.
//!
//! Tracks spatial data (enemy deaths, damage dealt, player paths, etc.)
//! on a tile grid and produces colored overlays for analysis.

use amigo_core::Color;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Heatmap types
// ---------------------------------------------------------------------------

/// What kind of data the heatmap tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeatmapType {
    EnemyDeaths,
    DamageTaken,
    TowerCoverage,
    PlayerPath,
    EnemyDensity,
    Custom,
}

/// A 2D grid of accumulated float values.
#[derive(Clone, Debug)]
pub struct Heatmap {
    pub kind: HeatmapType,
    pub width: u32,
    pub height: u32,
    pub tile_size: f32,
    data: Vec<f32>,
    max_value: f32,
}

impl Heatmap {
    pub fn new(kind: HeatmapType, width: u32, height: u32, tile_size: f32) -> Self {
        Self {
            kind,
            width,
            height,
            tile_size,
            data: vec![0.0; (width * height) as usize],
            max_value: 0.0,
        }
    }

    /// Record a value at a world position.
    pub fn record(&mut self, world_x: f32, world_y: f32, value: f32) {
        let tx = (world_x / self.tile_size) as i32;
        let ty = (world_y / self.tile_size) as i32;
        self.record_tile(tx, ty, value);
    }

    /// Record a value at a tile position.
    pub fn record_tile(&mut self, tx: i32, ty: i32, value: f32) {
        if tx < 0 || ty < 0 || tx >= self.width as i32 || ty >= self.height as i32 {
            return;
        }
        let idx = (ty as u32 * self.width + tx as u32) as usize;
        self.data[idx] += value;
        if self.data[idx] > self.max_value {
            self.max_value = self.data[idx];
        }
    }

    /// Get the raw value at a tile position.
    pub fn get(&self, tx: u32, ty: u32) -> f32 {
        if tx >= self.width || ty >= self.height {
            return 0.0;
        }
        self.data[(ty * self.width + tx) as usize]
    }

    /// Get the normalized value (0.0 - 1.0) at a tile position.
    pub fn get_normalized(&self, tx: u32, ty: u32) -> f32 {
        if self.max_value <= 0.0 {
            return 0.0;
        }
        self.get(tx, ty) / self.max_value
    }

    /// Maximum value in the heatmap.
    pub fn max_value(&self) -> f32 {
        self.max_value
    }

    /// Total accumulated value across all tiles.
    pub fn total(&self) -> f32 {
        self.data.iter().sum()
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        self.data.fill(0.0);
        self.max_value = 0.0;
    }

    /// Get a color for the given normalized intensity (0.0 - 1.0).
    ///
    /// Uses a cool→hot gradient: blue → cyan → green → yellow → red.
    pub fn intensity_color(t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        if t < 0.25 {
            let s = t / 0.25;
            Color::new(0.0, s, 1.0, 0.6)
        } else if t < 0.5 {
            let s = (t - 0.25) / 0.25;
            Color::new(0.0, 1.0, 1.0 - s, 0.6)
        } else if t < 0.75 {
            let s = (t - 0.5) / 0.25;
            Color::new(s, 1.0, 0.0, 0.7)
        } else {
            let s = (t - 0.75) / 0.25;
            Color::new(1.0, 1.0 - s, 0.0, 0.8)
        }
    }

    /// Generate colored tile overlay data.
    /// Returns (tile_x, tile_y, color) for each non-zero tile.
    pub fn overlay_tiles(&self) -> Vec<(u32, u32, Color)> {
        let mut tiles = Vec::new();
        if self.max_value <= 0.0 {
            return tiles;
        }
        for y in 0..self.height {
            for x in 0..self.width {
                let val = self.get_normalized(x, y);
                if val > 0.001 {
                    tiles.push((x, y, Self::intensity_color(val)));
                }
            }
        }
        tiles
    }

    /// Get the "hottest" tile positions (sorted by value, descending).
    pub fn hotspots(&self, count: usize) -> Vec<(u32, u32, f32)> {
        let mut indexed: Vec<(u32, u32, f32)> = (0..self.height)
            .flat_map(|y| (0..self.width).map(move |x| (x, y, self.get(x, y))))
            .filter(|(_, _, v)| *v > 0.0)
            .collect();
        indexed.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        indexed.truncate(count);
        indexed
    }
}

// ---------------------------------------------------------------------------
// HeatmapCollection — manages multiple heatmaps
// ---------------------------------------------------------------------------

/// Manages multiple named heatmaps for a level.
pub struct HeatmapCollection {
    maps: Vec<(String, Heatmap)>,
}

impl HeatmapCollection {
    pub fn new() -> Self {
        Self { maps: Vec::new() }
    }

    /// Create a standard set of heatmaps for a level.
    pub fn for_level(width: u32, height: u32, tile_size: f32) -> Self {
        let mut c = Self::new();
        c.add(
            "enemy_deaths",
            Heatmap::new(HeatmapType::EnemyDeaths, width, height, tile_size),
        );
        c.add(
            "damage_taken",
            Heatmap::new(HeatmapType::DamageTaken, width, height, tile_size),
        );
        c.add(
            "tower_coverage",
            Heatmap::new(HeatmapType::TowerCoverage, width, height, tile_size),
        );
        c.add(
            "enemy_density",
            Heatmap::new(HeatmapType::EnemyDensity, width, height, tile_size),
        );
        c
    }

    /// Add a named heatmap.
    pub fn add(&mut self, name: &str, heatmap: Heatmap) {
        self.maps.push((name.to_string(), heatmap));
    }

    /// Get a heatmap by name.
    pub fn get(&self, name: &str) -> Option<&Heatmap> {
        self.maps.iter().find(|(n, _)| n == name).map(|(_, h)| h)
    }

    /// Get a mutable heatmap by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Heatmap> {
        self.maps
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, h)| h)
    }

    /// List all heatmap names.
    pub fn names(&self) -> Vec<&str> {
        self.maps.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// Clear all heatmaps.
    pub fn clear_all(&mut self) {
        for (_, h) in &mut self.maps {
            h.clear();
        }
    }
}

impl Default for HeatmapCollection {
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

    // ── Recording & querying ──────────────────────────────────────

    #[test]
    fn record_and_get() {
        let mut hm = Heatmap::new(HeatmapType::EnemyDeaths, 10, 10, 16.0);
        hm.record(24.0, 24.0, 1.0); // tile (1, 1)
        hm.record(24.0, 24.0, 2.0); // accumulates

        assert_eq!(hm.get(1, 1), 3.0);
        assert_eq!(hm.max_value(), 3.0);
    }

    #[test]
    fn normalized() {
        let mut hm = Heatmap::new(HeatmapType::DamageTaken, 4, 4, 16.0);
        hm.record_tile(0, 0, 10.0);
        hm.record_tile(1, 0, 5.0);

        assert_eq!(hm.get_normalized(0, 0), 1.0);
        assert_eq!(hm.get_normalized(1, 0), 0.5);
        assert_eq!(hm.get_normalized(2, 0), 0.0);
    }

    // ── Visualization ─────────────────────────────────────────────

    #[test]
    fn overlay_tiles() {
        let mut hm = Heatmap::new(HeatmapType::EnemyDeaths, 4, 4, 16.0);
        hm.record_tile(1, 1, 5.0);
        hm.record_tile(2, 2, 10.0);

        let tiles = hm.overlay_tiles();
        assert_eq!(tiles.len(), 2);
    }

    #[test]
    fn hotspots() {
        let mut hm = Heatmap::new(HeatmapType::EnemyDeaths, 4, 4, 16.0);
        hm.record_tile(0, 0, 1.0);
        hm.record_tile(1, 1, 5.0);
        hm.record_tile(2, 2, 3.0);

        let hot = hm.hotspots(2);
        assert_eq!(hot.len(), 2);
        assert_eq!(hot[0], (1, 1, 5.0)); // highest first
        assert_eq!(hot[1], (2, 2, 3.0));
    }

    #[test]
    fn intensity_colors() {
        let c0 = Heatmap::intensity_color(0.0);
        assert!(c0.b > 0.5); // blue at low

        let c1 = Heatmap::intensity_color(1.0);
        assert!(c1.r > 0.5); // red at high
        assert!(c1.g < 0.5);
    }

    #[test]
    fn clear_resets() {
        let mut hm = Heatmap::new(HeatmapType::Custom, 4, 4, 16.0);
        hm.record_tile(0, 0, 10.0);
        hm.clear();
        assert_eq!(hm.get(0, 0), 0.0);
        assert_eq!(hm.max_value(), 0.0);
    }

    // ── HeatmapCollection ─────────────────────────────────────────

    #[test]
    fn collection() {
        let mut c = HeatmapCollection::for_level(10, 10, 16.0);
        assert_eq!(c.names().len(), 4);

        c.get_mut("enemy_deaths").unwrap().record_tile(5, 5, 1.0);
        assert_eq!(c.get("enemy_deaths").unwrap().get(5, 5), 1.0);

        c.clear_all();
        assert_eq!(c.get("enemy_deaths").unwrap().get(5, 5), 0.0);
    }

    // ── Edge cases ────────────────────────────────────────────────

    #[test]
    fn out_of_bounds() {
        let mut hm = Heatmap::new(HeatmapType::Custom, 4, 4, 16.0);
        hm.record(-16.0, -16.0, 1.0); // negative coords - no crash
        hm.record(1000.0, 1000.0, 1.0); // way out of bounds - no crash
        assert_eq!(hm.total(), 0.0);
    }
}
