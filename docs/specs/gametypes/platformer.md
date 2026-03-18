---
status: spec
crate: --
depends_on: ["engine/physics", "engine/tween"]
last_updated: 2026-03-18
---

# Platformer / Jump'n'Run

## Purpose

Template for 2D platformer games with tight, responsive controls. Covers the core "game feel" mechanics that distinguish a good platformer from a sluggish one: coyote time, jump buffering, variable jump height, wall mechanics, and dash systems. Target games: Celeste, Hollow Knight, Dead Cells, Super Meat Boy.

Game developers use these systems as composable building blocks -- a simple platformer might use only `PlatformerController`, while a Celeste-like adds `WallMechanics` and `DashSystem` on top.

## Public API

### PlatformerConfig

```rust
/// Tuning constants for platformer feel. All frame values are in fixed-timestep ticks (60 Hz).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlatformerConfig {
    /// Horizontal movement speed (pixels/tick in I16F16).
    pub move_speed: I16F16,
    /// Maximum fall speed (terminal velocity).
    pub max_fall_speed: I16F16,
    /// Gravity applied per tick.
    pub gravity: I16F16,
    /// Initial upward velocity on jump.
    pub jump_speed: I16F16,
    /// Gravity multiplier when jump button is released early (variable jump height).
    /// Typical value: 3.0 (cuts jump short quickly).
    pub jump_cut_multiplier: I16F16,
    /// Frames after leaving ground where jump is still allowed.
    pub coyote_frames: u8,         // default: 6
    /// Frames before landing where a jump input is remembered.
    pub jump_buffer_frames: u8,    // default: 8
    /// Horizontal acceleration (0.0 = instant, 1.0 = sluggish).
    pub ground_accel: I16F16,
    /// Horizontal deceleration when no input is held.
    pub ground_decel: I16F16,
    /// Air control multiplier (fraction of ground accel applied in air).
    pub air_control: I16F16,       // default: 0.7
}
```

### PlatformerState

```rust
/// Runtime state for the platformer controller. Attached as a component to the player entity.
#[derive(Clone, Debug, Default)]
pub struct PlatformerState {
    /// Remaining coyote time frames (counts down from coyote_frames when leaving ground).
    pub coyote_timer: u8,
    /// Remaining jump buffer frames (counts down from jump_buffer_frames on jump press).
    pub jump_buffer_timer: u8,
    /// Whether the player is currently on the ground.
    pub grounded: bool,
    /// Whether the jump button is currently held.
    pub jump_held: bool,
    /// Whether the player has used their jump since last grounding.
    pub jump_used: bool,
    /// Current facing direction (-1 or 1).
    pub facing: i8,
    /// Current velocity.
    pub velocity: SimVec2,
}
```

### PlatformerController

```rust
/// Core platformer update logic. Stateless -- operates on PlatformerState + PlatformerConfig.
pub struct PlatformerController;

impl PlatformerController {
    /// Process one tick of platformer movement.
    /// `input_x`: horizontal input (-1, 0, or 1).
    /// `jump_pressed`: true on the frame the jump button was pressed.
    /// `jump_held`: true while the jump button is held down.
    /// Returns the desired velocity delta to apply to the RigidBody.
    pub fn tick(
        state: &mut PlatformerState,
        config: &PlatformerConfig,
        input_x: i8,
        jump_pressed: bool,
        jump_held: bool,
        on_ground: bool,
    ) -> SimVec2;
}
```

### WallMechanics

```rust
/// Wall interaction configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WallConfig {
    /// Downward speed while sliding against a wall (slower than free-fall).
    pub wall_slide_speed: I16F16,
    /// Maximum duration in frames the player can cling to a wall without input.
    pub wall_cling_frames: u8,      // default: 0 (disabled). Set >0 for Mega Man X style.
    /// Horizontal velocity applied on wall jump (away from wall).
    pub wall_jump_horizontal: I16F16,
    /// Vertical velocity applied on wall jump.
    pub wall_jump_vertical: I16F16,
    /// Frames after wall jump where horizontal input is dampened
    /// (prevents immediately re-clinging to the same wall).
    pub wall_jump_lock_frames: u8,  // default: 8
}

/// Runtime state for wall mechanics. Attached alongside PlatformerState.
#[derive(Clone, Debug, Default)]
pub struct WallState {
    /// Which wall the player is touching: -1 (left), 0 (none), 1 (right).
    pub wall_contact: i8,
    /// Whether the player is currently wall-sliding.
    pub wall_sliding: bool,
    /// Remaining wall cling frames.
    pub wall_cling_timer: u8,
    /// Remaining wall jump input lock frames.
    pub wall_jump_lock_timer: u8,
}

pub struct WallMechanics;

impl WallMechanics {
    /// Process wall interactions for one tick.
    /// `wall_left`, `wall_right`: collision sensor results.
    /// Returns velocity override if wall sliding or wall jumping.
    pub fn tick(
        wall_state: &mut WallState,
        plat_state: &mut PlatformerState,
        config: &WallConfig,
        input_x: i8,
        jump_pressed: bool,
        wall_left: bool,
        wall_right: bool,
    ) -> Option<SimVec2>;
}
```

### DashSystem

```rust
/// Dash configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashConfig {
    /// Dash speed (pixels/tick).
    pub dash_speed: I16F16,
    /// Duration of dash in frames.
    pub dash_duration: u8,          // default: 8
    /// Cooldown frames after dash ends before another dash is allowed.
    pub dash_cooldown: u8,          // default: 12
    /// Number of air dashes allowed before touching ground. 0 = ground only.
    pub air_dashes: u8,             // default: 1
    /// Coyote frames for dash (can dash shortly after leaving ground).
    pub coyote_dash_frames: u8,     // default: 4
    /// Whether gravity is suspended during dash.
    pub freeze_gravity: bool,       // default: true
}

/// Dash direction resolved from input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashDirection {
    Left,
    Right,
    Up,
    Down,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

/// Runtime state for the dash system.
#[derive(Clone, Debug, Default)]
pub struct DashState {
    /// Whether the player is currently dashing.
    pub dashing: bool,
    /// Remaining dash duration frames.
    pub dash_timer: u8,
    /// Remaining cooldown frames.
    pub cooldown_timer: u8,
    /// Number of air dashes used since last grounding.
    pub air_dashes_used: u8,
    /// Coyote dash timer (counts down from coyote_dash_frames on leaving ground).
    pub coyote_dash_timer: u8,
    /// Direction of current dash.
    pub dash_direction: Option<DashDirection>,
}

pub struct DashSystem;

impl DashSystem {
    /// Process dash input for one tick.
    /// `dash_pressed`: true on the frame the dash button was pressed.
    /// `input_dir`: current directional input for dash aiming.
    /// Returns velocity override during active dash, or None.
    pub fn tick(
        dash_state: &mut DashState,
        config: &DashConfig,
        dash_pressed: bool,
        input_dir: (i8, i8),
        on_ground: bool,
    ) -> Option<SimVec2>;

    /// Reset air dash count (call when player touches ground).
    pub fn reset_on_ground(dash_state: &mut DashState);
}
```

### PlatformType

```rust
/// Types of platforms with distinct collision behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlatformType {
    /// Standard solid platform. Blocks movement from all directions.
    Solid,
    /// Can be passed through from below and sides. Only blocks from above.
    OneWay,
    /// Moves along a path. Carries the player when standing on it.
    Moving {
        /// Waypoints the platform moves between (in world coordinates).
        path: Vec<SimVec2>,
        /// Movement speed (pixels/tick).
        speed: I16F16,
        /// Whether the platform loops or ping-pongs.
        loops: bool,
    },
    /// Collapses after being stood on. Respawns after a cooldown.
    Crumbling {
        /// Frames after stepping on before the platform breaks.
        break_delay: u8,
        /// Frames before the platform respawns. 0 = never respawns.
        respawn_delay: u16,
    },
}

/// Runtime state for a crumbling platform.
#[derive(Clone, Debug, Default)]
pub struct CrumblingState {
    pub break_timer: u8,
    pub respawn_timer: u16,
    pub broken: bool,
    pub player_touching: bool,
}
```

### SquashStretch

```rust
/// Visual feedback component for juicy platformer feel.
/// Applied to the player sprite's scale via the tween system.
#[derive(Clone, Debug)]
pub struct SquashStretch {
    /// Current scale X multiplier (1.0 = normal).
    pub scale_x: I16F16,
    /// Current scale Y multiplier.
    pub scale_y: I16F16,
    /// How quickly the scale returns to normal (0.0-1.0, higher = faster).
    pub recovery_speed: I16F16,
}

impl SquashStretch {
    pub fn new() -> Self;

    /// Apply a landing squash (wide and short). Magnitude based on fall speed.
    pub fn on_land(&mut self, fall_speed: I16F16);

    /// Apply a jump stretch (tall and narrow).
    pub fn on_jump(&mut self);

    /// Apply a dash stretch in the dash direction.
    pub fn on_dash(&mut self, direction: DashDirection);

    /// Tick toward neutral scale. Call every frame.
    pub fn tick(&mut self);

    /// Current scale as (x, y) for the sprite renderer.
    pub fn scale(&self) -> (I16F16, I16F16);
}
```

## Behavior

### Coyote Time

When the player walks off a platform edge (transitions from grounded to airborne without jumping), `coyote_timer` is set to `coyote_frames`. Each tick it decrements by 1. If a jump input arrives while `coyote_timer > 0`, the jump executes as if the player were still grounded. Once `coyote_timer` hits 0, the window closes and the player must land before jumping again (unless wall jump is available).

### Jump Buffering

When the player presses jump while airborne, `jump_buffer_timer` is set to `jump_buffer_frames`. Each tick it decrements. If the player lands while `jump_buffer_timer > 0`, the buffered jump executes immediately on the landing frame. This prevents "eaten inputs" where the player presses jump 1-2 frames too early.

### Variable Jump Height

While the player is ascending (`velocity.y < 0`) and `jump_held` is false (button released), gravity is multiplied by `jump_cut_multiplier`. This creates a short hop on a quick tap and a full-height jump on a held press. The multiplier is only applied during the ascending phase to avoid affecting fall speed.

### Wall Slide and Wall Jump

When the player is airborne and pressing into a wall (detected by collision sensors), they enter a wall slide. Fall speed is clamped to `wall_slide_speed`. If `wall_cling_frames > 0`, the player sticks to the wall for that duration even without directional input.

Wall jump applies `wall_jump_vertical` upward and `wall_jump_horizontal` away from the wall. For `wall_jump_lock_frames` after a wall jump, the horizontal input is dampened to prevent the player from immediately re-clinging. The coyote time timer is reset on wall jump, allowing sequences of wall jumps.

### Dash

On dash press (if cooldown is 0 and air dashes remain), the player enters the dashing state. For `dash_duration` frames, the player moves at `dash_speed` in the resolved `DashDirection`. If `freeze_gravity` is true, gravity has no effect during dash. After the dash ends, `cooldown_timer` is set to `dash_cooldown`. Air dashes are reset when the player touches the ground.

Coyote dash: `coyote_dash_timer` starts counting down when the player leaves the ground. If dash is pressed within this window, it does not consume an air dash charge.

### Moving Platforms

Moving platforms interpolate between waypoints at their configured `speed`. Ping-pong platforms reverse direction at each endpoint; looping platforms wrap from the last waypoint back to the first.

**Carry Mechanics:** When a player stands on a moving platform (detected by ground sensor hitting the platform's `Kinematic` RigidBody), the carry system applies the platform's delta position to the player before the controller tick:

```
Tick Order:
1. Moving platforms update their positions along waypoints → compute delta_pos
2. For each entity on a platform (ground sensor hit platform):
   entity.position += platform.delta_pos       // carry
3. PlatformerController::tick() runs (ground detection, input, velocity)
4. Tile collision resolution (ensures player doesn't clip into walls after carry)
```

The `delta_pos` is computed in `SimVec2` for determinism. If the carry would push the player into a wall (detected by `sensor()`), the player is detached from the platform (prevents crushing). An optional `crush_damage` can be configured per `PlatformType::Moving`.

### Crumbling Platforms

When `player_touching` becomes true, `break_timer` starts counting down from `break_delay`. Visual shake feedback should be applied during this countdown. When the timer hits 0, `broken` is set to true and the platform disables its collision. If `respawn_delay > 0`, `respawn_timer` counts down and the platform re-enables when it reaches 0.

## Internal Design

### Physics Integration

The platformer controller operates on kinematic bodies — it computes velocity directly rather than applying forces. The controller does NOT call `PhysicsWorld::step()`. Instead:

1. **Controller** computes desired velocity in `SimVec2` (Fixed-Point).
2. **Collision queries** use `sensor()` and `raycast_tiles()` from [engine/physics](../engine/physics.md) for ground/wall detection.
3. **Velocity** is converted to `RenderVec2` via `SimVec2::to_f32()` and written to `RigidBody.velocity`.
4. **Position update** is tile-collision-based (not physics-step-based): the controller moves the entity and checks `CollisionLayer` per axis.

Ground detection: `sensor(feet_pos, DOWN, 1.0, collision_layer, tile_size)` — returns `true` if a solid or one-way tile is 1px below.
Wall detection: `sensor(side_pos, LEFT/RIGHT, 1.0, collision_layer, tile_size)` — returns `true` for wall contact.
OneWay platforms: `raycast_tiles()` reports `CollisionType::OneWay` — controller ignores if `velocity.y < 0` (moving upward).
Slopes: `raycast_tiles()` returns exact hit point interpolated from `Slope { left_height, right_height }`. Controller snaps player Y to slope surface while grounded.

### Fixed-Point Consistency

All velocities, positions, and configuration values use `I16F16` fixed-point arithmetic via `SimVec2`. This ensures deterministic behavior across platforms and enables state rewind / replay.

**SimVec2 ↔ RenderVec2 Boundary:** The controller runs entirely in SimVec2. The single conversion point is writing the result to `RigidBody.velocity` (which is `RenderVec2`). This happens once per tick at the end of the platformer update. No intermediate conversions.

### ECS Layout

Each player entity carries these components:

- `PlatformerState` + `PlatformerConfig` (always present)
- `WallState` + `WallConfig` (optional, for wall mechanics)
- `DashState` + `DashConfig` (optional, for dash)
- `SquashStretch` (optional, visual feedback)
- `RigidBody` (from physics system)
- `AnimPlayer` (from animation system, state-driven)

The platformer tick runs in this order: `DashSystem::tick` -> `WallMechanics::tick` -> `PlatformerController::tick` -> apply velocity to `RigidBody` -> `SquashStretch::tick`.

### Camera Integration

The camera uses `Follow` mode with a vertical deadzone so the camera does not jerk on every small jump. Horizontal look-ahead shifts the camera in the facing direction. Landing on a new platform triggers a smooth vertical catch-up rather than a snap.

## Non-Goals

- **Rope / grapple physics.** Swinging and grappling hook mechanics are separate systems not covered here.
- **Swimming / water physics.** Buoyancy and underwater movement require different tuning and are a separate template.
- **Procedural animation.** Squash/stretch is scale-based only; skeletal deformation is not included.
- **Networked multiplayer platforming.** Input prediction and rollback for competitive platformers is not addressed.
- **Slope movement.** Slope handling is delegated to the tilemap collision layer (`CollisionLayer::Slope`); the platformer controller treats slopes as ground.

## Open Questions

- Should the controller support double-jump as a built-in option (with configurable count), or should it be layered on top by game code?
- Should moving platforms use spline paths (`CatmullRom` from the spline system) instead of linear waypoint interpolation?
- Should the dash cancel into wall slide, or should wall slide be suppressed during dash cooldown?
- How should the controller interact with external velocity sources (knockback, conveyor belts, wind zones)?

## Referenzen

- [engine/physics](../engine/physics.md) -- Kinematic rigid bodies, AABB collision
- [engine/tween](../engine/tween.md) -- Scale tweening for squash/stretch
- [engine/camera](../engine/camera.md) -- Follow mode with deadzone
- [engine/animation](../engine/animation.md) -- State-based sprite animation (idle, run, jump, fall, dash)
- [engine/tilemap](../engine/tilemap.md) -- CollisionLayer for OneWay and Slope platforms
- Celeste (Maddy Thorson) -- Coyote time, jump buffer, dash, wall mechanics
- Super Meat Boy -- Tight air control, wall jump chains
- Hollow Knight -- Variable jump height, dash cooldown
