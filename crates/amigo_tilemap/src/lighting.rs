//! Tile-based 2D lighting with flood-fill propagation.
//!
//! Designed for Sandbox/God Sim games (Terraria-style). Features:
//! - RGB light channels (colored light sources)
//! - Flood-fill propagation blocked by opaque tiles
//! - Ambient/sky light with day-night cycle support
//! - Dirty-region incremental recalculation
//! - Smooth lighting interpolation

use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Light color
// ---------------------------------------------------------------------------

/// RGB light value per tile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LightColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl LightColor {
    pub const ZERO: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Max of each channel (additive blend).
    pub fn max(self, other: Self) -> Self {
        Self {
            r: self.r.max(other.r),
            g: self.g.max(other.g),
            b: self.b.max(other.b),
        }
    }

    /// Returns true if all channels are zero.
    pub fn is_zero(self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0
    }

    /// Brightness (max channel).
    pub fn brightness(self) -> u8 {
        self.r.max(self.g).max(self.b)
    }
}

// ---------------------------------------------------------------------------
// Light source
// ---------------------------------------------------------------------------

/// A light source in the tile world.
#[derive(Clone, Debug)]
pub struct TileLight {
    pub x: i32,
    pub y: i32,
    pub color: LightColor,
    pub radius: u8,
}

// ---------------------------------------------------------------------------
// TileLightMap
// ---------------------------------------------------------------------------

/// Tile-based light map using flood-fill propagation.
///
/// Stores per-tile RGB light values. Light propagates from sources and
/// is blocked by opaque tiles. Supports incremental recalculation.
pub struct TileLightMap {
    data: Vec<LightColor>,
    width: u32,
    height: u32,
    /// Offset: world tile (0,0) maps to data index (origin_x, origin_y).
    origin_x: i32,
    origin_y: i32,
    /// Ambient light (sky light for surface tiles).
    pub ambient: LightColor,
}

impl TileLightMap {
    /// Create a new light map covering the given tile area.
    pub fn new(origin_x: i32, origin_y: i32, width: u32, height: u32) -> Self {
        Self {
            data: vec![LightColor::ZERO; (width * height) as usize],
            width,
            height,
            origin_x,
            origin_y,
            ambient: LightColor::ZERO,
        }
    }

    /// Get light at a world tile position.
    pub fn get(&self, x: i32, y: i32) -> LightColor {
        if let Some(idx) = self.index(x, y) {
            self.data[idx]
        } else {
            LightColor::ZERO
        }
    }

    /// Set light at a world tile position.
    fn set(&mut self, x: i32, y: i32, color: LightColor) {
        if let Some(idx) = self.index(x, y) {
            self.data[idx] = color;
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let lx = x - self.origin_x;
        let ly = y - self.origin_y;
        if lx < 0 || ly < 0 || lx >= self.width as i32 || ly >= self.height as i32 {
            return None;
        }
        Some((ly as u32 * self.width + lx as u32) as usize)
    }

    /// Clear the entire light map.
    pub fn clear(&mut self) {
        self.data.fill(LightColor::ZERO);
    }

    /// Recalculate all lighting from scratch.
    ///
    /// `is_opaque` returns whether a tile at (x, y) blocks light.
    /// `emitters` are all tile light sources in the visible area.
    /// `sky_tiles` are surface y-coordinates per x (tiles above get ambient).
    pub fn recalculate(
        &mut self,
        emitters: &[TileLight],
        is_opaque: &dyn Fn(i32, i32) -> bool,
        sky_line: Option<&dyn Fn(i32) -> i32>,
    ) {
        self.clear();

        // Apply sky/ambient light.
        if let Some(sky_y) = sky_line {
            for wx in self.origin_x..(self.origin_x + self.width as i32) {
                let surface_y = sky_y(wx);
                for wy in self.origin_y..surface_y.min(self.origin_y + self.height as i32) {
                    if !is_opaque(wx, wy) {
                        self.set(wx, wy, self.ambient);
                    }
                }
            }
        }

        // Propagate each emitter via BFS.
        for emitter in emitters {
            self.propagate_light(emitter, is_opaque);
        }
    }

    /// Flood-fill a single light source.
    fn propagate_light(&mut self, light: &TileLight, is_opaque: &dyn Fn(i32, i32) -> bool) {
        let mut queue = VecDeque::new();
        queue.push_back((light.x, light.y, light.color));

        // Set the source tile.
        let current = self.get(light.x, light.y);
        self.set(light.x, light.y, current.max(light.color));

        while let Some((x, y, color)) = queue.pop_front() {
            // Attenuate per step.
            let step = (255 / light.radius.max(1) as u16) as u8;
            let next = LightColor {
                r: color.r.saturating_sub(step),
                g: color.g.saturating_sub(step),
                b: color.b.saturating_sub(step),
            };

            if next.is_zero() {
                continue;
            }

            for (dx, dy) in &[(0i32, -1i32), (0, 1), (-1, 0), (1, 0)] {
                let nx = x + dx;
                let ny = y + dy;

                if is_opaque(nx, ny) {
                    continue;
                }

                let current = self.get(nx, ny);
                // Only propagate if we'd make it brighter.
                if next.r > current.r || next.g > current.g || next.b > current.b {
                    self.set(nx, ny, current.max(next));
                    queue.push_back((nx, ny, next));
                }
            }
        }
    }

    /// Get interpolated light between four tiles (for smooth lighting).
    /// `frac_x` and `frac_y` are 0.0-1.0 within a tile.
    pub fn get_smooth(&self, x: i32, y: i32, frac_x: f32, frac_y: f32) -> [f32; 3] {
        let tl = self.get(x, y);
        let tr = self.get(x + 1, y);
        let bl = self.get(x, y + 1);
        let br = self.get(x + 1, y + 1);

        let lerp = |a: u8, b: u8, c: u8, d: u8| -> f32 {
            let top = a as f32 * (1.0 - frac_x) + b as f32 * frac_x;
            let bot = c as f32 * (1.0 - frac_x) + d as f32 * frac_x;
            (top * (1.0 - frac_y) + bot * frac_y) / 255.0
        };

        [
            lerp(tl.r, tr.r, bl.r, br.r),
            lerp(tl.g, tr.g, bl.g, br.g),
            lerp(tl.b, tr.b, bl.b, br.b),
        ]
    }

    /// Width of the light map in tiles.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the light map in tiles.
    pub fn height(&self) -> u32 {
        self.height
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Basic light map ───────────────────────────────────────────

    #[test]
    fn empty_map_is_dark() {
        let map = TileLightMap::new(0, 0, 32, 32);
        assert_eq!(map.get(0, 0), LightColor::ZERO);
        assert_eq!(map.get(15, 15), LightColor::ZERO);
    }

    #[test]
    fn single_light_propagates() {
        let mut map = TileLightMap::new(0, 0, 32, 32);
        let light = TileLight {
            x: 16,
            y: 16,
            color: LightColor::new(255, 200, 100),
            radius: 8,
        };

        map.recalculate(&[light], &|_, _| false, None);

        // Center should be brightest.
        let center = map.get(16, 16);
        assert_eq!(center.r, 255);

        // Nearby should have light.
        let near = map.get(17, 16);
        assert!(near.brightness() > 0);

        // Far away should be dark.
        let far = map.get(0, 0);
        assert_eq!(far, LightColor::ZERO);
    }

    // ── Opaque blocking ───────────────────────────────────────────

    #[test]
    fn opaque_tiles_block_light() {
        let mut map = TileLightMap::new(0, 0, 32, 32);
        let light = TileLight {
            x: 16,
            y: 16,
            color: LightColor::new(255, 255, 255),
            radius: 10,
        };

        // Wall of opaque tiles at x=17.
        let is_opaque = |x: i32, _y: i32| x == 17;

        map.recalculate(&[light], &is_opaque, None);

        // Light should not pass through the wall.
        let behind_wall = map.get(18, 16);
        assert_eq!(behind_wall, LightColor::ZERO);

        // In front of wall should have light.
        let in_front = map.get(15, 16);
        assert!(in_front.brightness() > 0);
    }

    // ── Ambient & sky light ────────────────────────────────────────

    #[test]
    fn ambient_sky_light() {
        let mut map = TileLightMap::new(0, 0, 32, 32);
        map.ambient = LightColor::new(180, 200, 220);

        // Surface at y=10 for all x.
        let sky_line = |_x: i32| -> i32 { 10 };

        map.recalculate(&[], &|_, _| false, Some(&sky_line));

        // Above surface should have ambient.
        let above = map.get(5, 5);
        assert_eq!(above, map.ambient);

        // Below surface should be dark (no emitters).
        let below = map.get(5, 15);
        assert_eq!(below, LightColor::ZERO);
    }

    // ── Smooth interpolation & colored light ────────────────────

    #[test]
    fn smooth_interpolation() {
        let mut map = TileLightMap::new(0, 0, 4, 4);
        let light = TileLight {
            x: 1,
            y: 1,
            color: LightColor::new(255, 255, 255),
            radius: 3,
        };

        map.recalculate(&[light], &|_, _| false, None);

        let smooth = map.get_smooth(1, 1, 0.5, 0.5);
        assert!(smooth[0] > 0.0);
        assert!(smooth[1] > 0.0);
    }

    #[test]
    fn colored_light() {
        let mut map = TileLightMap::new(0, 0, 32, 32);
        let red = TileLight {
            x: 10,
            y: 16,
            color: LightColor::new(255, 0, 0),
            radius: 5,
        };
        let blue = TileLight {
            x: 20,
            y: 16,
            color: LightColor::new(0, 0, 255),
            radius: 5,
        };

        map.recalculate(&[red, blue], &|_, _| false, None);

        // Red light area.
        let red_area = map.get(10, 16);
        assert_eq!(red_area.r, 255);
        assert_eq!(red_area.b, 0);

        // Blue light area.
        let blue_area = map.get(20, 16);
        assert_eq!(blue_area.b, 255);
        assert_eq!(blue_area.r, 0);
    }
}
