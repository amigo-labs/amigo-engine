---
status: done
crate: amigo_steering
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Steering Behaviors

## Purpose

Lokales Bewegungsverhalten für autonome Entities (Gegner, NPCs) via gewichteter
Kraft-Vektoren. Ergänzt die Makro-Navigation (`FlowField`, `NavAgent`) um
organische, kollisionsfreie Gruppen-Dynamik.

**Problem ohne Steering**: Eine Horde von 200 Gegnern stapelt sich auf jedem
Wegpunkt zu einem einzigen Pixel-Klumpen — visuell und spielerisch kaputt.

**Architektur**: Steering ist eine *Zusatz-Schicht über NavAgent*, kein Ersatz:

```
FlowField / A* (Makro: Wo soll ich hin?)
    ↓ desired_direction: SimVec2
SteeringSystem (Mikro: Wie bewege ich mich?)
    ↓ steering_force: SimVec2
NavAgent.velocity += steering_force
    ↓
Position += velocity * dt
```

## Public API

### SteeringAgent

```rust
// crates/amigo_steering/src/behaviors.rs

pub struct SteeringAgent {
    pub max_speed: Fix,
    pub max_force: Fix,
    /// List of (behavior, weight) pairs evaluated every tick.
    pub behaviors: Vec<(SteeringBehavior, Fix)>,
}

pub enum SteeringBehavior {
    Seek { target: SimVec2 },
    Arrive { target: SimVec2, decel_radius: Fix },
    Flee { target: SimVec2 },
    Separation { radius: Fix },      // Anti-Stacking
    Cohesion,                         // Gruppe zusammenhalten
    Alignment,                        // Geschwindigkeit angleichen
    PathFollow { waypoints: Vec<SimVec2>, look_ahead: Fix },
}

/// Compute combined steering force this tick.
/// Priority rule: if Separation force > threshold, it overrides all other behaviors.
pub fn compute_steering(
    agent: &SteeringAgent,
    self_pos: SimVec2,
    self_vel: SimVec2,
    neighbors: &[(SimVec2, SimVec2)],  // (pos, vel) der Nachbarn
) -> SimVec2;
```

### SteeringConfig (RON-Konfiguration)

```rust
// crates/amigo_steering/src/config.rs

#[derive(Serialize, Deserialize)]
pub struct SteeringConfig {
    pub max_speed: f32,
    pub max_force: f32,
    pub separation_radius: f32,
    pub separation_weight: f32,
    pub seek_weight: f32,
    pub decel_radius: f32,
}

impl SteeringConfig {
    pub fn to_agent(&self, target: SimVec2) -> SteeringAgent;
    pub fn from_ron(src: &str) -> Result<Self, ron::error::SpannedError>;
    pub fn to_ron(&self) -> String;
}
```

### SimVec2-Erweiterungen (amigo_core)

```rust
// crates/amigo_core/src/math.rs

impl SimVec2 {
    /// Overflow-sicheres Fixed-Point length via Pre-Scaling + Newton-Raphson sqrt.
    pub fn length(self) -> Fix;

    /// Unit vector. Gibt ZERO zurück wenn length == 0 (kein Panic).
    pub fn normalize(self) -> Self;
}

impl std::ops::Neg for SimVec2 { ... }
impl std::ops::Div<Fix> for SimVec2 { ... }
```

## Behavior

### Seek / Arrive / Flee

- **Seek**: `desired = (target - pos).normalize() * max_speed; force = desired - vel`
- **Arrive**: Wie Seek, aber `speed = max_speed * (dist / decel_radius)` wenn nah am Ziel — kein Overshoot
- **Flee**: Inverse Seek — weg vom Ziel

### Separation (Anti-Stacking)

Für jeden Nachbarn innerhalb von `radius`:
```
strength = max_speed * (radius - dist) / radius
force += (pos - neighbor_pos).normalize() * strength
```
Stärkere Kraft je näher der Nachbar. Summiert für alle Nachbarn.

### Cohesion + Alignment (Flocking)

- **Cohesion**: Seek auf das Gruppen-Zentrum (`avg(neighbor_positions)`)
- **Alignment**: Angleich an `avg(neighbor_velocities)` via Steering-Force

### PathFollow

Findet den nächsten Waypoint, seekt einen Waypoint weiter (Look-Ahead von 1).

### Priority Steering

Separation hat höhere Priorität als alle anderen Behaviors:
```
if separation_force.length() > 0.1:
    return separation_force  // Override: nur Separation
else:
    return weighted_sum(all_behaviors)
```

## Internal Design

### Overflow-sichere Fixed-Point Mathematik

I16F16 hat max-Wert ~32767. `x * x` overflowt ab `|x| > 181`. `length()` löst das mit Pre-Scaling:

```rust
if max(|x|, |y|) > 100 {
    let s = v / max_abs;  // s hat Komponenten in [-1, 1]
    length = max_abs * sqrt(s.x*s.x + s.y*s.y)  // s*s ≤ 2, kein Overflow
} else {
    length = sqrt(x*x + y*y)
}
```

`sqrt_fix()` verwendet Newton-Raphson: f32-Initialschätzung (IEEE 754 deterministische sqrt) + 8 Iterationen pure Fix-Arithmetik.

### Nachbarn-Lookup (Game-Layer)

Die Steering-API nimmt `neighbors: &[(SimVec2, SimVec2)]` — vorgefiltert durch den Aufrufer. Für typische TD-Enemy-Gruppen nutzt der Game-Layer den `SpatialHash` aus `amigo_core::collision` für O(1)-Nachbar-Queries per Radius.

### Fix der NavAgent-Regression

`NavAgent.update()` hat früher `f32.sqrt()` für Distanz-Berechnung genutzt
(`dist_sq.to_num::<f32>().sqrt()`). Dieser latente Determinismus-Bug wurde
im gleichen Commit durch `SimVec2::length()` behoben.

## Non-Goals

- **Obstacle Avoidance** auf Tilemap-Ebene (→ NavAgent / FlowField)
- **Wander Behavior** (nicht-deterministisch ohne Seed)
- **Formations** (→ [gametypes/rts](../gametypes/rts.md))
- **Flocking > 500 Entities** (Performance-Budget Spielcode-Aufgabe)
- **Steering für Projektile**

## Betroffene Dateien

| Datei | Änderung |
| ----- | -------- |
| `crates/amigo_steering/` | Neues Crate (vollständig neu) |
| `crates/amigo_core/src/math.rs` | `SimVec2::length()`, `normalize()`, `Neg`, `Div<Fix>` |
| `crates/amigo_core/src/navigation.rs` | f32-sqrt-Bug behoben |
| `Cargo.toml` (Workspace) | `amigo_steering` als Member + workspace dep |
