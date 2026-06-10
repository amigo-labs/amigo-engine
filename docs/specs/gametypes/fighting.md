---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/input"]
last_updated: 2026-06-09
---

# Fighting Game

## Purpose

Template for 2D fighting games built on per-frame data: every move is a list of frames with explicit startup/active/recovery phases, hitboxes and hurtboxes are rectangles resolved per frame, and hitstun/blockstun drive the frame-advantage math that defines a matchup. Includes motion-input recognition (quarter circles, dragon punches) and a damage-scaled combo tracker.

Examples: Street Fighter (frame data, special-move motions, meter), Guilty Gear (chain combos, damage scaling), Mortal Kombat (high/low guard mixups), Skullgirls (long scaled combos).

Implementation: `crates/amigo_core/src/fighting.rs`.

## Public API

### Frame Data

```rust
/// Phase of an attack animation frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePhase {
    Startup,    // Can't hit yet, can't cancel.
    Active,     // Hitbox is out.
    Recovery,   // Attack over, can't act yet.
}

/// A single frame of animation data for a move.
#[derive(Clone, Debug)]
pub struct FrameData {
    pub phase: FramePhase,
    pub hitboxes: Vec<HitBox>,              // Empty if Startup/Recovery.
    pub hurtbox_override: Option<Rect>,     // None = use fighter default.
    pub velocity: RenderVec2,               // Movement applied this frame.
    pub cancellable: bool,                  // Can cancel into another move.
    pub invincible: bool,
    pub super_armor: bool,                  // Takes damage but doesn't flinch.
}

impl FrameData {
    pub fn startup() -> Self;
    pub fn active(hitbox: HitBox) -> Self;
    pub fn recovery() -> Self;
    // Builders:
    pub fn with_velocity(self, vx: f32, vy: f32) -> Self;
    pub fn with_cancellable(self) -> Self;
    pub fn with_invincible(self) -> Self;
}
```

### Hitboxes and Hurtboxes

```rust
/// Attack hitbox with damage properties. `rect` is relative to the
/// fighter's position and mirrors automatically when facing left.
#[derive(Clone, Debug)]
pub struct HitBox {
    pub rect: Rect,
    pub damage: i32,
    pub hitstun: u32,           // Stun frames on hit (default 12).
    pub blockstun: u32,         // Stun frames on block (default 6).
    pub knockback: RenderVec2,  // Direction and force (default (3, 0)).
    pub hit_type: HitType,
    pub guard_type: GuardType,
}

impl HitBox {
    pub fn new(rect: Rect, damage: i32) -> Self;
    // Builders: with_hitstun, with_blockstun, with_knockback(x, y),
    //           with_hit_type, with_guard_type.
    /// Hitbox rect in world space given fighter position and facing.
    pub fn world_rect(&self, pos: RenderVec2, facing_right: bool) -> Rect;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitType { Normal, Launch, Sweep, Grab, Projectile }

/// Overhead (block standing), low (block crouching), mid, or unblockable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardType { Mid, High, Low, Unblockable }
```

### MoveDef

```rust
/// A complete move (normal, special, super) as a frame list.
#[derive(Clone, Debug)]
pub struct MoveDef {
    pub name: String,
    pub frames: Vec<FrameData>,
    pub can_chain: bool,        // Can combo into the next move on hit.
    pub priority: i32,          // Higher wins in trades.
}

impl MoveDef {
    pub fn new(name: impl Into<String>) -> Self;
    // Builders:
    pub fn with_frame(self, frame: FrameData) -> Self;
    pub fn with_startup(self, count: u32) -> Self;
    pub fn with_active(self, count: u32, hitbox: HitBox) -> Self;
    pub fn with_recovery(self, count: u32) -> Self;
    // Frame-data queries:
    pub fn total_frames(&self) -> usize;
    pub fn startup_frames(&self) -> usize;
    pub fn active_frames(&self) -> usize;
    pub fn recovery_frames(&self) -> usize;
    /// Frame advantage on hit: hitstun - recovery (positive = attacker
    /// acts first). On block: blockstun - recovery.
    pub fn frame_advantage_hit(&self) -> i32;
    pub fn frame_advantage_block(&self) -> i32;
}
```

### Input Buffer and Motions

```rust
/// Numpad-style directional input (already facing-relative).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputDir {
    Neutral, Up, UpForward, Forward, DownForward,
    Down, DownBack, Back, UpBack,
}

/// A single input event (direction + buttons). Implements Default
/// (neutral direction, no buttons).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputFrame {
    pub direction: InputDir,
    pub light: bool,
    pub medium: bool,
    pub heavy: bool,
    pub special: bool,
}

impl InputFrame {
    pub fn new() -> Self;
    pub fn any_button(&self) -> bool;
}

/// Ring-style input buffer for motion detection.
pub struct InputBuffer { /* buffer: Vec<InputFrame>, capacity */ }

impl InputBuffer {
    pub fn new(capacity: usize) -> Self;
    pub fn push(&mut self, input: InputFrame);
    pub fn clear(&mut self);
    /// Check if a command motion was performed within the last
    /// `window` frames (in-order subsequence match, gaps allowed).
    pub fn check_motion(&self, motion: &[InputDir], window: usize) -> bool;
    pub fn last(&self) -> Option<&InputFrame>;
}

/// Standard fighting game motions (fighting::motions).
pub mod motions {
    pub fn qcf() -> Vec<InputDir>;        // ↓↘→  Hadouken
    pub fn qcb() -> Vec<InputDir>;        // ↓↙←
    pub fn dp() -> Vec<InputDir>;         // →↓↘  Shoryuken
    pub fn hcf() -> Vec<InputDir>;        // ←↙↓↘→
    pub fn hcb() -> Vec<InputDir>;        // →↘↓↙←
    pub fn double_qcf() -> Vec<InputDir>; // Super motion
    pub fn spd() -> Vec<InputDir>;        // 360 command grab
}
```

### Combo Tracking

```rust
pub struct ComboHit { pub move_name: String, pub damage: i32, pub hit_number: u32 }

/// Tracks the current combo with damage scaling. Implements Default.
#[derive(Clone, Debug)]
pub struct ComboTracker {
    pub hits: Vec<ComboHit>,
    pub total_damage: i32,
    pub scaling: f32,           // Starts 1.0, decays per hit.
    pub min_scaling: f32,       // Floor, default 0.1.
    pub decay_per_hit: f32,     // Default 0.1.
    pub active: bool,
    pub gap_timer: u32,
    pub max_gap: u32,           // Default 30 frames (~0.5s).
}

impl ComboTracker {
    pub fn new() -> Self;
    /// Register a hit. Returns the scaled damage actually dealt.
    pub fn add_hit(&mut self, move_name: impl Into<String>, base_damage: i32) -> i32;
    /// Call every frame; drops the combo after max_gap frames without a hit.
    pub fn update(&mut self);
    pub fn end(&mut self);
    pub fn reset(&mut self);
    pub fn hit_count(&self) -> u32;
}
```

### Fighter

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FighterState {
    Idle, Walking, Crouching, Jumping, Attacking, Blocking,
    HitStun, BlockStun, KnockDown, GettingUp, Grabbed,
}

/// Runtime state for a fighter.
#[derive(Clone, Debug)]
pub struct Fighter {
    pub position: RenderVec2,
    pub velocity: RenderVec2,
    pub facing_right: bool,
    pub state: FighterState,
    pub hp: i32,
    pub max_hp: i32,
    pub meter: f32,             // Super meter, max_meter default 100.
    pub max_meter: f32,
    pub hurtbox: Rect,          // Default hurtbox, relative to position.
    pub current_move: Option<usize>,    // Index into the move list.
    pub current_frame: usize,
    pub stun_timer: u32,
    pub grounded: bool,
    pub combo: ComboTracker,    // This fighter's offense.
}

impl Fighter {
    pub fn new(position: RenderVec2, hp: i32) -> Self;
    /// Facing-mirrored hurtbox in world space.
    pub fn world_hurtbox(&self) -> Rect;
    pub fn start_move(&mut self, move_index: usize);
    /// Advance the current move by one frame; applies frame velocity
    /// (facing-mirrored). Returns the executed frame index, or None
    /// when the move ends (state returns to Idle).
    pub fn advance_move(&mut self, moves: &[MoveDef]) -> Option<usize>;
    /// Non-mutating frame-data lookup for the current move.
    pub fn get_frame<'a>(&self, moves: &'a [MoveDef], frame_idx: usize) -> Option<&'a FrameData>;
    /// Enter HitStun with facing-relative knockback velocity.
    pub fn apply_hitstun(&mut self, frames: u32, knockback: RenderVec2);
    pub fn apply_blockstun(&mut self, frames: u32);
    /// Tick stun; returns to Idle and zeroes velocity at 0.
    pub fn update_stun(&mut self);
    pub fn is_alive(&self) -> bool;
    /// True in Idle/Walking/Crouching — may start a new move.
    pub fn can_act(&self) -> bool;
    pub fn is_blocking(&self) -> bool;          // Blocking or BlockStun.
    pub fn add_meter(&mut self, amount: f32);   // Clamped to max_meter.
    pub fn spend_meter(&mut self, amount: f32) -> bool;
}

/// Check if any of a frame's hitboxes overlap a fighter's hurtbox.
/// Returns the first overlapping hitbox.
pub fn check_hit<'a>(
    attacker_pos: RenderVec2,
    attacker_facing_right: bool,
    frame: &'a FrameData,
    defender: &Fighter,
) -> Option<&'a HitBox>;
```

## Behavior

- **Per-tick flow** (60 Hz): push this frame's `InputFrame` into each fighter's `InputBuffer` → if `can_act()`, test motions (`check_motion(&motions::dp(), 10)` etc., longest motion first) plus buttons and `start_move` on a match → `advance_move` for attacking fighters → for each executed `Active` frame, `check_hit(pos, facing, frame, defender)` → on hit, `defender.apply_hitstun(hitbox.hitstun, hitbox.knockback)` (or `apply_blockstun` if blocking and `guard_type` matches the guard) and `attacker.combo.add_hit(...)` → `update_stun` and `combo.update` for both.
- **Frame advantage**: `frame_advantage_hit/block` derive +/- frames from the first active frame's hitstun/blockstun minus recovery — a jab with 12 hitstun and 6 recovery is +6 on hit, its 5 blockstun makes it -1 on block. This is the data balance designers tune.
- **Guard logic**: the template carries `GuardType` (Mid/High/Low/Unblockable) on each hitbox; the standing/crouching block check itself is game code (compare against `FighterState::Crouching`).
- **Combo scaling**: each `add_hit` multiplies base damage by the current `scaling`, then decays it by `decay_per_hit` down to `min_scaling` — an 80-damage second hit deals 72. Combos drop automatically after `max_gap` frames without a hit.
- **Facing**: hitboxes, hurtboxes, frame velocity, and knockback all mirror horizontally via `facing_right`; move data is authored once for the right-facing case.

## Internal Design

- A move is *data only* (`Vec<FrameData>`); there is no animation-system coupling. Games map `current_move`/`current_frame` to sprite frames via [engine/animation](../engine/animation.md).
- `check_motion` is an in-order subsequence scan over the last `window` buffered frames, tolerant of intermediate directions — lenient like modern fighters, no per-direction charge timing.
- `Fighter` uses `RenderVec2`/`f32` math. Deterministic lockstep netplay would require migrating to the fixed-point `SimVec2` path used by the platformer template.
- `advance_move` returns the executed frame index and `get_frame` is a separate non-mutating lookup, so hit detection can borrow frame data while the attacker is no longer mutably borrowed.

## Future Work

- Throw/grab resolution: `HitType::Grab` exists but there is no throw-tech or grab-state transition logic (`FighterState::Grabbed` is set by game code).
- Push-boxes and corner collision (fighters can currently overlap; spacing is game code).
- Projectile entities for `HitType::Projectile` moves (spawning/ownership not modeled; pair with the projectile or bullet-pattern systems).
- Cancel tables: `FrameData::cancellable` is a per-frame bool; real cancel rules (which moves cancel into which) need a table.
- Knockdown/wakeup timing data for `KnockDown`/`GettingUp` states.

## Open Questions

- Should frame data be RON-serializable for an external frame-data editor (currently not `Serialize`)?
- Should `check_hit` return all overlapping hitboxes for multi-hit moves instead of the first?
- Charge motions (hold back, then forward) — extend `check_motion` with per-step minimum durations?

## Referenzen

- [engine/input](../engine/input.md) — Raw input mapped into `InputFrame`s
- [engine/core](../engine/core.md) — `Rect`, `RenderVec2`
- [engine/animation](../engine/animation.md) — Sprite playback driven by move frames
- [engine/state-rewind](../engine/state-rewind.md) — Candidate basis for rollback netcode
- Street Fighter series — Frame advantage conventions, motion inputs
- Guilty Gear — Chain combos, damage scaling curves
- Skullgirls — Scaling floors for long combos
