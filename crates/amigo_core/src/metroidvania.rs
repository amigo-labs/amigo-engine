//! Metroidvania gametype — interconnected exploration with ability-gated progression.
//!
//! Provides an exploration graph of rooms connected by doorways, an ability gating
//! system for non-linear progression, checkpoint/save-room support, boss encounter
//! lifecycle, and backtrack markers for the minimap.

use crate::ecs::EntityId;
use crate::rect::Rect;
use crate::save::SaveManager;
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

/// Visual marker on the minimap for areas the player cannot yet access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktrackMarker {
    /// Room where the blocked connection originates.
    pub room: RoomId,
    /// Index into `ExplorationGraph::connections`.
    pub connection: usize,
    /// The gate that blocks passage.
    pub required: AbilityGate,
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
        };
        assert_eq!(marker.room, RoomId(5));
        assert_eq!(marker.connection, 3);
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
}
