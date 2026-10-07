//! CPU tessellation of filled shapes into [`QuadGeometry`] for the sprite
//! batcher: convex polygons, circles, rounded rectangles, lines and outlines.
//!
//! A quad is two triangles `(0, 1, 2)` and `(0, 2, 3)`, so a triangle fan from
//! the first point packs two fan triangles into each quad. Under raster art a
//! shape also gets a feather: a strip one scene pixel wide around its outer
//! edges whose outer corners have alpha 0, which anti-aliases without MSAA.

use crate::sprite_batcher::QuadGeometry;
use amigo_core::{Color, Rect};

/// Segments per full turn for a circle of `radius` virtual pixels drawn at
/// `render_scale` scene pixels per virtual pixel:
/// `clamp(ceil(2π · r · render_scale / 4), 12, 128)`.
pub fn circle_segments(radius: f32, render_scale: f32) -> u32 {
    let scale = if render_scale.is_finite() && render_scale > 0.0 {
        render_scale
    } else {
        1.0
    };
    let n = (std::f32::consts::TAU * radius.max(0.0) * scale / 4.0).ceil();
    if n.is_finite() {
        (n as u32).clamp(12, 128)
    } else {
        128
    }
}

/// A convex polygon with a colour per point and, per edge, whether that edge
/// is an outer edge that gets a feather. Edge `i` runs from point `i` to point
/// `i + 1` (wrapping).
#[derive(Clone, Debug, PartialEq)]
pub struct ConvexShape {
    pub points: Vec<[f32; 2]>,
    pub colors: Vec<Color>,
    pub feather_edges: Vec<bool>,
}

impl ConvexShape {
    /// One colour, every edge an outer edge.
    pub fn filled(points: Vec<[f32; 2]>, color: Color) -> Self {
        let n = points.len();
        Self {
            points,
            colors: vec![color; n],
            feather_edges: vec![true; n],
        }
    }

    /// Whether the shape can be drawn: at least 3 finite points.
    pub fn is_drawable(&self) -> bool {
        self.points.len() >= 3
            && self.colors.len() == self.points.len()
            && self
                .points
                .iter()
                .all(|p| p[0].is_finite() && p[1].is_finite())
    }

    /// Tessellate into quads, with a feather `feather` virtual pixels wide on
    /// the outer edges (none when `feather <= 0`). A non-convex point list
    /// yields a fan from the first point: defined, but not the filled polygon.
    pub fn tessellate(&self, feather: f32, out: &mut Vec<QuadGeometry>) {
        if !self.is_drawable() {
            return;
        }
        let p = &self.points;
        let c = &self.colors;
        let n = p.len();
        // Fan from point 0, two triangles per quad.
        let mut i = 1;
        while i + 1 < n {
            if i + 2 < n {
                out.push(QuadGeometry {
                    corners: [p[0], p[i], p[i + 1], p[i + 2]],
                    colors: [c[0], c[i], c[i + 1], c[i + 2]],
                });
                i += 2;
            } else {
                out.push(QuadGeometry::triangle(
                    [p[0], p[i], p[i + 1]],
                    [c[0], c[i], c[i + 1]],
                ));
                i += 1;
            }
        }

        if !(feather > 0.0 && feather.is_finite()) {
            return;
        }
        // Outward normals depend on the winding.
        let area2: f32 = (0..n)
            .map(|i| {
                let (a, b) = (p[i], p[(i + 1) % n]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum();
        let sign = if area2 >= 0.0 { 1.0 } else { -1.0 };
        let normal = |i: usize| -> Option<[f32; 2]> {
            let (a, b) = (p[i], p[(i + 1) % n]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt();
            // y points down: for positive area (clockwise on screen), the
            // outside is to the left of the direction of travel.
            (len > 1e-6).then(|| [sign * dy / len, -sign * dx / len])
        };
        let clear = |color: Color| Color { a: 0.0, ..color };
        let offset = |q: [f32; 2], m: [f32; 2]| [q[0] + m[0] * feather, q[1] + m[1] * feather];
        for i in 0..n {
            if !self.feather_edges.get(i).copied().unwrap_or(false) {
                continue;
            }
            let Some(m) = normal(i) else { continue };
            let j = (i + 1) % n;
            out.push(QuadGeometry {
                corners: [p[i], p[j], offset(p[j], m), offset(p[i], m)],
                colors: [c[i], c[j], clear(c[j]), clear(c[i])],
            });
            // The wedge at the end of this edge closes the gap to the next
            // feathered edge's strip.
            if self.feather_edges.get(j).copied().unwrap_or(false)
                && let Some(next) = normal(j)
            {
                out.push(QuadGeometry::triangle(
                    [p[j], offset(p[j], m), offset(p[j], next)],
                    [c[j], clear(c[j]), clear(c[j])],
                ));
            }
        }
    }
}

/// Points of a circle, `segments` of them, starting at angle 0.
pub fn circle_points(center: [f32; 2], radius: f32, segments: u32) -> Vec<[f32; 2]> {
    let segments = segments.max(3);
    (0..segments)
        .map(|k| {
            let t = std::f32::consts::TAU * k as f32 / segments as f32;
            [center[0] + radius * t.cos(), center[1] + radius * t.sin()]
        })
        .collect()
}

/// Points of a rounded rectangle, clockwise on screen, `segments_per_turn`
/// spread over the four corners. `radius` must already be clamped to half the
/// shorter side and be positive.
pub fn rounded_rect_points(rect: Rect, radius: f32, segments_per_turn: u32) -> Vec<[f32; 2]> {
    let per_corner = (segments_per_turn / 4).max(3);
    let corners = [
        // centre of the corner arc, start angle
        ([rect.x + rect.w - radius, rect.y + radius], -90.0f32),
        ([rect.x + rect.w - radius, rect.y + rect.h - radius], 0.0),
        ([rect.x + radius, rect.y + rect.h - radius], 90.0),
        ([rect.x + radius, rect.y + radius], 180.0),
    ];
    let mut points = Vec::with_capacity(4 * (per_corner as usize + 1));
    for (center, start) in corners {
        for k in 0..=per_corner {
            let t = (start + 90.0 * k as f32 / per_corner as f32).to_radians();
            points.push([center[0] + radius * t.cos(), center[1] + radius * t.sin()]);
        }
    }
    points
}

/// A usable size: positive and finite.
pub fn usable(v: f32) -> bool {
    v > 0.0 && v.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_area(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
        ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
    }

    fn covered_area(quads: &[QuadGeometry]) -> f32 {
        quads
            .iter()
            .map(|q| {
                let c = q.corners;
                triangle_area(c[0], c[1], c[2]) + triangle_area(c[0], c[2], c[3])
            })
            .sum()
    }

    #[test]
    fn segment_count_follows_the_formula() {
        assert_eq!(circle_segments(1.0, 1.0), 12);
        assert_eq!(circle_segments(50.0, 1.0), 79);
        assert_eq!(circle_segments(50.0, 2.0), 128);
        assert_eq!(circle_segments(20.0, 1.0), 32);
        assert_eq!(circle_segments(f32::NAN, 1.0), 12);
    }

    #[test]
    fn a_circle_covers_its_area() {
        let r = 50.0;
        let mut quads = Vec::new();
        let points = circle_points([0.0, 0.0], r, circle_segments(r, 1.0));
        ConvexShape::filled(points, Color::WHITE).tessellate(0.0, &mut quads);
        let analytic = std::f32::consts::PI * r * r;
        let area = covered_area(&quads);
        assert!(
            (area - analytic).abs() / analytic < 0.01,
            "{area} vs {analytic}"
        );
    }

    #[test]
    fn a_rounded_rect_covers_its_area() {
        let (w, h, r) = (200.0, 120.0, 50.0);
        let points = rounded_rect_points(Rect::new(10.0, 10.0, w, h), r, circle_segments(r, 1.0));
        let mut quads = Vec::new();
        ConvexShape::filled(points, Color::WHITE).tessellate(0.0, &mut quads);
        let analytic = w * h - (4.0 - std::f32::consts::PI) * r * r;
        let area = covered_area(&quads);
        assert!(
            (area - analytic).abs() / analytic < 0.01,
            "{area} vs {analytic}"
        );
    }

    #[test]
    fn a_quad_is_one_quad_and_a_triangle_repeats_its_last_corner() {
        let mut quads = Vec::new();
        let square = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        ConvexShape::filled(square, Color::WHITE).tessellate(0.0, &mut quads);
        assert_eq!(quads.len(), 1);
        quads.clear();
        let tri = vec![[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]];
        ConvexShape::filled(tri, Color::WHITE).tessellate(0.0, &mut quads);
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].corners[2], quads[0].corners[3]);
        assert!((covered_area(&quads) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn feathers_fade_to_transparent_outside_the_shape() {
        for points in [
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
            vec![[0.0, 10.0], [10.0, 10.0], [10.0, 0.0], [0.0, 0.0]],
        ] {
            let mut quads = Vec::new();
            ConvexShape::filled(points, Color::RED).tessellate(0.5, &mut quads);
            let feathers = &quads[1..];
            assert_eq!(feathers.len(), 8, "4 strips and 4 wedges");
            for q in feathers {
                for (corner, color) in q.corners.iter().zip(q.colors) {
                    let inside =
                        (0.0..=10.0).contains(&corner[0]) && (0.0..=10.0).contains(&corner[1]);
                    if inside {
                        assert_eq!(color.a, 1.0);
                    } else {
                        assert_eq!(color.a, 0.0, "{corner:?} lies outside, so it is clear");
                    }
                }
            }
        }
    }

    #[test]
    fn no_feather_means_only_the_fill() {
        let mut quads = Vec::new();
        let points = circle_points([0.0, 0.0], 5.0, 12);
        ConvexShape::filled(points, Color::RED).tessellate(0.0, &mut quads);
        assert!(quads.iter().all(|q| q.colors.iter().all(|c| c.a == 1.0)));
    }

    #[test]
    fn degenerate_shapes_draw_nothing() {
        let mut quads = Vec::new();
        ConvexShape::filled(vec![[0.0, 0.0], [1.0, 1.0]], Color::RED).tessellate(1.0, &mut quads);
        ConvexShape::filled(vec![], Color::RED).tessellate(1.0, &mut quads);
        ConvexShape::filled(vec![[0.0, 0.0], [f32::NAN, 1.0], [1.0, 0.0]], Color::RED)
            .tessellate(1.0, &mut quads);
        assert!(quads.is_empty());
    }
}
