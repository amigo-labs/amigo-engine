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

/// Fixed-point square root via Newton-Raphson.
/// Initial estimate from f32 (IEEE 754 sqrt is correctly rounded — deterministic).
/// Subsequent iterations are pure Fix arithmetic.
pub fn sqrt_fix(x: Fix) -> Fix {
    if x <= Fix::ZERO {
        return Fix::ZERO;
    }
    let approx: f32 = x.to_num::<f32>().sqrt();
    let mut g = Fix::from_num(approx.max(f32::MIN_POSITIVE));
    for _ in 0..8 {
        let g_next = (g + x / g) / Fix::from_num(2_i32);
        if g_next == g {
            break;
        }
        g = g_next;
    }
    g
}

/// Rendering vector using f32. Used only for rendering: screen positions,
/// particles, camera, screen shake, UI.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
