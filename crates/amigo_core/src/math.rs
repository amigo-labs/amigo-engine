use fixed::types::I16F16;
use serde::{Deserialize, Serialize};

/// Fixed-point type for deterministic simulation (Q16.16).
pub type Fix = I16F16;

/// Simulation vector using fixed-point arithmetic.
/// Used for all game logic: positions, velocities, health, damage, etc.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SimVec2 {
    pub x: Fix,
    pub y: Fix,
}

impl SimVec2 {
    pub const ZERO: Self = Self {
        x: Fix::ZERO,
        y: Fix::ZERO,
    };

    pub fn new(x: Fix, y: Fix) -> Self {
        Self { x, y }
    }

    pub fn from_f32(x: f32, y: f32) -> Self {
        Self {
            x: Fix::from_num(x),
            y: Fix::from_num(y),
        }
    }

    pub fn distance_squared(self, other: Self) -> Fix {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    pub fn to_render(self) -> RenderVec2 {
        RenderVec2 {
            x: self.x.to_num(),
            y: self.y.to_num(),
        }
    }
}

impl std::ops::Add for SimVec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::AddAssign for SimVec2 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl std::ops::Sub for SimVec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Mul<Fix> for SimVec2 {
    type Output = Self;
    fn mul(self, rhs: Fix) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl std::ops::Neg for SimVec2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl std::ops::Div<Fix> for SimVec2 {
    type Output = Self;
    fn div(self, rhs: Fix) -> Self {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl SimVec2 {
    /// Deterministic fixed-point length. Safe for any I16F16 value.
    /// Pre-scales large vectors to avoid x*x overflow (I16F16 max ~32767; sqrt(32767) ≈ 181).
    pub fn length(self) -> Fix {
        let ax = if self.x >= Fix::ZERO { self.x } else { -self.x };
        let ay = if self.y >= Fix::ZERO { self.y } else { -self.y };
        let max_abs = if ax > ay { ax } else { ay };
        if max_abs == Fix::ZERO {
            return Fix::ZERO;
        }
        // Scale down when max component > 100 to keep x*x within I16F16 range.
        let threshold = Fix::from_num(100_i32);
        if max_abs > threshold {
            // length(v) = max_abs * length(v / max_abs)
            // v/max_abs has components in [-1, 1], so (v/max_abs)^2 ≤ 2
            let sx = self.x / max_abs;
            let sy = self.y / max_abs;
            max_abs * sqrt_fix(sx * sx + sy * sy)
        } else {
            sqrt_fix(self.x * self.x + self.y * self.y)
        }
    }

    /// Unit vector. Returns ZERO if length is zero (no panic).
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len <= Fix::ZERO {
            return SimVec2::ZERO;
        }
        Self {
            x: self.x / len,
            y: self.y / len,
        }
    }
}

/// Pure-integer fixed-point square root (bit-by-bit algorithm on Q16.16).
///
/// Operates entirely on integer arithmetic — no floating-point at any stage.
/// This guarantees cross-platform determinism (x86, ARM, WASM produce identical
/// results). The input Q16.16 raw bits are shifted left by 16 into a u64, then
/// the standard non-restoring integer square root is computed. The result maps
/// back to Q16.16.
#[inline]
pub fn sqrt_fix(x: Fix) -> Fix {
    let bits = x.to_bits();
    if bits <= 0 {
        return Fix::ZERO;
    }
    // Shift into u64 to get Q32.32-equivalent so the integer sqrt yields Q16.16.
    let mut n = (bits as u64) << 16;
    let mut result: u64 = 0;
    // Start with the highest even bit that doesn't exceed n.
    let mut bit: u64 = 1u64 << 46;
    while bit > n {
        bit >>= 2;
    }
    while bit != 0 {
        let rb = result + bit;
        if n >= rb {
            n -= rb;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    Fix::from_bits(result as i32)
}

/// Rendering vector using f32. Used only for rendering: screen positions,
/// particles, camera, screen shake, UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RenderVec2 {
    pub x: f32,
    pub y: f32,
}

impl RenderVec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance_squared(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

impl std::ops::Add for RenderVec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub for RenderVec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Mul<f32> for RenderVec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

/// Helper to create a RenderVec2.
pub fn vec2(x: f32, y: f32) -> RenderVec2 {
    RenderVec2::new(x, y)
}

/// Integer vector for tile coordinates.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct IVec2 {
    pub x: i32,
    pub y: i32,
}

impl IVec2 {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_fix_perfect_squares() {
        for n in [0, 1, 4, 9, 16, 25, 100, 400, 10000] {
            let input = Fix::from_num(n);
            let result = sqrt_fix(input);
            let expected = Fix::from_num((n as f64).sqrt() as i32);
            assert_eq!(result, expected, "sqrt({n}) failed");
        }
    }

    #[test]
    fn sqrt_fix_fractional() {
        let input = Fix::from_num(0.25f32);
        let result = sqrt_fix(input);
        let diff = (result - Fix::from_num(0.5f32)).abs();
        assert!(diff <= Fix::from_bits(1), "sqrt(0.25) = {result}, expected ~0.5");
    }

    #[test]
    fn sqrt_fix_accuracy_sweep() {
        // Verify sqrt(x)^2 ≈ x within 2 ULP for many values.
        for i in 1..5000 {
            let x = Fix::from_bits(i * 13); // scattered values
            let s = sqrt_fix(x);
            let s_sq = s * s;
            let err = (s_sq - x).abs().to_bits();
            assert!(
                err <= 2,
                "sqrt({x})^2 = {s_sq}, expected {x}, error = {err} ULP"
            );
        }
    }

    #[test]
    fn sqrt_fix_zero_and_negative() {
        assert_eq!(sqrt_fix(Fix::ZERO), Fix::ZERO);
        assert_eq!(sqrt_fix(Fix::from_num(-1)), Fix::ZERO);
        assert_eq!(sqrt_fix(Fix::from_num(-100)), Fix::ZERO);
    }

    #[test]
    fn sqrt_fix_small_values() {
        // Smallest positive Q16.16: 1/65536 ≈ 0.0000153
        let tiny = Fix::from_bits(1);
        let result = sqrt_fix(tiny);
        assert!(result > Fix::ZERO, "sqrt of smallest positive should be positive");
        // sqrt(1/65536) = 1/256 = Fix::from_bits(256)
        assert_eq!(result, Fix::from_bits(256));
    }

    #[test]
    fn sqrt_fix_large_values() {
        // Max safe value: 32767
        let large = Fix::from_num(32000);
        let s = sqrt_fix(large);
        let expected = Fix::from_num(178); // floor(sqrt(32000)) ≈ 178.88
        let diff = (s - expected).abs();
        assert!(diff < Fix::from_num(1), "sqrt(32000) ≈ 178.88, got {s}");
    }

    #[test]
    fn sqrt_fix_deterministic_golden_values() {
        // Hardcoded .to_bits() results — must be identical on all platforms.
        let cases: &[(i32, i32)] = &[
            (Fix::from_num(1).to_bits(), Fix::from_num(1).to_bits()),
            (Fix::from_num(2).to_bits(), 92681),  // sqrt(2) ≈ 1.41421 (floor)
            (Fix::from_num(4).to_bits(), Fix::from_num(2).to_bits()),
        ];
        for &(input_bits, expected_bits) in cases {
            let result = sqrt_fix(Fix::from_bits(input_bits));
            assert_eq!(
                result.to_bits(),
                expected_bits,
                "golden value mismatch for input bits {input_bits}"
            );
        }
    }

    #[test]
    fn simvec2_length_normalize_deterministic() {
        let v = SimVec2::from_f32(3.0, 4.0);
        let len = v.length();
        let diff = (len - Fix::from_num(5)).abs();
        assert!(diff <= Fix::from_bits(1), "length(3,4) should be 5, got {len}");

        let n = v.normalize();
        let n_len = n.length();
        let err = (n_len - Fix::from_num(1)).abs();
        assert!(err <= Fix::from_bits(2), "normalized length should be ~1, got {n_len}");
    }

    #[test]
    fn simvec2_length_large() {
        // Test the pre-scaling path (components > 100).
        let v = SimVec2::from_f32(200.0, 150.0);
        let len = v.length();
        let expected = Fix::from_num(250); // 200^2+150^2=62500, sqrt=250
        let diff = (len - expected).abs();
        assert!(diff <= Fix::from_bits(4), "length(200,150) should be 250, got {len}");
    }
}
