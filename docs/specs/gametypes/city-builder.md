---
status: done
crate: --
depends_on: ["engine/chunks", "engine/procedural"]
last_updated: 2026-03-18
---

# City Builder

## Purpose

Management and simulation games centered on constructing and maintaining a settlement. The player places buildings and infrastructure, manages resource production chains, and balances citizen happiness across multiple factors. The simulation runs continuously with hundreds to thousands of agents making autonomous decisions.

Examples: SimCity (zoning + service coverage + traffic), Cities: Skylines (road-centric city planning + district policies), Dwarf Fortress (deep agent simulation + resource chains), Banished (survival colony management + harsh economy).

## Public API

### ResourceFlow

```rust
/// Directed graph of resource production, storage, and consumption.
/// Resources flow from producers through the road network to storage and consumers.
pub struct ResourceFlow {
    nodes: Vec<FlowNode>,
    edges: Vec<FlowEdge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowNode {
    pub id: FlowNodeId,
    pub node_type: FlowNodeType,
    pub position: GridPos,
    /// Resource buffer: how much is currently stored here.
    pub buffer: FxHashMap<ResourceType, i32>,
    /// Maximum buffer capacity per resource type.
    pub capacity: FxHashMap<ResourceType, i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowNodeId(pub u32);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FlowNodeType {
    /// Produces a resource at a fixed rate.
    Producer {
        output: ResourceType,
        rate_per_tick: i32,
        /// Optional input required for production (refinery: ore → metal).
        input: Option<(ResourceType, i32)>,
    },
    /// Stores resources for pickup.
    Storage,
    /// Consumes a resource (e.g. houses consume food).
    Consumer {
        input: ResourceType,
        rate_per_tick: i32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Food,
    Wood,
    Stone,
    Metal,
    Gold,
    Water,
    Power,
    /// Game-specific resource identified by index.
    Custom(u16),
}

#[derive(Clone, Debug)]
pub struct FlowEdge {
    pub from: FlowNodeId,
    pub to: FlowNodeId,
    /// Maximum throughput per tick (limited by road capacity).
    pub max_throughput: i32,
    /// Current throughput this tick.
    pub current_throughput: i32,
    /// Path through the road network (cached, recomputed on road changes).
    pub road_path: Vec<GridPos>,
}

impl ResourceFlow {
    pub fn new() -> Self;
    pub fn add_node(&mut self, node: FlowNode) -> FlowNodeId;
    pub fn remove_node(&mut self, id: FlowNodeId);
    pub fn connect(&mut self, from: FlowNodeId, to: FlowNodeId, road: &RoadNetwork);
    pub fn disconnect(&mut self, from: FlowNodeId, to: FlowNodeId);
    /// Simulate one tick of resource flow. Moves resources along edges
    /// respecting throughput limits and buffer capacities.
    pub fn tick(&mut self);
    /// Total production rate for a resource type across all producers.
    pub fn total_production(&self, resource: ResourceType) -> i32;
    /// Total consumption rate for a resource type across all consumers.
    pub fn total_consumption(&self, resource: ResourceType) -> i32;
    /// Net flow (production - consumption) for a resource type.
    pub fn net_flow(&self, resource: ResourceType) -> i32;
    /// Returns nodes with unsatisfied demand (consumers with empty buffers).
    pub fn shortages(&self) -> Vec<(FlowNodeId, ResourceType)>;
}
```

### ZoneSystem

```rust
/// Zoning layer on the tilemap. Zones auto-fill with appropriate buildings
/// when connected to the road network and resource demand exists.
#[derive(Clone, Debug)]
pub struct ZoneSystem {
    /// Zone assignment per tile (None = unzoned).
    zones: Vec<Option<ZoneType>>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ZoneType {
    Residential,
    Commercial,
    Industrial,
    /// Special zone types (parks, civic, etc.).
    Special(u16),
}

/// A building that has grown in a zone.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZoneBuilding {
    pub zone: ZoneType,
    pub position: GridPos,
    /// Footprint size in tiles.
    pub size: (u32, u32),
    /// Density level (1 = low-rise, 2 = mid-rise, 3 = high-rise).
    pub density: u8,
    /// Number of residents/workers/shoppers this building supports.
    pub capacity: u32,
    /// Current occupancy.
    pub occupancy: u32,
}

impl ZoneSystem {
    pub fn new(width: u32, height: u32) -> Self;
    /// Paint a zone type onto a rectangular area.
    pub fn paint_zone(&mut self, area: Rect, zone: ZoneType);
    /// Remove zoning from an area.
    pub fn clear_zone(&mut self, area: Rect);
    pub fn zone_at(&self, pos: GridPos) -> Option<ZoneType>;
    /// Find empty zoned tiles adjacent to roads that can accept new buildings.
    pub fn growable_tiles(&self, zone: ZoneType, roads: &RoadNetwork) -> Vec<GridPos>;
    /// Attempt to grow a building in a zone. Called periodically by the simulation.
    pub fn try_grow(
        &mut self,
        zone: ZoneType,
        demand: f32,
        roads: &RoadNetwork,
        registry: &BuildingRegistry,
    ) -> Option<ZoneBuilding>;
}
```

### RoadNetwork

```rust
/// Graph of road tiles used for pathfinding and connectivity.
/// All resource flow and agent movement uses roads.
pub struct RoadNetwork {
    /// Grid of road tiles (true = road present).
    tiles: Vec<bool>,
    pub width: u32,
    pub height: u32,
    /// Cached connectivity components (recomputed on road changes).
    components: Vec<u32>,
    /// Flow field cache for common destinations (recomputed lazily).
    flow_cache: FxHashMap<GridPos, FlowField>,
}

impl RoadNetwork {
    pub fn new(width: u32, height: u32) -> Self;
    /// Place a road tile. Triggers connectivity recomputation.
    pub fn place_road(&mut self, pos: GridPos);
    /// Remove a road tile.
    pub fn remove_road(&mut self, pos: GridPos);
    pub fn has_road(&self, pos: GridPos) -> bool;
    /// Check if two positions are connected via roads.
    pub fn connected(&self, a: GridPos, b: GridPos) -> bool;
    /// Get or compute a flow field toward a destination. Uses engine FlowField pathfinding.
    pub fn flow_field_to(&mut self, destination: GridPos) -> &FlowField;
    /// Find shortest road path between two points via A*.
    pub fn shortest_path(&self, from: GridPos, to: GridPos) -> Option<Vec<GridPos>>;
    /// All tiles in the same connected component as `pos`.
    pub fn connected_component(&self, pos: GridPos) -> Vec<GridPos>;
    /// Traffic density per road tile (number of agents using this tile per tick).
    pub fn traffic_density(&self, pos: GridPos) -> u32;
    /// Invalidate cached flow fields (called after road topology changes).
    pub fn invalidate_cache(&mut self);
}
```

### HappinessModel

```rust
/// Multi-factor happiness aggregation per zone and globally.
#[derive(Clone, Debug)]
pub struct HappinessModel {
    /// Per-factor weights (must sum to 1.0).
    pub weights: FxHashMap<HappinessFactor, f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HappinessFactor {
    Safety,
    Education,
    Health,
    Leisure,
    Employment,
    Environment,
    /// Inverse of commute length.
    Commute,
    /// Inverse of noise and pollution levels.
    Pollution,
}

/// Per-tile happiness scores (used for heatmap overlay).
#[derive(Clone, Debug)]
pub struct HappinessGrid {
    scores: Vec<FxHashMap<HappinessFactor, f32>>,
    pub width: u32,
    pub height: u32,
}

impl HappinessModel {
    pub fn new(weights: FxHashMap<HappinessFactor, f32>) -> Self;
    pub fn default_weights() -> Self;
    /// Compute aggregate happiness for a single tile.
    pub fn score_at(&self, grid: &HappinessGrid, pos: GridPos) -> f32;
    /// Compute average happiness for a zone area.
    pub fn zone_score(&self, grid: &HappinessGrid, area: Rect) -> f32;
    /// Global average happiness across all occupied tiles.
    pub fn global_score(&self, grid: &HappinessGrid) -> f32;
}

impl HappinessGrid {
    pub fn new(width: u32, height: u32) -> Self;
    /// Update happiness factors based on building placement.
    /// Service buildings (hospital, school, park) radiate positive effects.
    pub fn update_from_buildings(
        &mut self,
        buildings: &[PlacedBuilding],
        registry: &BuildingRegistry,
        roads: &RoadNetwork,
    );
    pub fn factor_at(&self, pos: GridPos, factor: HappinessFactor) -> f32;
    pub fn set_factor(&mut self, pos: GridPos, factor: HappinessFactor, value: f32);
}
```

### PopulationSim

```rust
/// Agent-based population simulation using Utility AI.
/// Each agent has needs (from engine Agents system) and makes autonomous decisions.
pub struct PopulationSim {
    pub agents: Vec<Citizen>,
    /// Birth rate modifier.
    pub birth_rate: f32,
    /// Death rate modifier.
    pub death_rate: f32,
    /// Immigration threshold (global happiness above this attracts immigrants).
    pub immigration_threshold: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Citizen {
    pub id: EntityId,
    pub home: Option<FlowNodeId>,
    pub workplace: Option<FlowNodeId>,
    /// Agent needs from the engine Needs system.
    pub needs: Needs,
    /// Current behavior state.
    pub state: CitizenState,
    /// Age in simulation ticks (for birth/death cycle).
    pub age: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CitizenState {
    AtHome,
    Commuting,
    Working,
    Shopping,
    Leisure,
    Seeking(CitizenSeekTarget),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CitizenSeekTarget {
    Home,
    Job,
    Food,
    Entertainment,
}

impl PopulationSim {
    pub fn new() -> Self;
    /// Simulate one tick. Each citizen evaluates needs via Utility AI
    /// and picks the highest-scoring action (go home, go to work, seek food, etc.).
    /// Uses FlowField pathfinding along RoadNetwork.
    pub fn tick(
        &mut self,
        happiness: &HappinessGrid,
        model: &HappinessModel,
        roads: &RoadNetwork,
        resources: &ResourceFlow,
    );
    /// Spawn new citizens (birth/immigration) based on demand and happiness.
    pub fn spawn_citizens(&mut self, count: u32);
    /// Remove citizens (death/emigration).
    pub fn remove_citizens(&mut self, count: u32);
    pub fn population(&self) -> usize;
    pub fn employment_rate(&self) -> f32;
    pub fn homeless_count(&self) -> usize;
}
```

### BuildingRegistry

```rust
/// Central registry of all building types and their properties.
pub struct BuildingRegistry {
    buildings: Vec<BuildingDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingDef {
    pub id: BuildingId,
    pub name: String,
    /// Footprint in tiles.
    pub size: (u32, u32),
    /// Construction cost in resources.
    pub cost: FxHashMap<ResourceType, i32>,
    /// Monthly upkeep cost.
    pub upkeep: FxHashMap<ResourceType, i32>,
    /// Resource production (if any).
    pub production: Option<(ResourceType, i32)>,
    /// Resource consumption (if any).
    pub consumption: Option<(ResourceType, i32)>,
    /// Radius of effect for service buildings (hospital, school, etc.).
    pub effect_radius: Option<u32>,
    /// Happiness factors this building provides within its radius.
    pub happiness_effects: FxHashMap<HappinessFactor, f32>,
    /// Negative effects within radius (noise, pollution).
    pub negative_effects: FxHashMap<HappinessFactor, f32>,
    /// Category for UI grouping.
    pub category: BuildingCategory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildingId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingCategory {
    Residential,
    Commercial,
    Industrial,
    Service,
    Infrastructure,
    Decoration,
}

/// A building that has been placed in the world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlacedBuilding {
    pub def_id: BuildingId,
    pub position: GridPos,
    /// Construction progress (0.0 = just placed, 1.0 = complete).
    pub construction: f32,
    /// Whether this building is operational (connected to roads and powered).
    pub operational: bool,
    /// Associated resource flow node.
    pub flow_node: Option<FlowNodeId>,
}

impl BuildingRegistry {
    pub fn new() -> Self;
    /// Load building definitions from RON.
    pub fn load(path: &str) -> Result<Self, LoadError>;
    pub fn get(&self, id: BuildingId) -> Option<&BuildingDef>;
    pub fn by_category(&self, category: BuildingCategory) -> Vec<&BuildingDef>;
    /// Check if a building can be placed at a position (space, terrain, road adjacency).
    pub fn can_place(
        &self,
        id: BuildingId,
        pos: GridPos,
        zones: &ZoneSystem,
        roads: &RoadNetwork,
    ) -> bool;
}
```

### StatisticsOverlay

```rust
/// Heatmap overlays for visualizing city data on the tilemap.
pub struct StatisticsOverlay {
    pub active_overlay: Option<OverlayType>,
    /// Color gradient for heatmap rendering (low → high).
    pub gradient: ColorGradient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayType {
    Happiness(Option<HappinessFactor>),
    Traffic,
    Pollution,
    ResourceFlow(ResourceType),
    LandValue,
    ServiceCoverage(BuildingCategory),
    /// Building effect radius preview (shown during placement).
    BuildingRadius(BuildingId),
}

#[derive(Clone, Debug)]
pub struct ColorGradient {
    pub stops: Vec<(f32, Color)>,
}

impl StatisticsOverlay {
    pub fn new() -> Self;
    pub fn set_overlay(&mut self, overlay: OverlayType);
    pub fn clear_overlay(&mut self);
    /// Render the active overlay as a semi-transparent color layer on the tilemap.
    pub fn render(
        &self,
        happiness: &HappinessGrid,
        roads: &RoadNetwork,
        resources: &ResourceFlow,
        ctx: &mut RenderContext,
    );
}

impl ColorGradient {
    pub fn new(stops: Vec<(f32, Color)>) -> Self;
    /// Default green-yellow-red gradient for most overlays.
    pub fn default_heatmap() -> Self;
    /// Interpolate a color for a value in [0.0, 1.0].
    pub fn sample(&self, t: f32) -> Color;
}
```

### DisasterSystem

```rust
/// Random event system for natural and man-made disasters.
pub struct DisasterSystem {
    /// Active disasters currently affecting the city.
    pub active: Vec<ActiveDisaster>,
    /// Cooldown ticks between disaster events.
    pub cooldown: u32,
    /// Remaining cooldown ticks.
    remaining_cooldown: u32,
    /// Whether disasters are enabled (can be disabled in sandbox mode).
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct ActiveDisaster {
    pub disaster_type: DisasterType,
    /// Affected area in world tiles.
    pub area: Rect,
    /// Remaining duration in ticks.
    pub remaining_ticks: u32,
    /// Damage applied per tick to buildings in the area.
    pub damage_per_tick: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisasterType {
    Fire,
    Flood,
    Earthquake,
    Tornado,
    Epidemic,
}

impl DisasterSystem {
    pub fn new(cooldown: u32) -> Self;
    /// Roll for a random disaster. Probability influenced by city state
    /// (low safety = more fires, bad health = epidemics).
    pub fn tick(&mut self, happiness: &HappinessGrid, model: &HappinessModel);
    /// Manually trigger a disaster (debug / scenario).
    pub fn trigger(&mut self, disaster_type: DisasterType, area: Rect, duration: u32);
    /// Apply damage to buildings in disaster areas.
    pub fn apply_damage(&self, buildings: &mut [PlacedBuilding]);
    /// Check if a position is affected by an active disaster.
    pub fn is_affected(&self, pos: GridPos) -> Option<DisasterType>;
}
```

## Behavior

- **Resource Flow Simulation**: Each tick, `ResourceFlow::tick()` propagates resources along edges. Producers generate output into their buffers. Edges transfer resources from source buffers to destination buffers, limited by `max_throughput` (determined by road capacity). Consumers draw from their buffers. If a consumer's buffer is empty, it reports a shortage. The flow graph is recomputed when buildings or roads change.
- **Zone Growth**: `ZoneSystem::try_grow()` is called periodically. If demand exists (more jobs needed, more shops needed) and empty zoned tiles are road-adjacent, a `ZoneBuilding` spawns at appropriate density. Density increases over time if happiness and demand remain high. Buildings visually upgrade (low-rise to high-rise) via `AnimPlayer` tag transitions.
- **Road Pathfinding**: All agent movement and resource transport uses the `RoadNetwork`. `FlowField` pathfinding (from engine) handles mass movement of citizens. `A*` handles individual routing queries (e.g. finding nearest workplace). Flow fields are cached per destination and invalidated when road topology changes.
- **Happiness Computation**: `HappinessGrid::update_from_buildings()` recalculates per-tile happiness factors using **linear distance falloff**: for a service building at position `B` with `effect_radius` R, each tile `T` within Manhattan distance R receives `effect_value * (1.0 - distance(B, T) / R)`. Multiple buildings of the same factor stack additively (clamped to 0.0-1.0). Industrial buildings radiate negative `Pollution` and noise using the same falloff. `HappinessModel::score_at()` computes the weighted aggregate. Citizens evaluate local happiness when deciding where to live/work. Recalculation is throttled to once per 60 ticks (not every frame) since building placement is infrequent.
- **Population AI**: Each `Citizen` has `Needs` (from engine Agents system) that decay over time. The Utility AI scores available actions: go home (satisfies Comfort), go to work (satisfies Employment need, earns Gold), go shopping (satisfies Hunger with Food resource), seek leisure (satisfies Fun). Citizens use `FlowFieldFollow` (from [engine/pathfinding](../engine/pathfinding.md)) along road flow fields to navigate — NOT `Steering::PathFollow` (which expects waypoints). **LOD Transition Protocol**: Citizens exist in two simulation modes:
  - **Full Simulation** (on-screen, within 2 chunks of camera): ECS entity with sprite, position updated by `FlowFieldFollow`, needs evaluated per tick.
  - **Statistical Simulation** (off-screen): No ECS entity, no pathfinding. Needs decay at fixed rates. Action choice uses probability tables (e.g., 60% chance at work during day ticks, 30% commuting, 10% shopping). When the camera scrolls to reveal a statistically-simulated citizen, a full ECS entity is spawned at the citizen's logical location (home/workplace/road). When the camera scrolls away, the entity is despawned and the citizen transitions back to statistical mode. Transition boundary: 2 chunks beyond camera viewport.
- **Building Placement**: The player selects a `BuildingDef` from the `BuildingRegistry`. During placement, `StatisticsOverlay` shows the `BuildingRadius` preview. `BuildingRegistry::can_place()` validates terrain, road adjacency, and zone compatibility. On placement, a `PlacedBuilding` is created with `construction = 0.0`. Construction progresses per tick (consuming resources) until `1.0`, at which point the building becomes `operational` and its `FlowNode` is added to `ResourceFlow`.
- **Statistics Overlays**: The player toggles overlays via `StatisticsOverlay::set_overlay()`. The overlay renders a semi-transparent color grid on top of the tilemap, sampling the appropriate data source (happiness factors, traffic density, resource flow) and mapping values through `ColorGradient::sample()`.
- **Disasters**: `DisasterSystem::tick()` rolls for random events influenced by city state. Fires are more likely with low Safety scores. Epidemics with low Health. Active disasters deal damage to `PlacedBuilding`s in their area, reducing `construction` (representing structural damage). Buildings at `0.0` are destroyed. Service buildings (fire station, hospital) within range reduce damage and duration.
- **Camera**: Uses free-pan camera with continuous zoom (from single-building detail to full-city overview). `Minimap` shows the entire map with `PinType::Dot` for key buildings and disaster indicators.

## Internal Design

- The world is divided into `Chunks` (from engine) for efficient rendering and simulation. Only chunks visible to the camera are fully rendered. Distant chunks use LOD (simplified tile rendering).
- `ZoneSystem` and `HappinessGrid` are stored as flat `Vec` grids matching the tilemap dimensions. `RoadNetwork` maintains a parallel boolean grid plus a union-find structure for connectivity.
- `ResourceFlow` is a separate directed graph structure, not tied to the tilemap grid. Nodes reference `GridPos` for spatial queries but the graph topology is independent. Edge paths are cached `Vec<GridPos>` computed via `RoadNetwork::shortest_path()`. **Cache invalidation**: `RoadNetwork::invalidate_cache()` is called on any `place_road()` or `remove_road()`. This clears the `flow_cache` (FlowField cache) and marks all `FlowEdge.road_path` caches as dirty. Dirty edge paths are lazily recomputed on next `ResourceFlow::tick()`. Additionally, `ResourceFlow::on_road_change(changed_pos)` disconnects edges whose cached paths passed through the changed tile, forcing reconnection via the new road topology.
- `PopulationSim` agents are lightweight `Citizen` structs, not full ECS entities for performance. Only visible citizens (near camera) get ECS entities with `RenderVec2` for sprite rendering. Off-screen citizens are pure simulation (statistical update). See LOD Transition Protocol in Behavior section.
- `BuildingRegistry` definitions are loaded from RON files. `PlacedBuilding`s are serialized via `SaveManager` for save/load. The entire city state (`ResourceFlow`, `ZoneSystem`, `RoadNetwork`, `PopulationSim`, `HappinessGrid`, active buildings, `DisasterSystem`) is the save payload.
- **Save Scaling for Large Cities**: Cities with >10,000 citizens serialize only the statistical representation (needs + state enum + home/workplace IDs), not full ECS components. `PlacedBuilding` and `RoadNetwork` are serialized as flat grids (one byte per tile for roads, building IDs indexed into `BuildingRegistry`). Total save size target: <1 MB for a 200x200 tile city with 50,000 citizens. LZ4 compression from [engine/save-load](../engine/save-load.md) further reduces to ~200 KB.
- `StatisticsOverlay` renders directly to the tilemap layer using per-tile color tinting. No separate render target — just a color multiply on existing tiles.

## Non-Goals

- **3D buildings / isometric rendering.** Strictly 2D top-down tilemap. Density is represented by sprite variants, not geometric height.
- **Realistic traffic simulation.** Road capacity is abstracted as throughput limits on `FlowEdge`. No per-vehicle simulation, lane merging, or traffic light logic.
- **Multiplayer / competitive city building.** Single-player simulation only.
- **Political simulation.** No elections, policies, or governance systems beyond happiness factors.
- **Terrain deformation.** Terrain is static tilemap. No terraforming, leveling, or water table simulation.

## Open Questions

- Should `PopulationSim` be fully agent-based (every citizen is simulated) or use a hybrid model (agents near camera, statistics elsewhere)?
- How should power and water distribution work — as a resource in `ResourceFlow` or as a separate grid-based network (pipes/power lines)?
- Should `ZoneSystem` support mixed-use zoning (commercial + residential in the same building)?
- What is the maximum map size that can sustain 60fps simulation with the chunk system?
- Should `DisasterSystem` support player-buildable defenses (levees, firebreaks) as special buildings?

## Referenzen

- SimCity (2013): Zone-based growth, service radius, traffic flow
- Cities: Skylines: Road-centric design, district policies, heatmap overlays
- Dwarf Fortress: Deep agent simulation, resource chain complexity
- Banished: Survival economy, limited resources, citizen lifecycle
- [engine/chunks](../engine/chunks.md) → Large world subdivision and LOD
- [engine/tilemap](../engine/tilemap.md) → Zone layer and collision grid
- [engine/agents](../engine/agents.md) → Utility AI with Needs for citizen behavior
- [engine/pathfinding](../engine/pathfinding.md) → FlowField for mass agent movement, A* for routing
- [engine/steering](../engine/steering.md) → PathFollow and Separation for citizen navigation
- [engine/camera](../engine/camera.md) → FreePan with continuous zoom for city overview
- [engine/procedural](../engine/procedural.md) → Terrain generation for new maps
- [engine/minimap](../engine/minimap.md) → City overview with building and disaster pins
- [engine/save-load](../engine/save-load.md) → Full city state persistence via SaveManager
