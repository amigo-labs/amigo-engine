//! Bullet pools, pattern shapes, emitters and phase sequences for shmups and
//! bullet hells, in fixed point.
//!
//! Positions, velocities and angles are [`Fix`]/[`SimVec2`] and the pattern
//! math uses [`crate::math::trig`], so a pattern fires the same bullets on
//! every machine: `f32::sin`/`cos` differ in the last bits between
//! platforms, which is enough to desync a replay or a lockstep match within
//! seconds of a spiral (ADR-0001).

use crate::math::trig::{TAU, sin_fix};
use crate::math::{Fix, SimVec2};
use crate::rect::SimRect;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Bullet pool — object pool for efficient bullet management
// ---------------------------------------------------------------------------

/// A single bullet in the pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bullet {
    pub pos: SimVec2,
    pub vel: SimVec2,
    pub lifetime: u32,
    pub max_lifetime: u32,
    pub radius: Fix,
    pub damage: Fix,
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
    pub bounds: SimRect,
}

impl BulletPool {
    pub fn new(capacity: usize) -> Self {
        let idle = Bullet {
            pos: SimVec2::ZERO,
            vel: SimVec2::ZERO,
            lifetime: 0,
            max_lifetime: 0,
            radius: Fix::from_num(2),
            damage: Fix::ONE,
            active: false,
            kind: 0,
        };
        Self {
            bullets: vec![idle; capacity],
            active_count: 0,
            bounds: SimRect::from_num(-100, -100, 1000, 1000),
        }
    }

    pub fn with_bounds(mut self, bounds: SimRect) -> Self {
        self.bounds = bounds;
        self
    }

    /// Spawn a bullet in the first free slot. Returns the index, or None if
    /// the pool is full.
    pub fn spawn(
        &mut self,
        pos: SimVec2,
        vel: SimVec2,
        lifetime: u32,
        radius: Fix,
        damage: Fix,
        kind: u32,
    ) -> Option<usize> {
        let (i, b) = self
            .bullets
            .iter_mut()
            .enumerate()
            .find(|(_, b)| !b.active)?;
        *b = Bullet {
            pos,
            vel,
            lifetime: 0,
            max_lifetime: lifetime,
            radius,
            damage,
            active: true,
            kind,
        };
        self.active_count += 1;
        Some(i)
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
        let bounds = self.bounds;

        for (i, b) in self.bullets.iter_mut().enumerate() {
            if !b.active {
                continue;
            }

            b.pos += b.vel;
            b.lifetime += 1;

            // Lifetime check
            if b.lifetime >= b.max_lifetime {
                b.active = false;
                self.active_count -= 1;
                events.push(BulletEvent::Expired { index: i });
                continue;
            }

            // Bounds check (inclusive edges)
            let p = b.pos;
            if p.x < bounds.x || p.x > bounds.right() || p.y < bounds.y || p.y > bounds.bottom() {
                b.active = false;
                self.active_count -= 1;
                events.push(BulletEvent::OutOfBounds { index: i });
            }
        }

        events
    }

    /// Indices of active bullets overlapping the circle at `center`.
    pub fn check_circle_hits(&self, center: SimVec2, radius: Fix) -> Vec<usize> {
        self.active_iter()
            .filter(|(_, b)| (b.pos - center).length() < b.radius + radius)
            .map(|(i, _)| i)
            .collect()
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

/// Shape of a bullet pattern spawn. Speeds are units per tick, angles radians.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PatternShape {
    /// Evenly distributed bullets in a circle.
    Radial { count: u32, speed: Fix },
    /// Spiral pattern that rotates over time.
    Spiral {
        count: u32,
        speed: Fix,
        /// Radians added per firing.
        rotation_speed: Fix,
    },
    /// Aimed at a target with spread.
    Aimed {
        count: u32,
        speed: Fix,
        /// Total spread angle in radians.
        spread_angle: Fix,
    },
    /// Sine wave pattern.
    Wave {
        count: u32,
        speed: Fix,
        amplitude: Fix,
        frequency: Fix,
    },
    /// Random directions.
    Random {
        count: u32,
        min_speed: Fix,
        max_speed: Fix,
    },
}

/// `count` angles evenly around the circle starting at `rotation`.
fn ring(count: u32, rotation: Fix) -> impl Iterator<Item = Fix> {
    let step = TAU / Fix::from_num(count.max(1));
    (0..count).map(move |i| rotation + step * Fix::from_num(i))
}

/// Compute bullet velocities for a pattern shape.
/// `rotation` is the current emitter rotation in radians.
/// `target_angle` is the angle toward the target (for Aimed).
/// `rng_state` is for Random patterns.
pub fn compute_pattern(
    shape: &PatternShape,
    rotation: Fix,
    target_angle: Fix,
    rng_state: &mut u64,
) -> Vec<SimVec2> {
    let shot = |angle: Fix, speed: Fix| SimVec2::from_angle(angle) * speed;
    match shape {
        // A spiral is a ring whose rotation the emitter advances.
        PatternShape::Radial { count, speed } | PatternShape::Spiral { count, speed, .. } => {
            ring(*count, rotation).map(|a| shot(a, *speed)).collect()
        }
        PatternShape::Aimed {
            count,
            speed,
            spread_angle,
        } => {
            if *count <= 1 {
                return (0..*count).map(|_| shot(target_angle, *speed)).collect();
            }
            let half = *spread_angle / 2;
            let step = *spread_angle / Fix::from_num(*count - 1);
            (0..*count)
                .map(|i| shot(target_angle - half + step * Fix::from_num(i), *speed))
                .collect()
        }
        PatternShape::Wave {
            count,
            speed,
            amplitude,
            frequency,
        } => ring(*count, rotation)
            .map(|base| {
                let wave_offset = sin_fix(base.saturating_mul(*frequency)) * *amplitude;
                shot(base + wave_offset, *speed)
            })
            .collect(),
        PatternShape::Random {
            count,
            min_speed,
            max_speed,
        } => (0..*count)
            .map(|_| {
                let angle = xorshift_unit(rng_state) * TAU;
                let speed = *min_speed + xorshift_unit(rng_state) * (*max_speed - *min_speed);
                shot(angle, speed)
            })
            .collect(),
    }
}

/// XorShift64 step returning a [`Fix`] in `[0, 1)` (16 random fraction bits).
fn xorshift_unit(state: &mut u64) -> Fix {
    let mut s = *state;
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    *state = s;
    Fix::from_bits((s & 0xFFFF) as i32)
}

// ---------------------------------------------------------------------------
// Bullet emitter — fires patterns at intervals
// ---------------------------------------------------------------------------

/// A bullet emitter that fires patterns at a fixed rate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BulletEmitter {
    pub pos: SimVec2,
    pub pattern: PatternShape,
    /// Ticks between firings.
    pub fire_interval: u32,
    /// Current tick counter.
    pub timer: u32,
    /// Current rotation in radians (accumulates for Spiral).
    pub rotation: Fix,
    /// Bullet lifetime in ticks.
    pub bullet_lifetime: u32,
    /// Bullet radius.
    pub bullet_radius: Fix,
    /// Bullet damage.
    pub bullet_damage: Fix,
    /// Bullet kind tag.
    pub bullet_kind: u32,
    /// If true, emitter is active.
    pub active: bool,
    /// Target position for Aimed patterns.
    pub target: SimVec2,
    /// RNG state for Random patterns.
    rng_state: u64,
}

impl BulletEmitter {
    pub fn new(pos: SimVec2, pattern: PatternShape, fire_interval: u32) -> Self {
        Self {
            pos,
            pattern,
            fire_interval,
            timer: 0,
            rotation: Fix::ZERO,
            bullet_lifetime: 300,
            bullet_radius: Fix::from_num(2),
            bullet_damage: Fix::ONE,
            bullet_kind: 0,
            active: true,
            target: SimVec2::ZERO,
            rng_state: 12345,
        }
    }

    pub fn with_bullet(mut self, lifetime: u32, radius: Fix, damage: Fix, kind: u32) -> Self {
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

    pub fn set_target(&mut self, target: SimVec2) {
        self.target = target;
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

        let target_angle = (self.target - self.pos).angle();
        let velocities = compute_pattern(
            &self.pattern,
            self.rotation,
            target_angle,
            &mut self.rng_state,
        );

        // Advance rotation for spiral patterns, kept in [0, 2π) so it never
        // grows out of the Q16.16 range over a long fight.
        if let PatternShape::Spiral { rotation_speed, .. } = &self.pattern {
            self.rotation = (self.rotation + *rotation_speed).rem_euclid(TAU);
        }

        let mut count = 0;
        for vel in velocities {
            if pool
                .spawn(
                    self.pos,
                    vel,
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

    fn f(v: impl fixed::traits::ToFixed) -> Fix {
        Fix::from_num(v)
    }
    fn v(x: impl fixed::traits::ToFixed, y: impl fixed::traits::ToFixed) -> SimVec2 {
        SimVec2::from_num(x, y)
    }
    fn spawn(
        pool: &mut BulletPool,
        pos: SimVec2,
        vel: SimVec2,
        lifetime: u32,
        radius: i32,
    ) -> Option<usize> {
        pool.spawn(pos, vel, lifetime, f(radius), Fix::ONE, 0)
    }
    fn radial(count: u32) -> PatternShape {
        PatternShape::Radial {
            count,
            speed: Fix::ONE,
        }
    }

    // ── Pool basics ─────────────────────────────────────────

    #[test]
    fn pool_spawn_and_despawn() {
        let mut pool = BulletPool::new(10);
        assert_eq!(pool.active_count, 0);

        let idx = spawn(&mut pool, v(0, 0), v(1, 0), 100, 2).unwrap();
        assert_eq!(pool.active_count, 1);
        assert!(pool.bullets[idx].active);

        pool.despawn(idx);
        assert_eq!(pool.active_count, 0);
    }

    #[test]
    fn pool_full() {
        let mut pool = BulletPool::new(2);
        spawn(&mut pool, v(0, 0), v(1, 0), 100, 2);
        spawn(&mut pool, v(0, 0), v(1, 0), 100, 2);
        assert!(spawn(&mut pool, v(0, 0), v(1, 0), 100, 2).is_none());
    }

    #[test]
    fn pool_tick_moves_bullets() {
        let mut pool = BulletPool::new(10);
        spawn(&mut pool, v(0, 0), v(2, 3), 100, 2);
        pool.tick();
        assert_eq!(pool.bullets[0].pos, v(2, 3));
    }

    #[test]
    fn pool_lifetime_expiry() {
        let mut pool = BulletPool::new(10);
        spawn(&mut pool, v(0, 0), v(0, 0), 3, 2);
        for _ in 0..3 {
            pool.tick();
        }
        assert_eq!(pool.active_count, 0);
    }

    #[test]
    fn pool_out_of_bounds() {
        let mut pool = BulletPool::new(10).with_bounds(SimRect::from_num(0, 0, 100, 100));
        spawn(&mut pool, v(50, 50), v(200, 0), 1000, 2);

        let events = pool.tick();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BulletEvent::OutOfBounds { .. }))
        );
        assert_eq!(pool.active_count, 0);
    }

    #[test]
    fn pool_circle_hit_check() {
        let mut pool = BulletPool::new(10);
        spawn(&mut pool, v(10, 10), v(0, 0), 100, 5);
        spawn(&mut pool, v(100, 100), v(0, 0), 100, 5);
        assert_eq!(pool.check_circle_hits(v(12, 12), f(5)), [0]);
    }

    // ── Pattern computation ─────────────────────────────────

    #[test]
    fn pattern_radial() {
        let mut rng = 1u64;
        let vels = compute_pattern(&radial(4), Fix::ZERO, Fix::ZERO, &mut rng);
        assert_eq!(vels.len(), 4);
        // First bullet goes right (angle 0), the second down (+y).
        assert_eq!(vels[0], v(1, 0));
        assert!((vels[1].y - Fix::ONE).abs() < f(0.001) && vels[1].x.abs() < f(0.001));
    }

    #[test]
    fn pattern_aimed() {
        let mut rng = 1u64;
        let vels = compute_pattern(
            &PatternShape::Aimed {
                count: 1,
                speed: f(2),
                spread_angle: Fix::ZERO,
            },
            Fix::ZERO,
            Fix::ZERO, // target to the right
            &mut rng,
        );
        assert_eq!(vels, [v(2, 0)]);
    }

    #[test]
    fn pattern_aimed_spread() {
        let mut rng = 1u64;
        let vels = compute_pattern(
            &PatternShape::Aimed {
                count: 3,
                speed: Fix::ONE,
                spread_angle: crate::math::trig::PI,
            },
            Fix::ZERO,
            Fix::ZERO,
            &mut rng,
        );
        assert_eq!(vels.len(), 3);
        // Outer bullets have y components in opposite directions
        assert!(vels[0].y < f(-0.1));
        assert!(vels[2].y > f(0.1));
    }

    #[test]
    fn random_and_wave_patterns_stay_within_their_speeds() {
        let mut rng = 99u64;
        let vels = compute_pattern(
            &PatternShape::Random {
                count: 50,
                min_speed: f(1),
                max_speed: f(3),
            },
            Fix::ZERO,
            Fix::ZERO,
            &mut rng,
        );
        for vel in &vels {
            let speed = vel.length();
            assert!(speed >= f(0.999) && speed < f(3.001), "{speed}");
        }
        let waves = compute_pattern(
            &PatternShape::Wave {
                count: 8,
                speed: f(2),
                amplitude: f(0.3),
                frequency: f(3),
            },
            Fix::ZERO,
            Fix::ZERO,
            &mut rng,
        );
        assert_eq!(waves.len(), 8);
        assert!(waves.iter().all(|w| (w.length() - f(2)).abs() < f(0.01)));
    }

    // ── Emitter behavior ────────────────────────────────────

    #[test]
    fn emitter_fires_at_interval() {
        let mut pool = BulletPool::new(100);
        let mut emitter = BulletEmitter::new(v(0, 0), radial(4), 5);

        // Should not fire for first 4 ticks
        for _ in 0..4 {
            assert_eq!(emitter.tick(&mut pool), 0);
        }

        // Should fire on tick 5
        assert_eq!(emitter.tick(&mut pool), 4);
        assert_eq!(pool.active_count, 4);
    }

    #[test]
    fn emitter_spiral_rotates_and_wraps() {
        let mut pool = BulletPool::new(1000);
        let mut emitter = BulletEmitter::new(
            v(0, 0),
            PatternShape::Spiral {
                count: 1,
                speed: Fix::ONE,
                rotation_speed: f(0.5),
            },
            1,
        );

        emitter.tick(&mut pool);
        emitter.tick(&mut pool);
        assert_ne!(
            pool.bullets[0].vel, pool.bullets[1].vel,
            "Spiral should rotate"
        );

        for _ in 0..500 {
            emitter.tick(&mut pool);
            assert!(emitter.rotation >= Fix::ZERO && emitter.rotation < TAU);
        }
    }

    #[test]
    fn emitter_aims_at_its_target() {
        let mut pool = BulletPool::new(10);
        let mut emitter = BulletEmitter::new(
            v(10, 10),
            PatternShape::Aimed {
                count: 1,
                speed: f(4),
                spread_angle: Fix::ZERO,
            },
            1,
        );
        emitter.set_target(v(10, 50)); // straight down
        emitter.tick(&mut pool);
        let vel = pool.bullets[0].vel;
        assert!(
            vel.x.abs() < f(0.001) && (vel.y - f(4)).abs() < f(0.001),
            "{vel:?}"
        );
    }

    // ── Sequence phases ─────────────────────────────────────

    #[test]
    fn sequence_phase_transition() {
        let mut pool = BulletPool::new(100);
        let phase1 = PatternPhase::new(vec![BulletEmitter::new(v(0, 0), radial(2), 1)], 5);
        let phase2 = PatternPhase::new(vec![BulletEmitter::new(v(0, 0), radial(4), 1)], 5);
        let mut seq = PatternSequence::new(vec![phase1, phase2], SequenceLoop::Once);

        // Phase 1
        for _ in 0..4 {
            assert!(!seq.tick(&mut pool));
            assert_eq!(seq.current_phase, 0);
        }

        // Phase transition on tick 5
        assert!(seq.tick(&mut pool));
        assert_eq!(seq.current_phase, 1);
    }

    #[test]
    fn sequence_loops() {
        let mut pool = BulletPool::new(1000);
        let phase = PatternPhase::new(vec![BulletEmitter::new(v(0, 0), radial(1), 1)], 3);
        let mut seq = PatternSequence::new(vec![phase], SequenceLoop::Loop);

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
            spawn(&mut pool, v(0, 0), v(1, 0), 100, 2);
        }
        assert_eq!(pool.active_count, 5);
        pool.clear();
        assert_eq!(pool.active_count, 0);
    }
}
