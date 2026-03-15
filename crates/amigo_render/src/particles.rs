use crate::sprite_batcher::SpriteInstance;
use crate::texture::TextureId;
use amigo_core::Color;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// XorShift64 PRNG - simple, fast, no dependencies
// ---------------------------------------------------------------------------

pub(crate) struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0xDEAD_BEEF_CAFE_BABE
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut s = self.state;
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        self.state = s;
        s
    }

    /// Returns a float in `[0.0, 1.0)`.
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() & 0x00FF_FFFF) as f32 / 16_777_216.0
    }

    /// Returns a float in `[min, max]`.
    fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

// ---------------------------------------------------------------------------
// BlendMode
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Additive,
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

// ---------------------------------------------------------------------------
// EmitterShape
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum EmitterShape {
    Point,
    Circle { radius: f32 },
    Line { length: f32, angle: f32 },
    Rect { width: f32, height: f32 },
}

impl Default for EmitterShape {
    fn default() -> Self {
        Self::Point
    }
}

// ---------------------------------------------------------------------------
// EmitterConfig
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmitterConfig {
    /// Maximum number of live particles this emitter can have.
    pub max_particles: usize,
    /// Particles emitted per second. Use a very large value for one-shot bursts.
    pub emission_rate: f32,
    /// Lifetime range in seconds.
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    /// Initial speed range (pixels / second).
    pub speed_min: f32,
    pub speed_max: f32,
    /// Base emission direction in radians (0 = right, PI/2 = down in screen coords).
    pub direction: f32,
    /// Half-angle spread in radians around `direction`.
    pub spread: f32,
    /// Constant acceleration applied every frame (pixels / s^2). Positive Y = down.
    pub gravity: f32,
    /// Color at birth.
    pub color_start: Color,
    /// Color at death (linearly interpolated).
    pub color_end: Color,
    /// Size (width & height) at birth in pixels.
    pub size_start: f32,
    /// Size at death in pixels.
    pub size_end: f32,
    /// Whether alpha fades to 0 over the particle lifetime.
    pub fade_out: bool,
    /// Blend mode hint (the renderer may use this to pick a pipeline).
    pub blend_mode: BlendMode,
    /// If true, all `max_particles` are emitted immediately and the emitter
    /// stops producing new particles after the initial burst.
    pub burst: bool,
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self {
            max_particles: 64,
            emission_rate: 10.0,
            lifetime_min: 0.5,
            lifetime_max: 1.0,
            speed_min: 20.0,
            speed_max: 60.0,
            direction: -std::f32::consts::FRAC_PI_2, // upward
            spread: std::f32::consts::PI,
            gravity: 0.0,
            color_start: Color::WHITE,
            color_end: Color::WHITE,
            size_start: 2.0,
            size_end: 2.0,
            fade_out: true,
            blend_mode: BlendMode::Normal,
            burst: false,
        }
    }
}

// ---- Presets --------------------------------------------------------------

impl EmitterConfig {
    /// One-shot burst of fast particles radiating outward in all directions.
    pub fn explosion() -> Self {
        Self {
            max_particles: 48,
            emission_rate: f32::MAX, // emit all at once via burst
            lifetime_min: 0.3,
            lifetime_max: 0.7,
            speed_min: 60.0,
            speed_max: 160.0,
            direction: 0.0,
            spread: std::f32::consts::PI, // full circle
            gravity: 40.0,
            color_start: Color::YELLOW,
            color_end: Color::RED,
            size_start: 3.0,
            size_end: 1.0,
            fade_out: true,
            blend_mode: BlendMode::Additive,
            burst: true,
        }
    }

    /// Slow-rising, expanding smoke puffs.
    pub fn smoke() -> Self {
        Self {
            max_particles: 32,
            emission_rate: 8.0,
            lifetime_min: 1.0,
            lifetime_max: 2.0,
            speed_min: 10.0,
            speed_max: 25.0,
            direction: -std::f32::consts::FRAC_PI_2, // up
            spread: 0.4,
            gravity: -5.0, // slight upward drift
            color_start: Color::new(0.6, 0.6, 0.6, 0.8),
            color_end: Color::new(0.3, 0.3, 0.3, 0.0),
            size_start: 2.0,
            size_end: 5.0,
            fade_out: true,
            blend_mode: BlendMode::Normal,
            burst: false,
        }
    }

    /// Small twinkling sparkles that flicker in place.
    pub fn sparkle() -> Self {
        Self {
            max_particles: 24,
            emission_rate: 12.0,
            lifetime_min: 0.15,
            lifetime_max: 0.5,
            speed_min: 0.0,
            speed_max: 10.0,
            direction: 0.0,
            spread: std::f32::consts::PI,
            gravity: 0.0,
            color_start: Color::WHITE,
            color_end: Color::GOLD,
            size_start: 1.0,
            size_end: 1.0,
            fade_out: true,
            blend_mode: BlendMode::Additive,
            burst: false,
        }
    }

    /// Continuous stream of particles, good for movement trails or fire.
    pub fn trail() -> Self {
        Self {
            max_particles: 64,
            emission_rate: 30.0,
            lifetime_min: 0.2,
            lifetime_max: 0.6,
            speed_min: 5.0,
            speed_max: 20.0,
            direction: std::f32::consts::PI, // left (opposite of travel)
            spread: 0.3,
            gravity: 0.0,
            color_start: Color::WHITE,
            color_end: Color::new(1.0, 1.0, 1.0, 0.0),
            size_start: 2.0,
            size_end: 1.0,
            fade_out: true,
            blend_mode: BlendMode::Normal,
            burst: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Particle
// ---------------------------------------------------------------------------

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

impl Particle {
    fn dead() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            lifetime: 0.0,
            max_lifetime: 0.0,
            size: 0.0,
            color: Color::TRANSPARENT,
            rotation: 0.0,
            angular_velocity: 0.0,
            alive: false,
        }
    }
}

// ---------------------------------------------------------------------------
// ParticleEmitter
// ---------------------------------------------------------------------------

pub struct ParticleEmitter {
    pub config: EmitterConfig,
    pub shape: EmitterShape,
    pub x: f32,
    pub y: f32,
    pub(crate) particles: Vec<Particle>,
    /// Tracks fractional particle emission across frames.
    emit_accumulator: f32,
    /// Number of currently alive particles (cached for fast count).
    alive_count: usize,
    pub(crate) rng: Rng,
    /// For burst emitters: whether the burst has already fired.
    burst_done: bool,
}

impl ParticleEmitter {
    pub fn new(config: EmitterConfig, shape: EmitterShape, x: f32, y: f32) -> Self {
        let capacity = config.max_particles;
        let mut particles = Vec::with_capacity(capacity);
        particles.resize_with(capacity, Particle::dead);

        Self {
            config,
            shape,
            x,
            y,
            particles,
            emit_accumulator: 0.0,
            alive_count: 0,
            rng: Rng::new(0xABCD_1234),
            burst_done: false,
        }
    }

    // -- Emission helpers ---------------------------------------------------

    fn spawn_offset(&mut self) -> (f32, f32) {
        match self.shape {
            EmitterShape::Point => (0.0, 0.0),
            EmitterShape::Circle { radius } => {
                let angle = self.rng.range_f32(0.0, std::f32::consts::TAU);
                let r = radius * self.rng.next_f32().sqrt();
                (angle.cos() * r, angle.sin() * r)
            }
            EmitterShape::Line { length, angle } => {
                let t = self.rng.range_f32(-0.5, 0.5) * length;
                (angle.cos() * t, angle.sin() * t)
            }
            EmitterShape::Rect { width, height } => {
                let ox = self.rng.range_f32(-0.5, 0.5) * width;
                let oy = self.rng.range_f32(-0.5, 0.5) * height;
                (ox, oy)
            }
        }
    }

    fn emit_particle(&mut self) {
        // Find a dead slot.
        let slot = match self.particles.iter().position(|p| !p.alive) {
            Some(i) => i,
            None => return, // pool full
        };

        let (ox, oy) = self.spawn_offset();
        let angle =
            self.config.direction + self.rng.range_f32(-self.config.spread, self.config.spread);
        let speed = self
            .rng
            .range_f32(self.config.speed_min, self.config.speed_max);
        let lifetime = self
            .rng
            .range_f32(self.config.lifetime_min, self.config.lifetime_max);

        self.particles[slot] = Particle {
            position_x: self.x + ox,
            position_y: self.y + oy,
            velocity_x: angle.cos() * speed,
            velocity_y: angle.sin() * speed,
            lifetime,
            max_lifetime: lifetime,
            size: self.config.size_start,
            color: self.config.color_start,
            rotation: self.rng.range_f32(0.0, std::f32::consts::TAU),
            angular_velocity: self.rng.range_f32(-2.0, 2.0),
            alive: true,
        };
        self.alive_count += 1;
    }

    // -- Public API ---------------------------------------------------------

    /// Advance the emitter by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        // --- Emit new particles ---
        if self.config.burst && !self.burst_done {
            let to_emit = self.config.max_particles.saturating_sub(self.alive_count);
            for _ in 0..to_emit {
                self.emit_particle();
            }
            self.burst_done = true;
        } else if !self.config.burst {
            self.emit_accumulator += self.config.emission_rate * dt;
            let to_emit = self.emit_accumulator as usize;
            self.emit_accumulator -= to_emit as f32;
            for _ in 0..to_emit {
                self.emit_particle();
            }
        }

        // --- Update existing particles ---
        let cfg = &self.config;
        let mut alive = 0usize;

        for p in self.particles.iter_mut() {
            if !p.alive {
                continue;
            }

            p.lifetime -= dt;
            if p.lifetime <= 0.0 {
                p.alive = false;
                continue;
            }

            // Physics.
            p.velocity_y += cfg.gravity * dt;
            p.position_x += p.velocity_x * dt;
            p.position_y += p.velocity_y * dt;
            p.rotation += p.angular_velocity * dt;

            // Interpolation factor 0..1 (0 = just born, 1 = about to die).
            let t = 1.0 - (p.lifetime / p.max_lifetime);

            // Size interpolation.
            p.size = lerp(cfg.size_start, cfg.size_end, t);

            // Color interpolation.
            p.color = lerp_color(cfg.color_start, cfg.color_end, t);

            // Fade out alpha.
            if cfg.fade_out {
                p.color.a *= p.lifetime / p.max_lifetime;
            }

            alive += 1;
        }

        self.alive_count = alive;
    }

    /// Returns `true` when a burst emitter has finished and all particles are dead.
    pub fn is_finished(&self) -> bool {
        self.config.burst && self.burst_done && self.alive_count == 0
    }

    pub fn particle_count(&self) -> usize {
        self.alive_count
    }

    /// Append visible particles as `SpriteInstance`s into the provided vec.
    pub fn collect_sprites(&self, sprites: &mut Vec<SpriteInstance>, texture_id: TextureId) {
        for p in &self.particles {
            if !p.alive {
                continue;
            }
            sprites.push(SpriteInstance {
                texture_id,
                x: p.position_x - p.size * 0.5,
                y: p.position_y - p.size * 0.5,
                width: p.size,
                height: p.size,
                // Full UV rect - assumes a small white-pixel texture is used.
                uv_x: 0.0,
                uv_y: 0.0,
                uv_w: 1.0,
                uv_h: 1.0,
                tint: p.color,
                flip_x: false,
                flip_y: false,
                z_order: 0,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Force fields
// ---------------------------------------------------------------------------

/// A force field that affects particles within range.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ForceField {
    /// Constant directional wind.
    Wind { force_x: f32, force_y: f32 },
    /// Attracts particles toward a point.
    Attractor {
        x: f32,
        y: f32,
        strength: f32,
        radius: f32,
    },
    /// Repels particles away from a point.
    Repulsor {
        x: f32,
        y: f32,
        strength: f32,
        radius: f32,
    },
    /// Swirling vortex.
    Vortex {
        x: f32,
        y: f32,
        strength: f32,
        radius: f32,
    },
    /// Drag / air resistance (slows particles over time).
    Drag { coefficient: f32 },
    /// Turbulence (random per-particle noise force).
    Turbulence { strength: f32 },
}

impl ForceField {
    /// Apply force to a particle, modifying its velocity.
    pub fn apply(
        &self,
        px: f32,
        py: f32,
        vx: &mut f32,
        vy: &mut f32,
        dt: f32,
        rng: &mut impl FnMut() -> f32,
    ) {
        match self {
            ForceField::Wind { force_x, force_y } => {
                *vx += force_x * dt;
                *vy += force_y * dt;
            }
            ForceField::Attractor {
                x,
                y,
                strength,
                radius,
            } => {
                let dx = x - px;
                let dy = y - py;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq < radius * radius && dist_sq > 0.01 {
                    let dist = dist_sq.sqrt();
                    let factor = strength * (1.0 - dist / radius) * dt;
                    *vx += (dx / dist) * factor;
                    *vy += (dy / dist) * factor;
                }
            }
            ForceField::Repulsor {
                x,
                y,
                strength,
                radius,
            } => {
                let dx = px - x;
                let dy = py - y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq < radius * radius && dist_sq > 0.01 {
                    let dist = dist_sq.sqrt();
                    let factor = strength * (1.0 - dist / radius) * dt;
                    *vx += (dx / dist) * factor;
                    *vy += (dy / dist) * factor;
                }
            }
            ForceField::Vortex {
                x,
                y,
                strength,
                radius,
            } => {
                let dx = px - x;
                let dy = py - y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq < radius * radius && dist_sq > 0.01 {
                    let dist = dist_sq.sqrt();
                    let factor = strength * (1.0 - dist / radius) * dt;
                    // Perpendicular force for rotation
                    *vx += (-dy / dist) * factor;
                    *vy += (dx / dist) * factor;
                }
            }
            ForceField::Drag { coefficient } => {
                let factor = (1.0 - coefficient * dt).max(0.0);
                *vx *= factor;
                *vy *= factor;
            }
            ForceField::Turbulence { strength } => {
                *vx += (rng() * 2.0 - 1.0) * strength * dt;
                *vy += (rng() * 2.0 - 1.0) * strength * dt;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ParticleSystem
// ---------------------------------------------------------------------------

pub struct ParticleSystem {
    emitters: Vec<(String, ParticleEmitter)>,
    force_fields: Vec<ForceField>,
}

impl ParticleSystem {
    pub fn new() -> Self {
        Self {
            emitters: Vec::new(),
            force_fields: Vec::new(),
        }
    }

    /// Spawn a new named emitter at the given world position.
    pub fn spawn(
        &mut self,
        name: &str,
        config: EmitterConfig,
        shape: EmitterShape,
        x: f32,
        y: f32,
    ) {
        self.emitters
            .push((name.to_owned(), ParticleEmitter::new(config, shape, x, y)));
    }

    /// Add a force field that affects all particles.
    pub fn add_force_field(&mut self, field: ForceField) {
        self.force_fields.push(field);
    }

    /// Remove all force fields.
    pub fn clear_force_fields(&mut self) {
        self.force_fields.clear();
    }

    /// Advance every emitter. Finished burst emitters are automatically removed.
    pub fn update(&mut self, dt: f32) {
        // Apply force fields to all live particles in all emitters.
        if !self.force_fields.is_empty() {
            for (_, emitter) in self.emitters.iter_mut() {
                for p in emitter.particles.iter_mut() {
                    if !p.alive {
                        continue;
                    }
                    for field in &self.force_fields {
                        field.apply(
                            p.position_x,
                            p.position_y,
                            &mut p.velocity_x,
                            &mut p.velocity_y,
                            dt,
                            &mut || emitter.rng.next_f32(),
                        );
                    }
                }
            }
        }
        for (_, emitter) in self.emitters.iter_mut() {
            emitter.update(dt);
        }
        // Remove finished burst emitters.
        self.emitters.retain(|(_, e)| !e.is_finished());
    }

    /// Collect all live particles as sprite instances for the sprite batcher.
    pub fn collect_sprites(&self, sprites: &mut Vec<SpriteInstance>, texture_id: TextureId) {
        for (_, emitter) in &self.emitters {
            emitter.collect_sprites(sprites, texture_id);
        }
    }

    /// Remove all emitters and their particles.
    pub fn clear(&mut self) {
        self.emitters.clear();
    }

    /// Number of active emitters.
    pub fn emitter_count(&self) -> usize {
        self.emitters.len()
    }

    /// Total number of live particles across all emitters.
    pub fn particle_count(&self) -> usize {
        self.emitters.iter().map(|(_, e)| e.particle_count()).sum()
    }

    /// Get a mutable reference to an emitter by name (first match).
    pub fn get_emitter_mut(&mut self, name: &str) -> Option<&mut ParticleEmitter> {
        self.emitters
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, e)| e)
    }
}

impl Default for ParticleSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: lerp(a.r, b.r, t),
        g: lerp(a.g, b.g, t),
        b: lerp(a.b, b.b, t),
        a: lerp(a.a, b.a, t),
    }
}
