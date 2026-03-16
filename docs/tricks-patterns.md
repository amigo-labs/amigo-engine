# Amigo Engine – Tricks, Techniques & Patterns

## Internal Reference Document for Developers

---

## Overview

This document describes all the clever techniques, optimizations, and architecture patterns that the Amigo Engine uses. Each technique explains: **What is the problem?**, **How does the engine solve it?**, **Where is it used?**, and **Code sketch**.

---

## DATA STRUCTURES & ECS

---

### 1. SparseSet

**Problem:** Random access on entity components (O(1)) AND cache-friendly iteration over all components at the same time. Arrays can do one, HashMaps can do the other – not both.

**Solution:** Two arrays: a large "sparse" array (index = entity ID, value = position in the dense array) and a small "dense" array (compact, no holes, cache-friendly).

```
Sparse: [_, _, 0, _, 2, _, _, 1, _, _, 3]   ← Index = Entity ID
Dense:  [Pos_A, Pos_B, Pos_C, Pos_D]        ← compact, iterable
IDs:    [  2,     7,     4,    10  ]          ← which entity per slot
```

- **Random Access:** `sparse[entity_id]` → Dense index → O(1)
- **Iteration:** Linear over dense array → perfect cache locality
- **Insert:** Append to dense, set sparse entry → O(1)
- **Remove:** Swap-remove in dense (last element fills the gap), update sparse → O(1)

**Where:** Each component type has its own SparseSet. Position, Velocity, Health, SpriteComp – all in separate dense arrays, all iterable in parallel.

**Serialization:** Only the dense array + IDs are saved. Sparse is rebuilt on load.

---

### 2. Change Tracking (BitSet)

**Problem:** 500 entities have a position. 30 change per frame. The renderer only wants to re-sort the changed sprites.

**Solution:** A BitSet alongside the dense array. Each bit = "has changed since the last query?" `get_mut()` sets the bit automatically.

```rust
pub fn get_mut(&mut self, entity: EntityId) -> &mut T {
    let dense_idx = self.sparse[entity.id()];
    self.changed.set(dense_idx, true);  // automatically marked!
    &mut self.dense[dense_idx]
}

pub fn query_changed(&self) -> impl Iterator<Item = (EntityId, &T)> {
    self.changed.iter_ones().map(|i| (self.ids[i], &self.dense[i]))
}

pub fn clear_changed(&mut self) {
    self.changed.clear();
}
```

**Where:** Rendering (only re-sort changed sprites), network (only send changed components), save (only store dirty-marked entities).

---

### 3. State-Scoped Entity Cleanup

**Problem:** State change (Playing → Menu). Hundreds of gameplay entities need to go. Forget one → memory leak or ghost entities.

**Solution:** `StateScoped` component. On state change: automatically despawn all entities with the old state.

```rust
// Spawn:
world.spawn()
    .with(Enemy { ... })
    .with(StateScoped(GameState::Playing));

// State change to Menu → Engine automatically despawns EVERYTHING with Playing
// No manual cleanup. No forgetting possible.
```

**Where:** Every state transition. Especially important for Threadwalker: world change despawns all entities of the previous world automatically.

---

### 4. Hybrid Component Storage

**Problem:** The engine has "hot" components (Position, Velocity – almost every entity has them, read every frame) and "cold" components (TowerData, QuestProgress – only certain entities, rarely read). A uniform system wastes cache on cold data or is slow for hot data.

**Solution:** Hot components as statically typed SparseSet fields. Cold, game-specific components in a `HashMap<TypeId, Box<dyn AnyStorage>>`.

```rust
pub struct World {
    // Hot path – static fields, no HashMap lookup
    pub positions: SparseSet<Position>,
    pub velocities: SparseSet<Velocity>,
    pub sprites: SparseSet<SpriteComp>,
    pub healths: SparseSet<Health>,

    // Cold path – dynamic, for game-specific components
    pub extensions: HashMap<TypeId, Box<dyn AnyStorage>>,
}
```

**Where:** Engine-internal components (Position, Velocity, Collider) = static. Game components (TowerData, QuestLog, ArmState) = dynamic.

---

### 5. Tick Scheduler

**Problem:** Some systems don't need to run every frame. Pathfinding every 10 frames is enough. AI decisions every 30 frames. But the game loop runs at 60 FPS.

**Solution:** `scheduler.every(n, system)` – system is only executed every N ticks.

```rust
scheduler.every(1, physics_system);        // every frame
scheduler.every(10, pathfinding_system);    // every 10 frames
scheduler.every(30, ai_decision_system);    // every 30 frames
scheduler.every(60, save_autosave);         // every second
```

**Where:** Pathfinding, AI, autosave, flow field recalculation, atmospheric transitions (slow, doesn't need 60 FPS).

---

## COLLISION & SPATIAL

---

### 6. Spatial Hash

**Problem:** "Which entities are nearby?" Naive: O(n²). With 500 entities = 250,000 checks per frame.

**Solution:** An invisible grid over the world. Entities are sorted into cells. Queries check only 9 neighboring cells instead of all entities.

```rust
pub struct SpatialHash {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<EntityId>>,
}

// Query: only 9 cells instead of N entities
fn query_nearby(&self, pos: Vec2, radius: f32) -> Vec<EntityId> {
    let r = (radius / self.cell_size).ceil() as i32;
    let cx = (pos.x / self.cell_size) as i32;
    let cy = (pos.y / self.cell_size) as i32;
    // iterate (cx-r..=cx+r) × (cy-r..=cy+r)
}
```

Rebuilt completely every frame (faster than incremental updates with moving entities).

**Where:** Collision detection, range checks (tower range), proximity queries (nearby NPCs), AoE damage.

---

### 7. AABB Collision (Axis-Aligned Bounding Box)

**Problem:** Two sprites overlap – are they colliding? Pixel-perfect collision is too expensive for 60 FPS.

**Solution:** Each entity has an invisible rectangle (AABB). Overlap of two rectangles = a single condition:

```rust
fn aabb_overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w &&
    a.x + a.w > b.x &&
    a.y < b.y + b.h &&
    a.y + a.h > b.y
}
```

Four comparisons. Can't get faster than that. Combined with spatial hash: first a coarse neighborhood search, then fine AABB check. Two-stage.

**Where:** Every physical interaction. Projectile hits enemy, player touches item, entity enters trigger zone.

---

## PATHFINDING

---

### 8. A* on Tile Grid

**Problem:** An NPC should find the shortest path through a tilemap, around obstacles.

**Solution:** A* on the tile grid. Each tile is a node, walkable neighbors are edges. Heuristic: Manhattan distance (or Chebyshev for diagonal movement).

```rust
fn a_star(start: TilePos, goal: TilePos, tilemap: &Tilemap) -> Option<Vec<TilePos>> {
    let mut open = BinaryHeap::new();  // Priority Queue
    let mut came_from = HashMap::new();
    let mut g_score = HashMap::new();  // cost from start

    open.push(Node { pos: start, f: heuristic(start, goal) });
    g_score.insert(start, 0);

    while let Some(current) = open.pop() {
        if current.pos == goal { return reconstruct_path(came_from, goal); }

        for neighbor in tilemap.walkable_neighbors(current.pos) {
            let tentative_g = g_score[&current.pos] + move_cost(current.pos, neighbor);
            if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                came_from.insert(neighbor, current.pos);
                g_score.insert(neighbor, tentative_g);
                open.push(Node { pos: neighbor, f: tentative_g + heuristic(neighbor, goal) });
            }
        }
    }
    None  // no path found
}
```

**Where:** RPG worlds (Meridian – NPCs navigate), dungeon crawler (Kabinett – enemies chase the player), survival (Knochenhain – enemies find the camp).

---

### 9. Waypoint Pathfinding

**Problem:** Tower Defense – enemies follow a fixed path. A* would be overkill.

**Solution:** Editor-defined points. Enemy follows the list, interpolates between points.

```rust
pub struct WaypointPath {
    pub points: Vec<SimVec2>,
}

pub struct PathFollower {
    pub path_index: usize,     // which path
    pub segment: usize,        // between point N and N+1
    pub progress: Fix,         // 0.0 = at point N, 1.0 = at point N+1
}

fn follow_path(follower: &mut PathFollower, path: &WaypointPath, speed: Fix, dt: Fix) {
    follower.progress += speed * dt / segment_length(path, follower.segment);
    while follower.progress >= Fix::ONE {
        follower.progress -= Fix::ONE;
        follower.segment += 1;
        if follower.segment >= path.points.len() - 1 {
            // Goal reached!
        }
    }
}
```

**Where:** TD (Rostgarten), any situation with fixed routes (patrols in stealth, NPC wandering routes).

---

### 10. Flow Field

**Problem:** 200 enemies should all head to the same goal. 200x A* per frame = too expensive.

**Solution:** Compute a direction field once (BFS from the goal). Each cell gets an arrow "go in this direction." Each enemy only reads its cell – O(1).

```
┌────┬────┬────┬────┐
│ ↘  │ →  │ →  │ ↓  │
├────┼────┼────┼────┤    Computation: BFS from goal (★)
│ ↓  │ ██ │ →  │ ↓  │    Each cell points to the neighbor
├────┼────┼────┼────┤    with the lowest cost
│ →  │ →  │ →  │ ★  │
└────┴────┴────┴────┘

Enemy at (0,0): reads ↘ → moves diagonally
```

Recomputation only when the tilemap changes or the goal changes. For static maps: compute once, use forever.

**Where:** Horde scenarios (Knochenhain – at night tree roots attack the camp), RTS-like situations, any scene with many entities heading to the same goal.

---

## RENDERING

---

### 11. Sprite Batcher (Texture Atlas Grouping)

**Problem:** Rendering 500 sprites = 500 draw calls. GPU overhead per draw call is high. Result: stuttering.

**Solution:** Collect all sprites, sort by texture atlas, ONE draw call per atlas.

```
Frame pipeline:
1. Collect all sprites: [(atlas_id, position, src_rect, z_index), ...]
2. Sort by atlas_id (secondary: z_index)
3. Per atlas group: a single vertex buffer upload + draw call

500 sprites, 3 atlases → 3 draw calls instead of 500
```

**Where:** EVERYTHING that is rendered. Entities, tiles, particles, UI – everything goes through the sprite batcher.

---

### 12. Per-Sprite Shaders

**Problem:** A hit enemy should flash white. A poisoned one should pulse greenish. An invisible one should flicker transparently. But the sprite batcher renders everything in one batch.

**Solution:** Shader ID as an attribute in the vertex buffer. The batcher also groups sprites by shader. Built-in shader set:

| Shader | Effect | Usage |
|--------|--------|-------|
| `default` | Normal rendering | Standard |
| `flash` | Briefly fully white | Hit feedback |
| `outline` | 1px colored outline | Selection, hover |
| `dissolve` | Pixels dissolve | Death animation |
| `palette_swap` | Swap colors | Team colors, variants |
| `silhouette` | Solid-color silhouette | Visible behind walls |
| `wave` | Wave-like distortion | Underwater, heat |

```rust
world.set(entity, SpriteEffect::Flash { duration: 0.1, color: Color::WHITE });
// → Sprite batcher detects the effect, batches with flash shader
```

**Where:** Combat feedback, status effects, visual highlighting, stealth (silhouette when behind cover).

---

### 13. Tilemap Chunk Caching

**Problem:** The tilemap has 500x500 tiles. Render all of them every frame? Wasteful.

**Solution:** The tilemap is divided into chunks (16x16 tiles). Only visible chunks are rendered. Visible chunks are cached into a texture – as long as the chunk doesn't change, only the cached texture is blitted (ONE quad instead of 256 tile draws).

```
Chunk cache:
┌────────┬────────┬────────┐
│Chunk(0,0)│Chunk(1,0)│Chunk(2,0)│  Visible: green
│ CACHED │ CACHED │  DIRTY │  Dirty: red (tile changed → re-render)
├────────┼────────┼────────┤
│Chunk(0,1)│Chunk(1,1)│Chunk(2,1)│  Not visible: don't load at all
│ CACHED │ CACHED │ CACHED │
├────────┼────────┼────────┤
│  OUT   │  OUT   │  OUT   │  OUT = outside viewport
└────────┴────────┴────────┘
```

**Where:** Every tilemap. Especially important for large worlds (Kabinett dungeon, Sporenwolke overworld).

---

### 14. Animated Tiles

**Problem:** Water should flow, lava should pulse, grass should sway. But tiles are static graphics.

**Solution:** Certain tile IDs have an `animation` tag. The engine swaps the tile graphic at regular intervals. The chunk cache is invalidated accordingly.

```ron
// tileset definition
(
    tiles: {
        42: (name: "water", walkable: false, animation: (
            frames: [42, 43, 44, 45],
            frame_duration: 0.25,  // seconds per frame
        )),
    },
)
```

**Where:** Water, lava, torches, glowing crystals, spore rain, anything that "lives" on the tilemap.

---

### 15. Auto-Tiling (Bitmask)

**Problem:** Water next to land needs transition tiles (shores, corners). 47 possible variants to place manually?

**Solution:** Each tile checks its 8 neighbors, computes a bitmask, and looks up the matching sprite in a lookup table.

```
Neighbor bitmask:                 Example:
┌───┬───┬───┐                 Land│Land│Land
│ 1 │ 2 │ 4 │                 ────┼────┼────
├───┼───┼───┤                 Water│ X │Land  → Bitmask = 2+4+16 = 22
│ 8 │ X │16 │                 ────┼────┼────
├───┼───┼───┤                 Water│Water│Water
│32 │64 │128│
└───┴───┴───┘                 Tile 22 → shore top-right
```

**Where:** Level editor (auto-tiling while painting), tilemap loading. Works for water, paths, walls, cliffs, elevation levels.

---

### 16. Post-Processing Stack

**Problem:** Different worlds need different visual effects. Caribbean = Bloom + Warm Color Grade. Matrix = Chromatic Aberration + CRT Scanlines. Switch at runtime.

**Solution:** Post-processing as `Vec<PostEffect>` per world/scene. Configurable in RON. The engine renders the scene into a texture, then applies each effect as a fullscreen pass on top.

```ron
// data/atmospheres/my_world.ron
(
    "normal": (
        post_effects: [
            Bloom(threshold: 0.8, intensity: 0.3),
            ColorGrade(lut: "luts/warm_sunset.png"),
            Vignette(strength: 0.2),
        ],
    ),
    "boss": (
        post_effects: [
            ChromaticAberration(offset: 2.0),
            Bloom(threshold: 0.6, intensity: 0.5),
            ColorGrade(lut: "luts/red_danger.png"),
            ScreenShake(intensity: 0.5),
        ],
    ),
)
```

Atmosphere transitions interpolate between post-processing stacks (bloom ramps up, vignette ramps down, over 2 seconds).

**Where:** Mood changes (calm → battle), world-specific look, boss encounters, cutscenes.

---

### 17. Dual-Layer Rendering (Game + Editor)

**Problem:** The game renders at 640x360 (pixel art). The editor needs sharp text and UI at native resolution (e.g. 2560x1440).

**Solution:** Two separate render targets:

```
1. Off-screen texture (640×360) → Game world + pixel UI
   └── Integer-scaled into a viewport in the editor window

2. Backbuffer (native resolution) → egui editor panels
   └── Text, thumbnails, dropdowns, properties – all sharp
```

In play mode: only target 1, fullscreen. In editor mode: target 1 as a panel, target 2 around it.

**Where:** Always. This is the fundamental render architecture.

---

## AUDIO

---

### 18. Adaptive Music – Vertical Layering

**Problem:** Music should change with the gameplay. But a simple track switch (crossfade) sounds like two different songs.

**Solution:** Multiple stems (drums, bass, melody, strings, brass) play simultaneously, in sync. Each stem has a volume controlled by game parameters.

```
Tension = 0.2 (calm):         Tension = 0.8 (combat):
  Drums:    ░░░░░░ (quiet)      Drums:    ██████ (loud)
  Bass:     ░░░░░░ (quiet)      Bass:     █████░ (loud)
  Melody:   ░░░░░░ (off)        Melody:   ████░░ (medium)
  Strings:  ████░░ (medium)     Strings:  ██████ (loud)
  Brass:    ░░░░░░ (off)        Brass:    ████░░ (medium)
```

Fades are smooth (lerp per frame). The piece always sounds like one song – only the density changes.

**Where:** Every world with adaptive music. Parameters: Tension, Danger, Boss, Victory.

---

### 19. Bar-Synced Music Transitions

**Problem:** Music should switch from "calm" to "battle." But a crossfade in the middle of a bar sounds terrible.

**Solution:** BarClock tracks the current position in the bar. Transitions are marked as "pending" and only executed at the next bar boundary.

```
Beat: 1 . . . 2 . . . 3 . . . 4 . . . | 1 . . . 2 . . .
                            ↑                ↑
                     Boss spawns!         This is where the
                     → Pending            switch happens (on the 1)
```

**Transition types:**
- `CrossfadeOnBar(2)` – crossfade over 2 bars
- `CutOnBar` – hard cut on the 1
- `StingerThen(sound, next)` – short accent, then transition
- `FadeOutThenPlay(1)` – fade out over 1 bar, silence, new piece
- `LayerSwap(2)` – swap one layer every 2 bars

---

### 20. Stinger Quantization

**Problem:** A tower is built → short musical accent (stinger). But the stinger should fit musically with the running beat, not be slapped on arbitrarily.

**Solution:** Stingers have a quantization level:

```rust
pub enum StingerQuantize {
    Immediate,    // play immediately (for urgent events like life lost)
    NextBeat,     // on the next beat (for small events: tower built)
    NextBar,      // on the next 1 (for big events: boss spawns)
}
```

**Where:** All gameplay events that have an audio accent. Tower build = NextBeat. Wave start = NextBar. Life lost = Immediate.

---

### 21. SFX Variant System

**Problem:** The same cannon shot 50 times in a row sounds like a broken record player.

**Solution:** Multiple variants per sound. Engine picks randomly + slight pitch variation.

```ron
(
    "impact_01": (
        files: ["sfx/impact_01a.ogg", "sfx/impact_01b.ogg", "sfx/impact_01c.ogg"],
        volume: 0.8,
        pitch_variance: 0.05,   // ±5% random pitch shift
        max_concurrent: 3,       // maximum 3 simultaneous
        cooldown: 0.05,          // minimum 50ms between plays
    ),
)
```

**Where:** Every repeating SFX. Shots, footsteps, hits, UI clicks.

---

## DETERMINISM & NETWORK

---

### 22. Fixed-Point Arithmetic

**Problem:** `f32` is not deterministic across CPUs. Multiplayer and replays desync after 1000+ frames.

**Solution:** `I16F16` (16-bit integer + 16-bit fraction) for all simulation. Integer operations are identical on every CPU.

```
I16F16: Value 3.75 = 0000000000000011.1100000000000000
                      ← 16-bit integer → ← 16-bit frac →

Addition = integer addition → deterministic
Multiplication = integer mult + shift → deterministic
```

**Where:** EVERY gameplay calculation: position, velocity, damage, timers, cooldowns. Rendering may continue to use f32 (only visual, not simulation-relevant).

---

### 23. Seeded RNG

**Problem:** Random values in gameplay (damage spread, spawn variance, particles) must be reproducible.

**Solution:** The RNG is part of the GameState, initialized with a seed. Every random value comes from the same deterministic generator.

```rust
pub struct GameState {
    pub rng: StdRng,          // Seeded, deterministic
    // ... everything else
}

// ALWAYS: state.rng.gen_range(0..100)
// NEVER:  rand::thread_rng()  ← not reproducible!
```

**Replay:** Same seed + same commands → exactly the same simulation. Even 10 years later, on different hardware.

---

### 24. No HashMap Iteration in Simulation

**Problem:** `HashMap::iter()` returns elements in undefined order. On different machines the order can differ → determinism broken.

**Solution:** Simulation uses `BTreeMap` (sorted) or `IndexMap` (insertion order). Iteration is always in the same order. `FxHashMap` (rustc-hash) is used for O(1) lookups where iteration is not needed.

```rust
// Good (deterministic):
let towers: BTreeMap<EntityId, TowerData> = ...;
for (id, tower) in &towers { ... }  // always sorted by ID

// Bad (non-deterministic):
let towers: HashMap<EntityId, TowerData> = ...;
for (id, tower) in &towers { ... }  // order varies!
```

**Where:** Every simulation loop that iterates over entities.

---

### 25. Command-Based Architecture

**Problem:** In multiplayer, Client A sends "place tower at (5,3)." Client B must do exactly the same. Direct ECS manipulation is not serializable.

**Solution:** All player actions as serializable `GameCommand` enums. Commands are sent over the network. The simulation executes commands – not raw input.

```rust
pub enum GameCommand {
    PlaceTower { x: Fix, y: Fix, tower_type: TowerId },
    SellTower { tower_id: EntityId },
    SetTargetPriority { tower_id: EntityId, priority: TargetPriority },
    StartWave,
    ActivateAbility { ability: AbilityId },
    // ... all player actions
}

// Replay = Vec<(Tick, GameCommand)>
// Multiplayer = send commands over UDP
// Undo = execute command in reverse
```

**Where:** Every player interaction. Replay system, multiplayer, undo/redo in the editor.

---

### 26. Lockstep Multiplayer

**Problem:** Two players play together. The simulation must be identical on both sides.

**Solution:** Lockstep protocol. Both clients run exactly in sync:

```
Tick 100:
  Client A sends: [PlaceTower(5,3)]     → to Client B
  Client B sends: [StartWave]            → to Client A

  Both wait until they have the other's commands.

  Then: both simulate tick 100 with [PlaceTower(5,3), StartWave]
  → identical result (thanks to fixed-point + seeded RNG + ordered iteration)
```

Desync detection via CRC: both clients compute a checksum over the GameState. Difference → desync warning.

**Where:** Co-op multiplayer. Works because the entire simulation is deterministic.

---

## MEMORY & PERFORMANCE

---

### 27. Object Pool (Particles)

**Problem:** Explosion spawns 200 particles, next frame all gone, 150 new ones. Hundreds of allocations per frame → allocator suffers, memory fragments.

**Solution:** Pre-allocated pool. Slots are activated/deactivated, never allocated/freed.

```rust
pub struct ParticlePool {
    particles: Vec<Particle>,      // fixed allocation at startup (e.g. 1000)
    active: BitSet,                 // which slots are active
    first_free: usize,              // next free slot (linked list through free slots)
}

fn spawn(&mut self) -> Option<&mut Particle> {
    if self.first_free < self.particles.len() {
        let idx = self.first_free;
        self.active.set(idx, true);
        self.first_free = self.particles[idx].next_free;
        Some(&mut self.particles[idx])
    } else { None }  // pool full
}

fn despawn(&mut self, idx: usize) {
    self.active.set(idx, false);
    self.particles[idx].next_free = self.first_free;
    self.first_free = idx;
}
```

**Where:** Particles, projectiles, floating text (damage numbers), temporary effects.

---

### 28. Capacity Hints (Pre-Allocation)

**Problem:** A SparseSet grows dynamically. Each `Vec::push()` can trigger a reallocation (expensive, copies everything).

**Solution:** Specify the expected entity count when creating a SparseSet. One-time pre-allocation at startup.

```rust
let mut positions = SparseSet::<Position>::with_capacity(1000);
let mut velocities = SparseSet::<Velocity>::with_capacity(1000);
let mut sprites = SparseSet::<SpriteComp>::with_capacity(1000);
// → 0 reallocations during the entire game
```

**Where:** All SparseSet instances, all Vec-based systems, particle pools.

---

### 29. Arena Allocator (Bumpalo)

**Problem:** Many small, temporary data items are created per frame (render commands, event lists, debug strings). `malloc`/`free` for each one → slow.

**Solution:** Bumpalo arena: one large memory block, allocations just "bump" a pointer forward. At frame end: the entire arena is reset in one step.

```rust
let arena = Bump::new();

// Frame start:
let render_cmds = arena.alloc_slice_fill_default::<RenderCmd>(500);
let events = bumpalo::vec![in &arena; Event::default(); 100];
// ... use render_cmds and events ...

// Frame end:
arena.reset();  // ONE pointer reset, done. No individual frees.
```

**Where:** Temporary per-frame data: render lists, event queues, debug output, spatial hash rebuild.

---

### 30. Memory Debug Overlay

**Problem:** Where is the memory going? Are there leaks? Is VRAM growing?

**Solution:** Debug overlay (F6) shows live: RAM usage, VRAM usage, entity count per type, particle pool utilization, texture atlas size.

```
┌─ Memory ──────────────────┐
│ RAM:   142 MB / 512 MB    │
│ VRAM:   48 MB / 2048 MB   │
│ Entities: 347             │
│   Position: 347           │
│   Velocity: 298           │
│   Sprite:   312           │
│ Particles: 84/1000 (8%)   │
│ Atlases: 3 (12 MB)        │
│ Audio: 8 MB               │
└───────────────────────────┘
```

**Where:** Debug mode. Updated every frame. Leak detection: if entity count steadily grows without a state change → warning in the console.

---

## INPUT

---

### 31. Action-Based Input (Abstraction Layer)

**Problem:** The player presses "W" on the keyboard, "Up" on the D-Pad, or pushes the left stick. All should mean "move up." And the player may want to rebind.

**Solution:** Abstraction layer. The game never asks for specific keys, but for actions.

```rust
// The game asks:
if engine.input().held(Action::MoveUp) { ... }
if engine.input().just_pressed(Action::Confirm) { ... }

// The mapping comes from input.ron:
(
    actions: {
        "move_up": [Key(W), Key(Up), GamepadAxis(LeftStickY, Negative)],
        "confirm": [Key(Space), Key(Enter), GamepadButton(South)],
    },
)
```

Hot-reloadable. Player can rebind in the settings menu. Gamepad support for free.

---

### 32. Gamepad Hot-Plug

**Problem:** Player plugs controller in/out while the game is running.

**Solution:** Engine fires events: `GamepadConnected(id)` / `GamepadDisconnected(id)`. The game decides what happens (pause? controller selection screen? ignore?).

```rust
for event in engine.events::<GamepadEvent>() {
    match event {
        GamepadConnected(id) => show_toast("Controller connected!"),
        GamepadDisconnected(id) => pause_game(),
    }
}
```

**Where:** Every platform. Especially important for couch gaming.

---

## ASSET MANAGEMENT

---

### 33. Dual Asset Loader

**Problem:** During development you want to load sprites directly from Aseprite files (hot reload!). For release you want everything in a packed archive (faster, smaller, tamper-proof).

**Solution:** Two loaders behind the same API:

```
Dev mode:                         Release mode:
  assets/sprites/hero.aseprite      game.pak (archive)
  assets/sprites/tiles.png          ├── textures.atlas (packed)
  assets/data/player.ron            ├── data.bin (serialized)
  → Load directly from disk         └── audio.bin (compressed)
  → Hot reload on change            → Load once at startup
```

```rust
// Game code: identical in both modes
let sprite = engine.assets().load_sprite("sprites/hero");
let data: PlayerStats = engine.assets().load_ron("data/player.ron");
// → The loader decides whether from disk or from .pak
```

**Where:** Everywhere. `amigo pack` CLI command packs everything for release.

---

### 34. Hot Reload (File Watcher)

**Problem:** Sprite changed → Alt-Tab → restart game → wait 30 seconds → see result. Creativity killer.

**Solution:** `notify` crate watches the assets directory. On file change: reload asset, replace in game. Without restarting.

```
1. Artist changes hero.aseprite in Aseprite, saves
2. notify fires FileChanged("assets/sprites/hero.aseprite")
3. Engine parses the file again
4. Sprite handle points to new data
5. Next frame: new sprite visible

Total time: < 100ms
```

Works for: sprites, tilemaps, RON files (stats, configs), audio, shaders. Not in release mode (no file watcher needed when everything is in .pak).

**Where:** Development. Especially powerful with Art Studio: sprite generated → into assets folder → immediately visible in game.

---

### 35. Synchronous Loading (Cartridge Style)

**Problem:** Asynchronous asset loading is complex (futures, loading states, placeholder textures). For a pixel art game with small assets: overkill.

**Solution:** Assets are loaded synchronously at startup. Like a game console cartridge: everything there, immediately available. Async only at level transitions (where a loading screen is shown anyway).

```rust
// Startup: synchronous, blocking
let assets = engine.assets().load_all("assets/");  // load everything, done

// Level transition: async with loading screen
engine.load_async("levels/world_3/", |progress| {
    render_loading_screen(progress);  // 0% ... 50% ... 100%
});
```

**Where:** Startup, level transitions. In the game loop: no load operations, everything immediately available.

---

## SAVE & REPLAY

---

### 36. Save-Slot System

**Problem:** Player wants multiple save files. Autosave should not overwrite the manual save. Corrupted saves should be detected.

**Solution:** Slot-based system with metadata, compression, and integrity checking.

```rust
pub struct SaveSlot {
    pub slot_id: u8,
    pub metadata: SlotInfo,     // Timestamp, play time, label – WITHOUT loading the save
    pub data: Vec<u8>,          // LZ4-compressed GameState
    pub crc: u32,               // Corruption check
}
```

- **Autosave:** Rotating N slots (Autosave_1, Autosave_2, ...) at configurable interval
- **Quicksave/Quickload:** F5/F9
- **Platform-aware:** Windows `AppData`, Linux `~/.local/share`
- **SlotInfo:** Readable without loading the entire save → fast slot overview in the menu

---

### 37. Replay System

**Problem:** "How did the player beat the level?" for debugging, sharing, leaderboards.

**Solution:** Replays = list of `(Tick, GameCommand)`. Playback = fresh GameState + feed in commands.

```rust
pub struct Replay {
    pub seed: u64,                          // RNG seed
    pub commands: Vec<(u64, GameCommand)>,   // (Tick, Command)
}

// Recording:
replay.commands.push((current_tick, command.clone()));

// Playback:
let mut state = GameState::new(replay.seed);
for (tick, cmd) in &replay.commands {
    while state.tick < *tick { state.simulate_tick(); }
    state.execute_command(cmd);
}
```

Works thanks to determinism (fixed-point + seeded RNG + ordered iteration). Replay files are tiny: only commands, no full state.

---

## DEBUG

---

### 38. Visual Debug Layers (F-Keys)

**Problem:** Where are the collision boxes? Where do the paths run? Why isn't the tower shooting?

**Solution:** F-key toggles for visual debug overlays:

| Key | Overlay | Shows |
|-----|---------|-------|
| F1 | HUD | FPS, entity count, draw calls, memory |
| F2 | Grid | Tile grid lines |
| F3 | Collision | AABB boxes of all colliders |
| F4 | Pathfinding | A* paths, flow fields, waypoints |
| F5 | Spawn/Build Zones | Where entities can spawn / be placed |
| F6 | Memory | RAM/VRAM, pool utilization, entity types |
| F7 | Entity List | All entities with components |
| F8 | Network | Lockstep stats, latency, desync warnings |

All behind `#[cfg(debug_assertions)]` – don't exist in the release build.

---

### 39. Tracy Integration

**Problem:** "The game stutters at wave 5 with 200 enemies." Where exactly is the bottleneck?

**Solution:** Tracy profiler integration via `tracing` + `tracy-client`. Every system, every render pass, every heavy operation is instrumented.

```rust
#[tracing::instrument]
fn physics_system(world: &mut World) {
    // ... Tracy sees: physics_system takes 2.3ms
}

#[tracing::instrument]
fn render_entities(renderer: &mut Renderer) {
    // ... Tracy sees: render_entities takes 1.1ms
}
```

Tracy shows: timeline of all systems, CPU flamegraph, memory allocations, GPU timings. Gold standard for game performance analysis.

---

### 40. State Snapshot to File

**Problem:** "There's a bug at wave 7 when the player has 3 cannon towers." How to reproduce?

**Solution:** Dump the complete GameState as a RON file at any time. Load → continue playing from exactly that point.

```bash
# In-game: press F10
# → saves debug_snapshot_2026-03-15_14-23-01.ron

# Claude Code or developer:
amigo run --load-snapshot debug_snapshot_2026-03-15_14-23-01.ron
# → Game starts in exactly this state
```

**Where:** Bug reports, AI playtesting (Claude Code takes snapshot → analyzes → changes code → loads snapshot).

---

## SPECIAL FEATURES

---

### 41. Event System (Double Buffer)

**Problem:** System A fires event → System B should react. But System B already ran BEFORE System A → doesn't see the event.

**Solution:** Two vectors per event type. Write buffer (current tick) and read buffer (previous tick). At tick end: swap.

```
Tick 5:
  Write: [EnemyDied(42)]       ← systems write here
  Read:  [WaveStarted]          ← systems read here (events from tick 4)

End of tick 5:  Swap!

Tick 6:
  Write: (empty)
  Read:  [EnemyDied(42)]        ← now visible to all
```

Events live exactly 1 tick for reading. 1 tick delay (~16ms) – imperceptible. No race conditions.

---

### 42. Atmosphere System (Smooth Interpolation)

**Problem:** The lighting mood should change when a boss spawns. An abrupt switch is noticeable.

**Solution:** `atmosphere.transition_to("boss", duration)` starts an interpolation. All atmosphere parameters (light, color, weather intensity, post effects, music) are smoothly blended over `duration` seconds.

```rust
pub struct Atmosphere {
    current: AtmosphereState,
    target: Option<(AtmosphereState, f32, f32)>,  // (target, duration, progress)
}

fn update_atmosphere(atm: &mut Atmosphere, dt: f32) {
    if let Some((target, duration, progress)) = &mut atm.target {
        *progress += dt / *duration;
        atm.current = AtmosphereState::lerp(&atm.current, target, *progress);
        if *progress >= 1.0 { atm.target = None; }
    }
}
```

**Where:** World mood (calm → storm), boss encounters, day/night cycles, dimension changes.

---

### 43. Scene Stack

**Problem:** Gameplay is running → pause menu opens → the gameplay should remain "frozen" in the background, not be despawned.

**Solution:** Scenes as a stack. The topmost scene is active. Scenes below are paused but still there.

```
Stack:
  ┌────────────┐
  │ Pause Menu │  ← active, renders on top of gameplay
  ├────────────┤
  │ Gameplay   │  ← paused, not despawned, still rendered (dimmed)
  ├────────────┤
  │ (base)     │
  └────────────┘

// Push → Pause opens on top of gameplay
// Pop → back to gameplay, exactly where it was
```

**Where:** Pause, inventory overlay, dialogue over gameplay, cutscene overlay.

---

### 44. Screen Transitions (Shader-based)

**Problem:** World change: the screen shouldn't just hard-cut.

**Solution:** Transition effects as post-processing shaders. The current scene renders into texture A, the new one into texture B, the transition shader blends them.

```rust
pub enum Transition {
    Fade { duration: f32, color: Color },           // Fade to black
    Dissolve { duration: f32, noise: TextureId },    // Pixels dissolve
    Wipe { duration: f32, direction: Direction },    // Slide
    Circle { duration: f32, center: Vec2 },          // Circle opens/closes
    VHSStatic { duration: f32 },                     // For Threadwalker dimension flip
    Custom { shader: ShaderId, duration: f32 },      // Custom WGSL
}
```

**Where:** World change (Loom → World), cutscene transitions, death/respawn, dimension flip (Stranger Things → VHS Static).

---

*This document is a reference guide. All techniques are anchored in the engine spec (amigo-engine-complete.md); here they are explained and illustrated.*
