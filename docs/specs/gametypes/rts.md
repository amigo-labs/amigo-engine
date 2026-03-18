---
status: done
crate: --
depends_on: ["engine/pathfinding", "engine/fog-of-war"]
last_updated: 2026-03-18
---

# Real-Time Strategy (RTS)

## Purpose

Template for real-time strategy games with unit selection, command queuing, formation movement, resource management, and building placement. Target games: StarCraft, Age of Empires, Warcraft III, Command & Conquer.

RTS games demand efficient handling of large unit counts (100-500), responsive selection/command input, and integration with pathfinding (flow fields for mass movement), fog of war, and steering behaviors for formation keeping.

## Public API

### SelectionSystem

```rust
/// Manages unit selection state.
#[derive(Clone, Debug)]
pub struct SelectionSystem {
    /// Currently selected unit entity IDs.
    pub selected: Vec<Entity>,
    /// Control groups (Ctrl+1-9). Each group stores entity IDs.
    pub control_groups: [Vec<Entity>; 10],
    /// Whether a box selection drag is active.
    pub box_selecting: bool,
    /// Start position of box selection in screen coordinates.
    pub box_start: (f32, f32),
    /// Current end position of box selection.
    pub box_end: (f32, f32),
}

impl SelectionSystem {
    pub fn new() -> Self;

    /// Start a box selection at the given screen position.
    pub fn begin_box_select(&mut self, x: f32, y: f32);

    /// Update the box selection end point (while dragging).
    pub fn update_box_select(&mut self, x: f32, y: f32);

    /// Finalize box selection. Selects all player-owned units within the
    /// screen-space rectangle. `units` provides (entity, screen_x, screen_y) tuples.
    /// If `additive` is true (Shift held), adds to existing selection.
    pub fn finish_box_select(
        &mut self,
        units: &[(Entity, f32, f32)],
        additive: bool,
    );

    /// Select a single unit by click. Replaces selection unless `additive`.
    pub fn click_select(&mut self, entity: Entity, additive: bool);

    /// Select all units of the same type as the clicked unit (double-click).
    pub fn select_same_type(
        &mut self,
        clicked: Entity,
        all_units: &[(Entity, UnitTypeId)],
    );

    /// Assign current selection to a control group (Ctrl+N).
    pub fn assign_group(&mut self, group: u8);

    /// Recall a control group (press N). Replaces current selection.
    pub fn recall_group(&mut self, group: u8);

    /// Append current selection to an existing control group (Shift+Ctrl+N).
    pub fn append_to_group(&mut self, group: u8);

    /// Cycle through subgroups of the selection by unit type (Tab key).
    /// Returns the unit type that is now the active subgroup.
    pub fn cycle_subgroup(
        &mut self,
        all_units: &[(Entity, UnitTypeId)],
    ) -> Option<UnitTypeId>;

    /// Remove destroyed entities from selection and all control groups.
    pub fn prune_destroyed(&mut self, alive: &FxHashSet<Entity>);

    /// Get the selection rectangle in screen coordinates (for rendering).
    pub fn box_rect(&self) -> Option<(f32, f32, f32, f32)>;
}

/// Identifier for a unit type (e.g., "marine", "siege_tank").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnitTypeId(pub u32);
```

### UnitCommand

```rust
/// Commands that can be issued to selected units.
#[derive(Clone, Debug)]
pub enum UnitCommand {
    /// Move to a world position.
    Move { target: SimVec2 },
    /// Attack-move: move to position, engaging enemies along the way.
    AttackMove { target: SimVec2 },
    /// Attack a specific entity.
    Attack { target: Entity },
    /// Patrol between current position and target, engaging enemies.
    Patrol { target: SimVec2 },
    /// Hold position: do not move, attack enemies in range.
    Hold,
    /// Stop: cancel all commands, cease firing.
    Stop,
    /// Build a structure at the given tile position.
    Build { building_type: UnitTypeId, tile: (i32, i32) },
    /// Gather a resource node.
    Gather { target: Entity },
    /// Return gathered resources to a depot.
    ReturnResources { depot: Entity },
    /// Use a special ability.
    Ability { ability_id: u32, target: AbilityTarget },
}

/// Target for an ability command.
#[derive(Clone, Debug)]
pub enum AbilityTarget {
    None,
    Point(SimVec2),
    Entity(Entity),
}
```

### CommandQueue

```rust
/// Per-unit command queue. Supports queueing via Shift+click.
#[derive(Clone, Debug, Default)]
pub struct CommandQueue {
    /// Ordered list of commands. First is currently executing.
    pub commands: VecDeque<UnitCommand>,
}

impl CommandQueue {
    pub fn new() -> Self;

    /// Issue a command. If `queued` (Shift held), append to queue.
    /// Otherwise, clear existing commands and set this as the only command.
    pub fn issue(&mut self, command: UnitCommand, queued: bool);

    /// Get the current (front) command, if any.
    pub fn current(&self) -> Option<&UnitCommand>;

    /// Complete the current command and advance to the next.
    pub fn advance(&mut self) -> Option<UnitCommand>;

    /// Clear all queued commands.
    pub fn clear(&mut self);

    /// Number of commands in the queue.
    pub fn len(&self) -> usize;

    /// Get all commands for waypoint rendering.
    pub fn iter(&self) -> impl Iterator<Item = &UnitCommand>;
}
```

### FormationSystem

```rust
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
    pub spacing: I16F16,
    /// How tightly units try to maintain formation while moving.
    /// 0.0 = loose (reach destination independently), 1.0 = strict (wait for stragglers).
    pub cohesion: I16F16,
    /// Default formation type when no specific one is set.
    pub default_formation: FormationType,
}

/// Computed formation positions for a group of units.
#[derive(Clone, Debug)]
pub struct FormationSlots {
    /// Positions relative to the formation center.
    pub slots: Vec<SimVec2>,
    /// Formation center in world coordinates.
    pub center: SimVec2,
    /// Direction the formation faces (radians).
    pub facing: I16F16,
}

pub struct FormationSystem;

impl FormationSystem {
    /// Compute formation slot positions for a group of units moving to a target.
    /// `unit_count`: number of units in the group.
    /// `target`: the move destination (world coordinates).
    /// `facing`: direction from the group center to the target.
    /// `config`: formation spacing and type.
    pub fn compute_slots(
        formation: FormationType,
        unit_count: usize,
        target: SimVec2,
        facing: I16F16,
        config: &FormationConfig,
    ) -> FormationSlots;

    /// Assign units to formation slots. Uses nearest-slot matching to
    /// minimize total travel distance (greedy assignment).
    /// Returns a mapping of entity -> slot world position.
    pub fn assign_units(
        units: &[(Entity, SimVec2)],
        slots: &FormationSlots,
    ) -> Vec<(Entity, SimVec2)>;
}
```

### ResourceSystem

```rust
/// A resource type in the game economy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Wood,
    Gold,
    Food,
    Stone,
    /// Game-defined custom resource.
    Custom(u16),
}

/// Current resource stockpile for a player.
#[derive(Clone, Debug, Default)]
pub struct ResourceStockpile {
    pub resources: FxHashMap<ResourceType, i64>,
    /// Maximum storage capacity per resource type. None = unlimited.
    pub capacity: FxHashMap<ResourceType, i64>,
}

impl ResourceStockpile {
    pub fn new() -> Self;

    /// Get current amount of a resource.
    pub fn get(&self, res: ResourceType) -> i64;

    /// Add resources (from gathering, tribute, etc.). Respects capacity.
    /// Returns the amount actually added (may be less if capped).
    pub fn add(&mut self, res: ResourceType, amount: i64) -> i64;

    /// Try to spend resources. Returns true if sufficient, false otherwise.
    /// On false, no resources are deducted.
    pub fn try_spend(&mut self, res: ResourceType, amount: i64) -> bool;

    /// Try to spend multiple resource types atomically (for build/train costs).
    /// Either all costs are paid or none are.
    pub fn try_spend_multi(&mut self, costs: &[(ResourceType, i64)]) -> bool;

    /// Set capacity for a resource type.
    pub fn set_capacity(&mut self, res: ResourceType, cap: i64);

    /// Check if the player can afford a cost.
    pub fn can_afford(&self, costs: &[(ResourceType, i64)]) -> bool;
}

/// A resource node on the map (tree, gold mine, berry bush).
#[derive(Clone, Debug)]
pub struct ResourceNode {
    pub resource_type: ResourceType,
    /// Remaining amount that can be gathered.
    pub remaining: i64,
    /// Maximum gatherers that can simultaneously harvest this node.
    pub max_gatherers: u8,
    /// Current gatherer count.
    pub current_gatherers: u8,
}
```

### BuildingPlacement

```rust
/// Building placement and construction system.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingDef {
    pub type_id: UnitTypeId,
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
    Valid,
    BlockedByTerrain,
    BlockedByUnit,
    BlockedByBuilding,
    InFogOfWar,
    InsufficientResources,
    MissingPrerequisite,
}

/// Runtime state for a building under construction.
#[derive(Clone, Debug)]
pub struct ConstructionState {
    pub building_def: UnitTypeId,
    /// Ticks of build progress accumulated.
    pub progress: u32,
    /// Total ticks required.
    pub total: u32,
    /// Whether construction is paused (no builder assigned).
    pub paused: bool,
}

pub struct BuildingPlacement;

impl BuildingPlacement {
    /// Validate whether a building can be placed at the given tile position.
    pub fn validate(
        def: &BuildingDef,
        tile_x: i32,
        tile_y: i32,
        tilemap: &Tilemap,
        fog: &TileVisibility,
        stockpile: &ResourceStockpile,
        existing_buildings: &[(UnitTypeId, bool)],
    ) -> PlacementResult;

    /// Snap a world position to the nearest valid tile for placement.
    pub fn snap_to_grid(world_x: f32, world_y: f32, tile_size: f32) -> (i32, i32);

    /// Get the tile footprint for a building at a position.
    pub fn footprint(
        def: &BuildingDef,
        tile_x: i32,
        tile_y: i32,
    ) -> Vec<(i32, i32)>;
}
```

### ProductionQueue

```rust
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
#[derive(Clone, Debug, Default)]
pub struct ProductionQueue {
    pub orders: VecDeque<ProductionOrder>,
    /// Maximum queue depth.
    pub max_queue: u8,              // default: 5
}

impl ProductionQueue {
    pub fn new(max_queue: u8) -> Self;

    /// Enqueue a production order. Deducts cost from stockpile.
    /// Returns false if queue is full or resources are insufficient.
    pub fn enqueue(
        &mut self,
        unit_type: UnitTypeId,
        build_time: u32,
        cost: Vec<(ResourceType, i64)>,
        stockpile: &mut ResourceStockpile,
    ) -> bool;

    /// Cancel the last order in the queue. Refunds resources.
    pub fn cancel_last(&mut self, stockpile: &mut ResourceStockpile);

    /// Cancel a specific order by index. Refunds resources.
    pub fn cancel_at(&mut self, index: usize, stockpile: &mut ResourceStockpile);

    /// Tick the production queue. Returns Some(UnitTypeId) if a unit
    /// finished training this tick.
    pub fn tick(&mut self) -> Option<UnitTypeId>;

    /// Get progress of the current order as a 0.0-1.0 fraction.
    pub fn current_progress(&self) -> Option<f32>;
}
```

### Minimap Integration

```rust
/// RTS minimap data pushed each frame.
#[derive(Clone, Debug)]
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
```

## Behavior

### Unit Selection

**Click select**: clicking a unit selects it exclusively. Shift+click adds/removes from selection. Double-clicking selects all visible units of the same type.

**Box select**: click-drag draws a rectangle on screen. On release, all player-owned units within the rectangle are selected. Shift+drag adds to existing selection. Units are tested against the screen-space rectangle (not world-space) so the selection feels consistent regardless of camera zoom.

**Control groups**: Ctrl+1-9 assigns the current selection to a numbered group. Pressing 1-9 recalls that group. Double-tapping a group number centers the camera on the group. Shift+Ctrl+N appends current selection to group N. Tab cycles through unit types within the selection (e.g., mixed infantry and vehicles).

**Pruning**: `prune_destroyed()` is called each frame to remove dead entities from the selection and all control groups.

### Command Issuing

Right-clicking on terrain issues a `Move` command. Right-clicking on an enemy issues `Attack`. Right-clicking on a resource node issues `Gather`. Right-clicking on a friendly depot issues `ReturnResources`.

Commands are issued to all selected units. Without Shift, the command replaces any existing queue. With Shift held, commands are appended to each unit's `CommandQueue`, creating waypoint chains.

The `A` key enters attack-move mode: the next click issues `AttackMove` to the target position. `H` issues `Hold`, `S` issues `Stop`, `P` issues `Patrol`.

### Command Execution

Each unit processes its `CommandQueue` front:

- **Move**: unit pathfinds to target using flow fields. Command completes when the unit reaches the target within a tolerance radius.
- **AttackMove**: unit moves toward target. If an enemy enters weapon range, the unit stops and attacks. After the enemy dies or leaves range, movement resumes.
- **Attack**: unit moves into weapon range of the target entity and attacks until the target is dead, then advances to the next command.
- **Patrol**: unit moves to target, then back to origin, repeating. Engages enemies along the path.
- **Hold**: unit stays in place and attacks enemies in range. Never moves.
- **Stop**: clears the queue. Unit goes idle.
- **Gather**: worker moves to resource node, harvests over time, then auto-issues `ReturnResources` to nearest depot, then re-issues `Gather` on the same node. Cycle repeats until the node is depleted.
- **Build**: worker moves to the build site and begins construction. Build progress increments each tick while the worker is assigned.

### Formation Movement

When a Move or AttackMove command is issued to a group, `FormationSystem::compute_slots()` generates positions for the selected formation type. `assign_units()` matches each unit to its nearest slot using a greedy algorithm (sort by distance, assign closest pairs first).

Units pathfind to their assigned slot positions rather than all converging on a single point. The steering system (`Separation`, `Cohesion`, `Alignment` from the steering module) maintains formation coherence during movement.

Formation types:
- **Line**: units spread perpendicular to movement direction. Width = `unit_count`, depth = 1 row. If count exceeds a threshold (12), wraps to multiple rows.
- **Wedge**: V-shape with the leader at the tip. Good for scouting.
- **Block**: rectangular grid. Width = `ceil(sqrt(count))`. Dense and defensible.
- **None**: each unit pathfinds independently to the click point. They cluster at the destination.

### Resource Gathering

Workers assigned to a resource node navigate to it and begin harvesting. Harvest rate is one tick per `gather_speed` ticks (configured per unit type). When the worker's carry capacity is full, it pathfinds to the nearest depot building and deposits resources into the player's `ResourceStockpile`. Then it returns to the same node.

If the node depletes, the worker auto-searches for the nearest node of the same resource type within a radius. If none is found, the worker goes idle.

`max_gatherers` on a `ResourceNode` limits how many workers can harvest simultaneously. Additional workers queue in a staging area around the node.

### Building Placement and Construction

The player enters build mode by selecting a building from the UI. A ghost of the building follows the cursor, snapped to the tile grid. `BuildingPlacement::validate()` is called each frame to color the ghost green (valid) or red (invalid). Validation checks:

1. All tiles in the footprint must be walkable terrain (not water, cliff, etc.).
2. No other buildings overlap the footprint.
3. No units are standing in the footprint (or they can be pushed aside).
4. Tiles must be explored (not in `TileVisibility::Hidden`).
5. Player must be able to afford the cost.
6. Prerequisites (tech buildings, etc.) must be built.

On placement confirmation, resources are deducted and a `ConstructionState` component is added to the building entity. A worker is dispatched to the build site. Construction ticks increment while a worker is assigned; it pauses if the worker is pulled away.

**Building Lifecycle (ECS):**
1. **Placement**: Entity spawned with `BuildingDef`, `ConstructionState { progress: 0, paused: true }`, collision shape matching footprint. Tilemap tiles under footprint marked as `Solid`.
2. **Construction**: Worker arrives → `paused = false`. Each tick: `progress += 1`. Collision shape active during construction (units path around it).
3. **Completion**: `progress >= total` → `ConstructionState` removed, `ProductionQueue` added (if applicable), `is_depot` flag activates. Building becomes operational.
4. **Destruction**: On `health <= 0`, building entity is despawned, tilemap tiles under footprint reset to `Empty`, any active `ProductionQueue` orders are refunded.

### Production

Buildings with a `ProductionQueue` can train units. Enqueueing deducts the cost immediately. The front order ticks each frame. When progress reaches `total`, the unit spawns at a rally point near the building. If the rally point is set to a position, the new unit auto-moves there.

Production can be cancelled. Cancelling refunds the full resource cost.

## Internal Design

### Unit AI (State Machine)

Each unit runs a lightweight FSM for behavior decisions within its current command:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitAiState {
    /// No command, scanning for nearby threats.
    Idle,
    /// Moving to a target position.
    Moving,
    /// Target acquired, closing to weapon range.
    Chasing { target: Entity },
    /// In weapon range, attacking.
    Attacking { target: Entity, cooldown: u16 },
    /// Gathering resources from a node.
    Gathering { node: Entity, progress: u16 },
    /// Constructing a building.
    Building { site: Entity, progress: u32 },
    /// Returning resources to a depot.
    Returning { depot: Entity },
    /// Holding position, attacking threats in range.
    Holding,
}
```

**State transitions** are driven by the current `CommandQueue` front and environmental sensors:
- `Idle` + enemy in vision range → `Chasing` (if not on `Hold` or `Stop`)
- `Moving` + enemy in weapon range (for `AttackMove`) → `Chasing`
- `Chasing` + target in weapon range → `Attacking`
- `Attacking` + target dead → back to previous command (`Moving`, `Idle`, or `Patrolling`)
- `Gathering` + carry full → `Returning`
- `Returning` + at depot → `Gathering` (back to same node)

The FSM is evaluated per tick per unit. With 500 units, this is ~500 state evaluations per tick — trivial cost.

### Pathfinding Integration

Move commands for groups of 4+ units use flow fields via `FlowFieldCache` from [engine/pathfinding](../engine/pathfinding.md). Each distinct destination gets one cached flow field, shared by all units moving there. The cache automatically invalidates when the tilemap changes (building placed/destroyed).

Single units or small groups (1-3) use A* for more precise paths.

**Selection heuristic:** `if group_size >= 4 { flow_field } else { a_star }`. The threshold is configurable per game.

### Steering Integration

Units use `FlowFieldFollow` (from [engine/pathfinding](../engine/pathfinding.md)) instead of `PathFollow` for flow-field-based movement. This resolves the mismatch between FlowField (returns direction per cell) and Steering::PathFollow (expects waypoints).

During movement, units combine:
- `FlowFieldFollow` or `Arrive` (for A*) — primary navigation
- `Separation` — avoid overlapping with nearby units
- Formation movement additionally adds `Cohesion` and `Alignment`

The steering output is clamped to the unit's `max_speed` and applied as velocity.

### Fog of War Integration

`TileVisibility` from the fog-of-war system determines:
- What the player can see on the minimap.
- Whether enemy units are visible (only in `Visible` tiles).
- Whether building placement is allowed (not in `Hidden` tiles).
- "Last seen" ghost buildings in `Explored` tiles (building was visible but now in fog).

Each unit has a `vision_range` that feeds into the fog-of-war BFS computation.

### Camera Integration

The RTS camera uses `FreePan` mode (middle-mouse drag or WASD) combined with `EdgePan` (cursor at screen edges scrolls the view). Scroll wheel controls zoom. Double-tapping a control group number centers the camera on that group's centroid. The minimap click teleports the camera.

### ECS Layout

Key components per entity:
- All units: `UnitTypeId`, `CommandQueue`, `Team`, `Health`, `VisionRange`
- Selectable units: `Selectable` (marker), screen position cached per frame
- Workers: `GatherState { carrying: ResourceType, amount, target_node }`
- Buildings: `BuildingDef`, `ProductionQueue`, `ConstructionState` (if building)
- Resource nodes: `ResourceNode`

## Non-Goals

- **Lockstep networking.** Multiplayer RTS requires deterministic lockstep simulation. This template defines the game systems but not the networking protocol. See [engine/networking](../engine/networking.md).
- **Tech tree editor.** Prerequisites are defined in `BuildingDef.requires` but there is no visual tech tree editor or graph visualization.
- **Hero units / RPG elements.** Warcraft III-style hero leveling is not included. Units have fixed stats.
- **Terrain deformation.** Building placement checks terrain but does not modify it.
- **AI opponent.** RTS AI (build orders, army composition, strategic decision-making) is a separate system not covered here.
- **Replay system.** Command-level replay recording is not included.

## Open Questions

- Should the formation system support custom formation templates loaded from RON, or are the 3 built-in types sufficient?
- How should resource gathering interact with moving resource nodes (e.g., herded animals in AoE)?
- Should the production queue support research (tech upgrades) in addition to unit training?
- How should rally points work for multiple buildings of the same type selected simultaneously?
- Should there be a global supply/population cap system (StarCraft-style)?
- What is the maximum unit count the engine should target before performance becomes a concern?

## Referenzen

- [engine/pathfinding](../engine/pathfinding.md) -- A* for single units, Flow Fields for group movement
- [engine/steering](../engine/steering.md) -- Separation, Cohesion, Alignment for formations
- [engine/fog-of-war](../engine/fog-of-war.md) -- TileVisibility, BFS vision computation
- [engine/camera](../engine/camera.md) -- FreePan, EdgePan for RTS camera control
- [engine/minimap](../engine/minimap.md) -- Minimap rendering and click-to-move
- [engine/tilemap](../engine/tilemap.md) -- Terrain data for placement validation
- StarCraft -- Control groups, box selection, command queue, supply cap
- Age of Empires -- Resource gathering loops, building placement, formations
- Warcraft III -- Hero units (reference for future extension), rally points
