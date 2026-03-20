use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Bullet pool — object pool for efficient bullet management
// ---------------------------------------------------------------------------

/// A single bullet in the pool.
#[derive(Clone, Debug)]
pub struct Bullet {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub lifetime: u32,
    pub max_lifetime: u32,
    pub radius: f32,
    pub damage: f32,
    pub active: bool,
    /// User-defined type tag for rendering/effects.
    pub kind: u32,
}

/// Events produced by the bullet system.
#[derive(Clone, Debug, PartialEq)]
pub enum BulletEvent {
    /// Bullet expired (lifetime ran out).
    Expired { index: usize },
    /// Bullet left the arena bounds.
    OutOfBounds { index: usize },
}

/// Object pool for bullets. Pre-allocates capacity to avoid per-frame allocations.
pub struct BulletPool {
    pub bullets: Vec<Bullet>,
    pub active_count: usize,
    /// Bounds for auto-despawning bullets that leave the arena.
    pub bounds_x: f32,
    pub bounds_y: f32,
    pub bounds_w: f32,
    pub bounds_h: f32,
}

impl BulletPool {
    pub fn new(capacity: usize) -> Self {
        let mut bullets = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            bullets.push(Bullet {
                x: 0.0,
                y: 0.0,
                vx: 0.0,
                vy: 0.0,
                lifetime: 0,
                max_lifetime: 0,
                radius: 2.0,
                damage: 1.0,
                active: false,
                kind: 0,
            });
        }
        Self {
            bullets,
            active_count: 0,
            bounds_x: -100.0,
            bounds_y: -100.0,
            bounds_w: 1000.0,
            bounds_h: 1000.0,
        }
    }

    pub fn with_bounds(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        self.bounds_x = x;
        self.bounds_y = y;
        self.bounds_w = w;
        self.bounds_h = h;
        self
    }

    /// Spawn a bullet. Returns the index, or None if pool is full.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &mut self,
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        lifetime: u32,
        radius: f32,
        damage: f32,
        kind: u32,
    ) -> Option<usize> {
        for (i, b) in self.bullets.iter_mut().enumerate() {
            if !b.active {
                b.x = x;
                b.y = y;
                b.vx = vx;
                b.vy = vy;
                b.lifetime = 0;
                b.max_lifetime = lifetime;
                b.radius = radius;
                b.damage = damage;
                b.kind = kind;
                b.active = true;
                self.active_count += 1;
                return Some(i);
            }
        }
        None
    }

    /// Despawn a bullet by index.
    pub fn despawn(&mut self, index: usize) {
        if index < self.bullets.len() && self.bullets[index].active {
            self.bullets[index].active = false;
            self.active_count -= 1;
        }
    }

    /// Advance all active bullets by one tick. Returns events.
    pub fn tick(&mut self) -> Vec<BulletEvent> {
        let mut events = Vec::new();
        let bx = self.bounds_x;
        let by = self.bounds_y;
        let bw = self.bounds_w;
        let bh = self.bounds_h;

        for i in 0..self.bullets.len() {
            if !self.bullets[i].active {
                continue;
            }

            self.bullets[i].x += self.bullets[i].vx;
            self.bullets[i].y += self.bullets[i].vy;
            self.bullets[i].lifetime += 1;

            // Lifetime check
            if self.bullets[i].lifetime >= self.bullets[i].max_lifetime {
                self.bullets[i].active = false;
                self.active_count -= 1;
                events.push(BulletEvent::Expired { index: i });
                continue;
            }

            // Bounds check
            let x = self.bullets[i].x;
            let y = self.bullets[i].y;
            if x < bx || x > bx + bw || y < by || y > by + bh {
                self.bullets[i].active = false;
                self.active_count -= 1;
                events.push(BulletEvent::OutOfBounds { index: i });
            }
        }

        events
    }

    /// Check if a circle at (cx, cy) with given radius overlaps any active bullet.
    /// Returns indices of hitting bullets.
    pub fn check_circle_hits(&self, cx: f32, cy: f32, radius: f32) -> Vec<usize> {
        let mut hits = Vec::new();
        for (i, b) in self.bullets.iter().enumerate() {
            if !b.active {
                continue;
            }
            let dx = b.x - cx;
            let dy = b.y - cy;
            let dist_sq = dx * dx + dy * dy;
            let combined_r = b.radius + radius;
            if dist_sq < combined_r * combined_r {
                hits.push(i);
            }
        }
        hits
    }

    /// Despawn all active bullets.
    pub fn clear(&mut self) {
        for b in &mut self.bullets {
            b.active = false;
        }
        self.active_count = 0;
    }

    /// Iterate over active bullets.
    pub fn active_iter(&self) -> impl Iterator<Item = (usize, &Bullet)> {
        self.bullets.iter().enumerate().filter(|(_, b)| b.active)
    }
}

// ---------------------------------------------------------------------------
// Pattern shapes — how bullets are spawned
// ---------------------------------------------------------------------------

/// Shape of a bullet pattern spawn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PatternShape {
    /// Evenly distributed bullets in a circle.
    Radial { count: u32, speed: f32 },
    /// Spiral pattern that rotates over time.
    Spiral {
        count: u32,
        speed: f32,
        /// Radians added per firing.
        rotation_speed: f32,
    },
    /// Aimed at a target with spread.
    Aimed {
        count: u32,
        speed: f32,
        /// Total spread angle in radians.
        spread_angle: f32,
    },
    /// Sine wave pattern.
    Wave {
        count: u32,
        speed: f32,
        amplitude: f32,
        frequency: f32,
    },
    /// Random directions.
    Random {
        count: u32,
        min_speed: f32,
        max_speed: f32,
    },
}

/// Compute bullet velocities for a pattern shape.
/// `rotation` is the current emitter rotation in radians.
/// `target_angle` is the angle toward the target (for Aimed).
/// `rng_state` is for Random patterns.
pub fn compute_pattern(
    shape: &PatternShape,
    rotation: f32,
    target_angle: f32,
    rng_state: &mut u64,
) -> Vec<(f32, f32)> {
    match shape {
        PatternShape::Radial { count, speed } => {
            let step = std::f32::consts::TAU / *count as f32;
            (0..*count)
                .map(|i| {
                    let angle = rotation + step * i as f32;
                    (angle.cos() * speed, angle.sin() * speed)
                })
                .collect()
        }
        PatternShape::Spiral {
            count,
            speed,
            rotation_speed: _,
        } => {
            // rotation already includes accumulated rotation_speed
            let step = std::f32::consts::TAU / *count as f32;
            (0..*count)
                .map(|i| {
                    let angle = rotation + step * i as f32;
                    (angle.cos() * speed, angle.sin() * speed)
                })
                .collect()
        }
        PatternShape::Aimed {
            count,
            speed,
            spread_angle,
        } => {
            if *count == 1 {
                return vec![(target_angle.cos() * speed, target_angle.sin() * speed)];
            }
            let half = spread_angle / 2.0;
            let step = spread_angle / (*count - 1) as f32;
            (0..*count)
                .map(|i| {
                    let angle = target_angle - half + step * i as f32;
                    (angle.cos() * speed, angle.sin() * speed)
                })
                .collect()
        }
        PatternShape::Wave {
            count,
            speed,
            amplitude,
            frequency,
        } => {
            let step = std::f32::consts::TAU / *count as f32;
            (0..*count)
                .map(|i| {
                    let base_angle = rotation + step * i as f32;
                    let wave_offset = (base_angle * frequency).sin() * amplitude;
                    let angle = base_angle + wave_offset;
                    (angle.cos() * speed, angle.sin() * speed)
                })
                .collect()
        }
        PatternShape::Random {
            count,
            min_speed,
            max_speed,
        } => (0..*count)
            .map(|_| {
                let angle = xorshift_f32(rng_state) * std::f32::consts::TAU;
                let speed = min_speed + xorshift_f32(rng_state) * (max_speed - min_speed);
                (angle.cos() * speed, angle.sin() * speed)
            })
            .collect(),
    }
}

/// Simple XorShift64 RNG returning f32 in 0.0..1.0.
fn xorshift_f32(state: &mut u64) -> f32 {
    let mut s = *state;
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    *state = s;
    (s & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

// ---------------------------------------------------------------------------
// Bullet emitter — fires patterns at intervals
// ---------------------------------------------------------------------------

/// A bullet emitter that fires patterns at a fixed rate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BulletEmitter {
    pub x: f32,
    pub y: f32,
    pub pattern: PatternShape,
    /// Ticks between firings.
    pub fire_interval: u32,
    /// Current tick counter.
    pub timer: u32,
    /// Current rotation in radians (accumulates for Spiral).
    pub rotation: f32,
    /// Bullet lifetime in ticks.
    pub bullet_lifetime: u32,
    /// Bullet radius.
    pub bullet_radius: f32,
    /// Bullet damage.
    pub bullet_damage: f32,
    /// Bullet kind tag.
    pub bullet_kind: u32,
    /// If true, emitter is active.
    pub active: bool,
    /// Target position for Aimed patterns.
    pub target_x: f32,
    pub target_y: f32,
    /// RNG state for Random patterns.
    rng_state: u64,
}

impl BulletEmitter {
    pub fn new(x: f32, y: f32, pattern: PatternShape, fire_interval: u32) -> Self {
        Self {
            x,
            y,
            pattern,
            fire_interval,
            timer: 0,
            rotation: 0.0,
            bullet_lifetime: 300,
            bullet_radius: 2.0,
            bullet_damage: 1.0,
            bullet_kind: 0,
            active: true,
            target_x: 0.0,
            target_y: 0.0,
            rng_state: 12345,
        }
    }

    pub fn with_bullet(mut self, lifetime: u32, radius: f32, damage: f32, kind: u32) -> Self {
        self.bullet_lifetime = lifetime;
        self.bullet_radius = radius;
        self.bullet_damage = damage;
        self.bullet_kind = kind;
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.rng_state = seed;
        self
    }

    pub fn set_target(&mut self, x: f32, y: f32) {
        self.target_x = x;
        self.target_y = y;
    }

    /// Tick the emitter. If it fires, spawns bullets into the pool.
    /// Returns the number of bullets spawned.
    pub fn tick(&mut self, pool: &mut BulletPool) -> u32 {
        if !self.active {
            return 0;
        }

        self.timer += 1;
        if self.timer < self.fire_interval {
            return 0;
        }
        self.timer = 0;

        let target_angle = (self.target_y - self.y).atan2(self.target_x - self.x);
        let velocities = compute_pattern(
            &self.pattern,
            self.rotation,
            target_angle,
            &mut self.rng_state,
        );

        // Advance rotation for spiral patterns
        if let PatternShape::Spiral { rotation_speed, .. } = &self.pattern {
            self.rotation += rotation_speed;
        }

        let mut count = 0;
        for (vx, vy) in velocities {
            if pool
                .spawn(
                    self.x,
                    self.y,
                    vx,
                    vy,
                    self.bullet_lifetime,
                    self.bullet_radius,
                    self.bullet_damage,
                    self.bullet_kind,
                )
                .is_some()
            {
                count += 1;
            }
        }
        count
    }
}

// ---------------------------------------------------------------------------
// Pattern sequencer — boss phase patterns
// ---------------------------------------------------------------------------

/// How the sequence loops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceLoop {
    /// Play once, then stop.
    Once,
    /// Loop back to start.
    Loop,
}

/// A phase in a pattern sequence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternPhase {
    /// Emitter configurations for this phase.
    pub emitters: Vec<BulletEmitter>,
    /// Duration of this phase in ticks.
    pub duration: u32,
}

impl PatternPhase {
    pub fn new(emitters: Vec<BulletEmitter>, duration: u32) -> Self {
        Self { emitters, duration }
    }
}

/// A sequence of pattern phases (for boss fights etc).
#[derive(Clone, Debug)]
pub struct PatternSequence {
    pub phases: Vec<PatternPhase>,
    pub current_phase: usize,
    pub phase_timer: u32,
    pub loop_mode: SequenceLoop,
    pub finished: bool,
}

impl PatternSequence {
    pub fn new(phases: Vec<PatternPhase>, loop_mode: SequenceLoop) -> Self {
        Self {
            phases,
            current_phase: 0,
            phase_timer: 0,
            loop_mode,
            finished: false,
        }
    }

    /// Tick the sequence. Fires active emitters into the pool.
    /// Returns true if the phase changed.
    pub fn tick(&mut self, pool: &mut BulletPool) -> bool {
        if self.finished || self.phases.is_empty() {
            return false;
        }

        // Tick all emitters in the current phase
        let phase = &mut self.phases[self.current_phase];
        for emitter in &mut phase.emitters {
            emitter.tick(pool);
        }

        self.phase_timer += 1;
        let duration = self.phases[self.current_phase].duration;

        if self.phase_timer >= duration {
            self.phase_timer = 0;
            self.current_phase += 1;

            if self.current_phase >= self.phases.len() {
                match self.loop_mode {
                    SequenceLoop::Once => {
                        self.finished = true;
                        self.current_phase = self.phases.len() - 1;
                    }
                    SequenceLoop::Loop => {
                        self.current_phase = 0;
                    }
                }
            }
            return true;
        }
        false
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pool basics ─────────────────────────────────────────

    #[test]
    fn pool_spawn_and_despawn() {
        let mut pool = BulletPool::new(10);
        assert_eq!(pool.active_count, 0);

        let idx = pool.spawn(0.0, 0.0, 1.0, 0.0, 100, 2.0, 1.0, 0).unwrap();
        assert_eq!(pool.active_count, 1);
        assert!(pool.bullets[idx].active);

        pool.despawn(idx);
        assert_eq!(pool.active_count, 0);
    }

    #[test]
    fn pool_full() {
        let mut pool = BulletPool::new(2);
        pool.spawn(0.0, 0.0, 1.0, 0.0, 100, 2.0, 1.0, 0);
        pool.spawn(0.0, 0.0, 1.0, 0.0, 100, 2.0, 1.0, 0);
        let result = pool.spawn(0.0, 0.0, 1.0, 0.0, 100, 2.0, 1.0, 0);
        assert!(result.is_none());
    }

    #[test]
    fn pool_tick_moves_bullets() {
        let mut pool = BulletPool::new(10);
        pool.spawn(0.0, 0.0, 2.0, 3.0, 100, 2.0, 1.0, 0);

        pool.tick();

        let b = &pool.bullets[0];
        assert_eq!(b.x, 2.0);
        assert_eq!(b.y, 3.0);
    }

    #[test]
    fn pool_lifetime_expiry() {
        let mut pool = BulletPool::new(10);
        pool.spawn(0.0, 0.0, 0.0, 0.0, 3, 2.0, 1.0, 0);

        for _ in 0..3 {
            pool.tick();
        }

        assert_eq!(pool.active_count, 0);
    }

    #[test]
    fn pool_out_of_bounds() {
        let mut pool = BulletPool::new(10).with_bounds(0.0, 0.0, 100.0, 100.0);
        pool.spawn(50.0, 50.0, 200.0, 0.0, 1000, 2.0, 1.0, 0);

        let events = pool.tick();
        assert!(events
            .iter()
            .any(|e| matches!(e, BulletEvent::OutOfBounds { .. })));
        assert_eq!(pool.active_count, 0);
    }

    #[test]
    fn pool_circle_hit_check() {
        let mut pool = BulletPool::new(10);
        pool.spawn(10.0, 10.0, 0.0, 0.0, 100, 5.0, 1.0, 0);
        pool.spawn(100.0, 100.0, 0.0, 0.0, 100, 5.0, 1.0, 0);

        let hits = pool.check_circle_hits(12.0, 12.0, 5.0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0], 0);
    }

    // ── Pattern computation ─────────────────────────────────

    #[test]
    fn pattern_radial() {
        let mut rng = 1u64;
        let vels = compute_pattern(
            &PatternShape::Radial {
                count: 4,
                speed: 1.0,
            },
            0.0,
            0.0,
            &mut rng,
        );
        assert_eq!(vels.len(), 4);
        // First bullet should go right (angle 0)
        assert!((vels[0].0 - 1.0).abs() < 0.01);
        assert!(vels[0].1.abs() < 0.01);
    }

    #[test]
    fn pattern_aimed() {
        let mut rng = 1u64;
        let vels = compute_pattern(
            &PatternShape::Aimed {
                count: 1,
                speed: 2.0,
                spread_angle: 0.0,
            },
            0.0,
            0.0, // target to the right
            &mut rng,
        );
        assert_eq!(vels.len(), 1);
        assert!((vels[0].0 - 2.0).abs() < 0.01);
    }

    #[test]
    fn pattern_aimed_spread() {
        let mut rng = 1u64;
        let vels = compute_pattern(
            &PatternShape::Aimed {
                count: 3,
                speed: 1.0,
                spread_angle: std::f32::consts::PI,
            },
            0.0,
            0.0,
            &mut rng,
        );
        assert_eq!(vels.len(), 3);
        // Outer bullets should have y components in opposite directions
        assert!(vels[0].1 < -0.1);
        assert!(vels[2].1 > 0.1);
    }

    // ── Emitter behavior ────────────────────────────────────

    #[test]
    fn emitter_fires_at_interval() {
        let mut pool = BulletPool::new(100);
        let mut emitter = BulletEmitter::new(
            0.0,
            0.0,
            PatternShape::Radial {
                count: 4,
                speed: 1.0,
            },
            5,
        );

        // Should not fire for first 4 ticks
        for _ in 0..4 {
            let spawned = emitter.tick(&mut pool);
            assert_eq!(spawned, 0);
        }

        // Should fire on tick 5
        let spawned = emitter.tick(&mut pool);
        assert_eq!(spawned, 4);
        assert_eq!(pool.active_count, 4);
    }

    #[test]
    fn emitter_spiral_rotates() {
        let mut pool = BulletPool::new(1000);
        let mut emitter = BulletEmitter::new(
            0.0,
            0.0,
            PatternShape::Spiral {
                count: 1,
                speed: 1.0,
                rotation_speed: 0.5,
            },
            1,
        );

        emitter.tick(&mut pool);
        let first_vx = pool.bullets[0].vx;

        emitter.tick(&mut pool);
        let second_vx = pool.bullets[1].vx;

        // Rotation should cause different velocity directions
        assert!((first_vx - second_vx).abs() > 0.01, "Spiral should rotate");
    }

    // ── Sequence phases ─────────────────────────────────────

    #[test]
    fn sequence_phase_transition() {
        let mut pool = BulletPool::new(100);

        let phase1 = PatternPhase::new(
            vec![BulletEmitter::new(
                0.0,
                0.0,
                PatternShape::Radial {
                    count: 2,
                    speed: 1.0,
                },
                1,
            )],
            5,
        );
        let phase2 = PatternPhase::new(
            vec![BulletEmitter::new(
                0.0,
                0.0,
                PatternShape::Radial {
                    count: 4,
                    speed: 1.0,
                },
                1,
            )],
            5,
        );

        let mut seq = PatternSequence::new(vec![phase1, phase2], SequenceLoop::Once);

        // Phase 1
        for _ in 0..4 {
            let changed = seq.tick(&mut pool);
            assert!(!changed);
            assert_eq!(seq.current_phase, 0);
        }

        // Phase transition on tick 5
        let changed = seq.tick(&mut pool);
        assert!(changed);
        assert_eq!(seq.current_phase, 1);
    }

    #[test]
    fn sequence_loops() {
        let mut pool = BulletPool::new(1000);

        let phase = PatternPhase::new(
            vec![BulletEmitter::new(
                0.0,
                0.0,
                PatternShape::Radial {
                    count: 1,
                    speed: 1.0,
                },
                1,
            )],
            3,
        );

        let mut seq = PatternSequence::new(vec![phase], SequenceLoop::Loop);

        // Go through the phase twice
        for _ in 0..3 {
            seq.tick(&mut pool);
        }
        assert_eq!(seq.current_phase, 0); // looped back
        assert!(!seq.is_finished());
    }

    #[test]
    fn sequence_once_finishes() {
        let mut pool = BulletPool::new(100);

        let phase = PatternPhase::new(vec![], 2);
        let mut seq = PatternSequence::new(vec![phase], SequenceLoop::Once);

        seq.tick(&mut pool);
        seq.tick(&mut pool);

        assert!(seq.is_finished());
    }

    // ── Pool clear ──────────────────────────────────────────

    #[test]
    fn pool_clear() {
        let mut pool = BulletPool::new(10);
        for _ in 0..5 {
            pool.spawn(0.0, 0.0, 1.0, 0.0, 100, 2.0, 1.0, 0);
        }
        assert_eq!(pool.active_count, 5);

        pool.clear();
        assert_eq!(pool.active_count, 0);
    }
}
