---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/simulation", "engine/pathfinding"]
last_updated: 2026-03-18
---

# Agent AI (Autonomous)

## Purpose

Utility-based autonomous agent system for God Sim and Sandbox NPC behavior. Agents have needs that decay over time (hunger, sleep, safety, social), evaluate available actions via utility scoring, maintain spatial memory of resources and dangers, and track relationships with other agents. Designed to scale to hundreds of concurrent agents with LOD-based simulation (full detail nearby, statistical updates at distance).

## Public API

Existing implementation in `crates/amigo_core/src/agents.rs`.

### Need

```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Need {
    pub value: f32,
    pub decay_rate: f32,
    pub weight: f32,
}

impl Need {
    pub fn new(value: f32, decay_rate: f32, weight: f32) -> Self;
    pub fn tick(&mut self);
    pub fn urgency(&self) -> f32;
    pub fn satisfy(&mut self, amount: f32);
}
```

### NeedType & Needs

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NeedType {
    Hunger, Sleep, Safety, Social, Comfort, Fun,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Needs {
    pub values: HashMap<NeedType, Need>,
}

impl Needs {
    pub fn human() -> Self;
    pub fn tick(&mut self);
    pub fn most_urgent(&self) -> Option<(NeedType, f32)>;
    pub fn get(&self, need: NeedType) -> f32;
    pub fn satisfy(&mut self, need: NeedType, amount: f32);
}
```

### AgentAction

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentAction {
    Idle, Eat, Sleep, Build, Harvest, Fight, Flee,
    Trade, Socialize, Explore, Worship, Craft, Gather,
}

impl AgentAction {
    pub fn all() -> &'static [AgentAction];
}
```

### AgentMemory

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub position: SimVec2,
    pub tag: String,
    pub last_seen: u64,
    pub confidence: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentMemory {
    pub locations: Vec<MemoryEntry>,
    pub relationships: HashMap<u64, f32>,
}

impl AgentMemory {
    pub fn new() -> Self;
    pub fn remember_location(&mut self, pos: SimVec2, tag: impl Into<String>, tick: u64);
    pub fn find_nearest(&self, pos: SimVec2, tag: &str) -> Option<&MemoryEntry>;
    pub fn decay(&mut self, current_tick: u64, decay_per_tick: f32);
    pub fn adjust_relationship(&mut self, entity_id: u64, delta: f32);
    pub fn relationship(&self, entity_id: u64) -> f32;
}
```

### Agent

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub entity: EntityId,
    pub archetype: String,
    pub needs: Needs,
    pub memory: AgentMemory,
    pub current_action: AgentAction,
    pub action_ticks: u32,
}

impl Agent {
    pub fn new(entity: EntityId, archetype: impl Into<String>, needs: Needs) -> Self;
    pub fn update(&mut self, ctx: &AgentWorldContext);
    pub fn evaluate_actions(&self, ctx: &AgentWorldContext) -> AgentAction;
}
```

### AgentWorldContext

```rust
pub struct AgentWorldContext {
    pub food_nearby: bool,
    pub danger_nearby: bool,
    pub resources_nearby: bool,
    pub agents_nearby: u32,
    pub current_tick: u64,
}
```

## Behavior

- **Need decay:** Each `tick()` call decreases need values by their `decay_rate`. Values clamp to `[0.0, 100.0]`. A need at 0.0 is critical; at 100.0 is fully satisfied.
- **Urgency:** Computed as `(1.0 - value / 100.0) * weight`. Higher urgency means the agent prioritizes satisfying that need.
- **Utility scoring:** `evaluate_actions` scores every action in `AgentAction::all()` against current needs and world context. The highest-scoring action wins. Scoring heuristics:
  - `Eat`: scales with hunger urgency, doubled when food is nearby.
  - `Flee`: very high priority (3x safety urgency) when danger is nearby.
  - `Explore`: chosen when all needs are well-satisfied (max urgency < 0.3).
  - `Idle`: lowest baseline (0.05) as fallback.
- **Action switching:** `update()` ticks needs, evaluates actions, and switches `current_action` if a better option emerges (resetting `action_ticks`).
- **Memory:** Agents remember tagged locations (food sources, shelters, dangers) with confidence that decays over time. `find_nearest` filters by confidence > 0.1 and returns the closest match.
- **Relationships:** Stored as `entity_id -> f32` scores in `[-100, 100]`. Adjusted by game events (trade, combat, social interactions).

## Internal Design

- Utility scoring is a simple `match` on `AgentAction` with per-action formulas referencing need urgencies and context booleans. This is intentionally straightforward for easy tuning.
- Memory uses linear search over a `Vec<MemoryEntry>`; suitable for typical agent memory sizes (10-50 entries).
- The `AgentWorldContext` is constructed by the game layer per agent per tick, querying spatial data (nearby entities, resources) from the ECS.
- Agents use [pathfinding](pathfinding.md) for navigation. For God Sim with many agents heading to the same target, flow fields are preferred over individual A*.

## Non-Goals

- **Behavior trees in diesem Crate.** Utility AI bleibt eigenständig. BTs sind in [engine/behavior-tree](behavior-tree.md) als Ergänzung spezifiziert — Utility AI entscheidet *was*, BT steuert *wie*.
- **Action execution.** The agent system decides *what* to do; the game layer implements *how* (animation, movement, resource consumption).
- **Group AI / settlements.** Agent grouping, shared resource pools, and settlement management are game-layer concerns.

## Open Questions

- Should agent archetypes (human, animal, monster) be data-driven with configurable need sets and action score weights?
- How should LOD simulation work for distant agents -- skip `evaluate_actions` and use statistical approximation?
- Should the scoring function be configurable per archetype (e.g., animals don't score `Craft` or `Trade`)?
