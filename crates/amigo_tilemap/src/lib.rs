pub mod auto_tile;
pub mod chunk;

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
                let tw = *tile_width as f32;
                let th = *tile_height as f32;
                let x = (screen_x / tw + screen_y / th) / 2.0;
                let y = (screen_y / th - screen_x / tw) / 2.0;
                IVec2::new(x.floor() as i32, y.floor() as i32)
            }
        }
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
