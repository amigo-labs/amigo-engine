//! Utility-based autonomous agent system for God Sim / Sandbox NPCs.
//!
//! Complements the FSM-based AI in [`crate::ai`] with a needs-driven
//! utility AI system where agents autonomously choose actions based on
//! the urgency of their needs and the scores of available actions.
//!
//! Key concepts:
//! - **Needs:** Hunger, Sleep, Safety, Social — each decays over time.
//! - **Actions:** Eat, Sleep, Build, Harvest, Fight, Flee, Trade, Explore, etc.
//! - **Utility scoring:** Each action scores based on needs + environment.
//! - **Memory:** Agents remember resource locations, dangers, relationships.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ecs::EntityId;
use crate::math::SimVec2;

// ---------------------------------------------------------------------------
// Needs
// ---------------------------------------------------------------------------

/// A single need (0.0 = critical, 100.0 = fully satisfied).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Need {
    pub value: f32,
    /// How fast this need decays per sim-tick.
    pub decay_rate: f32,
    /// Priority weight for utility scoring (higher = agent prioritizes this).
    pub weight: f32,
}

impl Need {
    pub fn new(value: f32, decay_rate: f32, weight: f32) -> Self {
        Self {
            value: value.clamp(0.0, 100.0),
            decay_rate,
            weight,
        }
    }

    /// Tick the need (decrease by decay_rate).
    pub fn tick(&mut self) {
        self.value = (self.value - self.decay_rate).max(0.0);
    }

    /// How urgent is this need? (0.0 = not urgent, 1.0 = critical).
    pub fn urgency(&self) -> f32 {
        (1.0 - self.value / 100.0) * self.weight
    }

    /// Satisfy the need by the given amount.
    pub fn satisfy(&mut self, amount: f32) {
        self.value = (self.value + amount).min(100.0);
    }
}

/// Standard need types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NeedType {
    Hunger,
    Sleep,
    Safety,
    Social,
    Comfort,
    Fun,
}

/// Collection of needs for an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Needs {
    pub values: HashMap<NeedType, Need>,
}

impl Needs {
    /// Create default human-like needs.
    pub fn human() -> Self {
        let mut values = HashMap::new();
        values.insert(NeedType::Hunger, Need::new(80.0, 0.05, 1.5));
        values.insert(NeedType::Sleep, Need::new(100.0, 0.02, 1.2));
        values.insert(NeedType::Safety, Need::new(100.0, 0.0, 2.0));
        values.insert(NeedType::Social, Need::new(70.0, 0.01, 0.6));
        values.insert(NeedType::Comfort, Need::new(60.0, 0.01, 0.4));
        values.insert(NeedType::Fun, Need::new(50.0, 0.015, 0.5));
        Self { values }
    }

    /// Tick all needs.
    pub fn tick(&mut self) {
        for need in self.values.values_mut() {
            need.tick();
        }
    }

    /// Get the most urgent need.
    pub fn most_urgent(&self) -> Option<(NeedType, f32)> {
        self.values
            .iter()
            .max_by(|a, b| a.1.urgency().partial_cmp(&b.1.urgency()).unwrap())
            .map(|(&t, n)| (t, n.urgency()))
    }

    /// Get a specific need value.
    pub fn get(&self, need: NeedType) -> f32 {
        self.values.get(&need).map_or(0.0, |n| n.value)
    }

    /// Satisfy a specific need.
    pub fn satisfy(&mut self, need: NeedType, amount: f32) {
        if let Some(n) = self.values.get_mut(&need) {
            n.satisfy(amount);
        }
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// An action an agent can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentAction {
    Idle,
    Eat,
    Sleep,
    Build,
    Harvest,
    Fight,
    Flee,
    Trade,
    Socialize,
    Explore,
    Worship,
    Craft,
    Gather,
}

impl AgentAction {
    /// All possible actions.
    pub fn all() -> &'static [AgentAction] {
        &[
            Self::Idle,
            Self::Eat,
            Self::Sleep,
            Self::Build,
            Self::Harvest,
            Self::Fight,
            Self::Flee,
            Self::Trade,
            Self::Socialize,
            Self::Explore,
            Self::Worship,
            Self::Craft,
            Self::Gather,
        ]
    }
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

/// A remembered location of interest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub position: SimVec2,
    /// What's at this location (food_source, danger, shelter, resource, etc.).
    pub tag: String,
    /// When this was last observed (sim tick).
    pub last_seen: u64,
    /// Confidence 0.0-1.0 (decays over time as info becomes stale).
    pub confidence: f32,
}

/// Agent memory: remembered locations, relationships, and knowledge.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentMemory {
    pub locations: Vec<MemoryEntry>,
    /// Relationship scores with other agents (-100 to +100).
    pub relationships: HashMap<u64, f32>,
}

impl AgentMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Remember a location.
    pub fn remember_location(&mut self, pos: SimVec2, tag: impl Into<String>, tick: u64) {
        let tag = tag.into();
        // Update existing or add new.
        if let Some(entry) = self.locations.iter_mut().find(|e| e.tag == tag) {
            entry.position = pos;
            entry.last_seen = tick;
            entry.confidence = 1.0;
        } else {
            self.locations.push(MemoryEntry {
                position: pos,
                tag,
                last_seen: tick,
                confidence: 1.0,
            });
        }
    }

    /// Find the nearest remembered location with the given tag.
    pub fn find_nearest(&self, pos: SimVec2, tag: &str) -> Option<&MemoryEntry> {
        self.locations
            .iter()
            .filter(|e| e.tag == tag && e.confidence > 0.1)
            .min_by(|a, b| {
                let da = dist_sq(pos, a.position);
                let db = dist_sq(pos, b.position);
                da.partial_cmp(&db).unwrap()
            })
    }

    /// Decay old memories (call periodically).
    pub fn decay(&mut self, current_tick: u64, decay_per_tick: f32) {
        for entry in &mut self.locations {
            let age = current_tick.saturating_sub(entry.last_seen) as f32;
            entry.confidence = (1.0 - age * decay_per_tick).max(0.0);
        }
        // Remove forgotten entries.
        self.locations.retain(|e| e.confidence > 0.01);
    }

    /// Adjust relationship with another entity.
    pub fn adjust_relationship(&mut self, entity_id: u64, delta: f32) {
        let score = self.relationships.entry(entity_id).or_insert(0.0);
        *score = (*score + delta).clamp(-100.0, 100.0);
    }

    /// Get relationship score with another entity (0.0 = neutral).
    pub fn relationship(&self, entity_id: u64) -> f32 {
        self.relationships.get(&entity_id).copied().unwrap_or(0.0)
    }
}

fn dist_sq(a: SimVec2, b: SimVec2) -> f64 {
    let dx = f64::from(a.x - b.x);
    let dy = f64::from(a.y - b.y);
    dx * dx + dy * dy
}

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

/// An autonomous agent with needs, memory, and utility-based decision making.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub entity: EntityId,
    pub archetype: String,
    pub needs: Needs,
    pub memory: AgentMemory,
    pub current_action: AgentAction,
    /// How many ticks the agent has been doing the current action.
    pub action_ticks: u32,
}

impl Agent {
    pub fn new(entity: EntityId, archetype: impl Into<String>, needs: Needs) -> Self {
        Self {
            entity,
            archetype: archetype.into(),
            needs,
            memory: AgentMemory::new(),
            current_action: AgentAction::Idle,
            action_ticks: 0,
        }
    }

    /// Tick needs and choose the best action.
    pub fn update(&mut self, ctx: &AgentWorldContext) {
        self.needs.tick();
        self.action_ticks += 1;

        // Evaluate all actions and pick the highest scoring one.
        let best = self.evaluate_actions(ctx);
        if best != self.current_action {
            self.current_action = best;
            self.action_ticks = 0;
        }
    }

    /// Score all actions and return the best one.
    pub fn evaluate_actions(&self, ctx: &AgentWorldContext) -> AgentAction {
        let mut best_action = AgentAction::Idle;
        let mut best_score = f32::NEG_INFINITY;

        for &action in AgentAction::all() {
            let score = self.score_action(action, ctx);
            if score > best_score {
                best_score = score;
                best_action = action;
            }
        }

        best_action
    }

    /// Score a single action based on needs and context.
    fn score_action(&self, action: AgentAction, ctx: &AgentWorldContext) -> f32 {
        let hunger_urgency = self.needs.values.get(&NeedType::Hunger).map_or(0.0, |n| n.urgency());
        let sleep_urgency = self.needs.values.get(&NeedType::Sleep).map_or(0.0, |n| n.urgency());
        let safety_urgency = self.needs.values.get(&NeedType::Safety).map_or(0.0, |n| n.urgency());
        let social_urgency = self.needs.values.get(&NeedType::Social).map_or(0.0, |n| n.urgency());

        match action {
            AgentAction::Eat => {
                if ctx.food_nearby {
                    hunger_urgency * 2.0
                } else {
                    hunger_urgency * 0.5 // Still want to, but can't easily
                }
            }
            AgentAction::Sleep => sleep_urgency * 1.8,
            AgentAction::Flee => {
                if ctx.danger_nearby {
                    safety_urgency * 3.0 // Very high priority
                } else {
                    0.0
                }
            }
            AgentAction::Fight => {
                if ctx.danger_nearby && safety_urgency < 0.5 {
                    1.0 // Fight if not too scared
                } else {
                    0.0
                }
            }
            AgentAction::Socialize => {
                if ctx.agents_nearby > 0 {
                    social_urgency * 1.5
                } else {
                    0.0
                }
            }
            AgentAction::Trade => {
                if ctx.agents_nearby > 0 {
                    0.5 // Moderate baseline
                } else {
                    0.0
                }
            }
            AgentAction::Harvest | AgentAction::Gather => {
                if ctx.resources_nearby {
                    0.8 + hunger_urgency * 0.5 // Proactive resource gathering
                } else {
                    0.2
                }
            }
            AgentAction::Build => {
                if self.needs.get(NeedType::Safety) < 50.0 {
                    0.6 // Build shelter when not safe
                } else {
                    0.3
                }
            }
            AgentAction::Craft => 0.4,
            AgentAction::Explore => {
                // Explore when all needs are reasonably satisfied.
                let max_urgency = hunger_urgency
                    .max(sleep_urgency)
                    .max(safety_urgency)
                    .max(social_urgency);
                if max_urgency < 0.3 {
                    0.7
                } else {
                    0.1
                }
            }
            AgentAction::Worship => 0.1,
            AgentAction::Idle => 0.05, // Lowest priority fallback
        }
    }
}

/// World context visible to an agent for decision-making.
pub struct AgentWorldContext {
    pub food_nearby: bool,
    pub danger_nearby: bool,
    pub resources_nearby: bool,
    pub agents_nearby: u32,
    pub current_tick: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    fn test_ctx(food: bool, danger: bool) -> AgentWorldContext {
        AgentWorldContext {
            food_nearby: food,
            danger_nearby: danger,
            resources_nearby: false,
            agents_nearby: 0,
            current_tick: 0,
        }
    }

    #[test]
    fn hungry_agent_eats() {
        let mut needs = Needs::human();
        needs.values.get_mut(&NeedType::Hunger).unwrap().value = 10.0; // Very hungry.
        let agent = Agent::new(EntityId::from_raw(0, 0), "human", needs);

        let ctx = test_ctx(true, false);
        let action = agent.evaluate_actions(&ctx);
        assert_eq!(action, AgentAction::Eat);
    }

    #[test]
    fn danger_triggers_flee() {
        let mut needs = Needs::human();
        needs.values.get_mut(&NeedType::Safety).unwrap().value = 20.0; // Scared.
        let agent = Agent::new(EntityId::from_raw(0, 0), "human", needs);

        let ctx = test_ctx(false, true);
        let action = agent.evaluate_actions(&ctx);
        assert_eq!(action, AgentAction::Flee);
    }

    #[test]
    fn satisfied_agent_explores() {
        let mut needs = Needs::human();
        // All needs satisfied.
        for n in needs.values.values_mut() {
            n.value = 95.0;
        }
        let agent = Agent::new(EntityId::from_raw(0, 0), "human", needs);

        let ctx = test_ctx(false, false);
        let action = agent.evaluate_actions(&ctx);
        assert_eq!(action, AgentAction::Explore);
    }

    #[test]
    fn needs_decay_over_time() {
        let mut needs = Needs::human();
        let initial_hunger = needs.get(NeedType::Hunger);
        for _ in 0..100 {
            needs.tick();
        }
        assert!(needs.get(NeedType::Hunger) < initial_hunger);
    }

    #[test]
    fn memory_remember_and_find() {
        let mut mem = AgentMemory::new();
        let pos = SimVec2::from_f32(100.0, 200.0);
        mem.remember_location(pos, "food_source", 10);

        let query_pos = SimVec2::from_f32(110.0, 210.0);
        let found = mem.find_nearest(query_pos, "food_source");
        assert!(found.is_some());
    }

    #[test]
    fn memory_decay() {
        let mut mem = AgentMemory::new();
        let pos = SimVec2::ZERO;
        mem.remember_location(pos, "old_camp", 0);

        mem.decay(10000, 0.001);
        // After many ticks, confidence should be very low.
        assert!(mem.locations.is_empty() || mem.locations[0].confidence < 0.1);
    }

    #[test]
    fn relationship_tracking() {
        let mut mem = AgentMemory::new();
        mem.adjust_relationship(42, 10.0);
        mem.adjust_relationship(42, 5.0);
        assert_eq!(mem.relationship(42), 15.0);
        assert_eq!(mem.relationship(99), 0.0); // Unknown
    }

    #[test]
    fn need_urgency() {
        let critical = Need::new(5.0, 0.05, 1.5);
        let satisfied = Need::new(95.0, 0.05, 1.5);
        assert!(critical.urgency() > satisfied.urgency());
    }

    #[test]
    fn agent_update_changes_action() {
        let mut needs = Needs::human();
        needs.values.get_mut(&NeedType::Hunger).unwrap().value = 10.0;
        let mut agent = Agent::new(EntityId::from_raw(0, 0), "human", needs);

        let ctx = test_ctx(true, false);
        agent.update(&ctx);
        assert_eq!(agent.current_action, AgentAction::Eat);
    }
}
