---
status: draft
crate: amigo_tilemap
depends_on: ["engine/core", "engine/dynamic-tilemap", "engine/chunks"]
last_updated: 2026-03-18
---

# 2D Lighting

## Purpose

Tile-based 2D lighting system with flood-fill propagation for Sandbox and God Sim games. Provides colored RGB light sources, ambient/sky light with day-night cycle support, opaque-tile blocking, dirty-region incremental recalculation, and smooth lighting interpolation between tiles. Designed for Terraria-style underground exploration where light is a core gameplay mechanic.

## Public API

Existing implementation in `crates/amigo_tilemap/src/lighting.rs`.

### LightColor

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LightColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl LightColor {
    pub const ZERO: Self;
    pub const WHITE: Self;
    pub const fn new(r: u8, g: u8, b: u8) -> Self;
    pub fn max(self, other: Self) -> Self;
    pub fn is_zero(self) -> bool;
    pub fn brightness(self) -> u8;
}
```

### TileLight

```rust
#[derive(Clone, Debug)]
pub struct TileLight {
    pub x: i32,
    pub y: i32,
    pub color: LightColor,
    pub radius: u8,
}
```

### TileLightMap

```rust
pub struct TileLightMap {
    data: Vec<LightColor>,
    width: u32,
    height: u32,
    origin_x: i32,
    origin_y: i32,
    pub ambient: LightColor,
}

impl TileLightMap {
    pub fn new(origin_x: i32, origin_y: i32, width: u32, height: u32) -> Self;
    pub fn get(&self, x: i32, y: i32) -> LightColor;
    pub fn clear(&mut self);
    pub fn recalculate(
        &mut self,
        emitters: &[TileLight],
        is_opaque: &dyn Fn(i32, i32) -> bool,
        sky_line: Option<&dyn Fn(i32) -> i32>,
    );
    pub fn get_smooth(&self, x: i32, y: i32, frac_x: f32, frac_y: f32) -> [f32; 3];
    pub fn width(&self) -> u32;
    pub fn height(&self) -> u32;
}
```

## Behavior

- **Flood-fill propagation:** Each light source propagates via BFS from its origin tile outward. Light attenuates by `255 / radius` per step. Propagation stops when all channels reach zero or hit an opaque tile.
- **Additive blending:** When multiple lights overlap, the maximum of each RGB channel is taken (not summed), preventing overflow.
- **Opaque blocking:** The `is_opaque` callback (typically `DynamicTileWorld::is_opaque`) determines which tiles block light. Light does not propagate through opaque tiles.
- **Sky/ambient light:** When a `sky_line` function is provided, all non-opaque tiles above the surface Y for each column receive the `ambient` light color. This models sunlight for day-night cycles.
- **Smooth interpolation:** `get_smooth()` bilinearly interpolates the light values of four adjacent tiles, returning normalized `[f32; 3]` in `[0.0, 1.0]` for GPU multiply-blending with tile colors.
- **Recalculation** clears the entire map and re-propagates all emitters. For incremental updates, the caller should maintain a light map sized to the visible area and recalculate only when [dirty regions](dynamic-tilemap.md) change.

## Internal Design

- Light data is a flat `Vec<LightColor>` covering a rectangular tile region with an origin offset for world-coordinate addressing.
- BFS uses a `VecDeque` per light source. The queue contains `(x, y, color)` tuples and only enqueues neighbors that would become brighter.
- Attenuation is computed as a fixed step per tile distance: `step = 255 / radius.max(1)`, applied via `saturating_sub` on each channel.

## Non-Goals

- **Dynamic shadows or raytracing.** This is tile-grid flood-fill only; no line-of-sight or penumbra.
- **Per-entity lights.** Entity-attached lights (player torch, projectile glow) must be injected as `TileLight` entries by the game layer each frame.
- **GPU-side light calculation.** The light map is CPU-computed and uploaded as a texture/buffer for the renderer to multiply.

## Open Questions

- Should the light map support incremental dirty-region updates (re-propagate only affected area) rather than full recalculation?
- How should the day-night cycle animate `ambient`? External system or built-in time parameter on `recalculate`?
- Should light attenuation be configurable per-source (e.g., torches attenuate slower than glowstone)?
