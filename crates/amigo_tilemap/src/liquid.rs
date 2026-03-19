//! Cellular-automata liquid simulation for Sandbox / God Sim.
//!
//! Features:
//! - Multiple liquid types (water, lava, custom)
//! - 8-level fill system per tile (0 = empty, 7 = full)
//! - Flow rules: down > sideways > spread
//! - Liquid-liquid interactions (water + lava = obsidian)
//! - Settled optimization (skip unchanged cells)
//! - Chunk-based processing

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Liquid types
// ---------------------------------------------------------------------------

/// Identifies a liquid type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LiquidType(pub u8);

impl LiquidType {
    pub const NONE: Self = Self(0);
    pub const WATER: Self = Self(1);
    pub const LAVA: Self = Self(2);
}

/// Properties for a liquid type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiquidDef {
    pub name: String,
    /// Flow speed (how many levels transfer per step). Higher = faster.
    pub flow_rate: u8,
    /// Whether it spreads sideways (water=true, honey=slower).
    pub spreads: bool,
    /// Light emission when present (lava glows).
    pub light_emission: u8,
    pub light_color: [u8; 3],
}

impl Default for LiquidDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            flow_rate: 1,
            spreads: true,
            light_emission: 0,
            light_color: [0, 0, 0],
        }
    }
}

/// Interaction rule: when two liquids meet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiquidInteraction {
    pub liquid_a: LiquidType,
    pub liquid_b: LiquidType,
    /// Resulting tile ID (e.g., obsidian). 0 = both liquids consumed.
    pub result_tile: u32,
    /// Whether to spawn particles at the interaction point.
    pub spawn_particles: bool,
}

// ---------------------------------------------------------------------------
// Liquid cell
// ---------------------------------------------------------------------------

/// State of liquid in a single tile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidCell {
    pub liquid_type: LiquidType,
    /// Fill level 0-7 (0 = empty).
    pub level: u8,
    /// If true, this cell hasn't changed recently and can be skipped.
    pub settled: bool,
}

impl LiquidCell {
    pub const EMPTY: Self = Self {
        liquid_type: LiquidType::NONE,
        level: 0,
        settled: true,
    };

    pub const MAX_LEVEL: u8 = 7;

    pub fn is_empty(self) -> bool {
        self.level == 0
    }

    pub fn is_full(self) -> bool {
        self.level == Self::MAX_LEVEL
    }
}

// ---------------------------------------------------------------------------
// LiquidMap
// ---------------------------------------------------------------------------

/// Stores liquid state for a rectangular tile region.
pub struct LiquidMap {
    cells: Vec<LiquidCell>,
    width: u32,
    height: u32,
    origin_x: i32,
    origin_y: i32,
    interactions: Vec<LiquidInteraction>,
    /// Tiles that became solid as a result of interactions this step.
    pub solidified: Vec<(i32, i32, u32)>,
}

impl LiquidMap {
    pub fn new(origin_x: i32, origin_y: i32, width: u32, height: u32) -> Self {
        Self {
            cells: vec![LiquidCell::EMPTY; (width * height) as usize],
            width,
            height,
            origin_x,
            origin_y,
            interactions: Vec::new(),
            solidified: Vec::new(),
        }
    }

    /// Register a liquid interaction rule.
    pub fn add_interaction(&mut self, interaction: LiquidInteraction) {
        self.interactions.push(interaction);
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let lx = x - self.origin_x;
        let ly = y - self.origin_y;
        if lx < 0 || ly < 0 || lx >= self.width as i32 || ly >= self.height as i32 {
            return None;
        }
        Some((ly as u32 * self.width + lx as u32) as usize)
    }

    pub fn get(&self, x: i32, y: i32) -> LiquidCell {
        self.index(x, y)
            .map(|i| self.cells[i])
            .unwrap_or(LiquidCell::EMPTY)
    }

    pub fn set(&mut self, x: i32, y: i32, cell: LiquidCell) {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = cell;
        }
    }

    /// Set liquid at a position. Convenience over raw set.
    pub fn set_liquid(&mut self, x: i32, y: i32, liquid_type: LiquidType, level: u8) {
        self.set(
            x,
            y,
            LiquidCell {
                liquid_type,
                level: level.min(LiquidCell::MAX_LEVEL),
                settled: false,
            },
        );
    }

    /// Remove liquid at a position.
    pub fn clear(&mut self, x: i32, y: i32) {
        self.set(x, y, LiquidCell::EMPTY);
    }

    /// Run one simulation step.
    ///
    /// `is_solid` checks whether a tile blocks liquid flow.
    /// Returns the number of cells that changed.
    pub fn step(&mut self, is_solid: &dyn Fn(i32, i32) -> bool) -> u32 {
        self.solidified.clear();
        let mut changes = 0u32;

        // Process bottom-to-top so gravity works in one pass.
        for ly in (0..self.height).rev() {
            for lx in 0..self.width {
                let x = self.origin_x + lx as i32;
                let y = self.origin_y + ly as i32;

                let cell = self.get(x, y);
                if cell.is_empty() || cell.settled {
                    continue;
                }

                if is_solid(x, y) {
                    continue;
                }

                // Try flow down first.
                if self.try_flow_down(x, y, is_solid) {
                    changes += 1;
                    continue;
                }

                // Then flow sideways.
                if self.try_flow_sideways(x, y, is_solid) {
                    changes += 1;
                    continue;
                }

                // No movement possible — settle.
                if let Some(i) = self.index(x, y) {
                    self.cells[i].settled = true;
                }
            }
        }

        // Check interactions.
        changes += self.check_interactions(is_solid);

        changes
    }

    fn try_flow_down(&mut self, x: i32, y: i32, is_solid: &dyn Fn(i32, i32) -> bool) -> bool {
        let below_y = y + 1;
        if is_solid(x, below_y) {
            return false;
        }

        let cell = self.get(x, y);
        let below = self.get(x, below_y);

        if below.is_full() {
            return false;
        }

        // Different liquid types don't merge — interaction handles that.
        if !below.is_empty() && below.liquid_type != cell.liquid_type {
            return false;
        }

        let space = LiquidCell::MAX_LEVEL - below.level;
        let transfer = cell.level.min(space);

        if transfer == 0 {
            return false;
        }

        // Move liquid down.
        let new_level = cell.level - transfer;
        if new_level == 0 {
            self.set(x, y, LiquidCell::EMPTY);
        } else {
            self.set(
                x,
                y,
                LiquidCell {
                    liquid_type: cell.liquid_type,
                    level: new_level,
                    settled: false,
                },
            );
        }

        self.set(
            x,
            below_y,
            LiquidCell {
                liquid_type: cell.liquid_type,
                level: below.level + transfer,
                settled: false,
            },
        );

        // Wake up neighbors.
        self.wake_neighbors(x, y);
        self.wake_neighbors(x, below_y);

        true
    }

    fn try_flow_sideways(&mut self, x: i32, y: i32, is_solid: &dyn Fn(i32, i32) -> bool) -> bool {
        let cell = self.get(x, y);
        if cell.level <= 1 {
            return false; // Need at least 2 to spread.
        }

        let left = self.get(x - 1, y);
        let right = self.get(x + 1, y);
        let left_ok = !is_solid(x - 1, y)
            && (left.is_empty()
                || (left.liquid_type == cell.liquid_type && left.level < cell.level));
        let right_ok = !is_solid(x + 1, y)
            && (right.is_empty()
                || (right.liquid_type == cell.liquid_type && right.level < cell.level));

        if !left_ok && !right_ok {
            return false;
        }

        let mut moved = false;

        // Equalize: transfer 1 level to each valid side.
        if left_ok && cell.level > left.level + 1 {
            let current = self.get(x, y);
            self.set(
                x,
                y,
                LiquidCell {
                    level: current.level - 1,
                    settled: false,
                    ..current
                },
            );
            self.set(
                x - 1,
                y,
                LiquidCell {
                    liquid_type: cell.liquid_type,
                    level: left.level + 1,
                    settled: false,
                },
            );
            self.wake_neighbors(x - 1, y);
            moved = true;
        }

        if right_ok && self.get(x, y).level > right.level + 1 {
            let current = self.get(x, y);
            self.set(
                x,
                y,
                LiquidCell {
                    level: current.level - 1,
                    settled: false,
                    ..current
                },
            );
            self.set(
                x + 1,
                y,
                LiquidCell {
                    liquid_type: cell.liquid_type,
                    level: right.level + 1,
                    settled: false,
                },
            );
            self.wake_neighbors(x + 1, y);
            moved = true;
        }

        if moved {
            self.wake_neighbors(x, y);
        }

        moved
    }

    fn wake_neighbors(&mut self, x: i32, y: i32) {
        for (dx, dy) in &[(0i32, -1i32), (0, 1), (-1, 0), (1, 0)] {
            if let Some(i) = self.index(x + dx, y + dy) {
                if !self.cells[i].is_empty() {
                    self.cells[i].settled = false;
                }
            }
        }
    }

    fn check_interactions(&mut self, _is_solid: &dyn Fn(i32, i32) -> bool) -> u32 {
        let mut changes = 0;

        for ly in 0..self.height {
            for lx in 0..self.width {
                let x = self.origin_x + lx as i32;
                let y = self.origin_y + ly as i32;
                let cell = self.get(x, y);
                if cell.is_empty() {
                    continue;
                }

                // Check all 4 neighbors.
                for (dx, dy) in &[(0i32, -1i32), (0, 1), (-1, 0), (1, 0)] {
                    let nx = x + dx;
                    let ny = y + dy;
                    let neighbor = self.get(nx, ny);
                    if neighbor.is_empty() || neighbor.liquid_type == cell.liquid_type {
                        continue;
                    }

                    // Check for matching interaction.
                    if let Some(result_tile) =
                        self.find_interaction(cell.liquid_type, neighbor.liquid_type)
                    {
                        self.set(x, y, LiquidCell::EMPTY);
                        self.set(nx, ny, LiquidCell::EMPTY);
                        self.solidified.push((x, y, result_tile));
                        changes += 1;
                        break;
                    }
                }
            }
        }

        changes
    }

    fn find_interaction(&self, a: LiquidType, b: LiquidType) -> Option<u32> {
        self.interactions.iter().find_map(|i| {
            if (i.liquid_a == a && i.liquid_b == b) || (i.liquid_a == b && i.liquid_b == a) {
                Some(i.result_tile)
            } else {
                None
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Gravity flow ──────────────────────────────────────────────

    #[test]
    fn water_falls_down() {
        let mut map = LiquidMap::new(0, 0, 8, 8);
        map.set_liquid(4, 2, LiquidType::WATER, 7);

        let changes = map.step(&|_, _| false);
        assert!(changes > 0);

        // Water should have moved down.
        assert!(map.get(4, 3).level > 0);
    }

    #[test]
    fn solid_blocks_flow() {
        let mut map = LiquidMap::new(0, 0, 8, 8);
        map.set_liquid(4, 2, LiquidType::WATER, 7);

        // Solid floor at y=3.
        let is_solid = |_x: i32, y: i32| y == 3;
        map.step(&is_solid);

        // Water should stay at y=2 and not go to y=3.
        assert_eq!(map.get(4, 3).level, 0);
        assert!(map.get(4, 2).level > 0);
    }

    // ── Sideways spread ───────────────────────────────────────────

    #[test]
    fn sideways_spread() {
        let mut map = LiquidMap::new(0, 0, 16, 8);
        map.set_liquid(8, 5, LiquidType::WATER, 7);

        // Solid floor at y=6.
        let is_solid = |_x: i32, y: i32| y == 6;

        // Run several steps.
        for _ in 0..10 {
            map.step(&is_solid);
        }

        // Should have spread sideways.
        let left = map.get(7, 5);
        let right = map.get(9, 5);
        assert!(
            left.level > 0 || right.level > 0,
            "water should spread sideways"
        );
    }

    // ── Liquid interactions ────────────────────────────────────────

    #[test]
    fn water_lava_interaction() {
        let mut map = LiquidMap::new(0, 0, 8, 8);
        map.add_interaction(LiquidInteraction {
            liquid_a: LiquidType::WATER,
            liquid_b: LiquidType::LAVA,
            result_tile: 42, // obsidian
            spawn_particles: true,
        });

        map.set_liquid(3, 4, LiquidType::WATER, 7);
        map.set_liquid(4, 4, LiquidType::LAVA, 7);

        let changes = map.step(&|_, _| false);
        assert!(changes > 0);
        assert!(!map.solidified.is_empty());
        assert_eq!(map.solidified[0].2, 42); // obsidian tile
    }

    // ── Settling & edge cases ──────────────────────────────────────

    #[test]
    fn settled_cells_skip() {
        let mut map = LiquidMap::new(0, 0, 8, 8);
        map.set_liquid(4, 6, LiquidType::WATER, 3);

        // Solid floor at y=7.
        let is_solid = |_x: i32, y: i32| y == 7;

        // Run many steps until settled.
        for _ in 0..20 {
            map.step(&is_solid);
        }

        // Should eventually settle (0 changes).
        let changes = map.step(&is_solid);
        assert_eq!(changes, 0);
    }

    #[test]
    fn empty_map_no_changes() {
        let mut map = LiquidMap::new(0, 0, 8, 8);
        let changes = map.step(&|_, _| false);
        assert_eq!(changes, 0);
    }
}
