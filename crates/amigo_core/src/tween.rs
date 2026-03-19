//! Tween system: value interpolation with easing functions.
//!
//! Interpolates values over time using configurable easing curves (Penner functions).
//! Used for UI animations, camera transitions, visual polish, and any smooth A→B transition.

use crate::color::Color;
use crate::math::{Fix, RenderVec2, SimVec2};
use rustc_hash::FxHashMap;
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Tweenable trait
// ---------------------------------------------------------------------------

/// Trait for types that can be interpolated.
pub trait Tweenable: Clone + Send + 'static {
    /// Linearly interpolate between `self` and `target` at parameter `t`.
    /// `t` is typically in [0.0, 1.0] but may exceed this range for
    /// Elastic/Back easing (extrapolation must be handled correctly).
    fn lerp(&self, target: &Self, t: f32) -> Self;
}

impl Tweenable for f32 {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        self + (target - self) * t
    }
}

impl Tweenable for RenderVec2 {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        RenderVec2 {
            x: self.x + (target.x - self.x) * t,
            y: self.y + (target.y - self.y) * t,
        }
    }
}

impl Tweenable for Color {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        Color {
            r: self.r + (target.r - self.r) * t,
            g: self.g + (target.g - self.g) * t,
            b: self.b + (target.b - self.b) * t,
            a: self.a + (target.a - self.a) * t,
        }
    }
}

impl Tweenable for Fix {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        let t_fix = Fix::from_num(t);
        *self + (*target - *self) * t_fix
    }
}

impl Tweenable for SimVec2 {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        let t_fix = Fix::from_num(t);
        SimVec2 {
            x: self.x + (target.x - self.x) * t_fix,
            y: self.y + (target.y - self.y) * t_fix,
        }
    }
}

// ---------------------------------------------------------------------------
// Easing functions (Penner curves)
// ---------------------------------------------------------------------------

/// Complete easing function library (Robert Penner curves).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EasingFn {
    Linear,
    // Quadratic
    QuadIn,
    QuadOut,
    QuadInOut,
    // Cubic
    CubicIn,
    CubicOut,
    CubicInOut,
    // Quartic
    QuartIn,
    QuartOut,
    QuartInOut,
    // Quintic
    QuintIn,
    QuintOut,
    QuintInOut,
    // Sine
    SineIn,
    SineOut,
    SineInOut,
    // Exponential
    ExpoIn,
    ExpoOut,
    ExpoInOut,
    // Circular
    CircIn,
    CircOut,
    CircInOut,
    // Elastic (spring-like overshoot)
    ElasticIn,
    ElasticOut,
    ElasticInOut,
    // Back (overshoot and return)
    BackIn,
    BackOut,
    BackInOut,
    // Bounce (ball-drop effect)
    BounceIn,
    BounceOut,
    BounceInOut,
}

impl EasingFn {
    /// Compute the eased value. Input `t` in [0.0, 1.0].
    /// Output is typically in [0.0, 1.0] but Elastic/Back may temporarily exceed.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,

            // Quadratic
            Self::QuadIn => t * t,
            Self::QuadOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::QuadInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }

            // Cubic
            Self::CubicIn => t * t * t,
            Self::CubicOut => 1.0 - (1.0 - t).powi(3),
            Self::CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }

            // Quartic
            Self::QuartIn => t * t * t * t,
            Self::QuartOut => 1.0 - (1.0 - t).powi(4),
            Self::QuartInOut => {
                if t < 0.5 {
                    8.0 * t.powi(4)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
                }
            }

            // Quintic
            Self::QuintIn => t.powi(5),
            Self::QuintOut => 1.0 - (1.0 - t).powi(5),
            Self::QuintInOut => {
                if t < 0.5 {
                    16.0 * t.powi(5)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
                }
            }

            // Sine
            Self::SineIn => 1.0 - (t * PI / 2.0).cos(),
            Self::SineOut => (t * PI / 2.0).sin(),
            Self::SineInOut => -(((t * PI).cos() - 1.0) / 2.0),

            // Exponential
            Self::ExpoIn => {
                if t == 0.0 {
                    0.0
                } else {
                    (2.0_f32).powf(10.0 * t - 10.0)
                }
            }
            Self::ExpoOut => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - (2.0_f32).powf(-10.0 * t)
                }
            }
            Self::ExpoInOut => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else if t < 0.5 {
                    (2.0_f32).powf(20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - (2.0_f32).powf(-20.0 * t + 10.0)) / 2.0
                }
            }

            // Circular
            Self::CircIn => 1.0 - (1.0 - t * t).sqrt(),
            Self::CircOut => (1.0 - (t - 1.0).powi(2)).sqrt(),
            Self::CircInOut => {
                if t < 0.5 {
                    (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
                } else {
                    ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
                }
            }

            // Elastic
            Self::ElasticIn => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = 2.0 * PI / 3.0;
                    -(2.0_f32).powf(10.0 * t - 10.0) * ((10.0 * t - 10.75) * c4).sin()
                }
            }
            Self::ElasticOut => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = 2.0 * PI / 3.0;
                    (2.0_f32).powf(-10.0 * t) * ((10.0 * t - 0.75) * c4).sin() + 1.0
                }
            }
            Self::ElasticInOut => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c5 = 2.0 * PI / 4.5;
                    if t < 0.5 {
                        -(2.0_f32).powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * c5).sin() / 2.0
                    } else {
                        (2.0_f32).powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c5).sin() / 2.0
                            + 1.0
                    }
                }
            }

            // Back
            Self::BackIn => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            Self::BackOut => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Self::BackInOut => {
                let c1 = 1.70158;
                let c2 = c1 * 1.525;
                if t < 0.5 {
                    ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
                } else {
                    ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (2.0 * t - 2.0) + c2) + 2.0) / 2.0
                }
            }

            // Bounce
            Self::BounceOut => bounce_out(t),
            Self::BounceIn => 1.0 - bounce_out(1.0 - t),
            Self::BounceInOut => {
                if t < 0.5 {
                    (1.0 - bounce_out(1.0 - 2.0 * t)) / 2.0
                } else {
                    (1.0 + bounce_out(2.0 * t - 1.0)) / 2.0
                }
            }
        }
    }
}

fn bounce_out(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

// ---------------------------------------------------------------------------
// Tween<T>
// ---------------------------------------------------------------------------

/// State of a tween.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TweenState {
    Running,
    Paused,
    Completed,
}

/// An active tween interpolating a value from `from` to `to` over `duration` seconds.
pub struct Tween<T: Tweenable> {
    from: T,
    to: T,
    easing: EasingFn,
    elapsed: f32,
    duration: f32,
    state: TweenState,
}

impl<T: Tweenable> Tween<T> {
    pub fn new(from: T, to: T, duration: f32, easing: EasingFn) -> Self {
        Self {
            from,
            to,
            easing,
            elapsed: 0.0,
            duration: duration.max(f32::EPSILON),
            state: TweenState::Running,
        }
    }

    /// Get the current interpolated value.
    pub fn current(&self) -> T {
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        let eased = self.easing.apply(t);
        self.from.lerp(&self.to, eased)
    }

    /// Get progress as 0.0..1.0.
    pub fn progress(&self) -> f32 {
        (self.elapsed / self.duration).clamp(0.0, 1.0)
    }

    pub fn is_complete(&self) -> bool {
        self.state == TweenState::Completed
    }

    /// Advance the tween by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        if self.state != TweenState::Running {
            return;
        }
        self.elapsed += dt;
        if self.elapsed >= self.duration {
            self.elapsed = self.duration;
            self.state = TweenState::Completed;
        }
    }

    pub fn pause(&mut self) {
        if self.state == TweenState::Running {
            self.state = TweenState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == TweenState::Paused {
            self.state = TweenState::Running;
        }
    }

    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.state = TweenState::Running;
    }
}

// ---------------------------------------------------------------------------
// TweenSequence<T>
// ---------------------------------------------------------------------------

enum TweenStep<T: Tweenable> {
    Animate { tween: Tween<T> },
    Delay { remaining: f32, total: f32 },
}

/// Repeat mode for sequences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepeatCount {
    Once,
    Times(u32),
    Forever,
}

/// A sequence of tweens with chaining, delays, repeat, and yoyo.
pub struct TweenSequence<T: Tweenable> {
    steps: Vec<TweenStep<T>>,
    current_step: usize,
    repeat_count: RepeatCount,
    repeats_done: u32,
    yoyo: bool,
    direction_forward: bool,
    last_value: T,
}

impl<T: Tweenable> TweenSequence<T> {
    pub fn new(from: T, to: T, duration: f32, easing: EasingFn) -> Self {
        let last = to.clone();
        Self {
            steps: vec![TweenStep::Animate {
                tween: Tween::new(from, to, duration, easing),
            }],
            current_step: 0,
            repeat_count: RepeatCount::Once,
            repeats_done: 0,
            yoyo: false,
            direction_forward: true,
            last_value: last,
        }
    }

    /// Chain another tween. The `from` is automatically the previous `to`.
    pub fn then(mut self, to: T, duration: f32, easing: EasingFn) -> Self {
        let from = self.last_value.clone();
        self.last_value = to.clone();
        self.steps.push(TweenStep::Animate {
            tween: Tween::new(from, to, duration, easing),
        });
        self
    }

    /// Insert a delay between tweens.
    pub fn delay(mut self, duration: f32) -> Self {
        self.steps.push(TweenStep::Delay {
            remaining: duration,
            total: duration,
        });
        self
    }

    /// Set repeat mode.
    pub fn repeat(mut self, count: RepeatCount) -> Self {
        self.repeat_count = count;
        self
    }

    /// Enable yoyo (plays forward then backward, implies repeat).
    pub fn yoyo(mut self) -> Self {
        self.yoyo = true;
        if self.repeat_count == RepeatCount::Once {
            self.repeat_count = RepeatCount::Times(2);
        }
        self
    }

    /// Get the current interpolated value.
    pub fn current(&self) -> T {
        if self.current_step >= self.steps.len() {
            return self.last_value.clone();
        }
        match &self.steps[self.current_step] {
            TweenStep::Animate { tween } => tween.current(),
            TweenStep::Delay { .. } => {
                // During delay, return the last animated value
                if self.current_step > 0 {
                    for i in (0..self.current_step).rev() {
                        if let TweenStep::Animate { tween } = &self.steps[i] {
                            return tween.current();
                        }
                    }
                }
                self.last_value.clone()
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        if self.current_step >= self.steps.len() {
            match self.repeat_count {
                RepeatCount::Once => true,
                RepeatCount::Times(n) => self.repeats_done >= n,
                RepeatCount::Forever => false,
            }
        } else {
            false
        }
    }

    /// Advance the sequence by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        if self.is_complete() {
            return;
        }

        if self.current_step >= self.steps.len() {
            // End of sequence — check repeat
            self.repeats_done += 1;
            match self.repeat_count {
                RepeatCount::Once => return,
                RepeatCount::Times(n) if self.repeats_done >= n => return,
                _ => {}
            }
            if self.yoyo {
                self.direction_forward = !self.direction_forward;
            }
            // Reset all steps
            for step in &mut self.steps {
                match step {
                    TweenStep::Animate { tween } => tween.reset(),
                    TweenStep::Delay { remaining, total } => *remaining = *total,
                }
            }
            self.current_step = 0;
        }

        let mut remaining_dt = dt;
        while remaining_dt > 0.0 && self.current_step < self.steps.len() {
            match &mut self.steps[self.current_step] {
                TweenStep::Animate { tween } => {
                    tween.update(remaining_dt);
                    if tween.is_complete() {
                        remaining_dt = tween.elapsed - tween.duration;
                        if remaining_dt < 0.0 {
                            remaining_dt = 0.0;
                        }
                        self.current_step += 1;
                    } else {
                        remaining_dt = 0.0;
                    }
                }
                TweenStep::Delay { remaining, .. } => {
                    *remaining -= remaining_dt;
                    if *remaining <= 0.0 {
                        remaining_dt = -(*remaining);
                        *remaining = 0.0;
                        self.current_step += 1;
                    } else {
                        remaining_dt = 0.0;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TweenHandle & TweenManager
// ---------------------------------------------------------------------------

/// Opaque handle for controlling a registered tween.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TweenHandle(u32);

/// Type-erased tween interface for the manager.
trait ErasedTween: Send {
    fn update(&mut self, dt: f32);
    fn is_complete(&self) -> bool;
    fn pause(&mut self);
    fn resume(&mut self);
}

struct ManagedTween<T: Tweenable> {
    tween: Tween<T>,
    on_update: Box<dyn Fn(&T) + Send>,
}

impl<T: Tweenable> ErasedTween for ManagedTween<T> {
    fn update(&mut self, dt: f32) {
        self.tween.update(dt);
        (self.on_update)(&self.tween.current());
    }

    fn is_complete(&self) -> bool {
        self.tween.is_complete()
    }

    fn pause(&mut self) {
        self.tween.pause();
    }

    fn resume(&mut self) {
        self.tween.resume();
    }
}

struct ManagedSequence<T: Tweenable> {
    seq: TweenSequence<T>,
    on_update: Box<dyn Fn(&T) + Send>,
}

impl<T: Tweenable> ErasedTween for ManagedSequence<T> {
    fn update(&mut self, dt: f32) {
        self.seq.update(dt);
        (self.on_update)(&self.seq.current());
    }

    fn is_complete(&self) -> bool {
        self.seq.is_complete()
    }

    fn pause(&mut self) {
        // Sequences don't have a direct pause — skip update when paused externally
    }

    fn resume(&mut self) {
        // Resume is handled by manager skipping paused tweens
    }
}

/// Central manager that updates all active tweens per frame.
pub struct TweenManager {
    next_id: u32,
    tweens: FxHashMap<u32, Box<dyn ErasedTween>>,
    paused: FxHashMap<u32, bool>,
}

impl TweenManager {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            tweens: FxHashMap::default(),
            paused: FxHashMap::default(),
        }
    }

    /// Register a tween and return a handle.
    pub fn start<T: Tweenable>(
        &mut self,
        tween: Tween<T>,
        on_update: impl Fn(&T) + Send + 'static,
    ) -> TweenHandle {
        let id = self.next_id;
        self.next_id += 1;
        self.tweens.insert(
            id,
            Box::new(ManagedTween {
                tween,
                on_update: Box::new(on_update),
            }),
        );
        TweenHandle(id)
    }

    /// Register a tween sequence and return a handle.
    pub fn start_sequence<T: Tweenable>(
        &mut self,
        seq: TweenSequence<T>,
        on_update: impl Fn(&T) + Send + 'static,
    ) -> TweenHandle {
        let id = self.next_id;
        self.next_id += 1;
        self.tweens.insert(
            id,
            Box::new(ManagedSequence {
                seq,
                on_update: Box::new(on_update),
            }),
        );
        TweenHandle(id)
    }

    /// Update all active tweens. Removes completed ones.
    pub fn update(&mut self, dt: f32) {
        let mut to_remove = Vec::new();
        for (&id, tween) in self.tweens.iter_mut() {
            if self.paused.get(&id).copied().unwrap_or(false) {
                continue;
            }
            tween.update(dt);
            if tween.is_complete() {
                to_remove.push(id);
            }
        }
        for id in to_remove {
            self.tweens.remove(&id);
            self.paused.remove(&id);
        }
    }

    pub fn pause(&mut self, handle: TweenHandle) {
        if self.tweens.contains_key(&handle.0) {
            self.paused.insert(handle.0, true);
        }
    }

    pub fn resume(&mut self, handle: TweenHandle) {
        self.paused.remove(&handle.0);
    }

    pub fn cancel(&mut self, handle: TweenHandle) {
        self.tweens.remove(&handle.0);
        self.paused.remove(&handle.0);
    }

    pub fn is_active(&self, handle: TweenHandle) -> bool {
        self.tweens.contains_key(&handle.0)
    }

    pub fn active_count(&self) -> usize {
        self.tweens.len()
    }
}

impl Default for TweenManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_linear() {
        assert_eq!(EasingFn::Linear.apply(0.0), 0.0);
        assert_eq!(EasingFn::Linear.apply(0.5), 0.5);
        assert_eq!(EasingFn::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn test_easing_boundaries() {
        // All easing functions should return 0.0 at t=0 and 1.0 at t=1
        let easings = [
            EasingFn::QuadIn,
            EasingFn::QuadOut,
            EasingFn::QuadInOut,
            EasingFn::CubicIn,
            EasingFn::CubicOut,
            EasingFn::CubicInOut,
            EasingFn::SineIn,
            EasingFn::SineOut,
            EasingFn::SineInOut,
            EasingFn::ExpoIn,
            EasingFn::ExpoOut,
            EasingFn::ExpoInOut,
            EasingFn::CircIn,
            EasingFn::CircOut,
            EasingFn::CircInOut,
            EasingFn::BounceIn,
            EasingFn::BounceOut,
            EasingFn::BounceInOut,
            EasingFn::BackIn,
            EasingFn::BackOut,
            EasingFn::BackInOut,
            EasingFn::ElasticIn,
            EasingFn::ElasticOut,
            EasingFn::ElasticInOut,
        ];
        for e in easings {
            let v0 = e.apply(0.0);
            let v1 = e.apply(1.0);
            assert!((v0 - 0.0).abs() < 0.001, "{:?} at t=0: got {}", e, v0);
            assert!((v1 - 1.0).abs() < 0.001, "{:?} at t=1: got {}", e, v1);
        }
    }

    #[test]
    fn test_tween_f32() {
        let mut tw = Tween::new(0.0_f32, 100.0, 1.0, EasingFn::Linear);
        assert!(!tw.is_complete());
        tw.update(0.5);
        let v = tw.current();
        assert!((v - 50.0).abs() < 0.01);
        tw.update(0.5);
        assert!(tw.is_complete());
        let v = tw.current();
        assert!((v - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_tween_pause_resume() {
        let mut tw = Tween::new(0.0_f32, 100.0, 1.0, EasingFn::Linear);
        tw.update(0.25);
        tw.pause();
        tw.update(0.5); // Should be ignored
        assert!((tw.current() - 25.0).abs() < 0.01);
        tw.resume();
        tw.update(0.25);
        assert!((tw.current() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_tween_rendervec2() {
        let mut tw = Tween::new(
            RenderVec2::new(0.0, 0.0),
            RenderVec2::new(100.0, 200.0),
            1.0,
            EasingFn::Linear,
        );
        tw.update(0.5);
        let v = tw.current();
        assert!((v.x - 50.0).abs() < 0.01);
        assert!((v.y - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_sequence_then() {
        let mut seq = TweenSequence::new(0.0_f32, 50.0, 1.0, EasingFn::Linear).then(
            100.0,
            1.0,
            EasingFn::Linear,
        );

        seq.update(0.5); // Midway through first tween
        assert!((seq.current() - 25.0).abs() < 0.5);

        seq.update(0.5); // End of first tween
        seq.update(0.5); // Midway through second tween
        assert!((seq.current() - 75.0).abs() < 0.5);

        seq.update(0.5); // End of second tween
        assert!(seq.is_complete());
        assert!((seq.current() - 100.0).abs() < 0.5);
    }

    #[test]
    fn test_sequence_delay() {
        let mut seq = TweenSequence::new(0.0_f32, 100.0, 1.0, EasingFn::Linear)
            .delay(0.5)
            .then(200.0, 1.0, EasingFn::Linear);

        seq.update(1.0); // Complete first tween
        assert!((seq.current() - 100.0).abs() < 0.5);

        seq.update(0.25); // In delay
        assert!((seq.current() - 100.0).abs() < 0.5); // Stays at 100

        seq.update(0.25); // End delay
        seq.update(0.5); // Midway second tween
        assert!((seq.current() - 150.0).abs() < 1.0);
    }

    #[test]
    fn test_manager_lifecycle() {
        use std::sync::{Arc, Mutex};

        let captured = Arc::new(Mutex::new(0.0_f32));
        let captured_clone = captured.clone();

        let mut mgr = TweenManager::new();
        let h = mgr.start(
            Tween::new(0.0_f32, 100.0, 1.0, EasingFn::Linear),
            move |v| {
                *captured_clone.lock().unwrap() = *v;
            },
        );

        assert!(mgr.is_active(h));
        assert_eq!(mgr.active_count(), 1);

        mgr.update(0.5);
        assert!((*captured.lock().unwrap() - 50.0).abs() < 1.0);

        mgr.update(0.5);
        // Tween completed — should be removed
        assert!(!mgr.is_active(h));
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_manager_cancel() {
        let mut mgr = TweenManager::new();
        let h = mgr.start(Tween::new(0.0_f32, 100.0, 10.0, EasingFn::Linear), |_| {});
        assert!(mgr.is_active(h));
        mgr.cancel(h);
        assert!(!mgr.is_active(h));
    }

    #[test]
    fn test_color_tween() {
        let mut tw = Tween::new(Color::BLACK, Color::WHITE, 1.0, EasingFn::Linear);
        tw.update(0.5);
        let c = tw.current();
        assert!((c.r - 0.5).abs() < 0.01);
        assert!((c.g - 0.5).abs() < 0.01);
        assert!((c.b - 0.5).abs() < 0.01);
        assert!((c.a - 1.0).abs() < 0.01); // Both BLACK and WHITE have a=1.0
    }
}
