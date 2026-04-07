use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Platformer controller — coyote time, jump buffering, variable jump, etc.
// ---------------------------------------------------------------------------

/// Configuration for jump buffering. When the player presses jump slightly
/// before landing, the jump is still registered.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JumpBufferConfig {
    /// How many ticks a jump press stays buffered.
    pub buffer_ticks: u32,
}

impl Default for JumpBufferConfig {
    fn default() -> Self {
        Self { buffer_ticks: 6 }
    }
}

/// Configuration for coyote time. After walking off a ledge, the player
/// can still jump for a few ticks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoyoteConfig {
    /// How many ticks after leaving the ground the player can still jump.
    pub coyote_ticks: u32,
}

impl Default for CoyoteConfig {
    fn default() -> Self {
        Self { coyote_ticks: 5 }
    }
}

/// Configuration for variable-height jumping. Releasing the button early
/// cuts the jump short.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariableJumpConfig {
    /// Initial upward velocity when jumping (negative = up in typical coords).
    pub jump_velocity: f32,
    /// Velocity multiplier applied when the player releases jump early.
    /// e.g. 0.5 means the remaining upward velocity is halved.
    pub cut_multiplier: f32,
}

impl Default for VariableJumpConfig {
    fn default() -> Self {
        Self {
            jump_velocity: -6.0,
            cut_multiplier: 0.4,
        }
    }
}

/// Configuration for wall interactions (slide + jump).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WallConfig {
    /// Maximum downward speed while sliding on a wall.
    pub slide_speed: f32,
    /// Horizontal velocity applied when wall-jumping.
    pub jump_horizontal: f32,
    /// Vertical velocity applied when wall-jumping.
    pub jump_vertical: f32,
    /// Ticks after a wall-jump during which horizontal input is reduced.
    pub lock_ticks: u32,
}

impl Default for WallConfig {
    fn default() -> Self {
        Self {
            slide_speed: 1.5,
            jump_horizontal: 4.0,
            jump_vertical: -5.5,
            lock_ticks: 8,
        }
    }
}

/// Configuration for a dash/dodge move.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashConfig {
    /// Speed during dash (replaces normal velocity).
    pub dash_speed: f32,
    /// Duration of the dash in ticks.
    pub dash_duration: u32,
    /// Cooldown between dashes in ticks.
    pub cooldown: u32,
    /// If true, dash refreshes on landing.
    pub refresh_on_ground: bool,
}

impl Default for DashConfig {
    fn default() -> Self {
        Self {
            dash_speed: 10.0,
            dash_duration: 6,
            cooldown: 30,
            refresh_on_ground: true,
        }
    }
}

/// Full platformer controller configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlatformerConfig {
    pub move_speed: f32,
    pub acceleration: f32,
    pub deceleration: f32,
    pub air_acceleration: f32,
    pub max_fall_speed: f32,
    pub jump_buffer: JumpBufferConfig,
    pub coyote: CoyoteConfig,
    pub variable_jump: VariableJumpConfig,
    pub wall: Option<WallConfig>,
    pub dash: Option<DashConfig>,
    pub max_jumps: u32,
}

impl Default for PlatformerConfig {
    fn default() -> Self {
        Self {
            move_speed: 3.0,
            acceleration: 0.6,
            deceleration: 0.8,
            air_acceleration: 0.4,
            max_fall_speed: 8.0,
            jump_buffer: JumpBufferConfig::default(),
            coyote: CoyoteConfig::default(),
            variable_jump: VariableJumpConfig::default(),
            wall: None,
            dash: None,
            max_jumps: 1,
        }
    }
}

impl PlatformerConfig {
    pub fn with_wall(mut self, wall: WallConfig) -> Self {
        self.wall = Some(wall);
        self
    }

    pub fn with_dash(mut self, dash: DashConfig) -> Self {
        self.dash = Some(dash);
        self
    }

    pub fn with_max_jumps(mut self, n: u32) -> Self {
        self.max_jumps = n;
        self
    }
}

// ---------------------------------------------------------------------------
// Input snapshot — what the game feeds into the controller each tick
// ---------------------------------------------------------------------------

/// Input state for one tick of platformer control.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformerInput {
    /// Horizontal axis: -1.0 (left), 0.0 (none), 1.0 (right).
    pub move_x: f32,
    /// True on the tick jump is pressed.
    pub jump_pressed: bool,
    /// True while jump is held.
    pub jump_held: bool,
    /// True on the tick dash is pressed.
    pub dash_pressed: bool,
    /// True if there is a wall to the left of the player.
    pub wall_left: bool,
    /// True if there is a wall to the right of the player.
    pub wall_right: bool,
    /// True if the player is on the ground.
    pub on_ground: bool,
}

// ---------------------------------------------------------------------------
// Controller output — velocity the game should apply
// ---------------------------------------------------------------------------

/// Output of a controller tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformerOutput {
    pub velocity_x: f32,
    pub velocity_y: f32,
}

// ---------------------------------------------------------------------------
// Events produced by the controller
// ---------------------------------------------------------------------------

/// Events the controller can produce on a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformerEvent {
    Jumped,
    WallJumped,
    DoubleJumped,
    DashStarted,
    DashEnded,
    Landed,
    LeftGround,
}

// ---------------------------------------------------------------------------
// Controller state
// ---------------------------------------------------------------------------

/// Runtime state for the platformer controller.
#[derive(Clone, Debug)]
pub struct PlatformerController {
    pub config: PlatformerConfig,
    pub velocity_x: f32,
    pub velocity_y: f32,

    // Grounded tracking
    was_grounded: bool,
    coyote_counter: u32,

    // Jump buffer
    jump_buffer_counter: u32,

    // Variable jump
    jump_held: bool,

    // Multi-jump
    jumps_remaining: u32,

    // Wall
    wall_dir: i8, // -1 left, 0 none, 1 right
    wall_lock_counter: u32,

    // Dash
    dash_active: bool,
    dash_counter: u32,
    dash_cooldown_counter: u32,
    dash_dir_x: f32,
    dash_dir_y: f32,
    can_dash: bool,
}

impl PlatformerController {
    pub fn new(config: PlatformerConfig) -> Self {
        let max_jumps = config.max_jumps;
        Self {
            config,
            velocity_x: 0.0,
            velocity_y: 0.0,
            was_grounded: false,
            coyote_counter: 0,
            jump_buffer_counter: 0,
            jump_held: false,
            jumps_remaining: max_jumps,
            wall_dir: 0,
            wall_lock_counter: 0,
            dash_active: false,
            dash_counter: 0,
            dash_cooldown_counter: 0,
            dash_dir_x: 0.0,
            dash_dir_y: 0.0,
            can_dash: true,
        }
    }

    /// Process one tick of input, returning the desired velocity and events.
    pub fn tick(
        &mut self,
        input: PlatformerInput,
        gravity: f32,
    ) -> (PlatformerOutput, Vec<PlatformerEvent>) {
        let mut events = Vec::new();

        // --- Landing / leaving ground detection ---
        if input.on_ground && !self.was_grounded {
            events.push(PlatformerEvent::Landed);
            self.jumps_remaining = self.config.max_jumps;
            if self
                .config
                .dash
                .as_ref()
                .is_some_and(|d| d.refresh_on_ground)
            {
                self.can_dash = true;
            }
        }
        if !input.on_ground && self.was_grounded {
            events.push(PlatformerEvent::LeftGround);
            self.coyote_counter = self.config.coyote.coyote_ticks;
        }
        self.was_grounded = input.on_ground;

        // --- Coyote time ---
        if self.coyote_counter > 0 {
            self.coyote_counter -= 1;
        }
        let can_coyote = self.coyote_counter > 0 && !input.on_ground;

        // --- Jump buffer ---
        if input.jump_pressed {
            self.jump_buffer_counter = self.config.jump_buffer.buffer_ticks;
        }
        if self.jump_buffer_counter > 0 {
            self.jump_buffer_counter -= 1;
        }
        let buffered_jump = self.jump_buffer_counter > 0;

        // --- Wall detection ---
        if input.wall_left {
            self.wall_dir = -1;
        } else if input.wall_right {
            self.wall_dir = 1;
        } else {
            self.wall_dir = 0;
        }

        // --- Wall lock counter ---
        if self.wall_lock_counter > 0 {
            self.wall_lock_counter -= 1;
        }

        // --- Dash ---
        if self.dash_cooldown_counter > 0 {
            self.dash_cooldown_counter -= 1;
        }

        if let Some(ref dash_cfg) = self.config.dash {
            if input.dash_pressed
                && self.can_dash
                && self.dash_cooldown_counter == 0
                && !self.dash_active
            {
                self.dash_active = true;
                let dash_duration = dash_cfg.dash_duration;
                let dash_speed = dash_cfg.dash_speed;
                let cooldown = dash_cfg.cooldown;
                self.dash_counter = dash_duration;
                self.dash_dir_x = if input.move_x != 0.0 {
                    input.move_x.signum()
                } else {
                    1.0
                };
                self.dash_dir_y = 0.0;
                self.velocity_x = self.dash_dir_x * dash_speed;
                self.velocity_y = 0.0;
                self.dash_cooldown_counter = cooldown;
                self.can_dash = false;
                events.push(PlatformerEvent::DashStarted);
            }
        }

        if self.dash_active {
            self.dash_counter -= 1;
            if self.dash_counter == 0 {
                self.dash_active = false;
                self.velocity_x *= 0.3;
                events.push(PlatformerEvent::DashEnded);
            }
            let output = PlatformerOutput {
                velocity_x: self.velocity_x,
                velocity_y: self.velocity_y,
            };
            return (output, events);
        }

        // --- Jumping ---
        let wants_jump = buffered_jump || input.jump_pressed;
        let mut jumped_this_tick = false;

        // Wall jump
        if wants_jump && self.wall_dir != 0 && !input.on_ground {
            if let Some(ref wall_cfg) = self.config.wall {
                let wall_jump_h = wall_cfg.jump_horizontal;
                let wall_jump_v = wall_cfg.jump_vertical;
                let lock = wall_cfg.lock_ticks;
                self.velocity_y = wall_jump_v;
                self.velocity_x = -(self.wall_dir as f32) * wall_jump_h;
                self.wall_lock_counter = lock;
                self.jump_buffer_counter = 0;
                self.coyote_counter = 0;
                jumped_this_tick = true;
                events.push(PlatformerEvent::WallJumped);
            }
        }
        // Normal jump (grounded or coyote)
        else if wants_jump && (input.on_ground || can_coyote) {
            self.velocity_y = self.config.variable_jump.jump_velocity;
            self.jump_held = true;
            self.jump_buffer_counter = 0;
            self.coyote_counter = 0;
            jumped_this_tick = true;
            if self.jumps_remaining > 0 {
                self.jumps_remaining -= 1;
            }
            events.push(PlatformerEvent::Jumped);
        }
        // Multi-jump (in air, no wall, no coyote)
        else if wants_jump && !input.on_ground && !can_coyote && self.jumps_remaining > 0 {
            self.velocity_y = self.config.variable_jump.jump_velocity;
            self.jump_held = true;
            self.jump_buffer_counter = 0;
            self.jumps_remaining -= 1;
            jumped_this_tick = true;
            events.push(PlatformerEvent::DoubleJumped);
        }

        // --- Variable jump height (cut on release) ---
        if self.jump_held && !input.jump_held {
            if self.velocity_y < 0.0 {
                self.velocity_y *= self.config.variable_jump.cut_multiplier;
            }
            self.jump_held = false;
        }

        // --- Horizontal movement ---
        let target_vx = input.move_x * self.config.move_speed;
        let accel = if input.on_ground {
            if input.move_x.abs() > 0.01 {
                self.config.acceleration
            } else {
                self.config.deceleration
            }
        } else {
            self.config.air_acceleration
        };

        // Reduce horizontal control during wall lock
        let effective_accel = if self.wall_lock_counter > 0 {
            accel * 0.3
        } else {
            accel
        };

        // Move towards target velocity
        let diff = target_vx - self.velocity_x;
        if diff.abs() < effective_accel {
            self.velocity_x = target_vx;
        } else {
            self.velocity_x += diff.signum() * effective_accel;
        }

        // --- Vertical movement ---
        if jumped_this_tick {
            // Just jumped — don't apply gravity or ground zeroing this tick
        } else if !input.on_ground {
            // Apply gravity first
            self.velocity_y += gravity;

            // Wall slide — cap fall speed when touching a wall
            if self.wall_dir != 0 && self.velocity_y > 0.0 {
                if let Some(ref wall_cfg) = self.config.wall {
                    let slide_speed = wall_cfg.slide_speed;
                    self.velocity_y = self.velocity_y.min(slide_speed);
                }
            }

            self.velocity_y = self.velocity_y.min(self.config.max_fall_speed);
        } else {
            self.velocity_y = 0.0;
        }

        let output = PlatformerOutput {
            velocity_x: self.velocity_x,
            velocity_y: self.velocity_y,
        };
        (output, events)
    }

    pub fn is_grounded(&self) -> bool {
        self.was_grounded
    }

    pub fn is_dashing(&self) -> bool {
        self.dash_active
    }

    pub fn is_wall_sliding(&self) -> bool {
        self.wall_dir != 0 && self.velocity_y > 0.0
    }
}

// ---------------------------------------------------------------------------
// Moving platform — path-following kinematic body
// ---------------------------------------------------------------------------

/// How a moving platform traverses its waypoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathMode {
    /// Loop back to the first waypoint after reaching the last.
    Loop,
    /// Reverse direction at each end.
    PingPong,
    /// Stop at the last waypoint.
    Once,
}

/// A waypoint-following moving platform.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MovingPlatform {
    pub waypoints: Vec<(f32, f32)>,
    pub speed: f32,
    pub mode: PathMode,
    pub current_index: usize,
    pub t: f32,
    pub forward: bool,
    pub paused: bool,
    /// Ticks to wait at each waypoint. 0 = no wait.
    pub wait_ticks: u32,
    wait_counter: u32,
}

impl MovingPlatform {
    pub fn new(waypoints: Vec<(f32, f32)>, speed: f32, mode: PathMode) -> Self {
        Self {
            waypoints,
            speed,
            mode,
            current_index: 0,
            t: 0.0,
            forward: true,
            paused: false,
            wait_ticks: 0,
            wait_counter: 0,
        }
    }

    pub fn with_wait(mut self, ticks: u32) -> Self {
        self.wait_ticks = ticks;
        self
    }

    /// Advance the platform by one tick. Returns (current_x, current_y, delta_x, delta_y).
    /// The delta can be applied to riders (entities standing on the platform).
    pub fn tick(&mut self) -> (f32, f32, f32, f32) {
        if self.paused || self.waypoints.len() < 2 {
            let (x, y) = self.current_position();
            return (x, y, 0.0, 0.0);
        }

        if self.wait_counter > 0 {
            self.wait_counter -= 1;
            let (x, y) = self.current_position();
            return (x, y, 0.0, 0.0);
        }

        let (old_x, old_y) = self.current_position();

        let next_index = self.next_index();
        let (ax, ay) = self.waypoints[self.current_index];
        let (bx, by) = self.waypoints[next_index];
        let dx = bx - ax;
        let dy = by - ay;
        let segment_len = (dx * dx + dy * dy).sqrt();

        if segment_len < 0.001 {
            self.advance_waypoint();
            let (x, y) = self.current_position();
            return (x, y, 0.0, 0.0);
        }

        self.t += self.speed / segment_len;

        if self.t >= 1.0 {
            self.t = 0.0;
            self.advance_waypoint();
            self.wait_counter = self.wait_ticks;
        }

        let (new_x, new_y) = self.current_position();
        (new_x, new_y, new_x - old_x, new_y - old_y)
    }

    /// Get the current interpolated position.
    pub fn current_position(&self) -> (f32, f32) {
        if self.waypoints.is_empty() {
            return (0.0, 0.0);
        }
        if self.waypoints.len() == 1 {
            return self.waypoints[0];
        }
        let next = self.next_index();
        let (ax, ay) = self.waypoints[self.current_index];
        let (bx, by) = self.waypoints[next];
        (ax + (bx - ax) * self.t, ay + (by - ay) * self.t)
    }

    fn next_index(&self) -> usize {
        if self.forward {
            if self.current_index + 1 < self.waypoints.len() {
                self.current_index + 1
            } else {
                match self.mode {
                    PathMode::Loop => 0,
                    PathMode::PingPong | PathMode::Once => self.current_index,
                }
            }
        } else if self.current_index > 0 {
            self.current_index - 1
        } else {
            match self.mode {
                PathMode::Loop => self.waypoints.len() - 1,
                PathMode::PingPong | PathMode::Once => 0,
            }
        }
    }

    fn advance_waypoint(&mut self) {
        if self.forward {
            if self.current_index + 1 < self.waypoints.len() {
                self.current_index += 1;
            } else {
                match self.mode {
                    PathMode::Loop => self.current_index = 0,
                    PathMode::PingPong => {
                        self.forward = false;
                        if self.current_index > 0 {
                            self.current_index -= 1;
                        }
                    }
                    PathMode::Once => {} // stay
                }
            }
        } else if self.current_index > 0 {
            self.current_index -= 1;
        } else {
            match self.mode {
                PathMode::Loop => self.current_index = self.waypoints.len() - 1,
                PathMode::PingPong => {
                    self.forward = true;
                    if self.current_index + 1 < self.waypoints.len() {
                        self.current_index += 1;
                    }
                }
                PathMode::Once => {} // stay
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_input() -> PlatformerInput {
        PlatformerInput::default()
    }

    fn grounded_input() -> PlatformerInput {
        PlatformerInput {
            on_ground: true,
            ..Default::default()
        }
    }

    // ── Basic jumping ───────────────────────────────────────

    #[test]
    fn basic_jump() {
        let config = PlatformerConfig::default();
        let mut ctrl = PlatformerController::new(config);

        // Start grounded
        let (_, events) = ctrl.tick(grounded_input(), 0.5);
        assert!(events.contains(&PlatformerEvent::Landed));

        // Jump
        let input = PlatformerInput {
            on_ground: true,
            jump_pressed: true,
            jump_held: true,
            ..Default::default()
        };
        let (out, events) = ctrl.tick(input, 0.5);
        assert!(events.contains(&PlatformerEvent::Jumped));
        assert!(out.velocity_y < 0.0, "Should have upward velocity");
    }

    // ── Coyote time and jump buffer ─────────────────────────

    #[test]
    fn coyote_time() {
        let mut config = PlatformerConfig::default();
        config.coyote.coyote_ticks = 5;
        let mut ctrl = PlatformerController::new(config);

        // Be grounded for a tick
        ctrl.tick(grounded_input(), 0.5);

        // Leave ground (no jump)
        ctrl.tick(default_input(), 0.5);

        // Jump 2 ticks later (within coyote window)
        ctrl.tick(default_input(), 0.5);
        let input = PlatformerInput {
            jump_pressed: true,
            jump_held: true,
            ..Default::default()
        };
        let (out, events) = ctrl.tick(input, 0.5);
        assert!(
            events.contains(&PlatformerEvent::Jumped),
            "Coyote jump should work"
        );
        assert!(out.velocity_y < 0.0);
    }

    #[test]
    fn coyote_time_expires() {
        let mut config = PlatformerConfig::default();
        config.coyote.coyote_ticks = 2;
        config.max_jumps = 1;
        let mut ctrl = PlatformerController::new(config);

        // Be grounded
        ctrl.tick(grounded_input(), 0.5);
        // Leave ground
        ctrl.tick(default_input(), 0.5);
        // Wait past coyote window
        for _ in 0..5 {
            ctrl.tick(default_input(), 0.5);
        }
        // Try to jump — should fail (no coyote, no jumps remaining since we left ground)
        let input = PlatformerInput {
            jump_pressed: true,
            jump_held: true,
            ..Default::default()
        };
        let (_, events) = ctrl.tick(input, 0.5);
        assert!(
            !events.contains(&PlatformerEvent::Jumped),
            "Should not jump after coyote expires"
        );
    }

    #[test]
    fn jump_buffer() {
        let mut config = PlatformerConfig::default();
        config.jump_buffer.buffer_ticks = 5;
        config.max_jumps = 1;
        config.coyote.coyote_ticks = 0;
        let mut ctrl = PlatformerController::new(config);

        // Be grounded first, then jump to use up jumps_remaining
        ctrl.tick(grounded_input(), 0.5);
        ctrl.tick(
            PlatformerInput {
                on_ground: true,
                jump_pressed: true,
                jump_held: true,
                ..Default::default()
            },
            0.5,
        );

        // In the air, no jumps remaining
        for _ in 0..3 {
            ctrl.tick(default_input(), 0.5);
        }

        // Press jump while still in air — should buffer (no jumps remaining to use)
        ctrl.tick(
            PlatformerInput {
                jump_pressed: true,
                jump_held: true,
                ..Default::default()
            },
            0.5,
        );

        // Land 1 tick later (within buffer window)
        let (out, events) = ctrl.tick(
            PlatformerInput {
                on_ground: true,
                jump_held: true,
                ..Default::default()
            },
            0.5,
        );
        assert!(
            events.contains(&PlatformerEvent::Jumped),
            "Buffered jump should trigger on landing"
        );
        assert!(out.velocity_y < 0.0);
    }

    // ── Variable jump and multi-jump ────────────────────────

    #[test]
    fn variable_jump_height() {
        let config = PlatformerConfig::default();
        let mut ctrl = PlatformerController::new(config.clone());

        // Full jump: hold button
        ctrl.tick(grounded_input(), 0.5);
        ctrl.tick(
            PlatformerInput {
                on_ground: true,
                jump_pressed: true,
                jump_held: true,
                ..Default::default()
            },
            0.5,
        );
        // Keep holding for 2 ticks
        for _ in 0..2 {
            ctrl.tick(
                PlatformerInput {
                    jump_held: true,
                    ..Default::default()
                },
                0.5,
            );
        }
        let full_vy = ctrl.velocity_y;

        // Short jump: release button immediately after jump
        let mut ctrl2 = PlatformerController::new(config);
        ctrl2.tick(grounded_input(), 0.5);
        ctrl2.tick(
            PlatformerInput {
                on_ground: true,
                jump_pressed: true,
                jump_held: true,
                ..Default::default()
            },
            0.5,
        );
        // Release immediately — this should cut the velocity
        ctrl2.tick(
            PlatformerInput {
                jump_held: false,
                ..Default::default()
            },
            0.5,
        );
        // One more tick to match total tick count
        ctrl2.tick(default_input(), 0.5);
        let short_vy = ctrl2.velocity_y;

        // Short jump should have more downward velocity (velocity cut happened)
        // Full: -6.0 + gravity*2 = -6.0 + 1.0 = -5.0
        // Short: -6.0 * 0.4 = -2.4, then -2.4 + gravity*2 = -2.4 + 1.0 = -1.4
        assert!(
            short_vy > full_vy,
            "Short jump should lose momentum faster: short={short_vy} full={full_vy}"
        );
    }

    #[test]
    fn double_jump() {
        let mut config = PlatformerConfig::default();
        config.max_jumps = 2;
        config.coyote.coyote_ticks = 1; // minimize coyote so it doesn't interfere
        let mut ctrl = PlatformerController::new(config);

        // Ground and jump
        ctrl.tick(grounded_input(), 0.5);
        ctrl.tick(
            PlatformerInput {
                on_ground: true,
                jump_pressed: true,
                jump_held: true,
                ..Default::default()
            },
            0.5,
        );

        // In air past coyote time
        for _ in 0..5 {
            ctrl.tick(default_input(), 0.5);
        }

        // Double jump — should work because jumps_remaining was 1 after first jump
        let input = PlatformerInput {
            jump_pressed: true,
            jump_held: true,
            ..Default::default()
        };
        let (_, events) = ctrl.tick(input, 0.5);
        assert!(events.contains(&PlatformerEvent::DoubleJumped));
    }

    // ── Wall slide, dash, and movement ─────────────────────

    #[test]
    fn wall_slide_and_jump() {
        let config = PlatformerConfig::default().with_wall(WallConfig::default());
        let mut ctrl = PlatformerController::new(config);

        // Be grounded then leave ground
        ctrl.tick(grounded_input(), 0.5);
        ctrl.tick(default_input(), 0.5);

        // Fall next to a wall for many ticks to build up velocity and let wall slide cap it
        for _ in 0..30 {
            ctrl.tick(
                PlatformerInput {
                    wall_right: true,
                    ..Default::default()
                },
                0.5,
            );
        }
        // Velocity should be capped at wall slide speed (1.5)
        assert!(
            ctrl.velocity_y <= 1.5 + 0.1,
            "Wall slide should cap fall speed, got {}",
            ctrl.velocity_y
        );

        // Wall jump
        let input = PlatformerInput {
            wall_right: true,
            jump_pressed: true,
            jump_held: true,
            ..Default::default()
        };
        let (out, events) = ctrl.tick(input, 0.5);
        assert!(events.contains(&PlatformerEvent::WallJumped));
        assert!(out.velocity_x < 0.0, "Wall jump should push away from wall");
        assert!(out.velocity_y < 0.0, "Wall jump should go up");
    }

    #[test]
    fn dash() {
        let config = PlatformerConfig::default().with_dash(DashConfig::default());
        let mut ctrl = PlatformerController::new(config);

        ctrl.tick(grounded_input(), 0.5);

        let input = PlatformerInput {
            on_ground: true,
            move_x: 1.0,
            dash_pressed: true,
            ..Default::default()
        };
        let (out, events) = ctrl.tick(input, 0.5);
        assert!(events.contains(&PlatformerEvent::DashStarted));
        assert!(out.velocity_x > 5.0, "Should be moving fast during dash");
        assert!(ctrl.is_dashing());

        // Tick through the dash
        for _ in 0..10 {
            ctrl.tick(grounded_input(), 0.5);
        }
        assert!(!ctrl.is_dashing(), "Dash should have ended");
    }

    #[test]
    fn horizontal_acceleration() {
        let config = PlatformerConfig::default();
        let mut ctrl = PlatformerController::new(config);

        ctrl.tick(grounded_input(), 0.5);

        // Move right
        let input = PlatformerInput {
            on_ground: true,
            move_x: 1.0,
            ..Default::default()
        };
        let (out1, _) = ctrl.tick(input, 0.5);
        let (out2, _) = ctrl.tick(input, 0.5);
        let (out3, _) = ctrl.tick(input, 0.5);

        assert!(out2.velocity_x > out1.velocity_x, "Should accelerate");
        assert!(
            out3.velocity_x >= out2.velocity_x,
            "Should keep accelerating"
        );
        assert!(
            out3.velocity_x <= 3.0 + 0.01,
            "Should not exceed move_speed"
        );
    }

    // ── Moving platforms ────────────────────────────────────

    #[test]
    fn moving_platform_loop() {
        let mut plat = MovingPlatform::new(vec![(0.0, 0.0), (100.0, 0.0)], 10.0, PathMode::Loop);

        let mut last_x = 0.0;
        let mut moved_right = false;
        let mut looped = false;

        for _ in 0..200 {
            let (x, _, _, _) = plat.tick();
            if x > 50.0 {
                moved_right = true;
            }
            if moved_right && x < 10.0 {
                looped = true;
                break;
            }
            last_x = x;
        }
        let _ = last_x;
        assert!(looped, "Platform should loop back");
    }

    #[test]
    fn moving_platform_pingpong() {
        let mut plat =
            MovingPlatform::new(vec![(0.0, 0.0), (100.0, 0.0)], 10.0, PathMode::PingPong);

        let mut reached_end = false;
        let mut came_back = false;

        for _ in 0..200 {
            let (x, _, _, _) = plat.tick();
            if x > 90.0 {
                reached_end = true;
            }
            if reached_end && x < 10.0 {
                came_back = true;
                break;
            }
        }
        assert!(reached_end, "Should reach end");
        assert!(came_back, "Should come back in PingPong mode");
    }

    #[test]
    fn moving_platform_once() {
        let mut plat = MovingPlatform::new(vec![(0.0, 0.0), (50.0, 0.0)], 10.0, PathMode::Once);

        for _ in 0..100 {
            plat.tick();
        }

        let (x, _, _, _) = plat.tick();
        assert!(
            (x - 50.0).abs() < 1.0,
            "Should stop at last waypoint, got {x}"
        );
    }
}
