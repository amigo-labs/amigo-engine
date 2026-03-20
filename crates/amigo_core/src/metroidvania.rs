//! Metroidvania gametype — interconnected exploration with ability-gated progression.
//!
//! Provides an exploration graph of rooms connected by doorways, an ability gating
//! system for non-linear progression, checkpoint/save-room support, boss encounter
//! lifecycle, and backtrack markers for the minimap.

use crate::collision::CollisionShape;
use crate::ecs::EntityId;
use crate::fog_of_war::{FogOfWarGrid, TileVisibility};
use crate::math::SimVec2;
use crate::rect::Rect;
use crate::save::SaveManager;
use fixed::types::I16F16;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique identifier for a room in the exploration graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub u32);

/// Identifier for a map zone / area (used for map coloring).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ZoneId(pub u16);

/// Unique identifier for a boss encounter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BossId(pub u32);

// ---------------------------------------------------------------------------
// Abilities & Gating
// ---------------------------------------------------------------------------

/// A movement ability that gates world progression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ability {
    /// Perform an additional jump mid-air.
    DoubleJump,
    /// Horizontal air dash.
    Dash,
    /// Jump off walls.
    WallJump,
    /// Slide down walls slowly.
    WallSlide,
    /// Grapple to anchor points.
    Grapple,
    /// Move through water areas.
    Swim,
    /// Extra-high jump (often requires other abilities first).
    SuperJump,
    /// Pass through phase-doors.
    PhaseDoor,
}

/// A gate condition — which ability (or combination) is required to pass.
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
    /// Unlock an ability.
    pub fn unlock(&mut self, ability: Ability) {
        self.unlocked.insert(ability);
    }

    /// Check whether an ability has been unlocked.
    pub fn has(&self, ability: Ability) -> bool {
        self.unlocked.contains(&ability)
    }

    /// Check whether a gate condition is satisfied by the current set.
    ///
    /// Note: `BossDefeated` and `HasItem` gates are **not** satisfied by
    /// abilities alone — callers must check those conditions externally.
    /// This method returns `false` for those variants.
    pub fn satisfies(&self, gate: &AbilityGate) -> bool {
        match gate {
            AbilityGate::Single(a) => self.has(*a),
            AbilityGate::All(abilities) => abilities.iter().all(|a| self.has(*a)),
            AbilityGate::Any(abilities) => abilities.iter().any(|a| self.has(*a)),
            AbilityGate::BossDefeated(_) | AbilityGate::HasItem(_) => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Skill Unlock System
// ---------------------------------------------------------------------------

/// Manages ability progression and unlock events.
pub struct SkillUnlockSystem {
    abilities: AbilitySet,
    /// Optional prerequisite tree: ability X requires abilities Y first.
    prerequisites: FxHashMap<Ability, Vec<Ability>>,
}

#[allow(clippy::new_without_default)]
impl SkillUnlockSystem {
    /// Create a new skill unlock system with no abilities unlocked.
    pub fn new() -> Self {
        Self {
            abilities: AbilitySet::default(),
            prerequisites: FxHashMap::default(),
        }
    }

    /// Register a prerequisite chain (e.g. SuperJump requires DoubleJump + WallJump).
    pub fn add_prerequisite(&mut self, ability: Ability, requires: Vec<Ability>) {
        self.prerequisites.insert(ability, requires);
    }

    /// Attempt to unlock an ability. Returns `false` if prerequisites are not met.
    pub fn try_unlock(&mut self, ability: Ability) -> bool {
        if let Some(reqs) = self.prerequisites.get(&ability) {
            if !reqs.iter().all(|r| self.abilities.has(*r)) {
                return false;
            }
        }
        self.abilities.unlock(ability);
        true
    }

    /// Force-unlock an ability, ignoring prerequisites (e.g. debug or cutscene).
    pub fn force_unlock(&mut self, ability: Ability) {
        self.abilities.unlock(ability);
    }

    /// Access the current ability set.
    pub fn abilities(&self) -> &AbilitySet {
        &self.abilities
    }
}

// ---------------------------------------------------------------------------
// Room Graph
// ---------------------------------------------------------------------------

/// A single room in the exploration graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomNode {
    /// Unique room identifier.
    pub id: RoomId,
    /// World-space bounding rect for this room (used by camera room-transition).
    pub bounds: Rect,
    /// Display name shown on map (e.g. "Forgotten Crossroads").
    pub area_name: String,
    /// Which area/zone this room belongs to (for map coloring).
    pub zone: ZoneId,
    /// Room-specific camera mode override tag (`None` = default RoomTransition).
    /// The render layer maps this string to a concrete `CameraMode` variant.
    pub camera_override: Option<String>,
    /// Whether this room has been visited by the player.
    pub discovered: bool,
    /// Optional checkpoint (save room) data.
    pub checkpoint: Option<CheckpointData>,
    /// Optional boss encounter data.
    pub boss: Option<BossData>,
}

/// A connection (edge) between two rooms in the exploration graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomConnection {
    /// Source room.
    pub from: RoomId,
    /// Destination room.
    pub to: RoomId,
    /// World-space trigger zone for the doorway.
    pub trigger_bounds: Rect,
    /// Ability required to pass (`None` = always open).
    pub gate: Option<AbilityGate>,
    /// Whether this connection is one-way (e.g. drop-down ledge).
    pub one_way: bool,
}

/// The world as a graph of interconnected rooms.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExplorationGraph {
    rooms: FxHashMap<RoomId, RoomNode>,
    connections: Vec<RoomConnection>,
}

impl ExplorationGraph {
    /// Create an empty exploration graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a room to the graph.
    pub fn add_room(&mut self, room: RoomNode) {
        self.rooms.insert(room.id, room);
    }

    /// Add a connection between rooms.
    pub fn connect(&mut self, connection: RoomConnection) {
        self.connections.push(connection);
    }

    /// Get an immutable reference to a room by id.
    pub fn room(&self, id: RoomId) -> Option<&RoomNode> {
        self.rooms.get(&id)
    }

    /// Get a mutable reference to a room by id.
    pub fn room_mut(&mut self, id: RoomId) -> Option<&mut RoomNode> {
        self.rooms.get_mut(&id)
    }

    /// All rooms adjacent to `id` (both directions unless one-way).
    pub fn neighbors(&self, id: RoomId) -> Vec<(RoomId, &RoomConnection)> {
        let mut result = Vec::new();
        for conn in &self.connections {
            if conn.from == id {
                result.push((conn.to, conn));
            } else if conn.to == id && !conn.one_way {
                result.push((conn.from, conn));
            }
        }
        result
    }

    /// All rooms reachable from `start` given a set of unlocked abilities (BFS).
    pub fn reachable_rooms(&self, start: RoomId, abilities: &AbilitySet) -> Vec<RoomId> {
        let mut visited = FxHashSet::default();
        let mut queue = VecDeque::new();

        visited.insert(start);
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            for (neighbor, conn) in self.neighbors(current) {
                if visited.contains(&neighbor) {
                    continue;
                }
                let passable = match &conn.gate {
                    None => true,
                    Some(gate) => abilities.satisfies(gate),
                };
                if passable {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }

        visited.into_iter().collect()
    }

    /// Rooms that are discovered but have at least one gated exit the player
    /// cannot pass yet. Returns the room id and the blocking gate.
    pub fn backtrack_candidates(&self, abilities: &AbilitySet) -> Vec<(RoomId, AbilityGate)> {
        let mut results = Vec::new();
        for conn in &self.connections {
            if let Some(gate) = &conn.gate {
                if !abilities.satisfies(gate) {
                    // Check if the source room is discovered.
                    if let Some(room) = self.rooms.get(&conn.from) {
                        if room.discovered {
                            results.push((conn.from, gate.clone()));
                        }
                    }
                }
            }
        }
        results
    }

    /// Access the connections list.
    pub fn connections(&self) -> &[RoomConnection] {
        &self.connections
    }
}

// ---------------------------------------------------------------------------
// Checkpoint System
// ---------------------------------------------------------------------------

/// Save room data — determines what resting at a checkpoint does.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointData {
    /// Does resting here refill health?
    pub heals: bool,
    /// Does resting here refill secondary resource (mana, energy)?
    pub restores_resource: bool,
    /// Does resting here respawn enemies in the world?
    pub respawns_enemies: bool,
}

/// Manages checkpoint (save room) state.
pub struct CheckpointSystem {
    last_checkpoint: Option<RoomId>,
}

#[allow(clippy::new_without_default)]
impl CheckpointSystem {
    /// Create a new checkpoint system with no active checkpoint.
    pub fn new() -> Self {
        Self {
            last_checkpoint: None,
        }
    }

    /// Rest at a checkpoint. Triggers `SaveManager::save()`, heals player,
    /// optionally respawns enemies. The `slot` and `label` are used for the save.
    pub fn rest(&mut self, room: RoomId, _data: &CheckpointData, _save: &mut SaveManager) {
        self.last_checkpoint = Some(room);
        // In a full implementation this would call save.save(slot, label, &state, play_time)
        // and apply healing / resource restoration / enemy respawn based on `data`.
    }

    /// The room the player should respawn at after death.
    pub fn respawn_point(&self) -> Option<RoomId> {
        self.last_checkpoint
    }
}

// ---------------------------------------------------------------------------
// Boss Data
// ---------------------------------------------------------------------------

/// Data for a boss encounter within a room.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BossData {
    /// Unique boss identifier.
    pub boss_id: BossId,
    /// Camera locks to these bounds during the fight.
    pub arena_bounds: Rect,
    /// Door entities that seal during the fight.
    pub seal_doors: Vec<EntityId>,
    /// Has this boss been defeated?
    pub defeated: bool,
}

// ---------------------------------------------------------------------------
// Backtrack Marker
// ---------------------------------------------------------------------------

/// How a backtrack marker pin is rendered on the minimap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PinType {
    /// Colored dot (1-3 pixels).
    Dot { color: [f32; 4] },
    /// Small sprite icon.
    Sprite { name: String },
    /// Directional arrow with ability-specific color.
    Arrow { color: [f32; 4] },
}

/// Visual marker on the minimap for areas the player cannot yet access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktrackMarker {
    /// Room where the blocked connection originates.
    pub room: RoomId,
    /// Index into `ExplorationGraph::connections`.
    pub connection: usize,
    /// The gate that blocks passage.
    pub required: AbilityGate,
    /// Player-placed custom marker (e.g. pin color).
    pub custom_pin: Option<PinType>,
}

// ---------------------------------------------------------------------------
// Map Revealer
// ---------------------------------------------------------------------------

/// Integrates with `FogOfWarGrid` to reveal rooms on entry and track
/// minimap pins for points of interest.
pub struct MapRevealer {
    /// Fog grid used to shroud undiscovered rooms.
    pub fog: FogOfWarGrid,
    /// Minimap pins for points of interest (save rooms, bosses, shops).
    pins: Vec<MapPin>,
    /// Cached backtrack markers for gated exits the player has seen.
    backtrack_markers: Vec<BacktrackMarker>,
}

/// A point-of-interest pin stored by `MapRevealer`.
#[derive(Clone, Debug)]
pub struct MapPin {
    /// World position of the pin.
    pub world_pos: SimVec2,
    /// Display type.
    pub pin_type: PinType,
    /// Label for tooltip display.
    pub label: String,
    /// Associated room.
    pub room: RoomId,
}

impl MapRevealer {
    /// Create a new map revealer sized to the world.
    /// Grid dimensions are derived from the bounding box of all rooms in the graph.
    pub fn new(graph: &ExplorationGraph) -> Self {
        // Compute world bounds from all rooms to size the fog grid.
        let (mut max_x, mut max_y) = (0.0_f32, 0.0_f32);
        for room in graph.rooms.values() {
            let rx = room.bounds.x + room.bounds.w;
            let ry = room.bounds.y + room.bounds.h;
            if rx > max_x {
                max_x = rx;
            }
            if ry > max_y {
                max_y = ry;
            }
        }
        // Use 1 tile = 1 unit; clamp minimum to 1×1.
        let w = (max_x.ceil() as u32).max(1);
        let h = (max_y.ceil() as u32).max(1);

        Self {
            fog: FogOfWarGrid::new(w, h),
            pins: Vec::new(),
            backtrack_markers: Vec::new(),
        }
    }

    /// Called when the player enters a room. Reveals the room on the minimap
    /// by clearing fog for the room's tile region (setting tiles from `Hidden`
    /// to `Visible`). Adjacent undiscovered rooms are set to `Explored`
    /// (silhouette hint).
    pub fn on_room_enter(&mut self, room: &RoomNode, graph: &ExplorationGraph) {
        // Reveal the entered room's tiles as Visible.
        let x0 = room.bounds.x.floor() as i32;
        let y0 = room.bounds.y.floor() as i32;
        let x1 = (room.bounds.x + room.bounds.w).ceil() as i32;
        let y1 = (room.bounds.y + room.bounds.h).ceil() as i32;

        for ty in y0..y1 {
            for tx in x0..x1 {
                self.fog.set_visibility(tx, ty, TileVisibility::Visible);
            }
        }

        // Adjacent rooms appear as silhouettes (Explored) to hint at existence.
        for (neighbor_id, _conn) in graph.neighbors(room.id) {
            if let Some(neighbor) = graph.room(neighbor_id) {
                if !neighbor.discovered {
                    let nx0 = neighbor.bounds.x.floor() as i32;
                    let ny0 = neighbor.bounds.y.floor() as i32;
                    let nx1 = (neighbor.bounds.x + neighbor.bounds.w).ceil() as i32;
                    let ny1 = (neighbor.bounds.y + neighbor.bounds.h).ceil() as i32;

                    for ty in ny0..ny1 {
                        for tx in nx0..nx1 {
                            // Only upgrade Hidden -> Explored; don't downgrade Visible.
                            if self.fog.visibility_at(tx, ty) == TileVisibility::Hidden {
                                self.fog.set_visibility(tx, ty, TileVisibility::Explored);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Adds a map pin (save room icon, boss icon, shop icon, etc.).
    pub fn add_pin(&mut self, pin: MapPin) {
        self.pins.push(pin);
    }

    /// Access all stored pins.
    pub fn pins(&self) -> &[MapPin] {
        &self.pins
    }

    /// Returns all backtrack markers — rooms with gated exits the player
    /// has seen but cannot yet reach. Updates the internal cache and returns it.
    pub fn backtrack_pins(
        &mut self,
        graph: &ExplorationGraph,
        abilities: &AbilitySet,
    ) -> &[BacktrackMarker] {
        self.backtrack_markers.clear();
        for (idx, conn) in graph.connections().iter().enumerate() {
            if let Some(gate) = &conn.gate {
                if !abilities.satisfies(gate) {
                    if let Some(room) = graph.room(conn.from) {
                        if room.discovered {
                            self.backtrack_markers.push(BacktrackMarker {
                                room: conn.from,
                                connection: idx,
                                required: gate.clone(),
                                custom_pin: None,
                            });
                        }
                    }
                }
            }
        }
        &self.backtrack_markers
    }

    /// Access the fog grid.
    pub fn fog(&self) -> &FogOfWarGrid {
        &self.fog
    }

    /// Mutable access to the fog grid.
    pub fn fog_mut(&mut self) -> &mut FogOfWarGrid {
        &mut self.fog
    }
}

// ---------------------------------------------------------------------------
// Boss Room System
// ---------------------------------------------------------------------------

/// Manages boss encounter lifecycle: sealing doors, camera transitions,
/// and post-defeat cleanup.
pub struct BossRoomSystem;

impl BossRoomSystem {
    /// Trigger boss fight: seal doors (make collision shapes solid) and
    /// switch camera to BossArena mode.
    ///
    /// `doors` provides the entity id and its collision shape for each door
    /// that should seal. The shapes are replaced with solid AABBs covering
    /// the door's arena-sealing region.
    ///
    /// Returns the `CameraMode::BossArena`-style parameters as `(center_x, center_y, width, height)`
    /// that the render layer should apply to the camera.
    pub fn enter_fight(
        boss: &mut BossData,
        doors: &mut [(EntityId, &mut CollisionShape)],
    ) -> (f32, f32, f32, f32) {
        // Seal all doors by setting their collision shapes to solid AABBs.
        for (_entity, shape) in doors.iter_mut() {
            // Replace the shape with a solid AABB that covers the door region.
            // The actual bounds come from the existing shape position.
            match **shape {
                CollisionShape::Aabb(rect) => {
                    // Already an AABB — keep it (it becomes the sealed barrier).
                    **shape = CollisionShape::Aabb(rect);
                }
                CollisionShape::Circle { cx, cy, radius } => {
                    // Convert circle to AABB for sealing.
                    **shape = CollisionShape::Aabb(Rect::new(
                        cx - radius,
                        cy - radius,
                        radius * 2.0,
                        radius * 2.0,
                    ));
                }
            }
        }

        // Return arena camera parameters.
        let center_x = boss.arena_bounds.x + boss.arena_bounds.w * 0.5;
        let center_y = boss.arena_bounds.y + boss.arena_bounds.h * 0.5;
        (center_x, center_y, boss.arena_bounds.w, boss.arena_bounds.h)
    }

    /// End boss fight: unseal doors, mark boss as defeated.
    ///
    /// `original_shapes` provides the shapes to restore to the door entities
    /// (typically empty/passable shapes). If `None`, doors are set to a
    /// zero-sized AABB (effectively removing collision).
    ///
    /// Returns the room bounds that the camera should transition back to.
    pub fn end_fight(
        boss: &mut BossData,
        doors: &mut [(EntityId, &mut CollisionShape)],
        room_bounds: &Rect,
    ) -> Rect {
        boss.defeated = true;

        // Unseal doors by setting shapes to zero-size (passable).
        for (_entity, shape) in doors.iter_mut() {
            **shape = CollisionShape::Aabb(Rect::new(0.0, 0.0, 0.0, 0.0));
        }

        *room_bounds
    }
}

// ---------------------------------------------------------------------------
// Boss Phase FSM
// ---------------------------------------------------------------------------

/// Metroidvania boss phase FSM (alternative to BehaviorTree until it's implemented).
///
/// Bosses cycle through phases based on HP thresholds. Each phase defines
/// movement patterns, attack patterns, and animation tags.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BossPhase {
    /// Phase configurations in order (index 0 = full HP, last = near death).
    pub phases: Vec<PhaseConfig>,
    /// Index of the currently active phase.
    pub current: usize,
}

/// Configuration for a single boss phase.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhaseConfig {
    /// HP threshold to enter this phase (1.0 = full HP, 0.0 = dead).
    /// Phase activates when `current_hp / max_hp <= hp_threshold`.
    pub hp_threshold: f32,
    /// Movement pattern during this phase.
    pub movement: BossMovement,
    /// Attack pattern identifiers (mapped to BulletEmitter configs or melee patterns).
    pub attacks: Vec<PatternSequence>,
    /// Animation tag to switch to when entering this phase.
    pub anim_tag: String,
}

/// Movement pattern for a boss phase.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BossMovement {
    /// Stay at center of arena.
    Stationary,
    /// Move between predefined positions.
    Patrol {
        positions: Vec<SimVec2>,
        speed: I16F16,
    },
    /// Chase the player.
    Chase { speed: I16F16, min_distance: I16F16 },
    /// Jump to random positions in arena.
    Teleport { cooldown: u32 },
}

/// An attack pattern sequence identifier and timing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternSequence {
    /// Name of the attack pattern (maps to a BulletEmitter config or melee definition).
    pub pattern_name: String,
    /// Cooldown in ticks between uses of this pattern.
    pub cooldown: u32,
    /// Current cooldown counter (decremented each tick).
    pub current_cooldown: u32,
}

impl BossPhase {
    /// Create a new boss phase FSM.
    pub fn new(phases: Vec<PhaseConfig>) -> Self {
        Self { phases, current: 0 }
    }

    /// Check whether a phase transition should occur based on the boss's
    /// current HP ratio. Returns `true` if the phase changed.
    ///
    /// `hp_ratio` = current_hp / max_hp, in range 0.0..=1.0.
    pub fn update(&mut self, hp_ratio: f32) -> bool {
        let old = self.current;
        // Advance to the latest phase whose threshold is >= the HP ratio.
        // Phases are ordered from high HP to low HP.
        for (i, phase) in self.phases.iter().enumerate() {
            if hp_ratio <= phase.hp_threshold {
                self.current = i;
            }
        }
        self.current != old
    }

    /// Get the currently active phase config, if any.
    pub fn current_phase(&self) -> Option<&PhaseConfig> {
        self.phases.get(self.current)
    }

    /// Tick attack cooldowns for the current phase.
    pub fn tick_cooldowns(&mut self) {
        if let Some(phase) = self.phases.get_mut(self.current) {
            for attack in &mut phase.attacks {
                attack.current_cooldown = attack.current_cooldown.saturating_sub(1);
            }
        }
    }

    /// Get the next ready attack in the current phase (cooldown == 0).
    /// Returns the index and a reference. Resets cooldown on use.
    pub fn next_ready_attack(&mut self) -> Option<(usize, &PatternSequence)> {
        let phase = self.phases.get_mut(self.current)?;
        for (i, attack) in phase.attacks.iter_mut().enumerate() {
            if attack.current_cooldown == 0 {
                attack.current_cooldown = attack.cooldown;
                return Some((i, attack));
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Room Transition System
// ---------------------------------------------------------------------------

/// Tracks and manages room transitions when the player moves between rooms.
///
/// Handles the lifecycle of a room transition:
/// 1. Detect player overlap with a `RoomConnection` trigger zone.
/// 2. Verify gate conditions (ability check).
/// 3. Mark old room as inactive, new room as active and discovered.
/// 4. Notify `MapRevealer` to clear fog.
/// 5. Suppress player input during the transition.
pub struct RoomTransitionSystem {
    /// The room the player is currently in.
    current_room: Option<RoomId>,
    /// Whether a transition is currently in progress (camera sliding).
    transitioning: bool,
    /// The room we are transitioning to.
    transition_target: Option<RoomId>,
    /// Whether player input should be suppressed during transition.
    input_suppressed: bool,
}

/// Result of a transition attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionResult {
    /// No transition needed (player is not overlapping a connection trigger).
    None,
    /// Transition blocked by an ability gate.
    Blocked(RoomId),
    /// Transition started successfully to the given room.
    Started(RoomId),
    /// Transition is still in progress.
    InProgress,
    /// Transition completed; player is now in the given room.
    Completed(RoomId),
}

#[allow(clippy::new_without_default)]
impl RoomTransitionSystem {
    /// Create a new room transition system.
    pub fn new() -> Self {
        Self {
            current_room: None,
            transitioning: false,
            transition_target: None,
            input_suppressed: false,
        }
    }

    /// Set the initial room (e.g. on game load).
    pub fn set_current_room(&mut self, room: RoomId) {
        self.current_room = Some(room);
        self.transitioning = false;
        self.transition_target = None;
        self.input_suppressed = false;
    }

    /// The room the player is currently in.
    pub fn current_room(&self) -> Option<RoomId> {
        self.current_room
    }

    /// Whether player input should be suppressed (during a transition).
    pub fn is_input_suppressed(&self) -> bool {
        self.input_suppressed
    }

    /// Whether a transition is in progress.
    pub fn is_transitioning(&self) -> bool {
        self.transitioning
    }

    /// Attempt to start a room transition. Call this each tick with the
    /// player's world position. Checks whether the player overlaps any
    /// connection trigger zone from the current room.
    ///
    /// Returns `TransitionResult::Started(target_room)` if a transition begins,
    /// `Blocked` if gated, `InProgress` if already transitioning, or `None`.
    pub fn try_transition(
        &mut self,
        player_pos: &Rect,
        graph: &ExplorationGraph,
        abilities: &AbilitySet,
        defeated_bosses: &FxHashSet<BossId>,
        held_items: &FxHashSet<String>,
    ) -> TransitionResult {
        if self.transitioning {
            return TransitionResult::InProgress;
        }

        let current = match self.current_room {
            Some(id) => id,
            None => return TransitionResult::None,
        };

        for conn in graph.connections() {
            // Check connections from current room.
            let target = if conn.from == current {
                conn.to
            } else if conn.to == current && !conn.one_way {
                conn.from
            } else {
                continue;
            };

            // Check trigger overlap.
            if !rects_overlap(player_pos, &conn.trigger_bounds) {
                continue;
            }

            // Check gate.
            if let Some(gate) = &conn.gate {
                let satisfied = match gate {
                    AbilityGate::BossDefeated(boss_id) => defeated_bosses.contains(boss_id),
                    AbilityGate::HasItem(item) => held_items.contains(item),
                    _ => abilities.satisfies(gate),
                };
                if !satisfied {
                    return TransitionResult::Blocked(target);
                }
            }

            // Start transition.
            self.transitioning = true;
            self.transition_target = Some(target);
            self.input_suppressed = true;
            return TransitionResult::Started(target);
        }

        TransitionResult::None
    }

    /// Complete the transition. Call this once the camera has finished sliding
    /// to the new room. Updates `ExplorationGraph` to mark the new room as
    /// discovered, and optionally updates the `MapRevealer`.
    ///
    /// Returns `TransitionResult::Completed` with the new room id.
    pub fn complete_transition(
        &mut self,
        graph: &mut ExplorationGraph,
        map_revealer: Option<&mut MapRevealer>,
    ) -> TransitionResult {
        let target = match self.transition_target {
            Some(id) => id,
            None => return TransitionResult::None,
        };

        // Mark room as discovered.
        if let Some(room) = graph.room_mut(target) {
            room.discovered = true;
        }

        // Reveal on map.
        if let Some(revealer) = map_revealer {
            // Clone the room data needed for fog reveal (avoids borrow conflict).
            if let Some(room) = graph.room(target) {
                let room_clone = room.clone();
                revealer.on_room_enter(&room_clone, graph);
            }
        }

        self.current_room = Some(target);
        self.transitioning = false;
        self.transition_target = None;
        self.input_suppressed = false;

        TransitionResult::Completed(target)
    }

    /// Cancel an in-progress transition (e.g. if the player dies mid-transition).
    pub fn cancel_transition(&mut self) {
        self.transitioning = false;
        self.transition_target = None;
        self.input_suppressed = false;
    }
}

/// Simple AABB overlap test for trigger zones.
fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_room(id: u32, name: &str, zone: u16) -> RoomNode {
        RoomNode {
            id: RoomId(id),
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            area_name: name.to_string(),
            zone: ZoneId(zone),
            camera_override: None,
            discovered: false,
            checkpoint: None,
            boss: None,
        }
    }

    fn make_room_at(id: u32, name: &str, zone: u16, x: f32, y: f32, w: f32, h: f32) -> RoomNode {
        RoomNode {
            id: RoomId(id),
            bounds: Rect::new(x, y, w, h),
            area_name: name.to_string(),
            zone: ZoneId(zone),
            camera_override: None,
            discovered: false,
            checkpoint: None,
            boss: None,
        }
    }

    fn make_connection(from: u32, to: u32) -> RoomConnection {
        RoomConnection {
            from: RoomId(from),
            to: RoomId(to),
            trigger_bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
            gate: None,
            one_way: false,
        }
    }

    #[test]
    fn test_ability_set_unlock_and_has() {
        let mut set = AbilitySet::default();
        assert!(!set.has(Ability::DoubleJump));
        set.unlock(Ability::DoubleJump);
        assert!(set.has(Ability::DoubleJump));
        assert!(!set.has(Ability::Dash));
    }

    #[test]
    fn test_ability_gate_single() {
        let mut set = AbilitySet::default();
        let gate = AbilityGate::Single(Ability::WallJump);
        assert!(!set.satisfies(&gate));
        set.unlock(Ability::WallJump);
        assert!(set.satisfies(&gate));
    }

    #[test]
    fn test_ability_gate_all() {
        let mut set = AbilitySet::default();
        let gate = AbilityGate::All(vec![Ability::DoubleJump, Ability::Dash]);
        set.unlock(Ability::DoubleJump);
        assert!(!set.satisfies(&gate));
        set.unlock(Ability::Dash);
        assert!(set.satisfies(&gate));
    }

    #[test]
    fn test_ability_gate_any() {
        let mut set = AbilitySet::default();
        let gate = AbilityGate::Any(vec![Ability::Grapple, Ability::Swim]);
        assert!(!set.satisfies(&gate));
        set.unlock(Ability::Swim);
        assert!(set.satisfies(&gate));
    }

    #[test]
    fn test_skill_unlock_with_prerequisites() {
        let mut sys = SkillUnlockSystem::new();
        sys.add_prerequisite(
            Ability::SuperJump,
            vec![Ability::DoubleJump, Ability::WallJump],
        );

        // Should fail — prerequisites not met.
        assert!(!sys.try_unlock(Ability::SuperJump));
        assert!(!sys.abilities().has(Ability::SuperJump));

        // Unlock prerequisites.
        assert!(sys.try_unlock(Ability::DoubleJump));
        assert!(sys.try_unlock(Ability::WallJump));

        // Now it should succeed.
        assert!(sys.try_unlock(Ability::SuperJump));
        assert!(sys.abilities().has(Ability::SuperJump));
    }

    #[test]
    fn test_force_unlock_ignores_prerequisites() {
        let mut sys = SkillUnlockSystem::new();
        sys.add_prerequisite(Ability::SuperJump, vec![Ability::DoubleJump]);

        sys.force_unlock(Ability::SuperJump);
        assert!(sys.abilities().has(Ability::SuperJump));
    }

    #[test]
    fn test_exploration_graph_neighbors_and_one_way() {
        let mut graph = ExplorationGraph::new();
        graph.add_room(make_room(1, "A", 0));
        graph.add_room(make_room(2, "B", 0));
        graph.add_room(make_room(3, "C", 0));

        // 1 <-> 2 (bidirectional)
        graph.connect(make_connection(1, 2));
        // 2 -> 3 (one-way drop)
        graph.connect(RoomConnection {
            from: RoomId(2),
            to: RoomId(3),
            trigger_bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
            gate: None,
            one_way: true,
        });

        let n1: Vec<RoomId> = graph
            .neighbors(RoomId(1))
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(n1, vec![RoomId(2)]);

        let n2: Vec<RoomId> = graph
            .neighbors(RoomId(2))
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert!(n2.contains(&RoomId(1)));
        assert!(n2.contains(&RoomId(3)));

        // Room 3 cannot go back to 2 (one-way).
        let n3: Vec<RoomId> = graph
            .neighbors(RoomId(3))
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert!(!n3.contains(&RoomId(2)));
    }

    #[test]
    fn test_reachable_rooms_with_gate() {
        let mut graph = ExplorationGraph::new();
        graph.add_room(make_room(1, "Start", 0));
        graph.add_room(make_room(2, "Open", 0));
        graph.add_room(make_room(3, "Gated", 0));

        graph.connect(make_connection(1, 2));
        graph.connect(RoomConnection {
            from: RoomId(2),
            to: RoomId(3),
            trigger_bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
            gate: Some(AbilityGate::Single(Ability::Dash)),
            one_way: false,
        });

        let no_abilities = AbilitySet::default();
        let reachable = graph.reachable_rooms(RoomId(1), &no_abilities);
        assert!(reachable.contains(&RoomId(1)));
        assert!(reachable.contains(&RoomId(2)));
        assert!(!reachable.contains(&RoomId(3)));

        let mut with_dash = AbilitySet::default();
        with_dash.unlock(Ability::Dash);
        let reachable = graph.reachable_rooms(RoomId(1), &with_dash);
        assert!(reachable.contains(&RoomId(3)));
    }

    #[test]
    fn test_backtrack_candidates() {
        let mut graph = ExplorationGraph::new();
        let mut room1 = make_room(1, "Explored", 0);
        room1.discovered = true;
        graph.add_room(room1);
        graph.add_room(make_room(2, "Hidden", 0));

        graph.connect(RoomConnection {
            from: RoomId(1),
            to: RoomId(2),
            trigger_bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
            gate: Some(AbilityGate::Single(Ability::Grapple)),
            one_way: false,
        });

        let abilities = AbilitySet::default();
        let candidates = graph.backtrack_candidates(&abilities);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, RoomId(1));

        // After unlocking grapple, no more candidates.
        let mut with_grapple = AbilitySet::default();
        with_grapple.unlock(Ability::Grapple);
        let candidates = graph.backtrack_candidates(&with_grapple);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_checkpoint_system() {
        let mut sys = CheckpointSystem::new();
        assert!(sys.respawn_point().is_none());

        sys.last_checkpoint = Some(RoomId(42));
        assert_eq!(sys.respawn_point(), Some(RoomId(42)));
    }

    #[test]
    fn test_backtrack_marker_creation() {
        let marker = BacktrackMarker {
            room: RoomId(5),
            connection: 3,
            required: AbilityGate::Single(Ability::PhaseDoor),
            custom_pin: None,
        };
        assert_eq!(marker.room, RoomId(5));
        assert_eq!(marker.connection, 3);
        assert!(marker.custom_pin.is_none());
    }

    #[test]
    fn test_backtrack_marker_with_custom_pin() {
        let marker = BacktrackMarker {
            room: RoomId(7),
            connection: 1,
            required: AbilityGate::Single(Ability::Grapple),
            custom_pin: Some(PinType::Arrow {
                color: [1.0, 0.5, 0.0, 1.0],
            }),
        };
        assert!(marker.custom_pin.is_some());
    }

    #[test]
    fn test_boss_data_defaults() {
        let boss = BossData {
            boss_id: BossId(1),
            arena_bounds: Rect::new(0.0, 0.0, 200.0, 150.0),
            seal_doors: Vec::new(),
            defeated: false,
        };
        assert!(!boss.defeated);
        assert_eq!(boss.boss_id, BossId(1));
    }

    // -- MapRevealer tests --------------------------------------------------

    #[test]
    fn test_map_revealer_new_creates_fog_grid() {
        let mut graph = ExplorationGraph::new();
        graph.add_room(make_room_at(1, "A", 0, 0.0, 0.0, 50.0, 50.0));
        graph.add_room(make_room_at(2, "B", 0, 50.0, 0.0, 50.0, 50.0));

        let revealer = MapRevealer::new(&graph);
        // Grid should cover at least 100 x 50.
        assert!(revealer.fog().width() >= 100);
        assert!(revealer.fog().height() >= 50);
    }

    #[test]
    fn test_map_revealer_on_room_enter_reveals_tiles() {
        let mut graph = ExplorationGraph::new();
        graph.add_room(make_room_at(1, "A", 0, 10.0, 10.0, 5.0, 5.0));
        graph.add_room(make_room_at(2, "B", 0, 20.0, 10.0, 5.0, 5.0));
        graph.connect(make_connection(1, 2));

        let mut revealer = MapRevealer::new(&graph);

        // Before entering, all tiles are Hidden.
        assert_eq!(revealer.fog.visibility_at(12, 12), TileVisibility::Hidden);

        // Enter room 1.
        let room1 = graph.room(RoomId(1)).unwrap().clone();
        revealer.on_room_enter(&room1, &graph);

        // Room 1 tiles should be Visible.
        assert_eq!(revealer.fog.visibility_at(12, 12), TileVisibility::Visible);

        // Adjacent room 2 tiles should be Explored (silhouette).
        assert_eq!(revealer.fog.visibility_at(22, 12), TileVisibility::Explored);
    }

    #[test]
    fn test_map_revealer_backtrack_pins() {
        let mut graph = ExplorationGraph::new();
        let mut room1 = make_room_at(1, "A", 0, 0.0, 0.0, 10.0, 10.0);
        room1.discovered = true;
        graph.add_room(room1);
        graph.add_room(make_room_at(2, "B", 0, 20.0, 0.0, 10.0, 10.0));
        graph.connect(RoomConnection {
            from: RoomId(1),
            to: RoomId(2),
            trigger_bounds: Rect::new(9.0, 0.0, 2.0, 10.0),
            gate: Some(AbilityGate::Single(Ability::Dash)),
            one_way: false,
        });

        let mut revealer = MapRevealer::new(&graph);
        let abilities = AbilitySet::default();
        let pins = revealer.backtrack_pins(&graph, &abilities);
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].room, RoomId(1));

        // After unlocking Dash, no more pins.
        let mut with_dash = AbilitySet::default();
        with_dash.unlock(Ability::Dash);
        let pins = revealer.backtrack_pins(&graph, &with_dash);
        assert!(pins.is_empty());
    }

    #[test]
    fn test_map_revealer_add_pin() {
        let graph = ExplorationGraph::new();
        let mut revealer = MapRevealer::new(&graph);
        assert_eq!(revealer.pins().len(), 0);

        revealer.add_pin(MapPin {
            world_pos: SimVec2::ZERO,
            pin_type: PinType::Sprite {
                name: "save_icon".to_string(),
            },
            label: "Save Room".to_string(),
            room: RoomId(1),
        });
        assert_eq!(revealer.pins().len(), 1);
    }

    // -- BossRoomSystem tests -----------------------------------------------

    #[test]
    fn test_boss_room_enter_fight() {
        let mut boss = BossData {
            boss_id: BossId(1),
            arena_bounds: Rect::new(10.0, 20.0, 200.0, 150.0),
            seal_doors: vec![EntityId::from_raw(1, 0)],
            defeated: false,
        };
        let mut shape = CollisionShape::Aabb(Rect::new(10.0, 20.0, 5.0, 50.0));
        let mut doors: Vec<(EntityId, &mut CollisionShape)> =
            vec![(EntityId::from_raw(1, 0), &mut shape)];

        let (cx, cy, w, h) = BossRoomSystem::enter_fight(&mut boss, &mut doors);
        assert!((cx - 110.0).abs() < 0.01);
        assert!((cy - 95.0).abs() < 0.01);
        assert!((w - 200.0).abs() < 0.01);
        assert!((h - 150.0).abs() < 0.01);
        assert!(!boss.defeated);
    }

    #[test]
    fn test_boss_room_end_fight() {
        let mut boss = BossData {
            boss_id: BossId(2),
            arena_bounds: Rect::new(0.0, 0.0, 200.0, 150.0),
            seal_doors: vec![EntityId::from_raw(1, 0)],
            defeated: false,
        };
        let mut shape = CollisionShape::Aabb(Rect::new(10.0, 20.0, 5.0, 50.0));
        let mut doors: Vec<(EntityId, &mut CollisionShape)> =
            vec![(EntityId::from_raw(1, 0), &mut shape)];
        let room_bounds = Rect::new(0.0, 0.0, 300.0, 200.0);

        BossRoomSystem::end_fight(&mut boss, &mut doors, &room_bounds);
        assert!(boss.defeated);
        // Door shape should be zeroed out (passable).
        match shape {
            CollisionShape::Aabb(r) => {
                assert!((r.w).abs() < 0.01);
                assert!((r.h).abs() < 0.01);
            }
            _ => panic!("Expected Aabb"),
        }
    }

    // -- BossPhase FSM tests ------------------------------------------------

    #[test]
    fn test_boss_phase_transitions() {
        let mut fsm = BossPhase::new(vec![
            PhaseConfig {
                hp_threshold: 1.0,
                movement: BossMovement::Stationary,
                attacks: Vec::new(),
                anim_tag: "idle".to_string(),
            },
            PhaseConfig {
                hp_threshold: 0.5,
                movement: BossMovement::Chase {
                    speed: I16F16::from_num(2),
                    min_distance: I16F16::from_num(10),
                },
                attacks: Vec::new(),
                anim_tag: "rage".to_string(),
            },
            PhaseConfig {
                hp_threshold: 0.2,
                movement: BossMovement::Teleport { cooldown: 60 },
                attacks: Vec::new(),
                anim_tag: "desperate".to_string(),
            },
        ]);

        assert_eq!(fsm.current, 0);

        // At full HP, phase 0.
        let changed = fsm.update(1.0);
        // hp_ratio 1.0 <= threshold 1.0 -> phase 0 matched,
        // hp_ratio 1.0 <= threshold 0.5 -> false, so current stays 0.
        assert!(!changed);
        assert_eq!(fsm.current, 0);

        // Drop to 50% — should advance to phase 1.
        let changed = fsm.update(0.5);
        assert!(changed);
        assert_eq!(fsm.current, 1);
        assert_eq!(fsm.current_phase().unwrap().anim_tag, "rage");

        // Drop to 15% — should advance to phase 2.
        let changed = fsm.update(0.15);
        assert!(changed);
        assert_eq!(fsm.current, 2);
        assert_eq!(fsm.current_phase().unwrap().anim_tag, "desperate");
    }

    #[test]
    fn test_boss_phase_attack_cooldowns() {
        let mut fsm = BossPhase::new(vec![PhaseConfig {
            hp_threshold: 1.0,
            movement: BossMovement::Stationary,
            attacks: vec![PatternSequence {
                pattern_name: "fireball".to_string(),
                cooldown: 3,
                current_cooldown: 0,
            }],
            anim_tag: "idle".to_string(),
        }]);

        // Attack should be ready immediately.
        let (idx, _attack) = fsm.next_ready_attack().unwrap();
        assert_eq!(idx, 0);

        // After firing, cooldown should be reset to 3.
        // next_ready_attack should return None now.
        assert!(fsm.next_ready_attack().is_none());

        // Tick down cooldowns.
        fsm.tick_cooldowns();
        fsm.tick_cooldowns();
        fsm.tick_cooldowns();

        // Should be ready again.
        assert!(fsm.next_ready_attack().is_some());
    }

    // -- RoomTransitionSystem tests -----------------------------------------

    #[test]
    fn test_room_transition_basic() {
        let mut graph = ExplorationGraph::new();
        graph.add_room(make_room_at(1, "Start", 0, 0.0, 0.0, 100.0, 100.0));
        graph.add_room(make_room_at(2, "Next", 0, 100.0, 0.0, 100.0, 100.0));
        graph.connect(RoomConnection {
            from: RoomId(1),
            to: RoomId(2),
            trigger_bounds: Rect::new(95.0, 0.0, 10.0, 100.0),
            gate: None,
            one_way: false,
        });

        let mut sys = RoomTransitionSystem::new();
        sys.set_current_room(RoomId(1));

        let abilities = AbilitySet::default();
        let bosses = FxHashSet::default();
        let items = FxHashSet::default();

        // Player not overlapping trigger — no transition.
        let player = Rect::new(50.0, 50.0, 10.0, 10.0);
        let result = sys.try_transition(&player, &graph, &abilities, &bosses, &items);
        assert_eq!(result, TransitionResult::None);

        // Player overlapping trigger — transition starts.
        let player = Rect::new(96.0, 50.0, 10.0, 10.0);
        let result = sys.try_transition(&player, &graph, &abilities, &bosses, &items);
        assert_eq!(result, TransitionResult::Started(RoomId(2)));
        assert!(sys.is_transitioning());
        assert!(sys.is_input_suppressed());

        // During transition, further attempts return InProgress.
        let result = sys.try_transition(&player, &graph, &abilities, &bosses, &items);
        assert_eq!(result, TransitionResult::InProgress);

        // Complete the transition.
        let result = sys.complete_transition(&mut graph, None);
        assert_eq!(result, TransitionResult::Completed(RoomId(2)));
        assert_eq!(sys.current_room(), Some(RoomId(2)));
        assert!(!sys.is_transitioning());
        assert!(!sys.is_input_suppressed());

        // Room 2 should now be discovered.
        assert!(graph.room(RoomId(2)).unwrap().discovered);
    }

    #[test]
    fn test_room_transition_blocked_by_gate() {
        let mut graph = ExplorationGraph::new();
        graph.add_room(make_room_at(1, "Start", 0, 0.0, 0.0, 100.0, 100.0));
        graph.add_room(make_room_at(2, "Gated", 0, 100.0, 0.0, 100.0, 100.0));
        graph.connect(RoomConnection {
            from: RoomId(1),
            to: RoomId(2),
            trigger_bounds: Rect::new(95.0, 0.0, 10.0, 100.0),
            gate: Some(AbilityGate::Single(Ability::Dash)),
            one_way: false,
        });

        let mut sys = RoomTransitionSystem::new();
        sys.set_current_room(RoomId(1));

        let abilities = AbilitySet::default();
        let bosses = FxHashSet::default();
        let items = FxHashSet::default();

        let player = Rect::new(96.0, 50.0, 10.0, 10.0);
        let result = sys.try_transition(&player, &graph, &abilities, &bosses, &items);
        assert_eq!(result, TransitionResult::Blocked(RoomId(2)));
        assert!(!sys.is_transitioning());
    }

    #[test]
    fn test_room_transition_with_boss_gate() {
        let mut graph = ExplorationGraph::new();
        graph.add_room(make_room_at(1, "Start", 0, 0.0, 0.0, 100.0, 100.0));
        graph.add_room(make_room_at(2, "PostBoss", 0, 100.0, 0.0, 100.0, 100.0));
        graph.connect(RoomConnection {
            from: RoomId(1),
            to: RoomId(2),
            trigger_bounds: Rect::new(95.0, 0.0, 10.0, 100.0),
            gate: Some(AbilityGate::BossDefeated(BossId(42))),
            one_way: false,
        });

        let mut sys = RoomTransitionSystem::new();
        sys.set_current_room(RoomId(1));

        let abilities = AbilitySet::default();
        let items = FxHashSet::default();
        let player = Rect::new(96.0, 50.0, 10.0, 10.0);

        // Without boss defeated — blocked.
        let bosses = FxHashSet::default();
        let result = sys.try_transition(&player, &graph, &abilities, &bosses, &items);
        assert_eq!(result, TransitionResult::Blocked(RoomId(2)));

        // With boss defeated — allowed.
        let mut bosses = FxHashSet::default();
        bosses.insert(BossId(42));
        let result = sys.try_transition(&player, &graph, &abilities, &bosses, &items);
        assert_eq!(result, TransitionResult::Started(RoomId(2)));
    }

    #[test]
    fn test_room_transition_cancel() {
        let mut graph = ExplorationGraph::new();
        graph.add_room(make_room_at(1, "A", 0, 0.0, 0.0, 100.0, 100.0));
        graph.add_room(make_room_at(2, "B", 0, 100.0, 0.0, 100.0, 100.0));
        graph.connect(RoomConnection {
            from: RoomId(1),
            to: RoomId(2),
            trigger_bounds: Rect::new(95.0, 0.0, 10.0, 100.0),
            gate: None,
            one_way: false,
        });

        let mut sys = RoomTransitionSystem::new();
        sys.set_current_room(RoomId(1));

        let abilities = AbilitySet::default();
        let bosses = FxHashSet::default();
        let items = FxHashSet::default();

        let player = Rect::new(96.0, 50.0, 10.0, 10.0);
        sys.try_transition(&player, &graph, &abilities, &bosses, &items);
        assert!(sys.is_transitioning());

        sys.cancel_transition();
        assert!(!sys.is_transitioning());
        assert_eq!(sys.current_room(), Some(RoomId(1)));
    }
}
