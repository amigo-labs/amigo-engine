use crate::combat::{DamageType, AoeShape, Projectile};
use crate::ecs::EntityId;
use crate::math::RenderVec2;

// ---------------------------------------------------------------------------
// Projectile spawn request
// ---------------------------------------------------------------------------

/// Request to spawn a new projectile (queued by tower/ability systems).
#[derive(Clone, Debug)]
pub struct SpawnProjectile {
    pub owner: Option<EntityId>,
    pub start: RenderVec2,
    pub target: ProjectileTarget,
    pub speed: f32,
    pub damage: i32,
    pub damage_type: DamageType,
    pub max_range: f32,
    pub aoe_on_hit: Option<AoeShape>,
    pub pierce: u32,
    pub sprite_name: String,
}

/// Where the projectile is aimed.
#[derive(Clone, Debug)]
pub enum ProjectileTarget {
    /// Track a specific entity (homing).
    Entity(EntityId),
    /// Fire in a direction (non-homing).
    Direction(RenderVec2),
    /// Fire toward a position (non-homing, direction computed at spawn).
    Position(RenderVec2),
}

// ---------------------------------------------------------------------------
// Projectile hit
// ---------------------------------------------------------------------------

/// Result when a projectile hits something.
#[derive(Clone, Debug)]
pub struct ProjectileHit {
    pub projectile_index: usize,
    pub owner: Option<EntityId>,
    pub target: EntityId,
    pub damage: i32,
    pub damage_type: DamageType,
    pub position: RenderVec2,
    pub aoe: Option<AoeShape>,
}

// ---------------------------------------------------------------------------
// Projectile manager
// ---------------------------------------------------------------------------

/// Manages a pool of active projectiles, handling movement and lifetime.
pub struct ProjectileManager {
    projectiles: Vec<Projectile>,
    sprite_names: Vec<String>,
}

impl ProjectileManager {
    pub fn new() -> Self {
        Self {
            projectiles: Vec::new(),
            sprite_names: Vec::new(),
        }
    }

    /// Spawn a projectile from a request.
    pub fn spawn(&mut self, req: SpawnProjectile) {
        let direction = match req.target {
            ProjectileTarget::Entity(eid) => {
                let mut p = Projectile::homing(
                    req.owner,
                    req.start,
                    eid,
                    req.speed,
                    req.damage,
                    req.damage_type,
                    req.max_range,
                );
                p.aoe_on_hit = req.aoe_on_hit;
                p.pierce_count = req.pierce;
                self.projectiles.push(p);
                self.sprite_names.push(req.sprite_name);
                return;
            }
            ProjectileTarget::Direction(dir) => dir,
            ProjectileTarget::Position(pos) => {
                RenderVec2::new(pos.x - req.start.x, pos.y - req.start.y)
            }
        };

        let mut p = Projectile::new(
            req.owner,
            req.start,
            direction,
            req.speed,
            req.damage,
            req.damage_type,
            req.max_range,
        );
        p.aoe_on_hit = req.aoe_on_hit;
        p.pierce_count = req.pierce;
        self.projectiles.push(p);
        self.sprite_names.push(req.sprite_name);
    }

    /// Update all projectiles. `target_positions` maps entity → position for homing.
    /// Returns hits detected via the provided collision callback.
    ///
    /// `check_hit` receives (projectile_position, projectile_radius) and returns
    /// a list of (entity_id, entity_position) that were hit.
    pub fn update<F>(
        &mut self,
        dt: f32,
        target_positions: &dyn Fn(EntityId) -> Option<RenderVec2>,
        check_hit: F,
    ) -> Vec<ProjectileHit>
    where
        F: Fn(RenderVec2, f32) -> Vec<(EntityId, RenderVec2)>,
    {
        let mut hits = Vec::new();
        let hit_radius = 8.0; // configurable per-projectile in future

        for (i, proj) in self.projectiles.iter_mut().enumerate() {
            if !proj.alive {
                continue;
            }

            // Resolve homing target position
            let target_pos = proj.target_entity.and_then(|eid| target_positions(eid));

            // If homing target is dead, keep flying straight
            if proj.target_entity.is_some() && target_pos.is_none() {
                proj.target_entity = None;
            }

            proj.update(dt, target_pos);

            if !proj.alive {
                continue;
            }

            // Check collisions
            let nearby = check_hit(proj.position, hit_radius);
            for (eid, _epos) in nearby {
                if proj.hit_entities.contains(&eid) {
                    continue;
                }
                if proj.owner == Some(eid) {
                    continue; // don't hit owner
                }

                hits.push(ProjectileHit {
                    projectile_index: i,
                    owner: proj.owner,
                    target: eid,
                    damage: proj.damage,
                    damage_type: proj.damage_type,
                    position: proj.position,
                    aoe: proj.aoe_on_hit.clone(),
                });

                if !proj.on_hit(eid) {
                    break; // projectile destroyed
                }
            }
        }

        hits
    }

    /// Remove dead projectiles. Call after processing hits.
    pub fn cleanup(&mut self) {
        let mut i = 0;
        while i < self.projectiles.len() {
            if !self.projectiles[i].alive {
                self.projectiles.swap_remove(i);
                self.sprite_names.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Number of active projectiles.
    pub fn count(&self) -> usize {
        self.projectiles.len()
    }

    /// Iterate active projectiles with their sprite names.
    pub fn iter(&self) -> impl Iterator<Item = (&Projectile, &str)> {
        self.projectiles.iter().zip(self.sprite_names.iter().map(|s| s.as_str()))
    }

    /// Clear all projectiles.
    pub fn clear(&mut self) {
        self.projectiles.clear();
        self.sprite_names.clear();
    }
}

impl Default for ProjectileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_target(_eid: EntityId) -> Option<RenderVec2> {
        None
    }

    fn no_hits(_pos: RenderVec2, _radius: f32) -> Vec<(EntityId, RenderVec2)> {
        Vec::new()
    }

    #[test]
    fn spawn_and_update() {
        let mut mgr = ProjectileManager::new();
        mgr.spawn(SpawnProjectile {
            owner: None,
            start: RenderVec2::ZERO,
            target: ProjectileTarget::Direction(RenderVec2::new(1.0, 0.0)),
            speed: 100.0,
            damage: 10,
            damage_type: DamageType::Physical,
            max_range: 500.0,
            aoe_on_hit: None,
            pierce: 0,
            sprite_name: "arrow".to_string(),
        });

        assert_eq!(mgr.count(), 1);

        let hits = mgr.update(0.1, &no_target, no_hits);
        assert!(hits.is_empty());
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn projectile_expires_and_cleanup() {
        let mut mgr = ProjectileManager::new();
        mgr.spawn(SpawnProjectile {
            owner: None,
            start: RenderVec2::ZERO,
            target: ProjectileTarget::Direction(RenderVec2::new(1.0, 0.0)),
            speed: 100.0,
            damage: 10,
            damage_type: DamageType::Physical,
            max_range: 50.0,
            aoe_on_hit: None,
            pierce: 0,
            sprite_name: "arrow".to_string(),
        });

        // Move past range
        mgr.update(1.0, &no_target, no_hits);
        mgr.cleanup();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn projectile_hits_enemy() {
        let mut mgr = ProjectileManager::new();
        let enemy = EntityId::from_raw(5, 0);

        mgr.spawn(SpawnProjectile {
            owner: Some(EntityId::from_raw(1, 0)),
            start: RenderVec2::ZERO,
            target: ProjectileTarget::Direction(RenderVec2::new(1.0, 0.0)),
            speed: 100.0,
            damage: 25,
            damage_type: DamageType::Fire,
            max_range: 500.0,
            aoe_on_hit: None,
            pierce: 0,
            sprite_name: "fireball".to_string(),
        });

        let hits = mgr.update(0.1, &no_target, |_pos, _r| {
            vec![(enemy, RenderVec2::new(10.0, 0.0))]
        });

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target, enemy);
        assert_eq!(hits[0].damage, 25);

        mgr.cleanup();
        assert_eq!(mgr.count(), 0); // destroyed on hit (no pierce)
    }

    #[test]
    fn pierce_projectile() {
        let mut mgr = ProjectileManager::new();
        let e1 = EntityId::from_raw(1, 0);
        let e2 = EntityId::from_raw(2, 0);

        mgr.spawn(SpawnProjectile {
            owner: None,
            start: RenderVec2::ZERO,
            target: ProjectileTarget::Direction(RenderVec2::new(1.0, 0.0)),
            speed: 100.0,
            damage: 10,
            damage_type: DamageType::Physical,
            max_range: 500.0,
            aoe_on_hit: None,
            pierce: 1,
            sprite_name: "arrow".to_string(),
        });

        // Hit first enemy
        let hits = mgr.update(0.05, &no_target, |_pos, _r| {
            vec![(e1, RenderVec2::new(5.0, 0.0))]
        });
        assert_eq!(hits.len(), 1);
        mgr.cleanup();
        assert_eq!(mgr.count(), 1); // still alive (pierced)

        // Hit second enemy
        let hits = mgr.update(0.05, &no_target, |_pos, _r| {
            vec![(e2, RenderVec2::new(10.0, 0.0))]
        });
        assert_eq!(hits.len(), 1);
        mgr.cleanup();
        assert_eq!(mgr.count(), 0); // destroyed after second hit
    }
}
