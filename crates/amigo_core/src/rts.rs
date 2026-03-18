//! Real-Time Strategy (RTS) gametype: unit selection, command queuing,
//! formation movement, resource management, building placement, and production.

use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::ecs::EntityId;
use crate::fog_of_war::{FogOfWarGrid, TileVisibility};
use crate::math::{Fix, SimVec2};

// ---------------------------------------------------------------------------
// UnitTypeId
// ---------------------------------------------------------------------------

/// Identifier for a unit type (e.g., "marine", "siege_tank").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnitTypeId(pub u32);

// ---------------------------------------------------------------------------
// SelectionSystem
// ---------------------------------------------------------------------------

/// Manages unit selection state for one player.
#[derive(Clone, Debug)]
pub struct SelectionSystem {
    /// Currently selected unit entity IDs.
    pub selected: Vec<EntityId>,
    /// Control groups (Ctrl+0-9). Each group stores entity IDs.
    pub control_groups: [Vec<EntityId>; 10],
    /// Whether a box selection drag is active.
    pub box_selecting: bool,
    /// Start position of box selection in screen coordinates.
    pub box_start: (f32, f32),
    /// Current end position of box selection.
    pub box_end: (f32, f32),
    /// Index of the current subgroup for Tab cycling.
    subgroup_index: usize,
}

impl SelectionSystem {
    /// Create a new empty selection system.
    pub fn new() -> Self {
        Self {
            selected: Vec::new(),
            control_groups: Default::default(),
            box_selecting: false,
            box_start: (0.0, 0.0),
            box_end: (0.0, 0.0),
            subgroup_index: 0,
        }
    }

    /// Start a box selection at the given screen position.
    pub fn begin_box_select(&mut self, x: f32, y: f32) {
        self.box_selecting = true;
        self.box_start = (x, y);
        self.box_end = (x, y);
    }

    /// Update the box selection end point (while dragging).
    pub fn update_box_select(&mut self, x: f32, y: f32) {
        self.box_end = (x, y);
    }

    /// Finalize box selection. Selects all player-owned units within the
    /// screen-space rectangle. `units` provides (entity, screen_x, screen_y) tuples.
    /// If `additive` is true (Shift held), adds to existing selection.
    pub fn finish_box_select(&mut self, units: &[(EntityId, f32, f32)], additive: bool) {
        let min_x = self.box_start.0.min(self.box_end.0);
        let max_x = self.box_start.0.max(self.box_end.0);
        let min_y = self.box_start.1.min(self.box_end.1);
        let max_y = self.box_start.1.max(self.box_end.1);

        if !additive {
            self.selected.clear();
        }

        let existing: FxHashSet<EntityId> = self.selected.iter().copied().collect();

        for &(entity, sx, sy) in units {
            if sx >= min_x && sx <= max_x && sy >= min_y && sy <= max_y {
                if !existing.contains(&entity) {
                    self.selected.push(entity);
                }
            }
        }

        self.box_selecting = false;
        self.subgroup_index = 0;
    }

    /// Select a single unit by click. Replaces selection unless `additive`.
    pub fn click_select(&mut self, entity: EntityId, additive: bool) {
        if additive {
            if let Some(pos) = self.selected.iter().position(|&e| e == entity) {
                self.selected.remove(pos);
            } else {
                self.selected.push(entity);
            }
        } else {
            self.selected.clear();
            self.selected.push(entity);
        }
        self.subgroup_index = 0;
    }

    /// Select all units of the same type as the clicked unit (double-click).
    pub fn select_same_type(
        &mut self,
        clicked: EntityId,
        all_units: &[(EntityId, UnitTypeId)],
    ) {
        let target_type = all_units
            .iter()
            .find(|(e, _)| *e == clicked)
            .map(|(_, t)| *t);

        if let Some(tt) = target_type {
            self.selected = all_units
                .iter()
                .filter(|(_, t)| *t == tt)
                .map(|(e, _)| *e)
                .collect();
        }
        self.subgroup_index = 0;
    }

    /// Assign current selection to a control group (Ctrl+N).
    pub fn assign_group(&mut self, group: u8) {
        if (group as usize) < self.control_groups.len() {
            self.control_groups[group as usize] = self.selected.clone();
        }
    }

    /// Recall a control group (press N). Replaces current selection.
    pub fn recall_group(&mut self, group: u8) {
        if (group as usize) < self.control_groups.len() {
            self.selected = self.control_groups[group as usize].clone();
            self.subgroup_index = 0;
        }
    }

    /// Append current selection to an existing control group (Shift+Ctrl+N).
    pub fn append_to_group(&mut self, group: u8) {
        if (group as usize) < self.control_groups.len() {
            let existing: FxHashSet<EntityId> =
                self.control_groups[group as usize].iter().copied().collect();
            for &e in &self.selected {
                if !existing.contains(&e) {
                    self.control_groups[group as usize].push(e);
                }
            }
        }
    }

    /// Cycle through subgroups of the selection by unit type (Tab key).
    /// Returns the unit type that is now the active subgroup.
    pub fn cycle_subgroup(
        &mut self,
        all_units: &[(EntityId, UnitTypeId)],
    ) -> Option<UnitTypeId> {
        let selected_set: FxHashSet<EntityId> = self.selected.iter().copied().collect();
        // Gather unique types present in the selection, preserving order.
        let mut types: Vec<UnitTypeId> = Vec::new();
        let mut seen = FxHashSet::default();
        for &(e, t) in all_units {
            if selected_set.contains(&e) && seen.insert(t) {
                types.push(t);
            }
        }

        if types.is_empty() {
            return None;
        }

        self.subgroup_index = self.subgroup_index % types.len();
        let chosen = types[self.subgroup_index];
        self.subgroup_index = (self.subgroup_index + 1) % types.len();

        // Filter selection to only the chosen type.
        self.selected = all_units
            .iter()
            .filter(|(e, t)| selected_set.contains(e) && *t == chosen)
            .map(|(e, _)| *e)
            .collect();

        Some(chosen)
    }

    /// Remove destroyed entities from selection and all control groups.
    pub fn prune_destroyed(&mut self, alive: &FxHashSet<EntityId>) {
        self.selected.retain(|e| alive.contains(e));
        for group in &mut self.control_groups {
            group.retain(|e| alive.contains(e));
        }
    }

    /// Get the selection rectangle in screen coordinates (for rendering).
    /// Returns `None` if not currently box-selecting.
    pub fn box_rect(&self) -> Option<(f32, f32, f32, f32)> {
        if !self.box_selecting {
            return None;
        }
        let min_x = self.box_start.0.min(self.box_end.0);
        let min_y = self.box_start.1.min(self.box_end.1);
        let max_x = self.box_start.0.max(self.box_end.0);
        let max_y = self.box_start.1.max(self.box_end.1);
        Some((min_x, min_y, max_x, max_y))
    }
}

impl Default for SelectionSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// UnitCommand / AbilityTarget
// ---------------------------------------------------------------------------

/// Target for an ability command.
#[derive(Clone, Debug)]
pub enum AbilityTarget {
    /// No target (self-cast).
    None,
    /// Ground-targeted ability.
    Point(SimVec2),
    /// Unit-targeted ability.
    Entity(EntityId),
}

/// Commands that can be issued to selected units.
#[derive(Clone, Debug)]
pub enum UnitCommand {
    /// Move to a world position.
    Move { target: SimVec2 },
    /// Attack-move: move to position, engaging enemies along the way.
    AttackMove { target: SimVec2 },
    /// Attack a specific entity.
    Attack { target: EntityId },
    /// Patrol between current position and target, engaging enemies.
    Patrol { target: SimVec2 },
    /// Hold position: do not move, attack enemies in range.
    Hold,
    /// Stop: cancel all commands, cease firing.
    Stop,
    /// Build a structure at the given tile position.
    Build {
        building_type: UnitTypeId,
        tile: (i32, i32),
    },
    /// Gather a resource node.
    Gather { target: EntityId },
    /// Return gathered resources to a depot.
    ReturnResources { depot: EntityId },
    /// Use a special ability.
    Ability {
        ability_id: u32,
        target: AbilityTarget,
    },
}

// ---------------------------------------------------------------------------
// CommandQueue
// ---------------------------------------------------------------------------

/// Per-unit command queue. Supports queueing via Shift+click.
#[derive(Clone, Debug, Default)]
pub struct CommandQueue {
    /// Ordered list of commands. First is currently executing.
    pub commands: VecDeque<UnitCommand>,
}

impl CommandQueue {
    /// Create an empty command queue.
    pub fn new() -> Self {
        Self {
            commands: VecDeque::new(),
        }
    }

    /// Issue a command. If `queued` (Shift held), append to queue.
    /// Otherwise, clear existing commands and set this as the only command.
    pub fn issue(&mut self, command: UnitCommand, queued: bool) {
        if queued {
            self.commands.push_back(command);
        } else {
            self.commands.clear();
            self.commands.push_back(command);
        }
    }

    /// Get the current (front) command, if any.
    pub fn current(&self) -> Option<&UnitCommand> {
        self.commands.front()
    }

    /// Complete the current command and advance to the next.
    pub fn advance(&mut self) -> Option<UnitCommand> {
        self.commands.pop_front()
    }

    /// Clear all queued commands.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Number of commands in the queue.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get all commands for waypoint rendering.
    pub fn iter(&self) -> impl Iterator<Item = &UnitCommand> {
        self.commands.iter()
    }
}

// ---------------------------------------------------------------------------
// FormationType / FormationConfig / FormationSlots / FormationSystem
// ---------------------------------------------------------------------------

/// Formation templates for group movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormationType {
    /// Units spread in a line perpendicular to move direction.
    Line,
    /// V-shaped formation with leader at the front.
    Wedge,
    /// Rectangular block formation.
    Block,
    /// No formation, units move individually to the target.
    None,
}

/// Configuration for formation behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormationConfig {
    /// Spacing between units in the formation (in world units).
    pub spacing: Fix,
    /// How tightly units try to maintain formation while moving.
    /// 0 = loose (reach destination independently), 1 = strict (wait for stragglers).
    pub cohesion: Fix,
    /// Default formation type when no specific one is set.
    pub default_formation: FormationType,
}

impl Default for FormationConfig {
    fn default() -> Self {
        Self {
            spacing: Fix::from_num(2),
            cohesion: Fix::from_num(0),
            default_formation: FormationType::None,
        }
    }
}

/// Computed formation positions for a group of units.
#[derive(Clone, Debug)]
pub struct FormationSlots {
    /// Positions relative to the formation center.
    pub slots: Vec<SimVec2>,
    /// Formation center in world coordinates.
    pub center: SimVec2,
    /// Direction the formation faces (radians as fixed-point).
    pub facing: Fix,
}

/// Formation computation utilities.
pub struct FormationSystem;

impl FormationSystem {
    /// Compute formation slot positions for a group of units moving to a target.
    ///
    /// `formation`: the desired formation layout.
    /// `unit_count`: number of units in the group.
    /// `target`: the move destination (world coordinates).
    /// `facing`: direction from the group center to the target (radians).
    /// `config`: formation spacing and type.
    pub fn compute_slots(
        formation: FormationType,
        unit_count: usize,
        target: SimVec2,
        facing: Fix,
        config: &FormationConfig,
    ) -> FormationSlots {
        if unit_count == 0 {
            return FormationSlots {
                slots: Vec::new(),
                center: target,
                facing,
            };
        }

        let spacing = config.spacing;
        let mut slots = Vec::with_capacity(unit_count);

        match formation {
            FormationType::None => {
                // All units converge on the single target point.
                for _ in 0..unit_count {
                    slots.push(SimVec2::ZERO);
                }
            }
            FormationType::Line => {
                // Line perpendicular to the facing direction.
                // Max 12 per row; wrap to multiple rows if needed.
                let max_per_row: usize = 12;
                let rows = (unit_count + max_per_row - 1) / max_per_row;
                let mut placed = 0usize;
                for row in 0..rows {
                    let remaining = unit_count - placed;
                    let cols_in_row = remaining.min(max_per_row);
                    let half = Fix::from_num(cols_in_row as i32 - 1) / Fix::from_num(2);
                    for col in 0..cols_in_row {
                        let lateral = (Fix::from_num(col as i32) - half) * spacing;
                        let depth = Fix::from_num(-(row as i32)) * spacing;
                        // Rotate by facing: lateral is perpendicular, depth is along facing.
                        let offset = rotate_offset(lateral, depth, facing);
                        slots.push(offset);
                        placed += 1;
                    }
                }
            }
            FormationType::Wedge => {
                // V-shape: leader at front, alternating left and right.
                slots.push(SimVec2::ZERO); // leader at center
                for i in 1..unit_count {
                    let rank = ((i + 1) / 2) as i32;
                    let side = if i % 2 == 1 { 1 } else { -1 };
                    let lateral = Fix::from_num(side * rank) * spacing;
                    let depth = Fix::from_num(-rank) * spacing;
                    let offset = rotate_offset(lateral, depth, facing);
                    slots.push(offset);
                }
            }
            FormationType::Block => {
                // Rectangular grid: width = ceil(sqrt(count)).
                let width = (unit_count as f32).sqrt().ceil() as usize;
                let width = width.max(1);
                let half_w = Fix::from_num(width as i32 - 1) / Fix::from_num(2);
                for i in 0..unit_count {
                    let col = i % width;
                    let row = i / width;
                    let lateral = (Fix::from_num(col as i32) - half_w) * spacing;
                    let depth = Fix::from_num(-(row as i32)) * spacing;
                    let offset = rotate_offset(lateral, depth, facing);
                    slots.push(offset);
                }
            }
        }

        FormationSlots {
            slots,
            center: target,
            facing,
        }
    }

    /// Assign units to formation slots using greedy nearest-slot matching.
    /// Returns a mapping of (entity, slot world position).
    pub fn assign_units(
        units: &[(EntityId, SimVec2)],
        slots: &FormationSlots,
    ) -> Vec<(EntityId, SimVec2)> {
        let slot_count = slots.slots.len().min(units.len());
        let mut result = Vec::with_capacity(slot_count);
        let mut used_slots = vec![false; slots.slots.len()];

        // Build (unit_idx, slot_idx, dist_sq) pairs and sort by distance.
        let mut pairs: Vec<(usize, usize, Fix)> = Vec::new();
        for (ui, (_, upos)) in units.iter().enumerate() {
            for (si, offset) in slots.slots.iter().enumerate() {
                let world_slot = SimVec2::new(
                    slots.center.x + offset.x,
                    slots.center.y + offset.y,
                );
                let dist_sq = upos.distance_squared(world_slot);
                pairs.push((ui, si, dist_sq));
            }
        }
        pairs.sort_by(|a, b| a.2.cmp(&b.2));

        let mut assigned_units = vec![false; units.len()];
        for (ui, si, _) in pairs {
            if assigned_units[ui] || used_slots[si] {
                continue;
            }
            let world_slot = SimVec2::new(
                slots.center.x + slots.slots[si].x,
                slots.center.y + slots.slots[si].y,
            );
            result.push((units[ui].0, world_slot));
            assigned_units[ui] = true;
            used_slots[si] = true;
        }

        result
    }
}

/// Rotate (lateral, depth) by a facing angle.
/// lateral = perpendicular to facing, depth = along facing direction.
fn rotate_offset(lateral: Fix, depth: Fix, facing: Fix) -> SimVec2 {
    // Use fixed-point-friendly approximation via f32 conversion for sin/cos.
    let angle: f32 = facing.to_num();
    let cos_a = Fix::from_num(angle.cos());
    let sin_a = Fix::from_num(angle.sin());
    SimVec2::new(
        lateral * cos_a - depth * sin_a,
        lateral * sin_a + depth * cos_a,
    )
}

// ---------------------------------------------------------------------------
// ResourceType / ResourceStockpile / ResourceNode
// ---------------------------------------------------------------------------

/// A resource type in the game economy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// Wood resource.
    Wood,
    /// Gold resource.
    Gold,
    /// Food resource.
    Food,
    /// Stone resource.
    Stone,
    /// Game-defined custom resource.
    Custom(u16),
}

/// Current resource stockpile for a player.
#[derive(Clone, Debug, Default)]
pub struct ResourceStockpile {
    /// Current amounts per resource type.
    pub resources: FxHashMap<ResourceType, i64>,
    /// Maximum storage capacity per resource type. Missing = unlimited.
    pub capacity: FxHashMap<ResourceType, i64>,
}

impl ResourceStockpile {
    /// Create an empty stockpile.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get current amount of a resource.
    pub fn get(&self, res: ResourceType) -> i64 {
        self.resources.get(&res).copied().unwrap_or(0)
    }

    /// Add resources (from gathering, tribute, etc.). Respects capacity.
    /// Returns the amount actually added (may be less if capped).
    pub fn add(&mut self, res: ResourceType, amount: i64) -> i64 {
        let current = self.get(res);
        let cap = self.capacity.get(&res).copied();
        let new_total = match cap {
            Some(c) => (current + amount).min(c),
            None => current + amount,
        };
        let added = new_total - current;
        *self.resources.entry(res).or_insert(0) = new_total;
        added
    }

    /// Try to spend resources. Returns true if sufficient, false otherwise.
    /// On false, no resources are deducted.
    pub fn try_spend(&mut self, res: ResourceType, amount: i64) -> bool {
        let current = self.get(res);
        if current >= amount {
            *self.resources.entry(res).or_insert(0) = current - amount;
            true
        } else {
            false
        }
    }

    /// Try to spend multiple resource types atomically (for build/train costs).
    /// Either all costs are paid or none are.
    pub fn try_spend_multi(&mut self, costs: &[(ResourceType, i64)]) -> bool {
        if !self.can_afford(costs) {
            return false;
        }
        for &(res, amount) in costs {
            let current = self.get(res);
            *self.resources.entry(res).or_insert(0) = current - amount;
        }
        true
    }

    /// Set capacity for a resource type.
    pub fn set_capacity(&mut self, res: ResourceType, cap: i64) {
        self.capacity.insert(res, cap);
    }

    /// Check if the player can afford a cost.
    pub fn can_afford(&self, costs: &[(ResourceType, i64)]) -> bool {
        costs.iter().all(|&(res, amount)| self.get(res) >= amount)
    }
}

/// A resource node on the map (tree, gold mine, berry bush).
#[derive(Clone, Debug)]
pub struct ResourceNode {
    /// Type of resource this node yields.
    pub resource_type: ResourceType,
    /// Remaining amount that can be gathered.
    pub remaining: i64,
    /// Maximum gatherers that can simultaneously harvest this node.
    pub max_gatherers: u8,
    /// Current gatherer count.
    pub current_gatherers: u8,
}

// ---------------------------------------------------------------------------
// BuildingDef / PlacementResult / ConstructionState / BuildingPlacement
// ---------------------------------------------------------------------------

/// Definition for a building type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingDef {
    /// Unit type identifier for this building.
    pub type_id: UnitTypeId,
    /// Human-readable name.
    pub name: String,
    /// Size in tiles (width, height).
    pub tile_size: (u8, u8),
    /// Construction time in ticks.
    pub build_time: u32,
    /// Resource cost to start construction.
    pub cost: Vec<(ResourceType, i64)>,
    /// Whether this building is a resource depot (gatherers return here).
    pub is_depot: bool,
    /// Types of units this building can produce (empty for non-production buildings).
    pub produces: Vec<UnitTypeId>,
    /// Required tech/building prerequisite IDs.
    pub requires: Vec<UnitTypeId>,
}

/// Validation result for building placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementResult {
    /// Placement is valid.
    Valid,
    /// Blocked by impassable terrain.
    BlockedByTerrain,
    /// Blocked by a unit.
    BlockedByUnit,
    /// Blocked by an existing building.
    BlockedByBuilding,
    /// Target tiles are in the fog of war (unexplored).
    InFogOfWar,
    /// Player cannot afford the building.
    InsufficientResources,
    /// A prerequisite building is missing.
    MissingPrerequisite,
}

/// Runtime state for a building under construction.
#[derive(Clone, Debug)]
pub struct ConstructionState {
    /// Building type being constructed.
    pub building_def: UnitTypeId,
    /// Ticks of build progress accumulated.
    pub progress: u32,
    /// Total ticks required.
    pub total: u32,
    /// Whether construction is paused (no builder assigned).
    pub paused: bool,
}

impl ConstructionState {
    /// Returns the build progress as a fraction 0.0 to 1.0.
    pub fn progress_fraction(&self) -> f32 {
        if self.total == 0 {
            return 1.0;
        }
        self.progress as f32 / self.total as f32
    }

    /// Returns true when construction is complete.
    pub fn is_complete(&self) -> bool {
        self.progress >= self.total
    }
}

/// Building placement and construction utilities.
pub struct BuildingPlacement;

impl BuildingPlacement {
    /// Validate whether a building can be placed at the given tile position.
    ///
    /// `tile_blocked` is a callback that returns `true` if the tile at (x, y)
    /// is impassable terrain.
    ///
    /// `occupied_tiles` is the set of tiles currently occupied by buildings.
    ///
    /// `existing_buildings` lists (type_id, completed) of all buildings the
    /// player owns, used for prerequisite checking.
    pub fn validate(
        def: &BuildingDef,
        tile_x: i32,
        tile_y: i32,
        tile_blocked: &dyn Fn(i32, i32) -> bool,
        fog: &FogOfWarGrid,
        stockpile: &ResourceStockpile,
        existing_buildings: &[(UnitTypeId, bool)],
    ) -> PlacementResult {
        // Check prerequisites.
        for req in &def.requires {
            let met = existing_buildings
                .iter()
                .any(|(t, completed)| t == req && *completed);
            if !met {
                return PlacementResult::MissingPrerequisite;
            }
        }

        // Check resources.
        if !stockpile.can_afford(&def.cost) {
            return PlacementResult::InsufficientResources;
        }

        // Check each tile in footprint.
        let tiles = Self::footprint(def, tile_x, tile_y);
        for &(tx, ty) in &tiles {
            // Fog check.
            if fog.visibility_at(tx, ty) == TileVisibility::Hidden {
                return PlacementResult::InFogOfWar;
            }
            // Terrain check.
            if tile_blocked(tx, ty) {
                return PlacementResult::BlockedByTerrain;
            }
        }

        PlacementResult::Valid
    }

    /// Snap a world position to the nearest valid tile for placement.
    pub fn snap_to_grid(world_x: f32, world_y: f32, tile_size: f32) -> (i32, i32) {
        let tx = (world_x / tile_size).floor() as i32;
        let ty = (world_y / tile_size).floor() as i32;
        (tx, ty)
    }

    /// Get the tile footprint for a building at a position.
    pub fn footprint(def: &BuildingDef, tile_x: i32, tile_y: i32) -> Vec<(i32, i32)> {
        let mut tiles = Vec::with_capacity(def.tile_size.0 as usize * def.tile_size.1 as usize);
        for dy in 0..def.tile_size.1 as i32 {
            for dx in 0..def.tile_size.0 as i32 {
                tiles.push((tile_x + dx, tile_y + dy));
            }
        }
        tiles
    }
}

// ---------------------------------------------------------------------------
// ProductionOrder / ProductionQueue
// ---------------------------------------------------------------------------

/// A single production order (unit training or research).
#[derive(Clone, Debug)]
pub struct ProductionOrder {
    /// What is being produced.
    pub unit_type: UnitTypeId,
    /// Ticks of progress accumulated.
    pub progress: u32,
    /// Total ticks required.
    pub total: u32,
    /// Resource cost (already deducted when queued).
    pub cost: Vec<(ResourceType, i64)>,
}

/// Per-building production queue.
#[derive(Clone, Debug)]
pub struct ProductionQueue {
    /// Queued production orders.
    pub orders: VecDeque<ProductionOrder>,
    /// Maximum queue depth.
    pub max_queue: u8,
}

impl Default for ProductionQueue {
    fn default() -> Self {
        Self {
            orders: VecDeque::new(),
            max_queue: 5,
        }
    }
}

impl ProductionQueue {
    /// Create a new production queue with the given maximum depth.
    pub fn new(max_queue: u8) -> Self {
        Self {
            orders: VecDeque::new(),
            max_queue,
        }
    }

    /// Enqueue a production order. Deducts cost from stockpile.
    /// Returns false if queue is full or resources are insufficient.
    pub fn enqueue(
        &mut self,
        unit_type: UnitTypeId,
        build_time: u32,
        cost: Vec<(ResourceType, i64)>,
        stockpile: &mut ResourceStockpile,
    ) -> bool {
        if self.orders.len() >= self.max_queue as usize {
            return false;
        }
        if !stockpile.try_spend_multi(&cost) {
            return false;
        }
        self.orders.push_back(ProductionOrder {
            unit_type,
            progress: 0,
            total: build_time,
            cost,
        });
        true
    }

    /// Cancel the last order in the queue. Refunds resources.
    pub fn cancel_last(&mut self, stockpile: &mut ResourceStockpile) {
        if let Some(order) = self.orders.pop_back() {
            for &(res, amount) in &order.cost {
                stockpile.add(res, amount);
            }
        }
    }

    /// Cancel a specific order by index. Refunds resources.
    pub fn cancel_at(&mut self, index: usize, stockpile: &mut ResourceStockpile) {
        if index < self.orders.len() {
            let order = self.orders.remove(index).unwrap();
            for &(res, amount) in &order.cost {
                stockpile.add(res, amount);
            }
        }
    }

    /// Tick the production queue. Returns `Some(UnitTypeId)` if a unit
    /// finished training this tick.
    pub fn tick(&mut self) -> Option<UnitTypeId> {
        if let Some(front) = self.orders.front_mut() {
            front.progress += 1;
            if front.progress >= front.total {
                let completed = self.orders.pop_front().unwrap();
                return Some(completed.unit_type);
            }
        }
        None
    }

    /// Get progress of the current order as a 0.0-1.0 fraction.
    pub fn current_progress(&self) -> Option<f32> {
        self.orders.front().map(|o| {
            if o.total == 0 {
                1.0
            } else {
                o.progress as f32 / o.total as f32
            }
        })
    }
}

// ---------------------------------------------------------------------------
// UnitAiState
// ---------------------------------------------------------------------------

/// Lightweight FSM state for per-unit AI behavior within the current command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitAiState {
    /// No command, scanning for nearby threats.
    Idle,
    /// Moving to a target position.
    Moving,
    /// Target acquired, closing to weapon range.
    Chasing { target: EntityId },
    /// In weapon range, attacking.
    Attacking { target: EntityId, cooldown: u16 },
    /// Gathering resources from a node.
    Gathering { node: EntityId, progress: u16 },
    /// Constructing a building.
    Building { site: EntityId, progress: u32 },
    /// Returning resources to a depot.
    Returning { depot: EntityId },
    /// Holding position, attacking threats in range.
    Holding,
}

// ---------------------------------------------------------------------------
// MinimapData
// ---------------------------------------------------------------------------

/// RTS minimap data pushed each frame.
#[derive(Clone, Debug, Default)]
pub struct MinimapData {
    /// Friendly unit positions (for green dots).
    pub friendly_units: Vec<(f32, f32)>,
    /// Enemy unit positions visible through fog (for red dots).
    pub enemy_units: Vec<(f32, f32)>,
    /// Resource node positions (for yellow dots).
    pub resource_nodes: Vec<(f32, f32)>,
    /// Camera viewport rectangle (for the white box).
    pub camera_rect: (f32, f32, f32, f32),
    /// Ping locations with countdown timers.
    pub pings: Vec<(f32, f32, u16)>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EntityId;

    fn eid(index: u32) -> EntityId {
        EntityId::from_raw(index, 0)
    }

    #[test]
    fn test_click_select_replace() {
        let mut sel = SelectionSystem::new();
        sel.click_select(eid(1), false);
        assert_eq!(sel.selected, vec![eid(1)]);
        sel.click_select(eid(2), false);
        assert_eq!(sel.selected, vec![eid(2)]);
    }

    #[test]
    fn test_click_select_additive() {
        let mut sel = SelectionSystem::new();
        sel.click_select(eid(1), false);
        sel.click_select(eid(2), true);
        assert_eq!(sel.selected, vec![eid(1), eid(2)]);
        // Toggle off.
        sel.click_select(eid(1), true);
        assert_eq!(sel.selected, vec![eid(2)]);
    }

    #[test]
    fn test_box_select() {
        let mut sel = SelectionSystem::new();
        sel.begin_box_select(0.0, 0.0);
        sel.update_box_select(10.0, 10.0);
        assert!(sel.box_rect().is_some());

        let units = vec![
            (eid(1), 5.0, 5.0),
            (eid(2), 15.0, 15.0),
            (eid(3), 8.0, 8.0),
        ];
        sel.finish_box_select(&units, false);
        assert_eq!(sel.selected, vec![eid(1), eid(3)]);
        assert!(!sel.box_selecting);
    }

    #[test]
    fn test_control_groups() {
        let mut sel = SelectionSystem::new();
        sel.click_select(eid(1), false);
        sel.click_select(eid(2), true);
        sel.assign_group(1);
        assert_eq!(sel.control_groups[1], vec![eid(1), eid(2)]);

        sel.click_select(eid(3), false);
        sel.recall_group(1);
        assert_eq!(sel.selected, vec![eid(1), eid(2)]);
    }

    #[test]
    fn test_prune_destroyed() {
        let mut sel = SelectionSystem::new();
        sel.click_select(eid(1), false);
        sel.click_select(eid(2), true);
        sel.assign_group(0);

        let mut alive = FxHashSet::default();
        alive.insert(eid(1));
        sel.prune_destroyed(&alive);

        assert_eq!(sel.selected, vec![eid(1)]);
        assert_eq!(sel.control_groups[0], vec![eid(1)]);
    }

    #[test]
    fn test_command_queue_issue_and_advance() {
        let mut q = CommandQueue::new();
        q.issue(UnitCommand::Move { target: SimVec2::ZERO }, false);
        q.issue(UnitCommand::Hold, true); // queued
        assert_eq!(q.len(), 2);

        // Non-queued replaces.
        q.issue(UnitCommand::Stop, false);
        assert_eq!(q.len(), 1);

        let cmd = q.advance();
        assert!(matches!(cmd, Some(UnitCommand::Stop)));
        assert!(q.is_empty());
    }

    #[test]
    fn test_resource_stockpile() {
        let mut s = ResourceStockpile::new();
        s.add(ResourceType::Gold, 500);
        assert_eq!(s.get(ResourceType::Gold), 500);

        assert!(s.try_spend(ResourceType::Gold, 200));
        assert_eq!(s.get(ResourceType::Gold), 300);

        assert!(!s.try_spend(ResourceType::Gold, 400));
        assert_eq!(s.get(ResourceType::Gold), 300); // unchanged

        // Capacity.
        s.set_capacity(ResourceType::Gold, 350);
        let added = s.add(ResourceType::Gold, 100);
        assert_eq!(added, 50);
        assert_eq!(s.get(ResourceType::Gold), 350);
    }

    #[test]
    fn test_try_spend_multi_atomic() {
        let mut s = ResourceStockpile::new();
        s.add(ResourceType::Gold, 100);
        s.add(ResourceType::Wood, 50);

        // Can't afford: wood is insufficient.
        let costs = vec![(ResourceType::Gold, 50), (ResourceType::Wood, 100)];
        assert!(!s.try_spend_multi(&costs));
        // Nothing deducted.
        assert_eq!(s.get(ResourceType::Gold), 100);
        assert_eq!(s.get(ResourceType::Wood), 50);

        // Can afford.
        let costs = vec![(ResourceType::Gold, 50), (ResourceType::Wood, 50)];
        assert!(s.try_spend_multi(&costs));
        assert_eq!(s.get(ResourceType::Gold), 50);
        assert_eq!(s.get(ResourceType::Wood), 0);
    }

    #[test]
    fn test_production_queue_enqueue_tick_cancel() {
        let mut stockpile = ResourceStockpile::new();
        stockpile.add(ResourceType::Gold, 200);

        let mut pq = ProductionQueue::new(3);
        let cost = vec![(ResourceType::Gold, 100)];

        assert!(pq.enqueue(UnitTypeId(1), 3, cost.clone(), &mut stockpile));
        assert_eq!(stockpile.get(ResourceType::Gold), 100);

        // Tick twice: not done yet.
        assert!(pq.tick().is_none());
        assert!(pq.tick().is_none());
        // Third tick completes it.
        let done = pq.tick();
        assert_eq!(done, Some(UnitTypeId(1)));

        // Enqueue and cancel.
        assert!(pq.enqueue(UnitTypeId(2), 5, cost.clone(), &mut stockpile));
        assert_eq!(stockpile.get(ResourceType::Gold), 0);
        pq.cancel_last(&mut stockpile);
        assert_eq!(stockpile.get(ResourceType::Gold), 100);
    }

    #[test]
    fn test_building_footprint_and_snap() {
        let def = BuildingDef {
            type_id: UnitTypeId(10),
            name: "Barracks".to_string(),
            tile_size: (3, 2),
            build_time: 100,
            cost: vec![],
            is_depot: false,
            produces: vec![UnitTypeId(1)],
            requires: vec![],
        };

        let fp = BuildingPlacement::footprint(&def, 5, 10);
        assert_eq!(fp.len(), 6);
        assert!(fp.contains(&(5, 10)));
        assert!(fp.contains(&(7, 11)));

        let (tx, ty) = BuildingPlacement::snap_to_grid(17.5, 33.9, 16.0);
        assert_eq!(tx, 1);
        assert_eq!(ty, 2);
    }

    #[test]
    fn test_formation_block_slot_count() {
        let config = FormationConfig::default();
        let slots = FormationSystem::compute_slots(
            FormationType::Block,
            9,
            SimVec2::ZERO,
            Fix::ZERO,
            &config,
        );
        assert_eq!(slots.slots.len(), 9);
    }

    #[test]
    fn test_construction_state() {
        let mut cs = ConstructionState {
            building_def: UnitTypeId(5),
            progress: 0,
            total: 100,
            paused: false,
        };
        assert!(!cs.is_complete());
        cs.progress = 100;
        assert!(cs.is_complete());
        assert!((cs.progress_fraction() - 1.0).abs() < f32::EPSILON);
    }
}
