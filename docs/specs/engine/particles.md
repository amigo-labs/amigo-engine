---
status: draft
crate: amigo_render
depends_on: ["engine/core", "engine/rendering"]
last_updated: 2026-03-18
---

# Particle System

## Purpose

Lightweight emitter-based particle system for visual effects across all game genres. Supports continuous emission and one-shot bursts, configurable emitter shapes, color/size interpolation over lifetime, force fields (wind, attractors, vortex, drag, turbulence), and built-in presets for common effects (explosion, smoke, sparkle, trail). Purely visual with no gameplay collision. Uses a fixed-capacity object pool per emitter to avoid runtime allocation.

## Public API

Existing implementation in `crates/amigo_render/src/particles.rs`.

### EmitterShape

```rust
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum EmitterShape {
    Point,
    Circle { radius: f32 },
    Line { length: f32, angle: f32 },
    Rect { width: f32, height: f32 },
}
```

### BlendMode

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Additive,
}
```

### EmitterConfig

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmitterConfig {
    pub max_particles: usize,
    pub emission_rate: f32,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub direction: f32,
    pub spread: f32,
    pub gravity: f32,
    pub color_start: Color,
    pub color_end: Color,
    pub size_start: f32,
    pub size_end: f32,
    pub fade_out: bool,
    pub blend_mode: BlendMode,
    pub burst: bool,
}

impl EmitterConfig {
    pub fn explosion() -> Self;
    pub fn smoke() -> Self;
    pub fn sparkle() -> Self;
    pub fn trail() -> Self;
}
```

### Particle

```rust
#[derive(Clone, Debug)]
pub struct Particle {
    pub position_x: f32,
    pub position_y: f32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub size: f32,
    pub color: Color,
    pub rotation: f32,
    pub angular_velocity: f32,
    alive: bool,
}
```

### ParticleEmitter

```rust
pub struct ParticleEmitter {
    pub config: EmitterConfig,
    pub shape: EmitterShape,
    pub x: f32,
    pub y: f32,
    particles: Vec<Particle>,
    // ...
}

impl ParticleEmitter {
    pub fn new(config: EmitterConfig, shape: EmitterShape, x: f32, y: f32) -> Self;
    pub fn update(&mut self, dt: f32);
    pub fn is_finished(&self) -> bool;
    pub fn particle_count(&self) -> usize;
    pub fn collect_sprites(&self, sprites: &mut Vec<SpriteInstance>, texture_id: TextureId);
}
```

### ForceField

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ForceField {
    Wind { force_x: f32, force_y: f32 },
    Attractor { x: f32, y: f32, strength: f32, radius: f32 },
    Repulsor { x: f32, y: f32, strength: f32, radius: f32 },
    Vortex { x: f32, y: f32, strength: f32, radius: f32 },
    Drag { coefficient: f32 },
    Turbulence { strength: f32 },
}

impl ForceField {
    pub fn apply(
        &self, px: f32, py: f32,
        vx: &mut f32, vy: &mut f32,
        dt: f32, rng: &mut impl FnMut() -> f32,
    );
}
```

### ParticleSystem

```rust
pub struct ParticleSystem {
    emitters: Vec<(String, ParticleEmitter)>,
    force_fields: Vec<ForceField>,
}

impl ParticleSystem {
    pub fn new() -> Self;
    pub fn spawn(&mut self, name: &str, config: EmitterConfig, shape: EmitterShape, x: f32, y: f32);
    pub fn add_force_field(&mut self, field: ForceField);
    pub fn clear_force_fields(&mut self);
    pub fn update(&mut self, dt: f32);
    pub fn collect_sprites(&self, sprites: &mut Vec<SpriteInstance>, texture_id: TextureId);
    pub fn clear(&mut self);
    pub fn emitter_count(&self) -> usize;
    pub fn particle_count(&self) -> usize;
    pub fn get_emitter_mut(&mut self, name: &str) -> Option<&mut ParticleEmitter>;
}
```

## Behavior

- **Object pool:** Each emitter pre-allocates `max_particles` dead particles. New particles reuse dead slots; if the pool is full, emission is silently dropped.
- **Burst mode:** When `burst: true`, all `max_particles` are emitted on the first `update()` call. No further particles are produced. The emitter reports `is_finished()` once all burst particles die.
- **Continuous mode:** Particles are emitted at `emission_rate` per second. Fractional emission accumulates across frames.
- **Interpolation:** Color and size linearly interpolate from start to end values over each particle's lifetime. When `fade_out` is true, alpha additionally fades to 0.
- **Force fields:** Applied to all live particles in all emitters before per-emitter updates. Forces modify velocity, not position directly.
- **Cleanup:** Finished burst emitters are automatically removed from the `ParticleSystem` during `update()`.
- **Rendering:** `collect_sprites()` appends `SpriteInstance`s for the sprite batcher; the caller provides the texture ID (typically a small white-pixel texture tinted by particle color).

## Internal Design

- Uses a custom `XorShift64` PRNG per emitter for deterministic, allocation-free random number generation.
- Spawn position offsets are computed per `EmitterShape` (point, circle, line, rect).
- Emission direction is `config.direction +/- config.spread` in radians with random speed in `[speed_min, speed_max]`.

## Non-Goals

- **Gameplay collision.** Particles do not interact with tiles, entities, or physics.
- **Particle textures/sprites.** Individual particle sprites are not supported; all particles in an emitter share a single texture. Animated sprite particles are out of scope.
- **3D particles.** This is strictly 2D.

## Open Questions

- Should `EmitterConfig` be loadable from TOML/RON asset files for data-driven effect authoring?
- Should particles optionally bounce off solid tiles (visual-only)?
- Should the system support sub-emitters (particle death spawns new emitter)?
