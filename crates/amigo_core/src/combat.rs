use crate::ecs::EntityId;
use crate::math::RenderVec2;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Damage types and events
// ---------------------------------------------------------------------------

/// Damage type for element/resist systems.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageType {
    Physical,
    Fire,
    Ice,
    Lightning,
    Poison,
    Holy,
    Shadow,
    Pure, // ignores armor/resists
}

/// A damage event produced by the combat system.
#[derive(Clone, Debug)]
pub struct DamageEvent {
    pub source: Option<EntityId>,
    pub target: EntityId,
    pub damage_type: DamageType,
    pub base_amount: i32,
    pub final_amount: i32,
    pub is_critical: bool,
    pub position: RenderVec2,
}

/// A heal event.
#[derive(Clone, Debug)]
pub struct HealEvent {
    pub source: Option<EntityId>,
    pub target: EntityId,
    pub amount: i32,
}

/// Kill event (entity just died).
#[derive(Clone, Debug)]
pub struct KillEvent {
    pub killer: Option<EntityId>,
    pub victim: EntityId,
    pub position: RenderVec2,
}

// ---------------------------------------------------------------------------
// Stats / Attributes
// ---------------------------------------------------------------------------

/// Combat stats for an entity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatStats {
    pub attack_power: i32,
    pub defense: i32,
    pub attack_speed: f32,
    pub critical_chance: f32,
    pub critical_multiplier: f32,
    pub attack_range: f32,
    pub resistances: Resistances,
}

impl Default for CombatStats {
    fn default() -> Self {
        Self {
            attack_power: 10,
            defense: 5,
            attack_speed: 1.0,
            critical_chance: 0.05,
            critical_multiplier: 1.5,
            attack_range: 32.0,
            resistances: Resistances::default(),
        }
    }
}

/// Damage type resistances (0.0 = no resist, 1.0 = immune, negative = vulnerability).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Resistances {
    pub physical: f32,
    pub fire: f32,
    pub ice: f32,
    pub lightning: f32,
    pub poison: f32,
    pub holy: f32,
    pub shadow: f32,
}

impl Resistances {
    pub fn get(&self, dtype: DamageType) -> f32 {
        match dtype {
            DamageType::Physical => self.physical,
            DamageType::Fire => self.fire,
            DamageType::Ice => self.ice,
            DamageType::Lightning => self.lightning,
            DamageType::Poison => self.poison,
            DamageType::Holy => self.holy,
            DamageType::Shadow => self.shadow,
            DamageType::Pure => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Cooldown
// ---------------------------------------------------------------------------

/// A cooldown timer for abilities / attacks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cooldown {
    pub duration: f32,
    pub remaining: f32,
}

impl Cooldown {
    pub fn new(duration: f32) -> Self {
        Self {
            duration,
            remaining: 0.0,
        }
    }

    pub fn trigger(&mut self) {
        self.remaining = self.duration;
    }

    pub fn update(&mut self, dt: f32) {
        self.remaining = (self.remaining - dt).max(0.0);
    }

    pub fn is_ready(&self) -> bool {
        self.remaining <= 0.0
    }

    pub fn fraction_remaining(&self) -> f32 {
        if self.duration <= 0.0 {
            0.0
        } else {
            self.remaining / self.duration
        }
    }
}

// ---------------------------------------------------------------------------
// Ability / Skill
// ---------------------------------------------------------------------------

/// Area of Effect shape.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AoeShape {
    Circle { radius: f32 },
    Cone { radius: f32, half_angle: f32 },
    Line { width: f32, length: f32 },
    Rect { width: f32, height: f32 },
}

/// An ability/skill definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ability {
    pub name: String,
    pub damage_type: DamageType,
    pub base_damage: i32,
    pub cooldown_duration: f32,
    pub range: f32,
    pub aoe: Option<AoeShape>,
    pub projectile_speed: Option<f32>,
    pub mana_cost: i32,
}

impl Ability {
    pub fn basic_attack(damage: i32, range: f32) -> Self {
        Self {
            name: "Attack".to_string(),
            damage_type: DamageType::Physical,
            base_damage: damage,
            cooldown_duration: 1.0,
            range,
            aoe: None,
            projectile_speed: None,
            mana_cost: 0,
        }
    }

    pub fn fireball(damage: i32, range: f32, radius: f32) -> Self {
        Self {
            name: "Fireball".to_string(),
            damage_type: DamageType::Fire,
            base_damage: damage,
            cooldown_duration: 3.0,
            range,
            aoe: Some(AoeShape::Circle { radius }),
            projectile_speed: Some(200.0),
            mana_cost: 25,
        }
    }
}

// ---------------------------------------------------------------------------
// Projectile
// ---------------------------------------------------------------------------

/// A projectile in flight.
#[derive(Clone, Debug)]
pub struct Projectile {
    pub owner: Option<EntityId>,
    pub position: RenderVec2,
    pub velocity: RenderVec2,
    pub damage_type: DamageType,
    pub damage: i32,
    pub speed: f32,
    pub max_range: f32,
    pub distance_traveled: f32,
    pub aoe_on_hit: Option<AoeShape>,
    pub target_entity: Option<EntityId>,
    pub alive: bool,
    pub pierce_count: u32,
    pub hit_entities: Vec<EntityId>,
}

impl Projectile {
    pub fn new(
        owner: Option<EntityId>,
        start: RenderVec2,
        direction: RenderVec2,
        speed: f32,
        damage: i32,
        damage_type: DamageType,
        max_range: f32,
    ) -> Self {
        let dist = (direction.x * direction.x + direction.y * direction.y).sqrt();
        let norm = if dist > 0.01 {
            RenderVec2::new(direction.x / dist, direction.y / dist)
        } else {
            RenderVec2::new(1.0, 0.0)
        };

        Self {
            owner,
            position: start,
            velocity: RenderVec2::new(norm.x * speed, norm.y * speed),
            damage_type,
            damage,
            speed,
            max_range,
            distance_traveled: 0.0,
            aoe_on_hit: None,
            target_entity: None,
            alive: true,
            pierce_count: 0,
            hit_entities: Vec::new(),
        }
    }

    /// Homing projectile that tracks a target entity.
    pub fn homing(
        owner: Option<EntityId>,
        start: RenderVec2,
        target: EntityId,
        speed: f32,
        damage: i32,
        damage_type: DamageType,
        max_range: f32,
    ) -> Self {
        let mut p = Self::new(
            owner,
            start,
            RenderVec2::new(1.0, 0.0),
            speed,
            damage,
            damage_type,
            max_range,
        );
        p.target_entity = Some(target);
        p
    }

    /// Update position. For homing projectiles, pass the target's current position.
    pub fn update(&mut self, dt: f32, target_pos: Option<RenderVec2>) {
        if !self.alive {
            return;
        }

        // Homing: adjust velocity toward target
        if let (Some(_target), Some(tpos)) = (self.target_entity, target_pos) {
            let dx = tpos.x - self.position.x;
            let dy = tpos.y - self.position.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > 0.01 {
                self.velocity.x = dx / dist * self.speed;
                self.velocity.y = dy / dist * self.speed;
            }
        }

        let move_dist = self.speed * dt;
        self.position.x += self.velocity.x * dt;
        self.position.y += self.velocity.y * dt;
        self.distance_traveled += move_dist;

        if self.distance_traveled >= self.max_range {
            self.alive = false;
        }
    }

    /// Mark as hit. Returns false if the projectile should be destroyed.
    pub fn on_hit(&mut self, target: EntityId) -> bool {
        if self.hit_entities.contains(&target) {
            return true; // already hit this entity
        }
        self.hit_entities.push(target);
        if self.pierce_count > 0 {
            self.pierce_count -= 1;
            true // still alive
        } else {
            self.alive = false;
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Damage calculation
// ---------------------------------------------------------------------------

/// Simple RNG for combat (crit rolls etc). Not cryptographic.
struct CombatRng(u64);

impl CombatRng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }

    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 & 0x00FF_FFFF) as f32 / 16_777_216.0
    }
}

/// Calculate final damage after defense and resistance.
pub fn calculate_damage(
    base: i32,
    damage_type: DamageType,
    attacker: &CombatStats,
    defender: &CombatStats,
    seed: u64,
) -> DamageResult {
    let mut rng = CombatRng::new(seed);

    // Critical hit check
    let is_critical = rng.next_f32() < attacker.critical_chance;
    let crit_mult = if is_critical {
        attacker.critical_multiplier
    } else {
        1.0
    };

    // Base damage with attack power scaling
    let raw = (base as f32 + attacker.attack_power as f32 * 0.5) * crit_mult;

    // Defense reduction (diminishing returns formula)
    let defense_reduction = defender.defense as f32 / (defender.defense as f32 + 100.0);

    // Resistance reduction
    let resist = defender.resistances.get(damage_type).clamp(-0.5, 0.9);

    let after_defense = raw * (1.0 - defense_reduction);
    let final_amount = (after_defense * (1.0 - resist)).max(1.0) as i32;

    DamageResult {
        final_amount,
        is_critical,
        was_resisted: resist > 0.0,
    }
}

pub struct DamageResult {
    pub final_amount: i32,
    pub is_critical: bool,
    pub was_resisted: bool,
}

// ---------------------------------------------------------------------------
// AoE query helper
// ---------------------------------------------------------------------------

/// Check if a point is inside an AoE shape centered at `origin` facing `direction`.
pub fn point_in_aoe(
    point: RenderVec2,
    origin: RenderVec2,
    direction: RenderVec2,
    shape: &AoeShape,
) -> bool {
    let dx = point.x - origin.x;
    let dy = point.y - origin.y;
    let dist = (dx * dx + dy * dy).sqrt();

    match shape {
        AoeShape::Circle { radius } => dist <= *radius,
        AoeShape::Cone { radius, half_angle } => {
            if dist > *radius {
                return false;
            }
            // Point at the cone origin is always inside
            if dist < 1e-4 {
                return true;
            }
            let dir_len = (direction.x * direction.x + direction.y * direction.y).sqrt();
            if dir_len < 0.01 {
                return false;
            }
            let dot = (dx * direction.x + dy * direction.y) / (dist * dir_len);
            dot.acos() <= *half_angle
        }
        AoeShape::Line { width, length } => {
            let dir_len = (direction.x * direction.x + direction.y * direction.y).sqrt();
            if dir_len < 0.01 {
                return false;
            }
            let ndx = direction.x / dir_len;
            let ndy = direction.y / dir_len;
            let proj = dx * ndx + dy * ndy;
            if proj < 0.0 || proj > *length {
                return false;
            }
            let perp = (dx * (-ndy) + dy * ndx).abs();
            perp <= *width * 0.5
        }
        AoeShape::Rect { width, height } => dx.abs() <= *width * 0.5 && dy.abs() <= *height * 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Damage calculation tests ───────────────────────────────

    #[test]
    fn damage_calculation() {
        let attacker = CombatStats {
            attack_power: 20,
            critical_chance: 0.0,
            ..Default::default()
        };
        let defender = CombatStats {
            defense: 10,
            ..Default::default()
        };

        let result = calculate_damage(10, DamageType::Physical, &attacker, &defender, 42);
        assert!(result.final_amount > 0);
        assert!(!result.is_critical);
    }

    // ── Cooldown tests ──────────────────────────────────────────

    #[test]
    fn cooldown_works() {
        let mut cd = Cooldown::new(2.0);
        assert!(cd.is_ready());
        cd.trigger();
        assert!(!cd.is_ready());
        cd.update(1.0);
        assert!(!cd.is_ready());
        cd.update(1.5);
        assert!(cd.is_ready());
    }

    // ── AoE shape tests ─────────────────────────────────────────

    #[test]
    fn aoe_circle() {
        let shape = AoeShape::Circle { radius: 50.0 };
        assert!(point_in_aoe(
            RenderVec2::new(30.0, 0.0),
            RenderVec2::ZERO,
            RenderVec2::new(1.0, 0.0),
            &shape,
        ));
        assert!(!point_in_aoe(
            RenderVec2::new(60.0, 0.0),
            RenderVec2::ZERO,
            RenderVec2::new(1.0, 0.0),
            &shape,
        ));
    }

    #[test]
    fn aoe_cone_at_origin() {
        let shape = AoeShape::Cone {
            radius: 50.0,
            half_angle: std::f32::consts::FRAC_PI_4,
        };
        // Point exactly at the cone origin should be inside
        assert!(point_in_aoe(
            RenderVec2::ZERO,
            RenderVec2::ZERO,
            RenderVec2::new(1.0, 0.0),
            &shape,
        ));
        // Point within cone angle and range
        assert!(point_in_aoe(
            RenderVec2::new(30.0, 0.0),
            RenderVec2::ZERO,
            RenderVec2::new(1.0, 0.0),
            &shape,
        ));
        // Point outside cone angle
        assert!(!point_in_aoe(
            RenderVec2::new(0.0, 40.0),
            RenderVec2::ZERO,
            RenderVec2::new(1.0, 0.0),
            &shape,
        ));
    }

    // ── Projectile tests ────────────────────────────────────────

    #[test]
    fn projectile_travels_and_expires() {
        let mut proj = Projectile::new(
            None,
            RenderVec2::ZERO,
            RenderVec2::new(1.0, 0.0),
            100.0,
            10,
            DamageType::Physical,
            200.0,
        );
        // Move for 1 second
        proj.update(1.0, None);
        assert!((proj.position.x - 100.0).abs() < 1.0);
        assert!(proj.alive);

        // Move past range
        proj.update(1.5, None);
        assert!(!proj.alive);
    }
}
