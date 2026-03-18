# Spline Paths

---
status: done
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

## Überblick

Splines ermöglichen glatte, kurvenförmige Bewegungspfade für Gegner und
Projektile — im Gegensatz zu harten Waypoint-Knicken. Basis für smooth Enemy
Paths im Tower-Defense und Projektil-Bögen (Parabel, Bezier-Kurven).

## Public API

### CatmullRomSpline

```rust
// crates/amigo_core/src/spline.rs

pub struct CatmullRomSpline {
    pub points: Vec<SimVec2>,
}

impl CatmullRomSpline {
    pub fn new(points: Vec<SimVec2>) -> Self;
    /// Position auf dem Spline, t ∈ [0.0, 1.0] über den gesamten Spline.
    pub fn sample(&self, t: Fix) -> SimVec2;
    /// Tangenten-Richtung an Position t (nicht normalisiert).
    pub fn tangent(&self, t: Fix) -> SimVec2;
    /// Länge in Segments: points.len() - 1 (mind. 2 Punkte nötig)
    pub fn segment_count(&self) -> usize;
}
```

### CubicBezier

```rust
pub struct CubicBezier {
    pub p0: SimVec2,  // Start
    pub p1: SimVec2,  // Kontrollpunkt 1
    pub p2: SimVec2,  // Kontrollpunkt 2
    pub p3: SimVec2,  // Ende
}

impl CubicBezier {
    pub fn new(p0: SimVec2, p1: SimVec2, p2: SimVec2, p3: SimVec2) -> Self;
    /// Position, t ∈ [0.0, 1.0]
    pub fn sample(&self, t: Fix) -> SimVec2;
    /// Tangente (Ableitung), nicht normalisiert
    pub fn tangent(&self, t: Fix) -> SimVec2;
}
```

## Mathematik

**Catmull-Rom** (Ghost-Punkte für Endpoints):
```
p[-1] = 2*p[0] - p[1]
p[n]  = 2*p[n-1] - p[n-2]
q(t) = 0.5 * ((2*P1) + (-P0+P2)*t + (2*P0-5*P1+4*P2-P3)*t² + (-P0+3*P1-3*P2+P3)*t³)
```

**Cubic Bezier:**
```
B(t)  = (1-t)³*P0 + 3*(1-t)²*t*P1 + 3*(1-t)*t²*P2 + t³*P3
B'(t) = 3*(1-t)²*(P1-P0) + 6*(1-t)*t*(P2-P1) + 3*t²*(P3-P2)
```

## Betroffene Dateien

| Datei | Änderung |
| ----- | -------- |
| `crates/amigo_core/src/spline.rs` | Neues Modul |
| `crates/amigo_core/src/lib.rs` | `pub mod spline;` + Re-Exports |

## Referenzen

- [engine/animation](animation.md) → Easing-Kurven als verwandtes Konzept
- [engine/pathfinding](pathfinding.md) → Waypoints als einfachere Alternative
- Godot SplinePath / Unity AnimationCurve als Referenz-APIs
