---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/simulation", "engine/fog-of-war"]
last_updated: 2026-06-09
---

# Social Deduction

## Purpose

Multiplayer games of hidden roles: most players (crewmates) complete tasks while a hidden minority (impostors) kills and sabotages. The core loop alternates free movement (tasks, kills, sabotage) with discussion and voting phases where players try to identify and eject the impostors. The genre lives on limited information — per-player vision, locked doors, and deterministic vote resolution.

Examples: Among Us (tasks + kill cooldowns + emergency meetings), Project Winter (survival twist), Town of Salem (pure vote-driven deduction).

Implemented in `crates/amigo_core/src/social_deduction.rs` (game flow) and `crates/amigo_core/src/voting.rs` (reusable voting subsystem).

## Public API

### Roles and Player State

```rust
/// The role assigned to a player for this round.
pub enum Role { Crewmate, Impostor }

/// Per-player state within a social deduction game.
pub struct PlayerState {
    pub entity: EntityId,
    pub role: Role,
    pub alive: bool, pub ejected: bool,
    pub kill_cooldown: Cooldown,       // combat::Cooldown
    pub sabotage_cooldown: Cooldown,
    pub can_use_vents: bool,           // true for impostors
    pub vision_radius: u32,
}
```

### Phases, Config, and State

```rust
/// Phases of a social deduction round.
pub enum Phase { Setup, Playing, Discussion { remaining: f32 }, Voting, Ejection, Results }

/// Reason the game ended.
pub enum WinCondition {
    TasksComplete,        // crew finished all tasks
    AllImpostorsEjected,  // crew win
    ImpostorParity,       // alive impostors >= alive crewmates
    SabotageUnresolved,   // critical sabotage timer expired
}

/// Configuration for a social deduction game.
pub struct SdConfig {
    pub impostor_count: u8,
    pub kill_cooldown: f32, pub sabotage_cooldown: f32,
    pub discussion_duration: f32, pub voting_duration: f32,
    pub crewmate_vision_radius: u32, pub impostor_vision_radius: u32,
    pub emergency_meetings_per_player: u8,
    pub kill_range: f32,
    pub seed: u64,
}
// Default: 1 impostor, 25s kill / 30s sabotage cooldown, 15s discussion,
// 30s voting, vision 5/7, 1 emergency meeting, kill range 2.0.

/// Top-level state for a social deduction game.
pub struct SdState {
    pub config: SdConfig,
    pub phase: Phase,
    pub players: Vec<PlayerState>,
    pub tasks: TaskState,                  // task_system::TaskState
    pub doors: DoorManager,                // door::DoorManager
    pub vote_session: VotingSession,
    pub active_sabotage: Option<ActiveSabotage>,
    pub emergency_meetings_remaining: Vec<(EntityId, u8)>,
    pub round_number: u32,
    pub winner: Option<(Role, WinCondition)>,
}

impl SdState {
    pub fn new(config: SdConfig) -> Self;
}
```

### Sabotage

```rust
/// Types of sabotage an impostor can trigger.
pub enum SabotageKind {
    /// Locks doors in a specified room for a duration.
    DoorLock { door_id: DoorId, duration: f32 },
    /// Disables a critical system; crew must repair within time limit.
    CriticalSystem { task_id: TaskId, time_limit: f32 },
    /// Reduces all crewmate vision radii.
    Lights { reduced_radius: u32, duration: f32 },
    /// Disables communications.
    Comms { duration: f32 },
}

/// A sabotage event in progress.
pub struct ActiveSabotage {
    pub kind: SabotageKind,
    pub timer: f32,
    pub resolved: bool,
}
```

### System Functions

```rust
/// Assign roles to players using the config seed (deterministic Fisher-Yates).
/// Transitions Setup → Playing.
pub fn assign_roles(state: &mut SdState, entities: &[EntityId]) -> Vec<SdEvent>;

/// Main tick function. Call every frame. Drives cooldowns, doors, sabotage
/// timers, discussion countdown, vote resolution, and win-condition checks.
pub fn sd_tick(state: &mut SdState, dt: f32) -> Vec<SdEvent>;

/// Report a dead body. Transitions Playing → Discussion, cancels sabotage.
pub fn report_body(state: &mut SdState, reporter: EntityId, body: EntityId) -> Vec<SdEvent>;

/// Call an emergency meeting (limited per player by config).
pub fn call_emergency_meeting(state: &mut SdState, caller: EntityId) -> Vec<SdEvent>;

/// Impostor attempts to kill a target. Validates role, cooldown, kill_range.
pub fn attempt_kill(state: &mut SdState, killer: EntityId, victim: EntityId, distance: f32) -> Vec<SdEvent>;

/// Impostor triggers a sabotage (one active at a time; respects cooldown).
pub fn trigger_sabotage(state: &mut SdState, impostor: EntityId, kind: SabotageKind) -> Vec<SdEvent>;

/// Cast a vote during the Voting phase (delegates to VotingSession).
pub fn cast_vote(state: &mut SdState, voter: EntityId, choice: u32) -> Vec<SdEvent>;

/// Check win conditions (tasks complete, impostors eliminated, parity).
pub fn check_win_conditions(state: &SdState) -> Option<(Role, WinCondition)>;

/// Effective vision radius (accounts for Lights sabotage; impostors unaffected).
pub fn effective_vision_radius(state: &SdState, entity: EntityId) -> u32;

/// Which entities the observer can see (radius + line-of-sight via
/// vision_ray::can_see against a TileQuery).
pub fn visible_entities(state: &SdState, observer: EntityId,
    all_positions: &[(EntityId, IVec2)], tiles: &dyn TileQuery) -> Vec<EntityId>;
```

### Events

```rust
/// Events specific to the social deduction genre.
pub enum SdEvent {
    RolesAssigned { impostor_count: u8 },
    PhaseChanged { from: Phase, to: Phase },
    BodyReported { reporter: EntityId, body: EntityId },
    EmergencyMeeting { caller: EntityId },
    PlayerEjected { entity: EntityId, was_impostor: bool },
    SabotageStarted { kind: SabotageKind }, SabotageResolved { kind: SabotageKind },
    PlayerKilled { killer: EntityId, victim: EntityId },
    GameOver { winner: Role, condition: WinCondition },
}
```

### Voting Subsystem (voting.rs)

```rust
/// A ballot cast by a voter. Choice semantics are defined by the caller.
pub struct Ballot { pub voter: EntityId, pub choice: u32 }

/// Result of tallying votes.
pub enum VoteOutcome {
    Decided { winner: u32, votes: u32 },
    Tie { choices: Vec<u32>, votes: u32 },
    NoVotes, Skipped,
}

pub enum VotePhase { Inactive, Open, Tallying, Resolved }

/// Configuration for a voting session.
pub struct VoteConfig {
    pub duration: f32,             // seconds voting is open (default 30)
    pub skip_choice: Option<u32>,  // choice ID meaning "skip" (default Some(0))
    pub plurality_wins: bool,      // false = strict majority (>50%) required
    pub warning_threshold: f32,    // remaining time at which TimeWarning fires
}

/// Manages a single voting session.
pub struct VotingSession { /* private: config, phase, ballots, eligible, timer, outcome */ }

impl VotingSession {
    pub fn new() -> Self;
    pub fn start(&mut self, eligible: &[EntityId], config: VoteConfig);
    /// Cast a ballot. Returns false if ineligible, already voted, or not Open.
    pub fn cast(&mut self, voter: EntityId, choice: u32) -> bool;
    pub fn has_voted(&self, voter: EntityId) -> bool;
    /// Tick the timer. Auto-closes when all eligible voters voted or time expires.
    pub fn update(&mut self, dt: f32) -> Vec<VoteEvent>;
    pub fn close_early(&mut self);
    pub fn phase(&self) -> VotePhase;
    pub fn outcome(&self) -> Option<&VoteOutcome>;
    pub fn vote_counts(&self) -> (u32, u32);  // (eligible, ballots cast)
    pub fn reset(&mut self);
}

/// Pure, deterministic tally function (BTreeMap-based for stable ordering).
pub fn tally(ballots: &[Ballot], skip_choice: Option<u32>, plurality_wins: bool) -> VoteOutcome;

pub enum VoteEvent {
    Started { eligible_voters: u32 },
    BallotCast { voter: EntityId, choice: u32 },
    TimeWarning { seconds_remaining: f32 },
    Closed, Resolved { outcome: VoteOutcome },
}
```

## Behavior

- **Phase Flow**: `assign_roles()` seeds a deterministic shuffle to pick impostors, then enters `Playing`. Bodies reported or emergency meetings transition to `Discussion { remaining }`; when the timer expires, `sd_tick()` starts a `VotingSession` among alive, non-ejected players and enters `Voting`. When the session resolves, an `Ejection` phase processes the outcome (choice `0` = skip, otherwise the choice is the entity index of the player to eject) and the game returns to `Playing` or ends.
- **Sabotage**: Only one sabotage may be active at a time. `DoorLock` immediately calls `DoorManager::lock()`. `CriticalSystem` is lethal: if its timer reaches zero unresolved, impostors win via `SabotageUnresolved`. `Lights` reduces crewmate vision through `effective_vision_radius()` without touching `PlayerState`. Reporting a body or calling a meeting cancels any active sabotage.
- **Win Conditions**: Checked each tick and after every ejection — crewmates win on `TasksComplete` (via `TaskState::all_complete()`) or `AllImpostorsEjected`; impostors win on `ImpostorParity` or `SabotageUnresolved`.
- **Visibility**: `visible_entities()` combines the effective radius with tile-based line-of-sight (`vision_ray::can_see`), so impostors keep full vision during Lights while crewmates are blinded.
- **Determinism**: Role assignment, and vote tallying (`tally()` uses a `BTreeMap`) are fully deterministic for a given seed/ballot order — required for lockstep multiplayer.

## Internal Design

- `SdState` composes existing engine systems instead of reimplementing them: `Cooldown` (combat), `TaskState` (task-system), `DoorManager` (doors), `VotingSession` (voting), `vision_ray` (fog-of-war style LoS). All state is `Serialize`/`Deserialize` for save and network sync.
- All system functions are free functions of the form `fn(&mut SdState, ...) -> Vec<SdEvent>`; the caller (game loop / netcode) consumes events for presentation and replication.
- The internal RNG is a xorshift64 seeded from `SdConfig::seed`.
- `VotingSession` is genre-agnostic and reusable (e.g. for council votes in a strategy game); choice IDs are opaque `u32`s whose semantics are defined by the caller.

## Future Work

- Vent traversal: `PlayerState::can_use_vents` exists, but there is no vent graph or movement function yet.
- Additional roles (Sheriff, Jester, etc.) — `Role` is currently a two-variant enum.
- Comms sabotage currently only runs a timer; hiding task progress UI is left to the game layer.
- Anonymous vs. revealed ballots and vote-weight modifiers.

## Referenzen

- Among Us: tasks, kill/sabotage cooldowns, emergency meetings, vision cones
- Town of Salem: pure voting loops, role variety (future work)
- [engine/core](../engine/core.md) → EntityId, event flow
- [engine/fog-of-war](../engine/fog-of-war.md) → per-player vision concepts
- [engine/save-load](../engine/save-load.md) → SdState is fully serializable
