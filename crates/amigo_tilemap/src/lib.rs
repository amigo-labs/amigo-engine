pub mod auto_tile;
pub mod chunk;
pub mod dynamic;
pub mod lighting;
pub mod liquid;

pub use auto_tile::{AutoTileResolver, AutoTileRule, AutoTileSet};

use amigo_core::math::IVec2;
use serde::{Deserialize, Serialize};

/// Tile identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileId(pub u32);

impl TileId {
    pub const EMPTY: Self = Self(0);

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Collision type for a tile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollisionType {
    #[default]
    Empty,
    Solid,
    OneWay,
    Slope {
        left_height: u8,
        right_height: u8,
    },
    Trigger {
        id: u32,
    },
}

/// Grid mode for the tilemap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GridMode {
    Orthogonal { tile_width: u32, tile_height: u32 },
    Isometric { tile_width: u32, tile_height: u32 },
}

impl Default for GridMode {
    fn default() -> Self {
        Self::Orthogonal {
            tile_width: 16,
            tile_height: 16,
        }
    }
}

/// A single tilemap layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileLayer {
    pub name: String,
    pub tiles: Vec<TileId>,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub scroll_factor_x: f32,
    pub scroll_factor_y: f32,
}

impl TileLayer {
    pub fn new(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            name: name.into(),
            tiles: vec![TileId::EMPTY; (width * height) as usize],
            width,
            height,
            visible: true,
            scroll_factor_x: 1.0,
            scroll_factor_y: 1.0,
        }
    }

    pub fn get(&self, x: u32, y: u32) -> TileId {
        if x >= self.width || y >= self.height {
            return TileId::EMPTY;
        }
        self.tiles[(y * self.width + x) as usize]
    }

    pub fn set(&mut self, x: u32, y: u32, tile: TileId) {
        if x < self.width && y < self.height {
            self.tiles[(y * self.width + x) as usize] = tile;
        }
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, tile: TileId) {
        for ty in y..y + h {
            for tx in x..x + w {
                self.set(tx, ty, tile);
            }
        }
    }
}

/// Collision layer for the tilemap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollisionLayer {
    pub data: Vec<CollisionType>,
    pub width: u32,
    pub height: u32,
}

impl CollisionLayer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![CollisionType::Empty; (width * height) as usize],
            width,
            height,
        }
    }

    pub fn get(&self, x: i32, y: i32) -> CollisionType {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return CollisionType::Solid;
        }
        self.data[(y as u32 * self.width + x as u32) as usize]
    }

    pub fn set(&mut self, x: u32, y: u32, collision: CollisionType) {
        if x < self.width && y < self.height {
            self.data[(y * self.width + x) as usize] = collision;
        }
    }

    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        matches!(self.get(x, y), CollisionType::Solid)
    }
}

/// The complete tilemap with multiple layers and collision.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileMap {
    pub grid_mode: GridMode,
    pub layers: Vec<TileLayer>,
    pub collision: CollisionLayer,
}

impl TileMap {
    pub fn new(grid_mode: GridMode, width: u32, height: u32) -> Self {
        Self {
            grid_mode,
            layers: vec![TileLayer::new("terrain", width, height)],
            collision: CollisionLayer::new(width, height),
        }
    }

    pub fn tile_size(&self) -> (u32, u32) {
        match &self.grid_mode {
            GridMode::Orthogonal {
                tile_width,
                tile_height,
            } => (*tile_width, *tile_height),
            GridMode::Isometric {
                tile_width,
                tile_height,
            } => (*tile_width, *tile_height),
        }
    }

    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        self.collision.is_solid(x, y)
    }

    pub fn screen_to_tile(&self, screen_x: f32, screen_y: f32) -> IVec2 {
        let (tw, th) = self.tile_size();
        match &self.grid_mode {
            GridMode::Orthogonal { .. } => IVec2::new(
                (screen_x / tw as f32).floor() as i32,
                (screen_y / th as f32).floor() as i32,
            ),
            GridMode::Isometric {
                tile_width,
                tile_height,
            } => {
                // Inverse of tile_to_screen:
                //   sx = (tx - ty) * (tw / 2)
                //   sy = (tx + ty) * (th / 2)
                // =>
                //   sx / (tw/2) = tx - ty
                //   sy / (th/2) = tx + ty
                // =>
                //   tx = (sx/(tw/2) + sy/(th/2)) / 2
                //   ty = (sy/(th/2) - sx/(tw/2)) / 2
                let half_w = *tile_width as f32 / 2.0;
                let half_h = *tile_height as f32 / 2.0;
                let a = screen_x / half_w; // tx - ty
                let b = screen_y / half_h; // tx + ty
                let tx = (a + b) / 2.0;
                let ty = (b - a) / 2.0;
                IVec2::new(tx.floor() as i32, ty.floor() as i32)
            }
        }
    }

    /// Convert tile coordinates back to screen/world pixel position.
    pub fn tile_to_screen(&self, tile_x: i32, tile_y: i32) -> (f32, f32) {
        let (tw, th) = self.tile_size();
        match &self.grid_mode {
            GridMode::Orthogonal { .. } => (tile_x as f32 * tw as f32, tile_y as f32 * th as f32),
            GridMode::Isometric {
                tile_width,
                tile_height,
            } => {
                let tw = *tile_width as f32;
                let th = *tile_height as f32;
                let sx = (tile_x - tile_y) as f32 * (tw / 2.0);
                let sy = (tile_x + tile_y) as f32 * (th / 2.0);
                (sx, sy)
            }
        }
    }

    /// Check if a tile coordinate is within bounds.
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && x < self.collision.width as i32
            && y < self.collision.height as i32
    }

    pub fn add_layer(&mut self, name: impl Into<String>) {
        let (w, h) = (self.collision.width, self.collision.height);
        self.layers.push(TileLayer::new(name, w, h));
    }

    pub fn layer(&self, name: &str) -> Option<&TileLayer> {
        self.layers.iter().find(|l| l.name == name)
    }

    pub fn layer_mut(&mut self, name: &str) -> Option<&mut TileLayer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Orthogonal Tests ─────────────────────────────────────────

    #[test]
    fn ortho_screen_to_tile() {
        let map = TileMap::new(GridMode::Orthogonal { tile_width: 16, tile_height: 16 }, 10, 10);
        assert_eq!(map.screen_to_tile(0.0, 0.0), IVec2::new(0, 0));
        assert_eq!(map.screen_to_tile(17.0, 33.0), IVec2::new(1, 2));
        assert_eq!(map.screen_to_tile(-1.0, -1.0), IVec2::new(-1, -1));
    }

    #[test]
    fn ortho_tile_to_screen() {
        let map = TileMap::new(GridMode::Orthogonal { tile_width: 16, tile_height: 16 }, 10, 10);
        assert_eq!(map.tile_to_screen(0, 0), (0.0, 0.0));
        assert_eq!(map.tile_to_screen(3, 5), (48.0, 80.0));
    }

    #[test]
    fn ortho_roundtrip() {
        let map = TileMap::new(GridMode::Orthogonal { tile_width: 16, tile_height: 16 }, 10, 10);
        for tx in 0..5 {
            for ty in 0..5 {
                let (sx, sy) = map.tile_to_screen(tx, ty);
                let result = map.screen_to_tile(sx + 1.0, sy + 1.0); // +1 to be inside tile
                assert_eq!(result, IVec2::new(tx, ty), "roundtrip failed for ({tx}, {ty})");
            }
        }
    }

    // ── Isometric Tests ──────────────────────────────────────────

    #[test]
    fn iso_screen_to_tile_origin() {
        let map = TileMap::new(GridMode::Isometric { tile_width: 64, tile_height: 32 }, 10, 10);
        let tile = map.screen_to_tile(0.0, 0.0);
        assert_eq!(tile, IVec2::new(0, 0));
    }

    #[test]
    fn iso_tile_to_screen() {
        let map = TileMap::new(GridMode::Isometric { tile_width: 64, tile_height: 32 }, 10, 10);
        // tile (0,0) should be at screen (0, 0)
        assert_eq!(map.tile_to_screen(0, 0), (0.0, 0.0));
        // tile (1,0) should be at screen (32, 16) — right-down
        assert_eq!(map.tile_to_screen(1, 0), (32.0, 16.0));
        // tile (0,1) should be at screen (-32, 16) — left-down
        assert_eq!(map.tile_to_screen(0, 1), (-32.0, 16.0));
    }

    #[test]
    fn iso_roundtrip() {
        let map = TileMap::new(GridMode::Isometric { tile_width: 64, tile_height: 32 }, 10, 10);
        for tx in 0..5i32 {
            for ty in 0..5i32 {
                let (sx, sy) = map.tile_to_screen(tx, ty);
                // Sample center of tile for accurate roundtrip
                let result = map.screen_to_tile(sx + 0.1, sy + 0.1);
                assert_eq!(result, IVec2::new(tx, ty), "iso roundtrip failed for ({tx}, {ty})");
            }
        }
    }

    // ── Layer Operations ─────────────────────────────────────────

    #[test]
    fn tile_layer_set_get() {
        let mut layer = TileLayer::new("test", 4, 4);
        layer.set(2, 3, TileId(42));
        assert_eq!(layer.get(2, 3), TileId(42));
        assert_eq!(layer.get(0, 0), TileId::EMPTY);
    }

    #[test]
    fn tile_layer_fill_rect() {
        let mut layer = TileLayer::new("test", 8, 8);
        layer.fill_rect(1, 1, 3, 2, TileId(5));
        assert_eq!(layer.get(1, 1), TileId(5));
        assert_eq!(layer.get(3, 2), TileId(5));
        assert_eq!(layer.get(0, 0), TileId::EMPTY);
        assert_eq!(layer.get(4, 1), TileId::EMPTY);
    }

    #[test]
    fn collision_layer_solid() {
        let mut collision = CollisionLayer::new(4, 4);
        collision.set(1, 1, CollisionType::Solid);
        assert!(collision.is_solid(1, 1));
        assert!(!collision.is_solid(0, 0));
        // Out of bounds is solid
        assert!(collision.is_solid(-1, 0));
    }

    #[test]
    fn tilemap_multiple_layers() {
        let mut map = TileMap::new(GridMode::default(), 8, 8);
        map.add_layer("foreground");
        assert_eq!(map.layers.len(), 2); // terrain + foreground
        map.layer_mut("foreground").unwrap().set(0, 0, TileId(99));
        assert_eq!(map.layer("foreground").unwrap().get(0, 0), TileId(99));
    }

    // ── Bounds Checking ──────────────────────────────────────────

    #[test]
    fn in_bounds_check() {
        let map = TileMap::new(GridMode::default(), 10, 10);
        assert!(map.in_bounds(0, 0));
        assert!(map.in_bounds(9, 9));
        assert!(!map.in_bounds(10, 0));
        assert!(!map.in_bounds(-1, 0));
    }
}
