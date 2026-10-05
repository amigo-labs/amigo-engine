use crate::math::{Fix, SimVec2};
use fixed::traits::ToFixed;
use serde::{Deserialize, Serialize};

/// Axis-aligned rectangle for collision detection and UI layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn from_center(cx: f32, cy: f32, w: f32, h: f32) -> Self {
        Self {
            x: cx - w * 0.5,
            y: cy - h * 0.5,
            w,
            h,
        }
    }

    pub fn left(&self) -> f32 {
        self.x
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn top(&self) -> f32 {
        self.y
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center_x(&self) -> f32 {
        self.x + self.w * 0.5
    }
    pub fn center_y(&self) -> f32 {
        self.y + self.h * 0.5
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = (self.x + self.w).min(other.x + other.w);
        let b = (self.y + self.h).min(other.y + other.h);
        if r > x && b > y {
            Some(Rect::new(x, y, r - x, b - y))
        } else {
            None
        }
    }
}

/// Axis-aligned rectangle in simulation space ([`Fix`]), for collision,
/// physics and anything else that has to come out the same on every machine
/// (ADR-0001). [`Rect`] is its `f32` counterpart for rendering and UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SimRect {
    pub x: Fix,
    pub y: Fix,
    pub w: Fix,
    pub h: Fix,
}

impl SimRect {
    pub fn new(x: Fix, y: Fix, w: Fix, h: Fix) -> Self {
        Self { x, y, w, h }
    }

    /// From any numbers `Fix::from_num` accepts, e.g. `SimRect::from_num(0, 0, 16, 16)`.
    /// Converting a constant is deterministic; only `f32` arithmetic is not.
    pub fn from_num(x: impl ToFixed, y: impl ToFixed, w: impl ToFixed, h: impl ToFixed) -> Self {
        Self::new(
            Fix::from_num(x),
            Fix::from_num(y),
            Fix::from_num(w),
            Fix::from_num(h),
        )
    }

    pub fn from_center(center: SimVec2, w: Fix, h: Fix) -> Self {
        Self::new(center.x - w / 2, center.y - h / 2, w, h)
    }

    pub fn left(&self) -> Fix {
        self.x
    }
    pub fn right(&self) -> Fix {
        self.x + self.w
    }
    pub fn top(&self) -> Fix {
        self.y
    }
    pub fn bottom(&self) -> Fix {
        self.y + self.h
    }
    pub fn center(&self) -> SimVec2 {
        SimVec2::new(self.x + self.w / 2, self.y + self.h / 2)
    }

    /// Moved by `offset`.
    pub fn translated(&self, offset: SimVec2) -> Self {
        Self::new(self.x + offset.x, self.y + offset.y, self.w, self.h)
    }

    pub fn contains(&self, p: SimVec2) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    pub fn overlaps(&self, other: &SimRect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    pub fn intersection(&self, other: &SimRect) -> Option<SimRect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        (r > x && b > y).then(|| SimRect::new(x, y, r - x, b - y))
    }

    /// The `f32` rectangle for drawing.
    pub fn to_render(&self) -> Rect {
        Rect::new(
            self.x.to_num(),
            self.y.to_num(),
            self.w.to_num(),
            self.h.to_num(),
        )
    }
}

#[cfg(test)]
mod sim_rect_tests {
    use super::*;

    #[test]
    fn sim_rect_geometry() {
        let a = SimRect::from_num(0, 0, 10, 10);
        let b = SimRect::from_num(5, 5, 10, 10);
        assert!(a.overlaps(&b));
        assert_eq!(a.intersection(&b), Some(SimRect::from_num(5, 5, 5, 5)));
        assert!(
            !a.overlaps(&SimRect::from_num(10, 0, 5, 5)),
            "edges touch only"
        );
        assert_eq!(a.center(), SimVec2::from_num(5, 5));
        assert!(a.contains(SimVec2::from_num(0, 9.5)));
        assert!(!a.contains(SimVec2::from_num(10, 0)));
        assert_eq!(
            a.translated(SimVec2::from_num(1, 2)).bottom(),
            Fix::from_num(12)
        );
        assert_eq!(a.to_render(), Rect::new(0.0, 0.0, 10.0, 10.0));
    }
}
