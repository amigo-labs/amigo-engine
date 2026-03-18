---
status: done
crate: amigo_core
depends_on: ["engine/tilemap"]
last_updated: 2026-03-18
---

# Pathfinding

## Purpose

Engine-level pathfinding for any genre that needs it. Provides A* search on tile grids, predefined waypoint paths for Tower Defense, and optional flow fields for large-scale navigation scenarios.

## Implementation

| File | Content |
|------|---------|
| `crates/amigo_core/src/pathfinding.rs` | A* search, `WaypointPath`, `PathFollower`, `FlowField`, `Walkable` trait |
| `crates/amigo_core/src/navigation.rs` | `NavAgent` (click-to-move), `Direction` enum, `update_nav_agents()` system |
| `crates/amigo_editor/src/auto_path.rs` | Editor auto-path generation using A* with path simplification |
| `examples/pathfinding_demo/src/main.rs` | Interactive pathfinding demo |

All types are re-exported from `amigo_core::pathfinding` and `amigo_core::navigation`.

**Tests:** 21 total (11 pathfinding, 2 navigation, 8 auto_path).

## Public API

### A* on Tile Grid

```rust
// Basic A* pathfinding
let path: Option<Vec<IVec2>> = pathfinder.find_path(
    start,                      // tile position
    goal,                       // tile position
    &tilemap.collision,         // walkability data
);

// Configurable per entity
pub struct PathRequest {
    pub start: IVec2,
    pub goal: IVec2,
    pub allow_diagonal: bool,   // 4-way or 8-way movement
    pub max_search: u32,        // max nodes to explore (budget)
}
```

### Predefined Waypoint Paths (TD)

For Tower Defense, paths are editor-defined waypoints -- no dynamic pathfinding needed:

```rust
pub struct WaypointPath {
    pub points: Vec<SimVec2>,
}

pub struct PathFollower {
    pub path_index: usize,
    pub progress: Fix,
    pub segment: usize,
}
```

### Flow Fields (optional, large-scale)

For scenarios with many entities navigating to the same goal (horde modes, RTS). A single grid-sized field where each cell stores the direction to the goal. O(1) per entity lookup.

Available as opt-in feature, not built by default.

---

## Extensions (Sandbox/God Sim)

> Added per gap analysis (`05-sandbox-godsim-gaps.md`). Pathfinding and flow field implementations are in `crates/amigo_core/src/pathfinding.rs` and `crates/amigo_core/src/navigation.rs`.

### Dynamic Navmesh Updates

When the tilemap changes at runtime (Sandbox: block placed/destroyed, God Sim: building constructed), existing paths may become invalid. The engine supports this through:

1. **Path invalidation:** When `DynamicTileWorld` emits `TileEvent::Placed` or `TileEvent::Destroyed`, game code should check active `NavAgent` paths against the changed tiles and re-request pathfinding for affected agents.

2. **FlowField recomputation:** If using `FlowField` for mass navigation, recompute the field when tiles in the covered area change. Since `FlowField::compute()` runs Dijkstra over the full grid, limit recomputation frequency (e.g., once per N frames or only when dirty chunks overlap the field area).

3. **NavAgent re-pathing:** The `NavAgent` struct (in `crates/amigo_core/src/navigation.rs`) exposes `move_to()` and `move_to_tile()` which re-run A* on demand. Call these again when the tilemap invalidates the current path.

```rust
// crates/amigo_core/src/navigation.rs

impl NavAgent {
    /// Request movement to a world position. Computes path via A*.
    pub fn move_to(&mut self, target: RenderVec2, map: &dyn Walkable);

    /// Request movement to a specific tile.
    pub fn move_to_tile(&mut self, goal: IVec2, map: &dyn Walkable);

    /// Stop all movement immediately.
    pub fn stop(&mut self);

    /// Get the current path for debug rendering.
    pub fn current_path(&self) -> &[IVec2];
}
```

**Re-pathing pattern (game code):**
```
for each TileEvent::Placed or TileEvent::Destroyed at (x, y):
    for each active NavAgent:
        if agent.current_path() passes through (x, y):
            agent.move_to(agent's original goal, &updated_map)
```

### Flow Fields for Mass Navigation

For God Sim scenarios with hundreds of agents navigating to the same goal (e.g., villagers returning to town center, military units marching), individual A* per agent is too expensive. The `FlowField` computes a single direction field via Dijkstra, then each agent looks up its direction in O(1).

```rust
// crates/amigo_core/src/pathfinding.rs

pub struct FlowField {
    pub width: u32,
    pub height: u32,
    directions: Vec<(i8, i8)>,
    costs: Vec<u32>,
}

impl FlowField {
    /// Compute a flow field from the goal using Dijkstra's algorithm.
    /// Cardinal moves cost 10, diagonal moves cost 14.
    /// Diagonal moves are only allowed if both adjacent cardinal cells are walkable.
    pub fn compute(goal: IVec2, width: u32, height: u32, map: &dyn Walkable) -> Self;

    /// Get the direction to move from a given cell (returns (dx, dy) as i8).
    pub fn direction_at(&self, x: i32, y: i32) -> (i8, i8);

    /// Get the cost to reach the goal from a cell (u32::MAX = unreachable).
    pub fn cost_at(&self, x: i32, y: i32) -> u32;

    /// Check if a cell is reachable (has a finite cost).
    pub fn is_reachable(&self, x: i32, y: i32) -> bool;
}
```

**God Sim usage pattern:**
- Compute one `FlowField` per shared goal (e.g., town center, rally point).
- Each tick, each agent reads `direction_at(agent_tile_x, agent_tile_y)` and moves in that direction.
- Recompute the field only when the tilemap changes or the goal moves.
- For multiple goals, maintain multiple `FlowField` instances (one per goal type).

### FlowField Cache (für RTS und City Builder)

Wenn viele Agents dasselbe Ziel ansteuern, soll nicht pro Agent ein neues FlowField berechnet werden. Der `FlowFieldCache` verwaltet berechnete Fields pro Ziel und invalidiert bei Map-Änderungen.

```rust
// crates/amigo_core/src/pathfinding.rs

pub struct FlowFieldCache {
    /// Cached fields keyed by goal position.
    cache: FxHashMap<IVec2, FlowField>,
    /// Generation counter — incremented on invalidation.
    generation: u64,
    /// Maximum number of cached fields (LRU eviction beyond this).
    pub max_entries: usize,
    /// Last-access order for LRU eviction.
    access_order: Vec<IVec2>,
}

impl FlowFieldCache {
    pub fn new(max_entries: usize) -> Self;

    /// Get or compute a flow field for the given goal.
    /// Returns a reference to the cached field.
    pub fn get_or_compute(
        &mut self,
        goal: IVec2,
        width: u32,
        height: u32,
        map: &dyn Walkable,
    ) -> &FlowField;

    /// Invalidate all cached fields (call after tilemap changes).
    pub fn invalidate_all(&mut self);

    /// Invalidate only fields whose goal or path passes through the changed area.
    /// More efficient than `invalidate_all()` for localized map changes.
    /// Checks if any cell in the changed region has a different cost than before.
    pub fn invalidate_region(&mut self, changed: &Rect, map: &dyn Walkable);

    /// Current generation counter (clients can compare to detect stale fields).
    pub fn generation(&self) -> u64;

    /// Number of cached fields.
    pub fn len(&self) -> usize;

    /// Remove all cached fields.
    pub fn clear(&mut self);
}
```

**Invalidation Strategy:**
- **Full invalidation** (`invalidate_all`): Für seltene große Map-Änderungen (Level-Load, Terraforming). Setzt `generation` hoch, leert den Cache.
- **Region invalidation** (`invalidate_region`): Für einzelne Tile-Änderungen (Road placed/removed, Building constructed/destroyed). Prüft ob der geänderte Bereich auf dem Pfad eines gecachten Fields liegt. Nur betroffene Fields werden entfernt.
- **LRU Eviction**: Bei `max_entries` Überschreitung wird das am längsten unbenutzte Field entfernt. Standard: 32 für RTS, 64 für City Builder.

**Steering-Integration:**
FlowField gibt `(i8, i8)` Direction pro Zelle zurück. Die Steering-Integration konvertiert dies in einen SimVec2-Heading:

```rust
/// Konvertiert FlowField-Direction in einen normalisierten Heading-Vektor.
pub fn flow_direction_to_heading(dir: (i8, i8)) -> SimVec2 {
    // (1,0) -> (FIX_ONE, 0), (1,1) -> normalized diagonal, etc.
}

/// Steering-Behavior das einem FlowField folgt statt Waypoints.
pub struct FlowFieldFollow {
    pub field: FlowFieldId,
    pub arrival_radius: I16F16,
}
```

Dies löst den Mismatch zwischen `Steering::PathFollow` (erwartet Waypoints) und `FlowField` (gibt Directions). Agents im RTS und City Builder verwenden `FlowFieldFollow` statt `PathFollow`.
