# Spline Paths

> Erstellt via `/thanos:spec` — 2026-03-18

## Kontext

Gegner und Projektile sollen sich entlang glatter, kurvenförmiger Pfade bewegen können —
nicht nur hart zwischen Wegpunkten knicken. Catmull-Rom-Splines bieten automatische
Tangenten für Designer-freundliche Pfaddefinition (nur Kontrollpunkte, keine
Handles nötig). Kubische Bezier-Kurven ermöglichen präzise, explizit kontrollierte
Bögen für Projektil-Trajectories und animierte Kamera-Dolly-Pfade.

**Problem ohne Splines**: Ein Gegner, der einem Waypoint-Pfad folgt, knickt an
jedem Punkt scharf ab — unnatürlich und optisch störend. Splines lösen das durch
C1-stetige Interpolation.

## Scope

### In Scope

- `CatmullRomSpline`: automatische Tangenten durch Kontrollpunkte, C1-stetig
- `CubicBezier`: explizite Kontrollpunkte (p0..p3) für Designer-Kontrolle
- `sample(t: Fix) -> SimVec2` — Position bei t ∈ [0, 1]
- `tangent(t: Fix) -> SimVec2` — Ableitungsvektor (nicht normalisiert) bei t
- Ghost-Punkt-Extrapolation für Catmull-Rom-Endpoints (kein Clamp-Artefakt)
- Vollständige Fixed-Point-Arithmetik (kein f32 in Simulation/Logik)
- Integration in `amigo_core` — kein separates Crate nötig

### Out of Scope

- Arc-Length-Reparameterisierung für konstante Geschwindigkeit (→ späteres Feature)
- Closed-Loop-Splines (geschlossene Kurven)
- B-Splines oder NURBS
- Editor-Integration (visuelles Spline-Editing mit Kontrollpunkt-Drag)
- Integration mit dem Steering-System (PathFollow mit SplinePath)
- Mehr als kubische Polynome (Quintic etc.)

## Akzeptanzkriterien

- [ ] AC1: `CatmullRomSpline::sample(Fix::ZERO)` gibt den ersten Kontrollpunkt zurück
- [ ] AC2: `CatmullRomSpline::sample(Fix::ONE)` gibt den letzten Kontrollpunkt zurück
- [ ] AC3: Catmull-Rom mit 2 Punkten, t=0.5 → arithmetischer Mittelpunkt
- [ ] AC4: `CubicBezier::sample(Fix::ZERO)` gibt p0 zurück
- [ ] AC5: `CubicBezier::sample(Fix::ONE)` gibt p3 zurück
- [ ] AC6: Bezier auf einer Geraden (p1=(33,0), p2=(66,0)) → sample(0.5) ≈ (50,0) mit ±2px Toleranz
- [ ] AC7: `CatmullRomSpline::tangent(0.5)` für mehrteiligen Pfad ist nicht ZERO
- [ ] AC8: `CubicBezier::tangent(Fix::ZERO)` zeigt in Richtung p1−p0
- [ ] AC9: Kein f32 in Berechnungslogik, kein unsafe
- [ ] AC10: `cargo test -p amigo_core -- spline` → alle 8 Tests grün

## Technische Notizen

### Catmull-Rom Mathematik

Für Segment i (zwischen `points[i]` und `points[i+1]`):
- Ghost-Punkt am Anfang: `p_minus1 = 2 * points[0] - points[1]`
- Ghost-Punkt am Ende: `p_n = 2 * points[n-1] - points[n-2]`
- Formel: `q(t) = 0.5 * ((2*P1) + (-P0 + P2)*t + (2*P0 - 5*P1 + 4*P2 - P3)*t² + (-P0 + 3*P1 - 3*P2 + P3)*t³)`

### Kubische Bezier Mathematik

- Position: `B(t) = (1-t)³*P0 + 3*(1-t)²*t*P1 + 3*(1-t)*t²*P2 + t³*P3`
- Ableitung: `B'(t) = 3*(1-t)²*(P1-P0) + 6*(1-t)*t*(P2-P1) + 3*t²*(P3-P2)`

### Fixed-Point Overflow-Analyse

- `Fix = I16F16`, Max-Wert ≈ 32767
- t ∈ [0, 1]: t² ≤ 1, t³ ≤ 1 → kein Overflow durch Potenzen von t
- Koordinaten-Werte: Tile-Koordinaten typischerweise < 1000, SimVec2 kann bis ~32767
- Bei Catmull-Rom: Koeffizient `2*P0 - 5*P1 + 4*P2 - P3` — Vorsicht bei großen Werten;
  Tile-Koordinaten liegen typischerweise weit unterhalb des Overflow-Bereichs

### Betroffene Dateien

| Datei | Änderung |
| ----- | -------- |
| `crates/amigo_core/src/spline.rs` | Neues Modul (vollständig neu) |
| `crates/amigo_core/src/lib.rs` | `pub mod spline;` + Re-Exports |
| `docs/specs/engine/spline.md` | Status: draft → done |
| `docs/specs/index.md` | Spline-Zeile: draft → done |
