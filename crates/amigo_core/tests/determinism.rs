//! Golden hashes for the deterministic math (ADR-0001).
//!
//! Each scenario hashes the raw bits of everything it computes. CI runs this
//! on Linux, Windows and macOS, so a value that differs on any one platform,
//! or after a change to the algorithms, fails here instead of surfacing as a
//! desync between two players.
//!
//! When a change to the math is intended, run the test, check the new values
//! are right, and update the constants in the same commit.

use amigo_core::math::trig::{PI, acos_fix, asin_fix, atan2_fix, exp2_fix, sin_cos_fix};
use amigo_core::math::{Fix, SimVec2};
use amigo_core::navigation::Direction;
use amigo_core::rts::{FormationConfig, FormationSystem, FormationType};
use amigo_core::tween::EasingFn;

/// FNV-1a over i32s: tiny, fixed, and the same everywhere.
#[derive(Default)]
struct Hash(u64);

impl Hash {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn fix(&mut self, v: Fix) {
        self.i32(v.to_bits());
    }
    fn i32(&mut self, v: i32) {
        for byte in v.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

fn fix(bits: i32) -> Fix {
    Fix::from_bits(bits)
}

#[test]
fn trigonometry_golden_hash() {
    let mut h = Hash::new();
    // Angles from -8π to 8π in steps of 1/64 rad (Q16.16 bits step 1024).
    for bits in (-(PI.to_bits() * 8)..=PI.to_bits() * 8).step_by(1024) {
        let (s, c) = sin_cos_fix(fix(bits));
        h.fix(s);
        h.fix(c);
        h.fix(atan2_fix(s, c));
    }
    // Inverse functions over [-1, 1].
    for bits in (-65536..=65536).step_by(256) {
        h.fix(acos_fix(fix(bits)));
        h.fix(asin_fix(fix(bits)));
    }
    // exp2 over [-16, 15).
    for bits in (-16 * 65536..15 * 65536).step_by(4099) {
        h.fix(exp2_fix(fix(bits)));
    }
    assert_eq!(h.0, TRIG, "trigonometry changed: {:#018x}", h.0);
}

#[test]
fn formations_and_facing_golden_hash() {
    let mut h = Hash::new();
    let config = FormationConfig::default();
    let target = SimVec2::new(Fix::from_num(100), Fix::from_num(-40));
    for formation in [
        FormationType::Line,
        FormationType::Wedge,
        FormationType::Block,
    ] {
        for step in 0..64 {
            let facing = Fix::from_num(step) / Fix::from_num(10);
            let slots = FormationSystem::compute_slots(formation, 13, target, facing, &config);
            for slot in &slots.slots {
                h.fix(slot.x);
                h.fix(slot.y);
            }
        }
    }
    for dy in -20..=20 {
        for dx in -20..=20 {
            let d = Direction::from_delta_fix(Fix::from_num(dx) / 7, Fix::from_num(dy) / 5);
            h.i32(d as i32);
        }
    }
    assert_eq!(h.0, FORMATIONS, "formations changed: {:#018x}", h.0);
}

#[test]
fn easing_golden_hash() {
    use EasingFn::*;
    let mut h = Hash::new();
    for easing in [
        Linear,
        QuadIn,
        QuadOut,
        QuadInOut,
        CubicIn,
        CubicOut,
        CubicInOut,
        QuartIn,
        QuartOut,
        QuartInOut,
        QuintIn,
        QuintOut,
        QuintInOut,
        SineIn,
        SineOut,
        SineInOut,
        ExpoIn,
        ExpoOut,
        ExpoInOut,
        CircIn,
        CircOut,
        CircInOut,
        ElasticIn,
        ElasticOut,
        ElasticInOut,
        BackIn,
        BackOut,
        BackInOut,
        BounceIn,
        BounceOut,
        BounceInOut,
    ] {
        for bits in (0..=65536).step_by(512) {
            h.fix(easing.apply_fix(fix(bits)));
        }
    }
    assert_eq!(h.0, EASING, "easing changed: {:#018x}", h.0);
}

const TRIG: u64 = 0x9ff8_15dc_2a8e_fc84;
const FORMATIONS: u64 = 0xb8f7_30c6_a982_a81e;
const EASING: u64 = 0xfab6_9897_d126_5980;
