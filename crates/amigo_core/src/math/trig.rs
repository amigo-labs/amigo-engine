//! Deterministic trigonometry on [`Fix`].
//!
//! `f32::sin` and friends call the platform's libm, whose last bits differ
//! between targets (glibc, musl, the MSVC CRT, Apple's libm, WASM), so a
//! simulation that steers by them drifts apart between two machines, which
//! breaks lockstep netcode and replays (ADR-0001). Everything here is integer
//! arithmetic: CORDIC on Q2.30 values in `i64`, with the arctangent table and
//! the gain written out as integer constants. The same input gives the same
//! bits on every platform.
//!
//! Angles are radians as [`Fix`]. Results are accurate to a few units in the
//! last Q16.16 place (about 1e-4); the tests pin exact bit patterns so any
//! change to the algorithm shows up as a test failure, not as a desync.

use super::{Fix, SimVec2};

/// Fractional bits of the internal representation.
const FRAC: u32 = 30;

/// `atan(2^-i)` in Q2.30, for `i` in `0..ITERATIONS`.
const ATAN_TABLE: [i64; ITERATIONS] = [
    843_314_857,
    497_837_829,
    263_043_837,
    133_525_159,
    67_021_687,
    33_543_516,
    16_775_851,
    8_388_437,
    4_194_283,
    2_097_149,
    1_048_576,
    524_288,
    262_144,
    131_072,
    65_536,
    32_768,
    16_384,
    8_192,
    4_096,
    2_048,
    1_024,
    512,
    256,
    128,
    64,
    32,
    16,
    8,
    4,
    2,
];

const ITERATIONS: usize = 30;

/// The CORDIC gain correction `∏ 1/√(1 + 2^-2i)` in Q2.30.
const GAIN: i64 = 652_032_874;

/// π in Q2.30.
const PI_30: i64 = 3_373_259_426;
/// π/2 in Q2.30.
const FRAC_PI_2_30: i64 = 1_686_629_713;

/// π as [`Fix`] (`205887 / 65536`).
pub const PI: Fix = Fix::from_bits(205_887);
/// 2π as [`Fix`].
pub const TAU: Fix = Fix::from_bits(411_775);
/// π/2 as [`Fix`].
pub const FRAC_PI_2: Fix = Fix::from_bits(102_944);

/// Q2.30 back to Q16.16, rounding to nearest.
fn to_fix(v: i64) -> Fix {
    let shift = FRAC - 16;
    let rounded = (v + (1 << (shift - 1))) >> shift;
    Fix::from_bits(rounded.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
}

/// Sine and cosine of `angle` (radians), computed together.
///
/// ```
/// use amigo_core::math::{Fix, trig};
/// let (s, c) = trig::sin_cos_fix(trig::FRAC_PI_2);
/// assert_eq!(s, Fix::ONE);
/// assert!(c.abs() < Fix::from_num(0.0002));
/// ```
pub fn sin_cos_fix(angle: Fix) -> (Fix, Fix) {
    // Reduce to [-π, π) in Q2.30. The reduction happens after widening, so
    // it loses nothing for any Q16.16 input.
    let two_pi = 2 * PI_30;
    let mut z = (i64::from(angle.to_bits()) << (FRAC - 16)).rem_euclid(two_pi);
    if z >= PI_30 {
        z -= two_pi;
    }
    // Fold into [-π/2, π/2], where CORDIC converges; the cosine changes sign.
    let mut cos_sign = 1;
    if z > FRAC_PI_2_30 {
        z = PI_30 - z;
        cos_sign = -1;
    } else if z < -FRAC_PI_2_30 {
        z = -PI_30 - z;
        cos_sign = -1;
    }

    let (mut x, mut y) = (GAIN, 0i64);
    for (i, &atan) in ATAN_TABLE.iter().enumerate() {
        let (dx, dy) = (y >> i, x >> i);
        if z >= 0 {
            x -= dx;
            y += dy;
            z -= atan;
        } else {
            x += dx;
            y -= dy;
            z += atan;
        }
    }
    (to_fix(y), to_fix(x * cos_sign))
}

/// Sine of `angle` (radians).
pub fn sin_fix(angle: Fix) -> Fix {
    sin_cos_fix(angle).0
}

/// Cosine of `angle` (radians).
pub fn cos_fix(angle: Fix) -> Fix {
    sin_cos_fix(angle).1
}

/// The angle of the vector `(x, y)` in `(-π, π]`, like `f32::atan2(y, x)`.
/// `atan2(0, 0)` is 0.
pub fn atan2_fix(y: Fix, x: Fix) -> Fix {
    let (mut x, mut y) = (i64::from(x.to_bits()), i64::from(y.to_bits()));
    if x == 0 && y == 0 {
        return Fix::ZERO;
    }
    // Start in the right half-plane; the half turn is added back at the end.
    let mut z = 0i64;
    if x < 0 {
        z = if y >= 0 { PI_30 } else { -PI_30 };
        x = -x;
        y = -y;
    }
    // Scale up so the iterations keep their precision: only the direction
    // matters. Inputs are at most 2^31, so this never overflows i64.
    let magnitude = x.max(y.abs());
    let shift = (FRAC - 1).saturating_sub(63 - magnitude.leading_zeros());
    x <<= shift;
    y <<= shift;

    for (i, &atan) in ATAN_TABLE.iter().enumerate() {
        let (dx, dy) = (y >> i, x >> i);
        if y > 0 {
            x += dx;
            y -= dy;
            z += atan;
        } else {
            x -= dx;
            y += dy;
            z -= atan;
        }
    }
    // `(-x, -0)` is π, not -π, like `f32::atan2`.
    if z <= -PI_30 {
        z += 2 * PI_30;
    }
    to_fix(z)
}

/// Arc cosine of `x`, clamped to `[-1, 1]`, in `[0, π]`.
pub fn acos_fix(x: Fix) -> Fix {
    let x = x.clamp(-Fix::ONE, Fix::ONE);
    atan2_fix(super::sqrt_fix(Fix::ONE - x * x), x)
}

/// Arc sine of `x`, clamped to `[-1, 1]`, in `[-π/2, π/2]`.
pub fn asin_fix(x: Fix) -> Fix {
    let x = x.clamp(-Fix::ONE, Fix::ONE);
    atan2_fix(x, super::sqrt_fix(Fix::ONE - x * x))
}

/// `2^x`, saturating at the Q16.16 range. `x` below -16 rounds to 0.
///
/// The integer part is a shift; the fractional part is the series of
/// `e^(f·ln 2)` to eight terms on Q2.30 integers (error below 2e-6).
pub fn exp2_fix(x: Fix) -> Fix {
    let bits = i64::from(x.to_bits());
    let int = bits >> 16; // floor
    let frac = bits - (int << 16); // in [0, 1) as Q16.16
    if int >= 15 {
        return Fix::MAX;
    }
    if int < -17 {
        return Fix::ZERO;
    }
    // ln 2 in Q2.30.
    const LN2: i64 = 744_261_118;
    let y = ((frac << (FRAC - 16)) * LN2) >> FRAC; // f·ln2, Q2.30
    let one = 1i64 << FRAC;
    let (mut term, mut sum) = (one, one);
    for k in 1..=8 {
        term = term * y / (k << FRAC);
        sum += term;
    }
    // sum is 2^frac in Q2.30; apply 2^int and convert to Q16.16.
    let shift = i64::from(FRAC - 16) - int;
    let value = if shift > 0 {
        (sum + (1 << (shift - 1))) >> shift // round to nearest
    } else {
        sum << -shift
    };
    Fix::from_bits(value.min(i64::from(i32::MAX)) as i32)
}

impl SimVec2 {
    /// The unit vector at `angle` radians from the +x axis.
    pub fn from_angle(angle: Fix) -> Self {
        let (sin, cos) = sin_cos_fix(angle);
        Self::new(cos, sin)
    }

    /// The angle of this vector from the +x axis, in `(-π, π]`.
    pub fn angle(self) -> Fix {
        atan2_fix(self.y, self.x)
    }

    /// This vector rotated by `angle` radians (counter-clockwise in a
    /// y-up frame, clockwise on screen where y points down).
    pub fn rotate(self, angle: Fix) -> Self {
        let (sin, cos) = sin_cos_fix(angle);
        Self::new(self.x * cos - self.y * sin, self.x * sin + self.y * cos)
    }

    /// Dot product, computed on widened integers and saturating at the
    /// Q16.16 range, like [`SimVec2::distance_squared`].
    pub fn dot(self, other: Self) -> Fix {
        let wide = |a: Fix, b: Fix| i128::from(a.to_bits()) * i128::from(b.to_bits());
        let sum = (wide(self.x, other.x) + wide(self.y, other.y)) >> 16;
        Fix::from_bits(sum.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: f64) -> Fix {
        Fix::from_num(v)
    }

    /// Within `tol` of the f64 reference, which these tests use only as a
    /// yardstick; the results themselves never touch floating point.
    fn close(actual: Fix, expected: f64, tol: f64) -> bool {
        (actual.to_num::<f64>() - expected).abs() <= tol
    }

    const TOL: f64 = 2e-4;

    #[test]
    fn constants_match_their_values() {
        assert!(close(PI, std::f64::consts::PI, 1e-5));
        assert!(close(TAU, std::f64::consts::TAU, 1e-5));
        assert!(close(FRAC_PI_2, std::f64::consts::FRAC_PI_2, 1e-5));
    }

    #[test]
    fn sin_and_cos_track_f64_over_the_whole_circle_and_beyond() {
        for step in -2000..=2000 {
            let a = step as f64 * 0.01; // -20 rad ..= 20 rad
            let (s, c) = sin_cos_fix(f(a));
            let a_fix = f(a).to_num::<f64>(); // the angle actually passed in
            assert!(close(s, a_fix.sin(), TOL), "sin({a}) = {s}");
            assert!(close(c, a_fix.cos(), TOL), "cos({a}) = {c}");
        }
    }

    #[test]
    fn exact_values_at_the_cardinal_angles() {
        assert_eq!(sin_cos_fix(Fix::ZERO), (Fix::ZERO, Fix::ONE));
        assert_eq!(sin_fix(FRAC_PI_2), Fix::ONE);
        assert_eq!(cos_fix(PI), -Fix::ONE);
        assert_eq!(sin_fix(-FRAC_PI_2), -Fix::ONE);
    }

    #[test]
    fn atan2_tracks_f64_in_every_quadrant() {
        for &(y, x) in &[
            (1.0, 1.0),
            (1.0, -1.0),
            (-1.0, -1.0),
            (-1.0, 1.0),
            (0.0, 1.0),
            (0.0, -1.0),
            (1.0, 0.0),
            (-1.0, 0.0),
            (3.0, 4.0),
            (-0.001, 500.0),
            (20000.0, -0.5),
            (0.0001, 0.0001),
        ] {
            let got = atan2_fix(f(y), f(x));
            let want = f(y).to_num::<f64>().atan2(f(x).to_num::<f64>());
            assert!(
                close(got, want, TOL),
                "atan2({y}, {x}) = {got}, want {want}"
            );
        }
        assert_eq!(atan2_fix(Fix::ZERO, Fix::ZERO), Fix::ZERO);
        assert!(atan2_fix(Fix::ZERO, -Fix::ONE) > Fix::ZERO, "(-1, 0) is +π");
    }

    #[test]
    fn inverse_functions_round_trip() {
        for step in -100..=100 {
            let x = step as f64 / 100.0;
            assert!(close(acos_fix(f(x)), x.acos(), 5e-4), "acos({x})");
            assert!(close(asin_fix(f(x)), x.asin(), 5e-4), "asin({x})");
        }
        assert_eq!(acos_fix(f(2.0)), acos_fix(Fix::ONE), "clamped");
        for step in -31..=31 {
            let a = f(step as f64 / 10.0);
            let back = SimVec2::from_angle(a).angle();
            assert!(
                (back - a).abs() < f(4e-4),
                "angle(from_angle({a})) = {back}"
            );
        }
    }

    #[test]
    fn exp2_tracks_f64() {
        for step in -160..=140 {
            let x = step as f64 / 10.0;
            let want = 2f64.powf(f(x).to_num::<f64>());
            let got = exp2_fix(f(x)).to_num::<f64>();
            let tol = (want * 1e-5).max(2.0 / 65536.0);
            assert!((got - want).abs() <= tol, "2^{x} = {got}, want {want}");
        }
        assert_eq!(exp2_fix(Fix::ZERO), Fix::ONE);
        assert_eq!(exp2_fix(f(3.0)), f(8.0));
        assert_eq!(exp2_fix(f(-1.0)), f(0.5));
        assert_eq!(exp2_fix(f(20.0)), Fix::MAX);
        assert_eq!(exp2_fix(f(-30.0)), Fix::ZERO);
    }

    #[test]
    fn vector_helpers() {
        let v = SimVec2::new(f(3.0), f(4.0));
        assert_eq!(v.dot(SimVec2::new(f(2.0), f(-1.0))), f(2.0));
        let r = SimVec2::new(Fix::ONE, Fix::ZERO).rotate(FRAC_PI_2);
        assert!(
            r.x.abs() < f(2e-4) && (r.y - Fix::ONE).abs() < f(2e-4),
            "{r:?}"
        );
        // Saturates instead of overflowing.
        let big = SimVec2::new(f(30000.0), f(30000.0));
        assert_eq!(big.dot(big), Fix::MAX);
    }

    /// The exact bits, so a change to the algorithm or its constants is a
    /// visible test failure rather than a silent desync between versions.
    #[test]
    fn golden_bits() {
        let samples: Vec<(i32, i32, i32)> = [0.5, 1.0, 2.5, -3.0, 100.0]
            .iter()
            .map(|&a| {
                let (s, c) = sin_cos_fix(f(a));
                (s.to_bits(), c.to_bits(), atan2_fix(s, c).to_bits())
            })
            .collect();
        assert_eq!(samples, GOLDEN);
    }

    const GOLDEN: [(i32, i32, i32); 5] = [
        (31420, 57513, 32768),
        (55147, 35409, 65536),
        (39221, -52504, 163841),
        (-9248, -64880, -196608),
        (-33185, 56513, -34797),
    ];
}
