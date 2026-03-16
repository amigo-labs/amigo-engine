---
status: draft
crate: amigo_core
depends_on: []
last_updated: 2026-03-16
---

# Memory & Performance

## Purpose

Defines the data-oriented design principles, memory allocation strategies, the fixed timestep game loop, collision detection approaches, threading model, and profiling infrastructure that ensure the engine runs efficiently and deterministically.

## Internal Design

### Data-Oriented Design (SoA)

Entity data stored as Structure of Arrays for cache efficiency.

### Allocator Strategy

| Allocator | Use Case |
|-----------|----------|
| Arena (`bumpalo`) | Per-frame temporary data. Reset at frame end. |
| Object Pool | Entities (enemies, projectiles, particles). Pre-allocated. |
| Standard heap | Long-lived data (assets, tilemap, config). Load-time only. |

### Fixed Timestep Game Loop

```
while running:
    accumulate time
    while accumulator >= TICK_DURATION (1/60s):
        gather input -> Commands
        transport.send(commands)
        all_commands = transport.receive()
        server.update(game_state, all_commands)
        accumulator -= TICK_DURATION
    render(game_state, interpolation_alpha)
```

### Collision Detection

- **Tile-based:** O(1) lookup per tile
- **AABB:** Entity-vs-entity
- **Spatial Hash:** Broad-phase, grid cells, O(n) instead of O(n^2)
- **Trigger Zones:** Non-physical areas that fire events

### Threading

Main thread: game loop + simulation + rendering. Separate threads: audio (kira), asset IO, network, AI API server.

### Profiling

Tracy integration from day 1. In-game debug overlay (own Pixel UI): FPS, entity count, draw calls, memory.
