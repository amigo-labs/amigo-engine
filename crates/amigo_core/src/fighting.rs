use crate::math::RenderVec2;
use crate::rect::Rect;

// ---------------------------------------------------------------------------
// Frame data
// ---------------------------------------------------------------------------

/// Phase of an attack animation frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePhase {
    /// Startup frames (can't hit yet, can't cancel).
    Startup,
    /// Active frames (hitbox is out).
    Active,
    /// Recovery frames (attack over, can't act yet).
    Recovery,
}

/// A single frame of animation data for a move.
#[derive(Clone, Debug)]
pub struct FrameData {
    pub phase: FramePhase,
    /// Hitboxes active this frame (empty if Startup/Recovery).
    pub hitboxes: Vec<HitBox>,
    /// Hurtbox override for this frame (None = use default).
    pub hurtbox_override: Option<Rect>,
    /// Movement applied this frame.
    pub velocity: RenderVec2,
    /// Whether this frame can be cancelled into another move.
    pub cancellable: bool,
    /// Whether the fighter is invincible this frame.
    pub invincible: bool,
    /// Whether the fighter has super armor (takes damage but doesn't flinch).
    pub super_armor: bool,
}

impl FrameData {
    pub fn startup() -> Self {
        Self {
            phase: FramePhase::Startup,
            hitboxes: Vec::new(),
            hurtbox_override: None,
            velocity: RenderVec2::ZERO,
            cancellable: false,
            invincible: false,
            super_armor: false,
        }
    }

    pub fn active(hitbox: HitBox) -> Self {
        Self {
            phase: FramePhase::Active,
            hitboxes: vec![hitbox],
            hurtbox_override: None,
            velocity: RenderVec2::ZERO,
            cancellable: false,
            invincible: false,
            super_armor: false,
        }
    }

    pub fn recovery() -> Self {
        Self {
            phase: FramePhase::Recovery,
            hitboxes: Vec::new(),
            hurtbox_override: None,
            velocity: RenderVec2::ZERO,
            cancellable: false,
            invincible: false,
            super_armor: false,
        }
    }

    pub fn with_velocity(mut self, vx: f32, vy: f32) -> Self {
        self.velocity = RenderVec2::new(vx, vy);
        self
    }

    pub fn with_cancellable(mut self) -> Self {
        self.cancellable = true;
        self
    }

    pub fn with_invincible(mut self) -> Self {
        self.invincible = true;
        self
    }
}

// ---------------------------------------------------------------------------
// Hitbox / Hurtbox
// ---------------------------------------------------------------------------

/// Attack hitbox with damage properties.
#[derive(Clone, Debug)]
pub struct HitBox {
    pub rect: Rect,
    pub damage: i32,
    /// Hitstun frames inflicted on the opponent.
    pub hitstun: u32,
    /// Blockstun frames if the opponent blocks.
    pub blockstun: u32,
    /// Knockback direction and force.
    pub knockback: RenderVec2,
    /// Hit type for combo tracking.
    pub hit_type: HitType,
    /// Whether this is an overhead (must block standing), low (must block crouching), or mid.
    pub guard_type: GuardType,
}

impl HitBox {
    pub fn new(rect: Rect, damage: i32) -> Self {
        Self {
            rect,
            damage,
            hitstun: 12,
            blockstun: 6,
            knockback: RenderVec2::new(3.0, 0.0),
            hit_type: HitType::Normal,
            guard_type: GuardType::Mid,
        }
    }

    pub fn with_hitstun(mut self, frames: u32) -> Self {
        self.hitstun = frames;
        self
    }

    pub fn with_blockstun(mut self, frames: u32) -> Self {
        self.blockstun = frames;
        self
    }

    pub fn with_knockback(mut self, x: f32, y: f32) -> Self {
        self.knockback = RenderVec2::new(x, y);
        self
    }

    pub fn with_hit_type(mut self, hit_type: HitType) -> Self {
        self.hit_type = hit_type;
        self
    }

    pub fn with_guard_type(mut self, guard_type: GuardType) -> Self {
        self.guard_type = guard_type;
        self
    }

    /// Get the hitbox rect in world space given fighter position and facing.
    pub fn world_rect(&self, pos: RenderVec2, facing_right: bool) -> Rect {
        if facing_right {
            Rect::new(
                pos.x + self.rect.x,
                pos.y + self.rect.y,
                self.rect.w,
                self.rect.h,
            )
        } else {
            Rect::new(
                pos.x - self.rect.x - self.rect.w,
                pos.y + self.rect.y,
                self.rect.w,
                self.rect.h,
            )
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitType {
    Normal,
    Launch,
    Sweep,
    Grab,
    Projectile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardType {
    Mid,
    High,
    Low,
    Unblockable,
}

// ---------------------------------------------------------------------------
// Move definition
// ---------------------------------------------------------------------------

/// A complete move (attack, special, etc).
#[derive(Clone, Debug)]
pub struct MoveDef {
    pub name: String,
    pub frames: Vec<FrameData>,
    /// If true, move can combo into the next move on hit.
    pub can_chain: bool,
    /// Priority (higher wins in trades).
    pub priority: i32,
}

impl MoveDef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            frames: Vec::new(),
            can_chain: true,
            priority: 0,
        }
    }

    pub fn with_frame(mut self, frame: FrameData) -> Self {
        self.frames.push(frame);
        self
    }

    /// Add multiple startup frames.
    pub fn with_startup(mut self, count: u32) -> Self {
        for _ in 0..count {
            self.frames.push(FrameData::startup());
        }
        self
    }

    /// Add active frames with a hitbox.
    pub fn with_active(mut self, count: u32, hitbox: HitBox) -> Self {
        for _ in 0..count {
            self.frames.push(FrameData::active(hitbox.clone()));
        }
        self
    }

    /// Add recovery frames.
    pub fn with_recovery(mut self, count: u32) -> Self {
        for _ in 0..count {
            self.frames.push(FrameData::recovery());
        }
        self
    }

    pub fn total_frames(&self) -> usize {
        self.frames.len()
    }

    pub fn startup_frames(&self) -> usize {
        self.frames
            .iter()
            .take_while(|f| f.phase == FramePhase::Startup)
            .count()
    }

    pub fn active_frames(&self) -> usize {
        self.frames
            .iter()
            .filter(|f| f.phase == FramePhase::Active)
            .count()
    }

    pub fn recovery_frames(&self) -> usize {
        self.frames
            .iter()
            .rev()
            .take_while(|f| f.phase == FramePhase::Recovery)
            .count()
    }

    /// Frame advantage on hit (positive = attacker can act first).
    pub fn frame_advantage_hit(&self) -> i32 {
        let recovery = self.recovery_frames() as i32;
        let hitstun = self
            .frames
            .iter()
            .find(|f| f.phase == FramePhase::Active)
            .and_then(|f| f.hitboxes.first())
            .map(|h| h.hitstun as i32)
            .unwrap_or(0);
        hitstun - recovery
    }

    /// Frame advantage on block.
    pub fn frame_advantage_block(&self) -> i32 {
        let recovery = self.recovery_frames() as i32;
        let blockstun = self
            .frames
            .iter()
            .find(|f| f.phase == FramePhase::Active)
            .and_then(|f| f.hitboxes.first())
            .map(|h| h.blockstun as i32)
            .unwrap_or(0);
        blockstun - recovery
    }
}

// ---------------------------------------------------------------------------
// Input buffer and command recognition
// ---------------------------------------------------------------------------

/// A directional input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputDir {
    Neutral,
    Up,
    UpForward,
    Forward,
    DownForward,
    Down,
    DownBack,
    Back,
    UpBack,
}

/// A single input event (direction + buttons).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputFrame {
    pub direction: InputDir,
    pub light: bool,
    pub medium: bool,
    pub heavy: bool,
    pub special: bool,
}

impl InputFrame {
    pub fn new() -> Self {
        Self {
            direction: InputDir::Neutral,
            light: false,
            medium: false,
            heavy: false,
            special: false,
        }
    }

    pub fn any_button(&self) -> bool {
        self.light || self.medium || self.heavy || self.special
    }
}

impl Default for InputFrame {
    fn default() -> Self {
        Self::new()
    }
}

/// Input buffer for motion detection.
pub struct InputBuffer {
    buffer: Vec<InputFrame>,
    capacity: usize,
}

impl InputBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, input: InputFrame) {
        if self.buffer.len() >= self.capacity {
            self.buffer.remove(0);
        }
        self.buffer.push(input);
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Check if a command motion was performed recently (within `window` frames).
    pub fn check_motion(&self, motion: &[InputDir], window: usize) -> bool {
        if motion.is_empty() || self.buffer.len() < motion.len() {
            return false;
        }

        let search_start = self.buffer.len().saturating_sub(window);
        let search_range = &self.buffer[search_start..];

        let mut motion_idx = 0;
        for input in search_range {
            if input.direction == motion[motion_idx] {
                motion_idx += 1;
                if motion_idx >= motion.len() {
                    return true;
                }
            }
        }
        false
    }

    /// Last input in the buffer.
    pub fn last(&self) -> Option<&InputFrame> {
        self.buffer.last()
    }
}

// ---------------------------------------------------------------------------
// Common motions
// ---------------------------------------------------------------------------

/// Standard fighting game motions.
pub mod motions {
    use super::InputDir;

    /// Quarter circle forward (↓↘→) — Hadouken motion.
    pub fn qcf() -> Vec<InputDir> {
        vec![InputDir::Down, InputDir::DownForward, InputDir::Forward]
    }

    /// Quarter circle back (↓↙←) — Reverse fireball.
    pub fn qcb() -> Vec<InputDir> {
        vec![InputDir::Down, InputDir::DownBack, InputDir::Back]
    }

    /// Dragon punch (→↓↘) — Shoryuken motion.
    pub fn dp() -> Vec<InputDir> {
        vec![InputDir::Forward, InputDir::Down, InputDir::DownForward]
    }

    /// Half circle forward (←↙↓↘→).
    pub fn hcf() -> Vec<InputDir> {
        vec![
            InputDir::Back,
            InputDir::DownBack,
            InputDir::Down,
            InputDir::DownForward,
            InputDir::Forward,
        ]
    }

    /// Half circle back (→↘↓↙←).
    pub fn hcb() -> Vec<InputDir> {
        vec![
            InputDir::Forward,
            InputDir::DownForward,
            InputDir::Down,
            InputDir::DownBack,
            InputDir::Back,
        ]
    }

    /// Double quarter circle forward (↓↘→↓↘→) — Super motion.
    pub fn double_qcf() -> Vec<InputDir> {
        vec![
            InputDir::Down,
            InputDir::DownForward,
            InputDir::Forward,
            InputDir::Down,
            InputDir::DownForward,
            InputDir::Forward,
        ]
    }

    /// 360 motion (SPD / command grab).
    pub fn spd() -> Vec<InputDir> {
        vec![
            InputDir::Forward,
            InputDir::DownForward,
            InputDir::Down,
            InputDir::DownBack,
            InputDir::Back,
            InputDir::UpBack,
            InputDir::Up,
        ]
    }
}

// ---------------------------------------------------------------------------
// Combo system
// ---------------------------------------------------------------------------

/// A hit in a combo.
#[derive(Clone, Debug)]
pub struct ComboHit {
    pub move_name: String,
    pub damage: i32,
    pub hit_number: u32,
}

/// Tracks the current combo.
#[derive(Clone, Debug)]
pub struct ComboTracker {
    pub hits: Vec<ComboHit>,
    pub total_damage: i32,
    /// Damage scaling factor (decreases with each hit).
    pub scaling: f32,
    /// Minimum scaling floor.
    pub min_scaling: f32,
    /// Scaling decay per hit.
    pub decay_per_hit: f32,
    /// Whether the combo is still going.
    pub active: bool,
    /// Frames since last hit (combo drops if too long).
    pub gap_timer: u32,
    /// Max gap allowed between hits.
    pub max_gap: u32,
}

impl ComboTracker {
    pub fn new() -> Self {
        Self {
            hits: Vec::new(),
            total_damage: 0,
            scaling: 1.0,
            min_scaling: 0.1,
            decay_per_hit: 0.1,
            active: false,
            gap_timer: 0,
            max_gap: 30, // ~0.5 seconds at 60fps
        }
    }

    /// Register a hit in the combo.
    pub fn add_hit(&mut self, move_name: impl Into<String>, base_damage: i32) -> i32 {
        let scaled_damage = (base_damage as f32 * self.scaling).max(1.0) as i32;

        self.hits.push(ComboHit {
            move_name: move_name.into(),
            damage: scaled_damage,
            hit_number: self.hits.len() as u32 + 1,
        });

        self.total_damage += scaled_damage;
        self.scaling = (self.scaling - self.decay_per_hit).max(self.min_scaling);
        self.active = true;
        self.gap_timer = 0;

        scaled_damage
    }

    /// Call every frame to check for combo drops.
    pub fn update(&mut self) {
        if !self.active {
            return;
        }
        self.gap_timer += 1;
        if self.gap_timer > self.max_gap {
            self.end();
        }
    }

    /// End the current combo.
    pub fn end(&mut self) {
        self.active = false;
    }

    /// Reset for a new combo.
    pub fn reset(&mut self) {
        self.hits.clear();
        self.total_damage = 0;
        self.scaling = 1.0;
        self.active = false;
        self.gap_timer = 0;
    }

    /// Current hit count.
    pub fn hit_count(&self) -> u32 {
        self.hits.len() as u32
    }
}

impl Default for ComboTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Fighter state
// ---------------------------------------------------------------------------

/// Fighter state in a fighting game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FighterState {
    Idle,
    Walking,
    Crouching,
    Jumping,
    Attacking,
    Blocking,
    HitStun,
    BlockStun,
    KnockDown,
    GettingUp,
    Grabbed,
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
    pub meter: f32,
    pub max_meter: f32,
    /// Default hurtbox (relative to position).
    pub hurtbox: Rect,
    /// Current move being performed.
    pub current_move: Option<usize>,
    /// Current frame in the move.
    pub current_frame: usize,
    /// Stun frames remaining.
    pub stun_timer: u32,
    /// Whether on the ground.
    pub grounded: bool,
    /// Combo tracker for this fighter's offense.
    pub combo: ComboTracker,
}

impl Fighter {
    pub fn new(position: RenderVec2, hp: i32) -> Self {
        Self {
            position,
            velocity: RenderVec2::ZERO,
            facing_right: true,
            state: FighterState::Idle,
            hp,
            max_hp: hp,
            meter: 0.0,
            max_meter: 100.0,
            hurtbox: Rect::new(-16.0, -48.0, 32.0, 48.0),
            current_move: None,
            current_frame: 0,
            stun_timer: 0,
            grounded: true,
            combo: ComboTracker::new(),
        }
    }

    /// Get the world-space hurtbox.
    pub fn world_hurtbox(&self) -> Rect {
        if self.facing_right {
            Rect::new(
                self.position.x + self.hurtbox.x,
                self.position.y + self.hurtbox.y,
                self.hurtbox.w,
                self.hurtbox.h,
            )
        } else {
            Rect::new(
                self.position.x - self.hurtbox.x - self.hurtbox.w,
                self.position.y + self.hurtbox.y,
                self.hurtbox.w,
                self.hurtbox.h,
            )
        }
    }

    /// Start a move.
    pub fn start_move(&mut self, move_index: usize) {
        self.current_move = Some(move_index);
        self.current_frame = 0;
        self.state = FighterState::Attacking;
    }

    /// Advance the current move by one frame.
    /// Advance the current move by one frame. Returns the frame index that was executed.
    pub fn advance_move(&mut self, moves: &[MoveDef]) -> Option<usize> {
        let move_idx = self.current_move?;
        let move_def = moves.get(move_idx)?;

        if self.current_frame >= move_def.frames.len() {
            self.current_move = None;
            self.state = FighterState::Idle;
            return None;
        }

        let frame_idx = self.current_frame;
        let frame = &move_def.frames[frame_idx];
        self.current_frame += 1;

        // Apply frame velocity
        if self.facing_right {
            self.position.x += frame.velocity.x;
        } else {
            self.position.x -= frame.velocity.x;
        }
        self.position.y += frame.velocity.y;

        Some(frame_idx)
    }

    /// Get the current frame data from a move (non-mutating lookup).
    pub fn get_frame<'a>(&self, moves: &'a [MoveDef], frame_idx: usize) -> Option<&'a FrameData> {
        let move_idx = self.current_move?;
        moves.get(move_idx)?.frames.get(frame_idx)
    }

    /// Apply hitstun.
    pub fn apply_hitstun(&mut self, frames: u32, knockback: RenderVec2) {
        self.state = FighterState::HitStun;
        self.stun_timer = frames;
        if self.facing_right {
            self.velocity.x = -knockback.x;
        } else {
            self.velocity.x = knockback.x;
        }
        self.velocity.y = knockback.y;
    }

    /// Apply blockstun.
    pub fn apply_blockstun(&mut self, frames: u32) {
        self.state = FighterState::BlockStun;
        self.stun_timer = frames;
    }

    /// Update stun timer.
    pub fn update_stun(&mut self) {
        if self.stun_timer > 0 {
            self.stun_timer -= 1;
            if self.stun_timer == 0 {
                self.state = FighterState::Idle;
                self.velocity = RenderVec2::ZERO;
            }
        }
    }

    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    pub fn can_act(&self) -> bool {
        matches!(
            self.state,
            FighterState::Idle | FighterState::Walking | FighterState::Crouching
        )
    }

    pub fn is_blocking(&self) -> bool {
        matches!(self.state, FighterState::Blocking | FighterState::BlockStun)
    }

    /// Add meter (e.g. on hit or being hit).
    pub fn add_meter(&mut self, amount: f32) {
        self.meter = (self.meter + amount).min(self.max_meter);
    }

    /// Spend meter. Returns false if not enough.
    pub fn spend_meter(&mut self, amount: f32) -> bool {
        if self.meter >= amount {
            self.meter -= amount;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Hit detection
// ---------------------------------------------------------------------------

/// Check if any of a move's hitboxes hit a fighter's hurtbox.
pub fn check_hit<'a>(
    attacker_pos: RenderVec2,
    attacker_facing_right: bool,
    frame: &'a FrameData,
    defender: &Fighter,
) -> Option<&'a HitBox> {
    let defender_hurtbox = defender.world_hurtbox();

    for hitbox in &frame.hitboxes {
        let world_hitbox = hitbox.world_rect(attacker_pos, attacker_facing_right);
        if world_hitbox.overlaps(&defender_hurtbox) {
            return Some(hitbox);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jab() -> MoveDef {
        MoveDef::new("Jab")
            .with_startup(3)
            .with_active(
                2,
                HitBox::new(Rect::new(8.0, -32.0, 32.0, 16.0), 30)
                    .with_hitstun(12)
                    .with_blockstun(5),
            )
            .with_recovery(6)
    }

    fn heavy_kick() -> MoveDef {
        MoveDef::new("Heavy Kick")
            .with_startup(8)
            .with_active(
                4,
                HitBox::new(Rect::new(4.0, -16.0, 48.0, 20.0), 80)
                    .with_hitstun(20)
                    .with_blockstun(10)
                    .with_knockback(8.0, -2.0),
            )
            .with_recovery(12)
    }

    #[test]
    fn frame_data_counts() {
        let j = jab();
        assert_eq!(j.startup_frames(), 3);
        assert_eq!(j.active_frames(), 2);
        assert_eq!(j.recovery_frames(), 6);
        assert_eq!(j.total_frames(), 11);
    }

    #[test]
    fn frame_advantage() {
        let j = jab();
        // hitstun=12, recovery=6 → +6 on hit
        assert_eq!(j.frame_advantage_hit(), 6);
        // blockstun=5, recovery=6 → -1 on block
        assert_eq!(j.frame_advantage_block(), -1);
    }

    #[test]
    fn combo_tracker() {
        let mut combo = ComboTracker::new();
        let dmg1 = combo.add_hit("Jab", 30);
        assert_eq!(dmg1, 30);
        assert_eq!(combo.hit_count(), 1);

        let dmg2 = combo.add_hit("Heavy Kick", 80);
        // Should be scaled: 80 * 0.9 = 72
        assert_eq!(dmg2, 72);
        assert_eq!(combo.hit_count(), 2);
        assert_eq!(combo.total_damage, 102);
    }

    #[test]
    fn combo_drops_after_gap() {
        let mut combo = ComboTracker::new();
        combo.add_hit("Jab", 30);
        assert!(combo.active);

        for _ in 0..31 {
            combo.update();
        }
        assert!(!combo.active);
    }

    #[test]
    fn input_buffer_motion_detection() {
        let mut buf = InputBuffer::new(30);

        // Input a quarter circle forward
        buf.push(InputFrame {
            direction: InputDir::Down,
            ..Default::default()
        });
        buf.push(InputFrame {
            direction: InputDir::DownForward,
            ..Default::default()
        });
        buf.push(InputFrame {
            direction: InputDir::Forward,
            light: true,
            ..Default::default()
        });

        assert!(buf.check_motion(&motions::qcf(), 10));
        assert!(!buf.check_motion(&motions::dp(), 10));
    }

    #[test]
    fn hitbox_world_rect_facing() {
        let hitbox = HitBox::new(Rect::new(8.0, -32.0, 32.0, 16.0), 30);
        let pos = RenderVec2::new(100.0, 200.0);

        let right = hitbox.world_rect(pos, true);
        assert_eq!(right.x, 108.0);

        let left = hitbox.world_rect(pos, false);
        assert_eq!(left.x, 100.0 - 8.0 - 32.0);
    }

    #[test]
    fn hit_detection() {
        let moves = vec![jab()];
        let mut attacker = Fighter::new(RenderVec2::new(100.0, 200.0), 1000);
        attacker.facing_right = true;
        attacker.start_move(0);

        let defender = Fighter::new(RenderVec2::new(130.0, 200.0), 1000);

        // Skip startup
        for _ in 0..3 {
            attacker.advance_move(&moves);
        }
        // Active frame
        let frame_idx = attacker.advance_move(&moves).unwrap();
        let pos = attacker.position;
        let facing = attacker.facing_right;
        let frame = attacker.get_frame(&moves, frame_idx).unwrap();
        let hit = check_hit(pos, facing, frame, &defender);
        assert!(hit.is_some());
    }

    #[test]
    fn fighter_meter() {
        let mut fighter = Fighter::new(RenderVec2::ZERO, 1000);
        fighter.add_meter(30.0);
        assert_eq!(fighter.meter, 30.0);

        assert!(!fighter.spend_meter(50.0)); // not enough
        assert!(fighter.spend_meter(25.0));
        assert_eq!(fighter.meter, 5.0);
    }

    #[test]
    fn move_builder_pattern() {
        let fireball = MoveDef::new("Hadouken")
            .with_startup(12)
            .with_active(
                3,
                HitBox::new(Rect::new(16.0, -20.0, 24.0, 24.0), 60)
                    .with_hit_type(HitType::Projectile)
                    .with_guard_type(GuardType::Mid),
            )
            .with_recovery(18);

        assert_eq!(fireball.total_frames(), 33);
        assert_eq!(fireball.startup_frames(), 12);
    }
}
