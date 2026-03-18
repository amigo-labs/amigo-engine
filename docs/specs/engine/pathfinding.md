---
status: draft
crate: amigo_pathfinding
depends_on: ["engine/tilemap"]
last_updated: 2026-03-16
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
