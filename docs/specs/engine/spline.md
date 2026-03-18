# Spline Paths

> **Status:** draft
> **Crate:** `amigo_core` (tbd)
> **Priorität:** unbedingt

## Überblick

Splines ermöglichen glatte, kurvenförmige Bewegungspfade für Gegner und
Projektile — im Gegensatz zu harten Waypoint-Knicken. Basis für smooth Enemy
Paths im Tower-Defense und Projektil-Bögen (Parabel, Bezier-Kurven).

## Scope (tbd)

- [ ] **Catmull-Rom Spline**: automatische Tangenten durch Kontrollpunkte
- [ ] **Cubic Bezier**: explizite Kontrollpunkte für Designer-Kontrolle
- [ ] Gleichmäßige Parameterisierung (Arc-Length Reparameterization) für konstante Geschwindigkeit
- [ ] `sample(t: Fixed) -> SimVec2` API für Positions-Abfrage bei beliebigem t ∈ [0,1]
- [ ] `tangent(t: Fixed) -> SimVec2` für Richtungs-Orientierung
- [ ] Editor-Integration: Visuelles Spline-Editing mit Kontrollpunkt-Drag
- [ ] Integration mit [steering](steering.md) Path Following
- [ ] Fixed-Point-Arithmetik für Determinismus
- [ ] Offene Fragen: Wie viele Kontrollpunkte sind typisch? Closed-Loop-Support?

## Referenzen

- [engine/animation](animation.md) → Easing-Kurven als verwandtes Konzept
- [engine/pathfinding](pathfinding.md) → Waypoints als einfachere Alternative
- Godot SplinePath / Unity AnimationCurve als Referenz-APIs
