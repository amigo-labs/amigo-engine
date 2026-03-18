# Physics (Rigid Body)

> **Status:** draft
> **Crate:** `amigo_physics` (tbd)
> **Priorität:** fehlt komplett

## Überblick

Rigid Body Physics via Rapier2D für physikbasierte Spielmechaniken, die über
AABB-Kollision hinausgehen. Kein Pendant in aktuellen Specs — amigo_engine hat
nur tile-basierte AABB-Kollision. Rapier2D ist deterministisch und Fixed-Point-kompatibel.

## Scope (tbd)

- [ ] Rapier2D-Integration als optionales Feature (`physics` feature flag)
- [ ] **Collider-Typen**: Ball, Cuboid, Capsule, Convex Hull, Heightfield
- [ ] **Body-Typen**: Dynamic, Kinematic, Static
- [ ] **Joints**: Fixed, Ball, Revolute, Prismatic
- [ ] ECS-Integration: `RigidBodyComponent`, `ColliderComponent`
- [ ] Synchronisation zwischen Rapier-Welt und ECS-Positionen
- [ ] Determinismus-Garantie: Rapier2D nutzt f32 mit fester Integrations-Reihenfolge
- [ ] Brücke zwischen Fixed-Point-Simulation und f32-Physik (tbd)
- [ ] Kollisions-Events als ECS-Events weiterleiten
- [ ] Offene Fragen: Vollständige Rapier-Integration oder nur Subset? Fixed-Point-Brücke?

## Referenzen

- rapier2d (dimforge) als Physik-Engine
- [engine/memory-performance](memory-performance.md) → Kollisions-Broad-Phase
- [gametypes/platformer](../gametypes/platformer.md) → Hauptabnehmer
