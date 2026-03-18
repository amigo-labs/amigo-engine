---
status: draft
crate: amigo_pathfinding
depends_on: ["engine/tilemap"]
last_updated: 2026-03-18
---

# Pathfinding

## Purpose

Engine-level pathfinding for any genre that needs it. Provides A* search on tile grids, predefined waypoint paths for Tower Defense, and optional flow fields for large-scale navigation scenarios.

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
