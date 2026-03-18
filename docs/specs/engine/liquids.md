---
status: draft
crate: amigo_tilemap
depends_on: ["engine/core", "engine/dynamic-tilemap", "engine/chunks"]
last_updated: 2026-03-18
---

# Liquid Simulation

## Purpose

Cellular-automata-based liquid simulation for Sandbox and God Sim games. Supports multiple liquid types (water, lava, custom), an 8-level fill system per tile, gravity-first flow rules, liquid-liquid interactions (e.g., water + lava = obsidian), and a settled-cell optimization to skip unchanged regions. Designed to run as a periodic [simulation system](simulation.md) rather than every render frame.

## Public API

Existing implementation in `crates/amigo_tilemap/src/liquid.rs`.

### LiquidType

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LiquidType(pub u8);

impl LiquidType {
    pub const NONE: Self;
    pub const WATER: Self;
    pub const LAVA: Self;
}
```

### LiquidDef

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiquidDef {
    pub name: String,
    pub flow_rate: u8,
    pub spreads: bool,
    pub light_emission: u8,
    pub light_color: [u8; 3],
}
```

### LiquidInteraction

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiquidInteraction {
    pub liquid_a: LiquidType,
    pub liquid_b: LiquidType,
    pub result_tile: u32,
    pub spawn_particles: bool,
}
```

### LiquidCell

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidCell {
    pub liquid_type: LiquidType,
    pub level: u8,
    pub settled: bool,
}

impl LiquidCell {
    pub const EMPTY: Self;
    pub const MAX_LEVEL: u8 = 7;
    pub fn is_empty(self) -> bool;
    pub fn is_full(self) -> bool;
}
```

### LiquidMap

```rust
pub struct LiquidMap {
    cells: Vec<LiquidCell>,
    width: u32,
    height: u32,
    origin_x: i32,
    origin_y: i32,
    interactions: Vec<LiquidInteraction>,
    pub solidified: Vec<(i32, i32, u32)>,
}

impl LiquidMap {
    pub fn new(origin_x: i32, origin_y: i32, width: u32, height: u32) -> Self;
    pub fn add_interaction(&mut self, interaction: LiquidInteraction);
    pub fn get(&self, x: i32, y: i32) -> LiquidCell;
    pub fn set(&mut self, x: i32, y: i32, cell: LiquidCell);
    pub fn set_liquid(&mut self, x: i32, y: i32, liquid_type: LiquidType, level: u8);
    pub fn clear(&mut self, x: i32, y: i32);
    pub fn step(&mut self, is_solid: &dyn Fn(i32, i32) -> bool) -> u32;
}
```

## Behavior

- **Flow priority:** Down first, then sideways. Liquid transfers as much volume as possible downward before attempting horizontal spread.
- **Level system:** Each cell holds 0-7 units. Level 0 = empty, level 7 = full. Sideways flow only occurs when the source level exceeds the neighbor's level by more than 1 (equalization).
- **Settling:** A cell that cannot flow in any direction is marked `settled = true` and skipped in subsequent steps. Neighboring cells wake settled cells when they change.
- **Interactions:** After flow, all cells are checked for adjacent different-liquid-type neighbors. Matching `LiquidInteraction` rules consume both cells and record the result tile in `solidified` for the caller to place via [DynamicTileWorld](dynamic-tilemap.md).
- **Solid blocking:** The `is_solid` callback (typically `DynamicTileWorld::is_solid`) determines which tiles block liquid flow.
- **Processing order:** Bottom-to-top within the map so gravity works correctly in a single pass.
- **Empty maps** produce zero changes when stepped.

## Internal Design

- Flat `Vec<LiquidCell>` with origin offset, same pattern as `TileLightMap`.
- Flow helpers (`try_flow_down`, `try_flow_sideways`) modify cells in-place and call `wake_neighbors` on affected positions.
- Interaction checking is O(cells * interactions) but interactions lists are typically tiny (2-5 rules).

## Non-Goals

- **Pressure simulation.** Deep-water pressure pushing liquid upward is not implemented; only gravity and horizontal equalization.
- **Rendering.** Visual representation (animated surfaces, transparency) is handled by [engine/rendering](rendering.md).
- **Per-liquid-type flow rates.** `LiquidDef.flow_rate` is defined but not yet used in the step logic; all liquids currently flow at the same rate.

## Open Questions

- Should `LiquidDef` properties (flow_rate, spreads) be integrated into the step logic for per-type viscosity?
- How should the liquid map coordinate with chunk loading/unloading? One `LiquidMap` per chunk, or a single map resized to the active area?
- Should liquid light emission (lava glow) automatically feed into the [lighting system](lighting.md)?
