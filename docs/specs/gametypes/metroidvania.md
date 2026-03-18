---
status: spec
crate: --
depends_on: ["engine/minimap", "engine/fog-of-war"]
last_updated: 2026-03-18
---

# Metroidvania

## Purpose

Interconnected exploration games with ability-gated progression, non-linear world traversal, and incremental map reveal. The player unlocks movement abilities that open previously inaccessible paths, encouraging backtracking through a large, continuous world composed of discrete rooms.

Examples: Hollow Knight (exploration + combat + charms), Metroid Dread (EMMI zones + ability gating), Ori and the Blind Forest (movement mastery + emotional narrative).

## Public API

### ExplorationGraph

```rust
/// The world as a graph of interconnected rooms.
/// Each node is a room (a distinct camera region), edges are doorways/passages.
pub struct ExplorationGraph {
    rooms: FxHashMap<RoomId, RoomNode>,
    connections: Vec<RoomConnection>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub u32);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomNode {
    pub id: RoomId,
    /// World-space bounding rect for this room (used by Camera::RoomTransition).
    pub bounds: Rect,
    /// Display name shown on map (e.g. "Forgotten Crossroads").
    pub area_name: String,
    /// Which area/zone this room belongs to (for map coloring).
    pub zone: ZoneId,
    /// Room-specific camera mode override (None = default RoomTransition).
    pub camera_override: Option<CameraMode>,
    /// Whether this room has been visited by the player.
    pub discovered: bool,
    /// Optional checkpoint (save room) data.
    pub checkpoint: Option<CheckpointData>,
    /// Optional boss encounter data.
    pub boss: Option<BossData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomConnection {
    pub from: RoomId,
    pub to: RoomId,
    /// World-space trigger zone for the doorway.
    pub trigger_bounds: Rect,
    /// Ability required to pass (None = always open).
    pub gate: Option<AbilityGate>,
    /// Whether this connection is one-way (e.g. drop-down ledge).
    pub one_way: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ZoneId(pub u16);

impl ExplorationGraph {
    pub fn new() -> Self;
    pub fn add_room(&mut self, room: RoomNode);
    pub fn connect(&mut self, connection: RoomConnection);
    pub fn room(&self, id: RoomId) -> Option<&RoomNode>;
    pub fn room_mut(&mut self, id: RoomId) -> Option<&mut RoomNode>;
    /// All rooms adjacent to `id`, optionally filtered by reachability.
    pub fn neighbors(&self, id: RoomId) -> Vec<(RoomId, &RoomConnection)>;
    /// All rooms reachable from `start` given a set of unlocked abilities.
    pub fn reachable_rooms(&self, start: RoomId, abilities: &AbilitySet) -> Vec<RoomId>;
    /// Rooms that are discovered but have at least one gated exit the player cannot pass yet.
    pub fn backtrack_candidates(&self, abilities: &AbilitySet) -> Vec<(RoomId, AbilityGate)>;
}
```

### AbilityGating

```rust
/// A movement ability that gates world progression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ability {
    DoubleJump,
    Dash,
    WallJump,
    WallSlide,
    Grapple,
    Swim,
    SuperJump,
    PhaseDoor,
}

/// A gate condition — which ability (or combination) is required.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AbilityGate {
    /// Single ability required.
    Single(Ability),
    /// All listed abilities required.
    All(Vec<Ability>),
    /// Any one of the listed abilities suffices.
    Any(Vec<Ability>),
    /// Gate opened by defeating a specific boss.
    BossDefeated(BossId),
    /// Gate opened by possessing a key item.
    HasItem(String),
}

/// Tracks which abilities the player has unlocked.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AbilitySet {
    unlocked: FxHashSet<Ability>,
}

impl AbilitySet {
    pub fn unlock(&mut self, ability: Ability);
    pub fn has(&self, ability: Ability) -> bool;
    pub fn satisfies(&self, gate: &AbilityGate) -> bool;
}
```

### SkillUnlockSystem

```rust
/// Manages ability progression and unlock events.
pub struct SkillUnlockSystem {
    abilities: AbilitySet,
    /// Optional prerequisite tree: ability X requires ability Y first.
    prerequisites: FxHashMap<Ability, Vec<Ability>>,
}

impl SkillUnlockSystem {
    pub fn new() -> Self;
    /// Register a prerequisite chain (e.g. SuperJump requires DoubleJump + WallJump).
    pub fn add_prerequisite(&mut self, ability: Ability, requires: Vec<Ability>);
    /// Attempt to unlock an ability. Returns false if prerequisites not met.
    pub fn try_unlock(&mut self, ability: Ability) -> bool;
    /// Force-unlock an ability (ignoring prerequisites, e.g. debug or cutscene).
    pub fn force_unlock(&mut self, ability: Ability);
    pub fn abilities(&self) -> &AbilitySet;
}
```

### MapRevealer

```rust
/// Integrates with Minimap and FogOfWarGrid to reveal rooms on entry.
pub struct MapRevealer {
    /// Fog grid used to shroud undiscovered rooms.
    fog: FogOfWarGrid,
    /// Minimap pins for points of interest.
    pins: Vec<MinimapPin>,
}

impl MapRevealer {
    pub fn new(graph: &ExplorationGraph) -> Self;
    /// Called when player enters a room. Reveals the room on the minimap
    /// and clears fog for the room's tile region.
    pub fn on_room_enter(&mut self, room: &RoomNode, graph: &ExplorationGraph);
    /// Adds a map pin (save room icon, boss icon, shop icon, etc.).
    pub fn add_pin(&mut self, pin: MinimapPin);
    /// Returns all backtrack markers — rooms with gated exits the player
    /// has seen but cannot yet reach.
    pub fn backtrack_pins(&self, graph: &ExplorationGraph, abilities: &AbilitySet) -> Vec<MinimapPin>;
}
```

### BacktrackMarker

```rust
/// Visual marker on the minimap for areas the player cannot yet access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktrackMarker {
    pub room: RoomId,
    pub connection: usize,  // index into ExplorationGraph::connections
    pub required: AbilityGate,
    /// Player-placed custom marker (e.g. pin color).
    pub custom_pin: Option<PinType>,
}
```

### CheckpointSystem

```rust
/// Save rooms that heal and act as respawn points.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointData {
    /// Does resting here refill health?
    pub heals: bool,
    /// Does resting here refill secondary resource (mana, energy)?
    pub restores_resource: bool,
    /// Does resting here respawn enemies in the world?
    pub respawns_enemies: bool,
}

pub struct CheckpointSystem {
    last_checkpoint: Option<RoomId>,
}

impl CheckpointSystem {
    pub fn new() -> Self;
    /// Rest at a checkpoint. Triggers SaveManager::save_to_slot(), heals player,
    /// optionally respawns enemies.
    pub fn rest(&mut self, room: RoomId, data: &CheckpointData, save: &mut SaveManager);
    /// Respawn at last checkpoint after death.
    pub fn respawn_point(&self) -> Option<RoomId>;
}
```

### BossRoom

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BossId(pub u32);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BossData {
    pub boss_id: BossId,
    /// Camera locks to BossArena mode during fight.
    pub arena_bounds: Rect,
    /// Door entities that seal during the fight.
    pub seal_doors: Vec<EntityId>,
    /// Has this boss been defeated?
    pub defeated: bool,
}

/// Manages boss encounter lifecycle.
pub struct BossRoomSystem;

impl BossRoomSystem {
    /// Trigger boss fight: seal doors, switch camera to BossArena, start boss AI.
    pub fn enter_fight(
        boss: &mut BossData,
        camera: &mut Camera,
        doors: &mut [(EntityId, &mut CollisionShape)],
    );
    /// End boss fight: unseal doors, mark defeated, switch camera back to RoomTransition.
    pub fn end_fight(
        boss: &mut BossData,
        camera: &mut Camera,
        doors: &mut [(EntityId, &mut CollisionShape)],
    );
}
```

## Behavior

- **Room Transitions**: When the player crosses a `RoomConnection` trigger zone, the camera performs a `RoomTransition` (slide from current room bounds to next room bounds). During the transition, player input is suppressed and entities in the old room are deactivated (LOD). The `ExplorationGraph` marks the new room as `discovered = true`.
- **Ability Gating**: Each `RoomConnection` may have an `AbilityGate`. When the player approaches a gated passage without the required ability, a visual indicator appears (e.g. colored barrier matching the ability). The connection's trigger zone is blocked by a `CollisionShape(Aabb)` that is removed once the gate is satisfied.
- **Map Reveal**: On first room entry, `MapRevealer::on_room_enter()` clears the `FogOfWarGrid` tiles within the room's bounds (setting them from `Hidden` to `Visible`) and adds the room outline to the `Minimap`. Adjacent rooms appear as silhouettes (fog `Explored` state) to hint at their existence.
- **Backtrack Markers**: After entering a room with gated exits, `ExplorationGraph::backtrack_candidates()` identifies unreachable connections. These appear as colored icons on the minimap via `BacktrackMarker`. When the player later unlocks the required ability, markers for newly reachable areas pulse or change color.
- **Checkpoints**: Save rooms contain a `CheckpointData` trigger. Interacting with it calls `CheckpointSystem::rest()`, which invokes `SaveManager::save_to_slot()` for the active slot, restores health/resources, and optionally respawns cleared enemies via a global enemy-respawn flag.
- **Boss Encounters**: Entering a boss room triggers `BossRoomSystem::enter_fight()`. The camera switches to `BossArena` mode (locked to `arena_bounds`), door `CollisionShape`s become `Solid` (sealing the room), and the boss entity's `BehaviorTree` is activated. On boss death, doors unseal, `BossData::defeated` is set, and any `AbilityGate::BossDefeated` conditions referencing this boss are now satisfied.
- **Animation Integration**: Ability unlock sequences use `AnimPlayer` with Aseprite tags for pickup animations. `TweenSequence` drives the camera zoom-in on ability shrines. `CinematicPan` can showcase the newly opened path.

## Internal Design

- `ExplorationGraph` is serialized as RON and loaded at game start. Runtime mutations (discovered flags, boss defeated) are saved via `SaveManager`.
- Room bounds define `Camera::RoomTransition` regions directly — no separate camera-room mapping needed.
- `FogOfWarGrid` is shared between `MapRevealer` and the main render pipeline. Room reveal sets tile visibility for the room's bounding rect in a single batch operation.
- `Minimap` uses `PinType::Sprite` for save room icons, boss icons, and shop icons. `PinType::Dot` for the player position. `BacktrackMarker` custom pins use `PinType::Arrow` with ability-specific colors.
- Gate collision shapes are stored as regular ECS entities with a `GateTag` component. When `AbilitySet` changes, a system iterates all `GateTag` entities and removes those whose `AbilityGate` is now satisfied.
- Boss AI uses `BehaviorTree` with `Blackboard` storing health phase thresholds. Phase transitions trigger `AnimPlayer` tag switches and `BulletEmitter` pattern changes.

## Non-Goals

- **Procedural room generation.** Metroidvania worlds are hand-crafted. Procedural generation is covered by [gametypes/roguelike](roguelike.md).
- **Multiplayer exploration.** Single-player only. Shared-world multiplayer fog is out of scope.
- **Ability combat effects.** This spec covers movement abilities for gating. Offensive upgrades (spell system, weapon combos) are game-specific and not part of the engine gametype.
- **In-game map editor.** The exploration graph is authored externally (RON files or editor tool). Runtime map editing is not supported.

## Open Questions

- Should the `ExplorationGraph` support conditional connections beyond abilities (e.g. quest flags, NPC state)?
- How should warp points (fast travel between save rooms) integrate — as a special `RoomConnection` type or a separate system?
- Should `MapRevealer` support purchasable map items that reveal an entire zone at once?
- How detailed should the minimap room outlines be — bounding rect only, or tile-accurate room shape?

## Referenzen

- Hollow Knight: Exploration graph, ability gating (Monarch Wings, Mantis Claw), map reveal via Cornifer
- Metroid Dread: EMMI zones as special gated areas, boss-gated progression
- Ori and the Blind Forest: Fluid movement abilities, save shrines
- [engine/minimap](../engine/minimap.md) → Exploration map with room reveal and backtrack pins
- [engine/fog-of-war](../engine/fog-of-war.md) → Room shroud via FogOfWarGrid / TileVisibility
- [engine/camera](../engine/camera.md) → RoomTransition and BossArena camera modes
- [engine/physics](../engine/physics.md) → Gate collision shapes (Aabb) and rigid body
- [engine/tween](../engine/tween.md) → Ability unlock camera zoom, door animations
- [engine/behavior-tree](../engine/behavior-tree.md) → Boss AI behavior trees
- [engine/save-load](../engine/save-load.md) → Checkpoint persistence via SaveManager
- [engine/animation](../engine/animation.md) → Ability pickup and boss phase animations
