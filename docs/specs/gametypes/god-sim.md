---
status: done
crate: amigo_core
depends_on: ["engine/agents", "engine/simulation", "engine/core"]
last_updated: 2026-06-09
---

# God Sim

## Purpose

God games and colony simulations: the player does not control units directly. Autonomous agents with decaying needs (hunger, sleep, safety, social) choose their own actions via utility scoring, remember resource locations, form relationships, and work station-based tasks, while the player observes, nudges, and adjusts simulation speed (pause through 10x).

Examples: Dwarf Fortress (autonomous dwarves with needs and jobs), RimWorld (colonist needs, work stations), Black & White (worshipping, god intervention), WorldBox (watch-the-world simulation speeds).

Backed by `amigo_core::agents`, `amigo_core::task_system`, `amigo_core::economy`, with the tick loop and speed control from `amigo_core::simulation`.

## Public API

### Needs (`amigo_core::agents`)

```rust
/// A single need: 0.0 = critical, 100.0 = fully satisfied.
pub struct Need {
    pub value: f32,
    pub decay_rate: f32, // decrease per sim-tick
    pub weight: f32,     // priority in utility scoring
}
impl Need {
    pub fn new(value: f32, decay_rate: f32, weight: f32) -> Self;
    pub fn tick(&mut self);
    pub fn urgency(&self) -> f32; // (1 - value/100) * weight
    pub fn satisfy(&mut self, amount: f32);
}

pub enum NeedType { Hunger, Sleep, Safety, Social, Comfort, Fun }

/// Collection of needs per agent.
pub struct Needs { pub values: HashMap<NeedType, Need> }
impl Needs {
    pub fn human() -> Self; // preset decay rates and weights for human-like NPCs
    pub fn tick(&mut self);
    pub fn most_urgent(&self) -> Option<(NeedType, f32)>;
    pub fn get(&self, need: NeedType) -> f32;
    pub fn satisfy(&mut self, need: NeedType, amount: f32);
}
```

### Agents and Utility AI (`amigo_core::agents`)

```rust
/// Actions an agent can take.
pub enum AgentAction { Idle, Eat, Sleep, Build, Harvest, Fight, Flee, Trade,
                       Socialize, Explore, Worship, Craft, Gather }

/// An autonomous agent with needs, memory, and utility-based decision making.
pub struct Agent {
    pub entity: EntityId,
    pub archetype: String,      // e.g. "human", "villager"
    pub needs: Needs,
    pub memory: AgentMemory,
    pub current_action: AgentAction,
    pub action_ticks: u32,      // ticks spent on the current action
}
impl Agent {
    pub fn new(entity: EntityId, archetype: impl Into<String>, needs: Needs) -> Self;
    /// Tick needs and switch to the highest-scoring action.
    pub fn update(&mut self, ctx: &AgentWorldContext);
    /// Score all actions against needs + context, return the best.
    pub fn evaluate_actions(&self, ctx: &AgentWorldContext) -> AgentAction;
}

/// What the agent can perceive this tick (filled by the game from world queries).
pub struct AgentWorldContext {
    pub food_nearby: bool,
    pub danger_nearby: bool,
    pub resources_nearby: bool,
    pub agents_nearby: u32,
    pub current_tick: u64,
}
```

Built-in scoring (in `Agent::evaluate_actions`): `Flee` dominates when danger is near and safety is low (3x safety urgency); `Eat` scales with hunger urgency (doubled when food is nearby); `Sleep` with sleep urgency; `Socialize`/`Trade` require nearby agents; `Harvest`/`Gather` favor nearby resources; `Build` rises when safety is below 50; `Explore` wins only when all urgencies are below 0.3; `Idle` is the 0.05-score fallback.

### Memory and Relationships (`amigo_core::agents`)

```rust
/// A remembered point of interest with staleness decay.
pub struct MemoryEntry { pub position: SimVec2, pub tag: String,
                         pub last_seen: u64, pub confidence: f32 }

pub struct AgentMemory {
    pub locations: Vec<MemoryEntry>,
    pub relationships: HashMap<u64, f32>, // -100..+100 per other agent
}
impl AgentMemory {
    pub fn remember_location(&mut self, pos: SimVec2, tag: impl Into<String>, tick: u64);
    pub fn find_nearest(&self, pos: SimVec2, tag: &str) -> Option<&MemoryEntry>; // e.g. "food_source"
    pub fn decay(&mut self, current_tick: u64, decay_per_tick: f32); // forgets stale entries
    pub fn adjust_relationship(&mut self, entity_id: u64, delta: f32);
    pub fn relationship(&self, entity_id: u64) -> f32;
}
```

### Settlement Work: Stations and Tasks (`amigo_core::task_system`)

```rust
pub struct TaskId(pub u32);
pub struct StationId(pub u32); // corresponds to a TriggerZone id in the tilemap

/// Definition of work performed at a station (well, farm plot, shrine, workshop).
pub struct TaskDef {
    pub id: TaskId, pub name: String,
    pub station_id: StationId,
    pub duration: f32,                  // seconds; 0.0 = instant
    pub eligibility: TaskEligibility,   // Anyone | Team(u8) | Entities(Vec<EntityId>)
    pub repeatable: bool,
}
pub struct TaskRegistry { /* register/get/by_station */ }

/// Live task instances with progress tracking.
pub struct TaskState { /* tasks: Vec<TaskInstance> */ }
impl TaskState {
    pub fn spawn_tasks(&mut self, registry: &TaskRegistry);
    /// Agent starts work; checks eligibility; instant tasks complete immediately.
    pub fn begin_task(&mut self, station_id: StationId, worker: EntityId,
                      team: u8, registry: &TaskRegistry) -> bool;
    pub fn interrupt(&mut self, worker: EntityId);            // fled/died -> back to Available
    pub fn update(&mut self, dt: f32) -> Vec<TaskEvent>;      // Progressed / Completed events
    pub fn completion_count(&self) -> (u32, u32);
    pub fn all_complete(&self) -> bool;
    pub fn task_at_station(&self, station_id: StationId) -> Option<&TaskInstance>;
    pub fn reset(&mut self);
}
```

### Settlement Economy (`amigo_core::economy`)

```rust
/// Shared treasury with auditable transaction ledger.
pub struct Economy { pub gold: i32, pub score: u64, pub interest_rate: f32, pub interest_cap: i32, /* ... */ }
impl Economy {
    pub fn new(starting_gold: i32, starting_lives: i32) -> Self;
    pub fn set_tick(&mut self, tick: u64);                                  // stamp transactions
    pub fn add_gold(&mut self, amount: i32, kind: TransactionKind) -> i32;  // harvest/trade income
    pub fn try_spend(&mut self, cost: i32, kind: TransactionKind) -> bool;  // construction costs
    pub fn apply_interest(&mut self);                                       // periodic treasury growth
    pub fn history(&self) -> &[Transaction];                                // ledger UI
}
// TransactionKind::Custom { tag } labels god-sim sources ("tithe", "trade", "construction").
```

### Simulation Speed (`amigo_core::simulation`)

```rust
/// Paused, Normal (1x), Fast (2x), VeryFast (5x), Ultra (10x), Custom(f32).
pub enum SimSpeed { Paused, Normal, Fast, VeryFast, Ultra, Custom(f32) }
impl SimSpeed { pub fn multiplier(self) -> f32; pub fn next(self) -> Self; }

/// Fixed-rate sim loop decoupled from FPS, designed for God Sim / Sandbox games.
pub struct SimulationRunner { pub ticks_per_second: u32, pub speed: SimSpeed,
                              pub tick: u64, pub max_ticks_per_frame: u32 }
impl SimulationRunner {
    pub fn new(ticks_per_second: u32) -> Self;
    pub fn add_system(&mut self, system: Box<dyn SimSystem>); // priority-ordered, per-system tick_interval
    pub fn set_speed(&mut self, speed: SimSpeed);
    pub fn toggle_speed(&mut self);
    pub fn advance(&mut self, real_dt: f64) -> u32; // runs 0..max_ticks_per_frame ticks
}
```

## Behavior

- **Agent tick**: An agent `SimSystem` runs each sim tick (or every N ticks via `tick_interval`). The game fills an `AgentWorldContext` per agent from spatial queries, then calls `Agent::update()`. The chosen `AgentAction` is executed by game systems: `Eat` walks to `memory.find_nearest(pos, "food_source")` via pathfinding and calls `needs.satisfy(Hunger, ...)`; `Harvest`/`Build`/`Craft` map to `TaskState::begin_task` at the matching station.
- **Work loop**: `TaskState::update(dt)` advances all in-progress tasks. `TaskEvent::Completed` produces output (food stores, buildings, `Economy::add_gold`). If an agent flees or dies, `interrupt(worker)` resets the station to `Available` for the next agent.
- **Social fabric**: `Socialize`/`Trade` outcomes adjust `AgentMemory::adjust_relationship`; relationship scores can bias future target selection (allies vs. rivals). `AgentMemory::decay` runs periodically so stale knowledge (moved resources, dead threats) fades out.
- **God speed**: All of the above runs inside `SimulationRunner::advance` so the player can pause or run at up to 10x; `max_ticks_per_frame` caps catch-up to avoid the spiral of death. Need decay is per-tick, so faster speeds naturally accelerate the world.

## Future Work

- **Settlements as first-class objects**: there is no settlement/village entity (population rosters, territory, storage piles). Settlements are emergent — built from `Agent` groups, `TaskDef` stations, and a shared `Economy` in game code.
- No reproduction/aging/lifecycle for agents, and no archetype-specific scoring — `score_action` is fixed; archetype only labels the agent.
- No divine-intervention verb set (miracles, terrain shaping); model these as game systems that mutate needs/world and let the utility AI react.
- `Worship` is scored at a flat 0.1 — faith mechanics need game-side scoring overrides.

## Referenzen

- [engine/agents](../engine/agents.md) — utility AI architecture and FSM counterpart
- [engine/simulation](../engine/simulation.md) — SimSystem registration and tick budgeting
- [engine/pathfinding](../engine/pathfinding.md) — agents walking to remembered locations and stations
- [gametypes/city-builder](city-builder.md) — population/economy systems for denser settlement sims
- Dwarf Fortress — needs-driven autonomous labor
- RimWorld — work stations, interruption, colonist moods
- WorldBox — simulation speed as the primary player verb
