//! Shoot'em Up (Shmup) game template.
//!
//! Provides precision hitboxes, grazing mechanics, adaptive difficulty (rank),
//! bomb/deathbomb system, score-based extends, and chain scoring for bullet hell
//! style games in the vein of Touhou, DoDonPachi, and Mushihimesama.

use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

use crate::bullet_pattern::BulletPool;

// ---------------------------------------------------------------------------
// ScrollMode
// ---------------------------------------------------------------------------

/// Scroll direction mode for a shmup stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollMode {
    /// Classic vertical scroller (Touhou, DoDonPachi).
    Vertical,
    /// Horizontal scroller (Gradius, R-Type).
    Horizontal,
    /// Player-controlled direction (Geometry Wars).
    MultiDirectional,
    /// No scrolling, fixed arena (Asteroids-style).
    FixedScreen,
}

// ---------------------------------------------------------------------------
// ShmupConfig
// ---------------------------------------------------------------------------

/// Global shmup configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShmupConfig {
    /// Scroll mode for the stage.
    pub scroll_mode: ScrollMode,
    /// Scroll speed in pixels/tick.
    pub scroll_speed: f32,
    /// Player movement speed in pixels/tick.
    pub player_speed: f32,
    /// Focused (slow) movement speed (while holding focus button).
    pub focus_speed: f32,
    /// Number of starting lives.
    pub starting_lives: u8,
    /// Maximum number of bombs the player can hold.
    pub max_bombs: u8,
    /// Starting bomb count.
    pub starting_bombs: u8,
    /// Arena bounds (playfield rectangle): (x, y, width, height).
    pub arena: (f32, f32, f32, f32),
}

impl Default for ShmupConfig {
    fn default() -> Self {
        Self {
            scroll_mode: ScrollMode::Vertical,
            scroll_speed: 1.0,
            player_speed: 4.0,
            focus_speed: 1.5,
            starting_lives: 3,
            max_bombs: 3,
            starting_bombs: 3,
            arena: (0.0, 0.0, 384.0, 448.0),
        }
    }
}

// ---------------------------------------------------------------------------
// ShmupHitbox
// ---------------------------------------------------------------------------

/// Precision hitbox for shmup entities. Player hitboxes are much smaller than
/// sprites; enemy hitboxes match their visual size.
#[derive(Clone, Debug)]
pub struct ShmupHitbox {
    /// Circle hitbox radius for collision with bullets/enemies.
    /// Player: typically 1–3 pixels. Enemies: match their visual size.
    pub collision_radius: f32,
    /// Graze detection radius. Bullets within this radius but outside
    /// `collision_radius` trigger graze events. Player only.
    pub graze_radius: f32,
    /// Center offset from the entity's position (X).
    pub offset_x: f32,
    /// Center offset from the entity's position (Y).
    pub offset_y: f32,
}

impl ShmupHitbox {
    /// Create a player hitbox with tiny collision and larger graze radius.
    pub fn player(collision: f32, graze: f32) -> Self {
        Self {
            collision_radius: collision,
            graze_radius: graze,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// Create an enemy hitbox (no graze radius).
    pub fn enemy(radius: f32) -> Self {
        Self {
            collision_radius: radius,
            graze_radius: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// Check if a point (bullet position) is within the collision circle.
    /// Uses squared-distance comparison (no sqrt).
    pub fn hit_test(&self, self_x: f32, self_y: f32, point_x: f32, point_y: f32) -> bool {
        let cx = self_x + self.offset_x;
        let cy = self_y + self.offset_y;
        let dx = point_x - cx;
        let dy = point_y - cy;
        let dist_sq = dx * dx + dy * dy;
        dist_sq <= self.collision_radius * self.collision_radius
    }

    /// Check if a point is within the graze circle but outside collision.
    pub fn graze_test(&self, self_x: f32, self_y: f32, point_x: f32, point_y: f32) -> bool {
        let cx = self_x + self.offset_x;
        let cy = self_y + self.offset_y;
        let dx = point_x - cx;
        let dy = point_y - cy;
        let dist_sq = dx * dx + dy * dy;
        let col_sq = self.collision_radius * self.collision_radius;
        let grz_sq = self.graze_radius * self.graze_radius;
        dist_sq > col_sq && dist_sq <= grz_sq
    }
}

// ---------------------------------------------------------------------------
// GrazingSystem
// ---------------------------------------------------------------------------

/// Tracks grazing state and rewards.
///
/// Each active bullet is tested against the graze radius every frame. To
/// prevent counting the same bullet multiple times, `grazed_bullets` stores
/// `(pool_index, generation)` tuples. Since `BulletPool` recycles indices,
/// callers should supply a generation counter per slot so recycled indices
/// are treated as new bullets.
#[derive(Clone, Debug)]
pub struct GrazingSystem {
    /// Number of bullets grazed this frame.
    pub frame_graze_count: u32,
    /// Total bullets grazed this life.
    pub total_graze: u64,
    /// Score bonus per graze tick.
    pub graze_score: u64,
    /// Meter (power/bomb gauge) gained per graze.
    pub graze_meter: f32,
    /// Set of (pool_index, generation) already counted as grazed this life.
    grazed_bullets: FxHashSet<(usize, u32)>,
}

impl GrazingSystem {
    /// Create a new grazing system with the given per-graze rewards.
    pub fn new(graze_score: u64, graze_meter: f32) -> Self {
        Self {
            frame_graze_count: 0,
            total_graze: 0,
            graze_score,
            graze_meter,
            grazed_bullets: FxHashSet::default(),
        }
    }

    /// Process grazing for one frame. Tests all active bullets against the
    /// player's graze hitbox. `generations` maps pool index to a generation
    /// counter (pass `None` to use index-only dedup, treating index 0 as
    /// generation 0). Returns the number of new grazes this frame.
    pub fn tick(
        &mut self,
        player_x: f32,
        player_y: f32,
        hitbox: &ShmupHitbox,
        pool: &BulletPool,
    ) -> u32 {
        self.frame_graze_count = 0;
        for (i, bullet) in pool.active_iter() {
            if hitbox.graze_test(player_x, player_y, bullet.x, bullet.y) {
                // Use kind as a stand-in for generation when BulletPool lacks
                // a dedicated generation counter. Callers who track generations
                // can store them in bullet.kind or wrap the pool.
                let key = (i, bullet.kind);
                if self.grazed_bullets.insert(key) {
                    self.frame_graze_count += 1;
                    self.total_graze += 1;
                }
            }
        }
        self.frame_graze_count
    }

    /// Reset graze tracking (on death or new life).
    pub fn reset(&mut self) {
        self.frame_graze_count = 0;
        self.total_graze = 0;
        self.grazed_bullets.clear();
    }
}

// ---------------------------------------------------------------------------
// RankSystem (Dynamic Difficulty)
// ---------------------------------------------------------------------------

/// Dynamic difficulty adjustment configuration.
/// Rank rises on skilled play, falls on death/bombing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RankConfig {
    /// Minimum rank value.
    pub min_rank: f32,
    /// Maximum rank value.
    pub max_rank: f32,
    /// Starting rank.
    pub initial_rank: f32,
    /// Rank increase per tick of survival.
    pub survival_rate: f32,
    /// Rank increase per graze.
    pub graze_bonus: f32,
    /// Rank increase per enemy killed.
    pub kill_bonus: f32,
    /// Rank decrease on death.
    pub death_penalty: f32,
    /// Rank decrease on bomb use.
    pub bomb_penalty: f32,
    /// Rank increase on extend (extra life earned).
    pub extend_bonus: f32,
}

impl Default for RankConfig {
    fn default() -> Self {
        Self {
            min_rank: 0.0,
            max_rank: 1.0,
            initial_rank: 0.3,
            survival_rate: 0.00002,
            graze_bonus: 0.001,
            kill_bonus: 0.002,
            death_penalty: 0.15,
            bomb_penalty: 0.08,
            extend_bonus: 0.05,
        }
    }
}

/// Runtime rank state.
#[derive(Clone, Debug)]
pub struct RankState {
    /// Current rank value, clamped between config min and max.
    pub current_rank: f32,
    /// Configuration parameters.
    pub config: RankConfig,
}

impl RankState {
    /// Create a new rank state from config.
    pub fn new(config: RankConfig) -> Self {
        let rank = config.initial_rank;
        Self {
            current_rank: rank,
            config,
        }
    }

    /// Clamp rank to configured bounds.
    fn clamp(&mut self) {
        if self.current_rank < self.config.min_rank {
            self.current_rank = self.config.min_rank;
        } else if self.current_rank > self.config.max_rank {
            self.current_rank = self.config.max_rank;
        }
    }

    /// Tick survival time rank increase.
    pub fn tick_survival(&mut self) {
        self.current_rank += self.config.survival_rate;
        self.clamp();
    }

    /// Apply rank bonus for a graze.
    pub fn on_graze(&mut self) {
        self.current_rank += self.config.graze_bonus;
        self.clamp();
    }

    /// Apply rank bonus for an enemy kill.
    pub fn on_kill(&mut self) {
        self.current_rank += self.config.kill_bonus;
        self.clamp();
    }

    /// Apply rank penalty for a death.
    pub fn on_death(&mut self) {
        self.current_rank -= self.config.death_penalty;
        self.clamp();
    }

    /// Apply rank penalty for a bomb use.
    pub fn on_bomb(&mut self) {
        self.current_rank -= self.config.bomb_penalty;
        self.clamp();
    }

    /// Apply rank bonus for earning an extend.
    pub fn on_extend(&mut self) {
        self.current_rank += self.config.extend_bonus;
        self.clamp();
    }

    /// Get rank as a 0.0–1.0 normalized value for gameplay scaling.
    pub fn normalized(&self) -> f32 {
        let range = self.config.max_rank - self.config.min_rank;
        if range <= 0.0 {
            return 0.0;
        }
        (self.current_rank - self.config.min_rank) / range
    }

    /// Get a bullet speed multiplier derived from current rank.
    /// At min rank: 0.8x. At max rank: 1.3x. Linearly interpolated.
    pub fn speed_multiplier(&self) -> f32 {
        let t = self.normalized();
        0.8 + t * (1.3 - 0.8)
    }

    /// Get a bullet density multiplier derived from current rank.
    /// At min rank: 0.7x. At max rank: 1.5x. Linearly interpolated.
    pub fn density_multiplier(&self) -> f32 {
        let t = self.normalized();
        0.7 + t * (1.5 - 0.7)
    }
}

// ---------------------------------------------------------------------------
// BombSystem
// ---------------------------------------------------------------------------

/// Bomb (panic button) configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BombConfig {
    /// Duration of bomb invincibility in frames.
    pub invincibility_frames: u16,
    /// Duration of the screen-clear effect in frames.
    pub clear_duration: u8,
    /// Deathbomb window: frames after being hit where bomb input is still
    /// accepted.
    pub deathbomb_frames: u8,
    /// Damage dealt to on-screen enemies during bomb.
    pub bomb_damage: f32,
}

impl Default for BombConfig {
    fn default() -> Self {
        Self {
            invincibility_frames: 180,
            clear_duration: 30,
            deathbomb_frames: 6,
            bomb_damage: 100.0,
        }
    }
}

/// Runtime bomb state.
#[derive(Clone, Debug)]
pub struct BombState {
    /// Current bomb count.
    pub bombs: u8,
    /// Remaining invincibility frames (>0 means active).
    pub invincibility_timer: u16,
    /// Whether the bomb is currently clearing bullets.
    pub clearing: bool,
    /// Frames remaining in the clear effect.
    pub clear_timer: u8,
    /// Deathbomb window timer. Set when hit, counts down.
    pub deathbomb_timer: u8,
}

impl BombState {
    /// Create a new bomb state with the given starting bomb count.
    pub fn new(starting_bombs: u8) -> Self {
        Self {
            bombs: starting_bombs,
            invincibility_timer: 0,
            clearing: false,
            clear_timer: 0,
            deathbomb_timer: 0,
        }
    }

    /// Attempt to use a bomb. Returns `true` if bomb was activated.
    pub fn try_bomb(&mut self, config: &BombConfig) -> bool {
        if self.bombs == 0 {
            return false;
        }
        self.bombs -= 1;
        self.invincibility_timer = config.invincibility_frames;
        self.clearing = true;
        self.clear_timer = config.clear_duration;
        // Cancel any deathbomb window — the bomb was used.
        self.deathbomb_timer = 0;
        true
    }

    /// Called when the player is hit. Starts the deathbomb window.
    /// Returns `true` if the player actually dies (no bombs available for
    /// deathbombing).
    pub fn on_hit(&mut self, config: &BombConfig) -> bool {
        // If already invincible, ignore the hit.
        if self.is_invincible() {
            return false;
        }
        if self.bombs > 0 && config.deathbomb_frames > 0 {
            self.deathbomb_timer = config.deathbomb_frames;
            false
        } else {
            // No bombs, instant death.
            true
        }
    }

    /// Tick bomb timers. Returns `true` if the clear effect is active
    /// (caller should despawn all enemy bullets).
    pub fn tick(&mut self) -> bool {
        if self.invincibility_timer > 0 {
            self.invincibility_timer -= 1;
        }
        if self.clear_timer > 0 {
            self.clear_timer -= 1;
            if self.clear_timer == 0 {
                self.clearing = false;
            }
        }
        if self.deathbomb_timer > 0 {
            self.deathbomb_timer -= 1;
        }
        self.clearing
    }

    /// Whether the player is currently invincible.
    pub fn is_invincible(&self) -> bool {
        self.invincibility_timer > 0
    }

    /// Add bombs (from pickups). Clamped to `max`.
    pub fn add_bombs(&mut self, count: u8, max: u8) {
        self.bombs = (self.bombs + count).min(max);
    }
}

// ---------------------------------------------------------------------------
// ExtendSystem
// ---------------------------------------------------------------------------

/// Extra life system based on score thresholds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtendConfig {
    /// Score thresholds at which extra lives are awarded.
    /// E.g., `[1_000_000, 5_000_000, 15_000_000]` for 3 extends.
    pub score_thresholds: Vec<u64>,
    /// If true, after the last threshold, repeat the last interval forever.
    pub repeating: bool,
}

/// Runtime extend state.
#[derive(Clone, Debug)]
pub struct ExtendState {
    /// Configuration.
    pub config: ExtendConfig,
    /// Index of the next threshold to check.
    pub next_threshold_index: usize,
    /// Number of extends awarded so far.
    pub extends_awarded: u32,
}

impl ExtendState {
    /// Create a new extend state from config.
    pub fn new(config: ExtendConfig) -> Self {
        Self {
            config,
            next_threshold_index: 0,
            extends_awarded: 0,
        }
    }

    /// Check if the current score has crossed the next extend threshold.
    /// Returns the number of new extends earned (usually 0 or 1).
    pub fn check_score(&mut self, score: u64) -> u32 {
        let thresholds = &self.config.score_thresholds;
        if thresholds.is_empty() {
            return 0;
        }

        let mut earned = 0u32;

        loop {
            let threshold = if self.next_threshold_index < thresholds.len() {
                thresholds[self.next_threshold_index]
            } else if self.config.repeating && thresholds.len() >= 2 {
                // Repeat the last interval.
                let last = thresholds[thresholds.len() - 1];
                let prev = thresholds[thresholds.len() - 2];
                let interval = last - prev;
                let extra = (self.next_threshold_index - thresholds.len() + 1) as u64;
                last + interval * extra
            } else {
                // No more thresholds.
                break;
            };

            if score >= threshold {
                earned += 1;
                self.extends_awarded += 1;
                self.next_threshold_index += 1;
            } else {
                break;
            }
        }

        earned
    }
}

// ---------------------------------------------------------------------------
// ShmupScoring
// ---------------------------------------------------------------------------

/// Score tracking with chain/combo mechanics.
#[derive(Clone, Debug)]
pub struct ShmupScoring {
    /// Current score.
    pub score: u64,
    /// Current chain (consecutive kills without being hit).
    pub chain: u32,
    /// Maximum chain achieved this run.
    pub max_chain: u32,
    /// Score multiplier from chain (grows with chain length).
    pub chain_multiplier: f32,
    /// Frames remaining in chain window. Resets on kill, depletes over time.
    pub chain_timer: u16,
    /// Frames for chain timeout.
    pub chain_timeout: u16,
}

impl ShmupScoring {
    /// Create a new scoring system with the given chain timeout (in frames).
    pub fn new(chain_timeout: u16) -> Self {
        Self {
            score: 0,
            chain: 0,
            max_chain: 0,
            chain_multiplier: 1.0,
            chain_timer: 0,
            chain_timeout,
        }
    }

    /// Register an enemy kill. Returns score earned (base_score * multiplier).
    pub fn on_kill(&mut self, base_score: u64) -> u64 {
        self.chain += 1;
        if self.chain > self.max_chain {
            self.max_chain = self.chain;
        }
        // Multiplier grows with chain length: 1.0 + chain * 0.1, capped at 10x.
        self.chain_multiplier = (1.0 + self.chain as f32 * 0.1).min(10.0);
        self.chain_timer = self.chain_timeout;
        let earned = (base_score as f32 * self.chain_multiplier) as u64;
        self.score += earned;
        earned
    }

    /// Register a graze. Adds `graze_score` directly (no multiplier).
    pub fn on_graze(&mut self, graze_score: u64) {
        self.score += graze_score;
    }

    /// Break the chain (on hit or timeout).
    pub fn break_chain(&mut self) {
        self.chain = 0;
        self.chain_multiplier = 1.0;
        self.chain_timer = 0;
    }

    /// Tick chain timer. Breaks chain if timeout.
    pub fn tick(&mut self) {
        if self.chain_timer > 0 {
            self.chain_timer -= 1;
            if self.chain_timer == 0 {
                self.break_chain();
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

    #[test]
    fn hitbox_hit_test() {
        let hb = ShmupHitbox::player(2.0, 10.0);
        // Point inside collision radius.
        assert!(hb.hit_test(100.0, 100.0, 101.0, 100.0));
        // Point outside collision but inside graze.
        assert!(!hb.hit_test(100.0, 100.0, 105.0, 100.0));
        // Point far away.
        assert!(!hb.hit_test(100.0, 100.0, 200.0, 200.0));
    }

    #[test]
    fn hitbox_graze_test() {
        let hb = ShmupHitbox::player(2.0, 10.0);
        // Inside graze but outside collision.
        assert!(hb.graze_test(100.0, 100.0, 105.0, 100.0));
        // Inside collision — not a graze.
        assert!(!hb.graze_test(100.0, 100.0, 101.0, 100.0));
        // Outside graze radius entirely.
        assert!(!hb.graze_test(100.0, 100.0, 200.0, 200.0));
    }

    #[test]
    fn hitbox_enemy_no_graze() {
        let hb = ShmupHitbox::enemy(8.0);
        assert!(hb.hit_test(50.0, 50.0, 55.0, 50.0));
        // Graze radius is 0, so graze_test always false.
        assert!(!hb.graze_test(50.0, 50.0, 55.0, 50.0));
    }

    #[test]
    fn rank_clamp_and_multipliers() {
        let config = RankConfig::default();
        let mut rank = RankState::new(config);
        assert!((rank.current_rank - 0.3).abs() < 0.001);

        // Speed multiplier at initial rank (0.3 normalized).
        let spd = rank.speed_multiplier();
        let expected = 0.8 + 0.3 * 0.5;
        assert!((spd - expected).abs() < 0.001);

        // Drive rank to max.
        for _ in 0..100_000 {
            rank.on_kill();
        }
        assert!((rank.current_rank - 1.0).abs() < 0.001);
        assert!((rank.speed_multiplier() - 1.3).abs() < 0.001);
        assert!((rank.density_multiplier() - 1.5).abs() < 0.001);

        // Death penalty drops rank.
        rank.on_death();
        assert!(rank.current_rank < 1.0);
    }

    #[test]
    fn rank_death_floors_at_min() {
        let config = RankConfig {
            initial_rank: 0.1,
            death_penalty: 0.5,
            ..RankConfig::default()
        };
        let mut rank = RankState::new(config);
        rank.on_death();
        assert!((rank.current_rank - 0.0).abs() < 0.001);
    }

    #[test]
    fn bomb_try_and_invincibility() {
        let config = BombConfig::default();
        let mut state = BombState::new(2);
        assert!(state.try_bomb(&config));
        assert_eq!(state.bombs, 1);
        assert!(state.is_invincible());
        assert!(state.clearing);

        // Tick down clear timer.
        for _ in 0..30 {
            state.tick();
        }
        assert!(!state.clearing);
        assert!(state.is_invincible()); // invincibility lasts longer

        // Tick remaining invincibility.
        for _ in 0..150 {
            state.tick();
        }
        assert!(!state.is_invincible());
    }

    #[test]
    fn bomb_deathbomb_window() {
        let config = BombConfig::default();
        let mut state = BombState::new(1);
        // Player gets hit — should not die, deathbomb window opens.
        let died = state.on_hit(&config);
        assert!(!died);
        assert_eq!(state.deathbomb_timer, 6);

        // Use bomb within window.
        assert!(state.try_bomb(&config));
        assert_eq!(state.deathbomb_timer, 0);
        assert!(state.is_invincible());
    }

    #[test]
    fn bomb_no_bombs_instant_death() {
        let config = BombConfig::default();
        let mut state = BombState::new(0);
        let died = state.on_hit(&config);
        assert!(died);
    }

    #[test]
    fn extend_score_thresholds() {
        let config = ExtendConfig {
            score_thresholds: vec![100, 500, 1500],
            repeating: false,
        };
        let mut ext = ExtendState::new(config);
        assert_eq!(ext.check_score(50), 0);
        assert_eq!(ext.check_score(100), 1);
        assert_eq!(ext.check_score(200), 0); // already past first, not at second
        assert_eq!(ext.check_score(1500), 2); // crosses both 500 and 1500
        assert_eq!(ext.extends_awarded, 3);
        // No more thresholds.
        assert_eq!(ext.check_score(999_999), 0);
    }

    #[test]
    fn extend_repeating() {
        let config = ExtendConfig {
            score_thresholds: vec![100, 300],
            repeating: true,
        };
        let mut ext = ExtendState::new(config);
        assert_eq!(ext.check_score(100), 1); // threshold 0: 100
        assert_eq!(ext.check_score(300), 1); // threshold 1: 300
                                             // Repeating interval is 300 - 100 = 200, so next at 500.
        assert_eq!(ext.check_score(499), 0);
        assert_eq!(ext.check_score(500), 1);
        assert_eq!(ext.check_score(700), 1); // next at 700
        assert_eq!(ext.extends_awarded, 4);
    }

    #[test]
    fn scoring_chain_mechanics() {
        let mut scoring = ShmupScoring::new(120);
        let s1 = scoring.on_kill(1000);
        assert_eq!(scoring.chain, 1);
        // Multiplier = 1.0 + 1 * 0.1 = 1.1
        assert_eq!(s1, 1100);

        let s2 = scoring.on_kill(1000);
        assert_eq!(scoring.chain, 2);
        // Multiplier = 1.0 + 2 * 0.1 = 1.2
        assert_eq!(s2, 1200);
        assert_eq!(scoring.score, 2300);

        // Break chain.
        scoring.break_chain();
        assert_eq!(scoring.chain, 0);
        assert!((scoring.chain_multiplier - 1.0).abs() < 0.001);

        let s3 = scoring.on_kill(1000);
        // Chain restarted at 1.
        assert_eq!(s3, 1100);
    }

    #[test]
    fn scoring_chain_timeout() {
        let mut scoring = ShmupScoring::new(5);
        scoring.on_kill(100);
        assert_eq!(scoring.chain, 1);
        assert_eq!(scoring.chain_timer, 5);

        // Tick 4 times — chain should still be alive.
        for _ in 0..4 {
            scoring.tick();
        }
        assert_eq!(scoring.chain, 1);

        // 5th tick breaks the chain.
        scoring.tick();
        assert_eq!(scoring.chain, 0);
    }
}
