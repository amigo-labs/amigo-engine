use crate::combat::{DamageType, Resistances};
use crate::ecs::EntityId;
use crate::math::RenderVec2;
use crate::pathfinding::{PathFollower, WaypointPath};
use crate::status_effect::StatusEffects;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enemy definition (data-driven template)
// ---------------------------------------------------------------------------

/// Defines a type of enemy (loaded from RON/data files).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnemyDef {
    pub id: u32,
    pub name: String,
    /// Base health points.
    pub health: i32,
    /// Armor / defense value.
    pub armor: i32,
    /// Movement speed (tiles per tick for PathFollower).
    pub speed: f32,
    /// Gold reward on kill.
    pub bounty: i32,
    /// Score reward on kill.
    pub score: u64,
    /// Lives lost when this enemy reaches the exit.
    pub leak_damage: i32,
    /// Resistances to damage types.
    pub resistances: Resistances,
    /// Sprite name for rendering.
    pub sprite_name: String,
    /// Whether this enemy is a boss.
    pub is_boss: bool,
    /// Whether this enemy flies (ignores ground path, goes straight).
    pub flying: bool,
}

impl EnemyDef {
    /// Simple enemy for testing.
    pub fn basic(id: u32, name: &str, health: i32, speed: f32, bounty: i32) -> Self {
        Self {
            id,
            name: name.to_string(),
            health,
            armor: 0,
            speed,
            bounty,
            score: bounty as u64 * 10,
            leak_damage: 1,
            resistances: Resistances::default(),
            sprite_name: format!("enemy_{}", name.to_lowercase()),
            is_boss: false,
            flying: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Enemy instance (runtime state)
// ---------------------------------------------------------------------------

/// Runtime state for a spawned enemy entity.
#[derive(Clone, Debug)]
pub struct EnemyInstance {
    pub def_id: u32,
    pub health: i32,
    pub max_health: i32,
    pub armor: i32,
    pub base_speed: f32,
    pub bounty: i32,
    pub score: u64,
    pub leak_damage: i32,
    pub resistances: Resistances,
    pub flying: bool,
    pub is_boss: bool,

    /// Path follower component.
    pub path_follower: PathFollower,
    /// Current world position.
    pub position: RenderVec2,
    /// Active status effects.
    pub status_effects: StatusEffects,
    /// Whether this enemy is alive.
    pub alive: bool,
}

impl EnemyInstance {
    pub fn from_def(def: &EnemyDef) -> Self {
        Self {
            def_id: def.id,
            health: def.health,
            max_health: def.health,
            armor: def.armor,
            base_speed: def.speed,
            bounty: def.bounty,
            score: def.score,
            leak_damage: def.leak_damage,
            resistances: def.resistances.clone(),
            flying: def.flying,
            is_boss: def.is_boss,
            path_follower: PathFollower::new(def.speed),
            position: RenderVec2::ZERO,
            status_effects: StatusEffects::new(),
            alive: true,
        }
    }

    /// Get effective speed (base speed modified by status effects).
    pub fn effective_speed(&self) -> f32 {
        self.base_speed * self.status_effects.speed_multiplier()
    }

    /// Apply damage after defense/resistance calculations.
    pub fn take_damage(&mut self, amount: i32, damage_type: DamageType) -> i32 {
        let resist = self.resistances.get(damage_type).clamp(-0.5, 0.9);
        let after_resist = (amount as f32 * (1.0 - resist)) as i32;
        let after_armor = (after_resist - self.armor).max(1);
        self.health -= after_armor;
        if self.health <= 0 {
            self.health = 0;
            self.alive = false;
        }
        after_armor
    }

    /// Apply raw damage (bypasses armor/resistance).
    pub fn take_raw_damage(&mut self, amount: i32) {
        self.health -= amount;
        if self.health <= 0 {
            self.health = 0;
            self.alive = false;
        }
    }

    /// Health fraction (0.0 to 1.0).
    pub fn health_fraction(&self) -> f32 {
        if self.max_health <= 0 {
            0.0
        } else {
            self.health as f32 / self.max_health as f32
        }
    }

    /// Update enemy for one tick: advance along path and process status effects.
    /// Returns `true` if the enemy reached the exit (leaked).
    pub fn update(&mut self, dt: f32, path: &WaypointPath) -> bool {
        if !self.alive {
            return false;
        }

        // Update status effects
        self.status_effects.update(dt);

        // Apply damage-over-time from status effects
        let dot = self.status_effects.damage_per_second();
        if dot > 0.0 {
            let tick_damage = (dot * dt) as i32;
            if tick_damage > 0 {
                self.take_raw_damage(tick_damage);
                if !self.alive {
                    return false;
                }
            }
        }

        // Update path follower speed based on status effects
        self.path_follower.speed = crate::math::Fix::from_num(self.effective_speed());

        // Advance along path
        let sim_pos = self.path_follower.update(path);
        self.position = sim_pos.to_render();

        // Check if reached end of path (leaked)
        self.path_follower.finished
    }
}

// ---------------------------------------------------------------------------
// Enemy manager
// ---------------------------------------------------------------------------

/// Manages all active enemy instances in the game.
pub struct EnemyManager {
    enemies: Vec<(EntityId, EnemyInstance)>,
    next_id: u32,
    generation: u32,
}

impl EnemyManager {
    pub fn new() -> Self {
        Self {
            enemies: Vec::new(),
            next_id: 0,
            generation: 0,
        }
    }

    /// Spawn a new enemy. Returns its EntityId.
    pub fn spawn(&mut self, mut instance: EnemyInstance, position: RenderVec2) -> EntityId {
        instance.position = position;
        let id = EntityId::from_raw(self.next_id, self.generation);
        self.next_id += 1;
        self.enemies.push((id, instance));
        id
    }

    /// Get an enemy by id.
    pub fn get(&self, id: EntityId) -> Option<&EnemyInstance> {
        self.enemies
            .iter()
            .find(|(eid, _)| *eid == id)
            .map(|(_, e)| e)
    }

    /// Get a mutable reference to an enemy.
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut EnemyInstance> {
        self.enemies
            .iter_mut()
            .find(|(eid, _)| *eid == id)
            .map(|(_, e)| e)
    }

    /// Get enemy position by id (for projectile homing).
    pub fn position_of(&self, id: EntityId) -> Option<RenderVec2> {
        self.get(id).map(|e| e.position)
    }

    /// Number of alive enemies.
    pub fn alive_count(&self) -> usize {
        self.enemies.iter().filter(|(_, e)| e.alive).count()
    }

    /// Total enemies (including dead, before cleanup).
    pub fn total_count(&self) -> usize {
        self.enemies.len()
    }

    /// Iterate all alive enemies.
    pub fn iter_alive(&self) -> impl Iterator<Item = (EntityId, &EnemyInstance)> {
        self.enemies
            .iter()
            .filter(|(_, e)| e.alive)
            .map(|(id, e)| (*id, e))
    }

    /// Iterate all alive enemies mutably.
    pub fn iter_alive_mut(&mut self) -> impl Iterator<Item = (EntityId, &mut EnemyInstance)> {
        self.enemies
            .iter_mut()
            .filter(|(_, e)| e.alive)
            .map(|(id, e)| (*id, e))
    }

    /// Remove dead enemies. Returns a list of (id, def_id, bounty, score, position)
    /// for enemies that died (for kill processing).
    pub fn cleanup_dead(&mut self) -> Vec<DeadEnemy> {
        let mut dead = Vec::new();
        let mut i = 0;
        while i < self.enemies.len() {
            if !self.enemies[i].1.alive {
                let (id, enemy) = self.enemies.swap_remove(i);
                dead.push(DeadEnemy {
                    id,
                    def_id: enemy.def_id,
                    bounty: enemy.bounty,
                    score: enemy.score,
                    position: enemy.position,
                    leaked: enemy.path_follower.finished,
                    leak_damage: enemy.leak_damage,
                });
            } else {
                i += 1;
            }
        }
        dead
    }

    /// Clear all enemies.
    pub fn clear(&mut self) {
        self.enemies.clear();
    }
}

impl Default for EnemyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Info about a dead/removed enemy for processing rewards/leaks.
#[derive(Clone, Debug)]
pub struct DeadEnemy {
    pub id: EntityId,
    pub def_id: u32,
    pub bounty: i32,
    pub score: u64,
    pub position: RenderVec2,
    /// True if the enemy reached the exit (not killed).
    pub leaked: bool,
    pub leak_damage: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_def() -> EnemyDef {
        EnemyDef::basic(1, "Goblin", 100, 0.05, 10)
    }

    fn test_path() -> WaypointPath {
        WaypointPath::from_f32_pairs(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)])
    }

    #[test]
    fn enemy_from_def() {
        let def = test_def();
        let enemy = EnemyInstance::from_def(&def);
        assert_eq!(enemy.health, 100);
        assert_eq!(enemy.max_health, 100);
        assert_eq!(enemy.bounty, 10);
        assert!(enemy.alive);
    }

    #[test]
    fn take_damage_kills() {
        let def = test_def();
        let mut enemy = EnemyInstance::from_def(&def);

        let dealt = enemy.take_damage(60, DamageType::Physical);
        assert!(dealt > 0);
        assert!(enemy.alive);

        enemy.take_damage(200, DamageType::Physical);
        assert!(!enemy.alive);
        assert_eq!(enemy.health, 0);
    }

    #[test]
    fn health_fraction() {
        let def = test_def();
        let mut enemy = EnemyInstance::from_def(&def);
        assert!((enemy.health_fraction() - 1.0).abs() < 0.01);

        enemy.take_damage(50, DamageType::Pure);
        assert!(enemy.health_fraction() < 1.0);
        assert!(enemy.health_fraction() > 0.0);
    }

    #[test]
    fn enemy_manager_spawn_and_cleanup() {
        let mut mgr = EnemyManager::new();
        let def = test_def();

        let id1 = mgr.spawn(EnemyInstance::from_def(&def), RenderVec2::new(10.0, 10.0));
        let id2 = mgr.spawn(EnemyInstance::from_def(&def), RenderVec2::new(20.0, 20.0));
        assert_eq!(mgr.alive_count(), 2);

        // Kill one
        mgr.get_mut(id1).unwrap().alive = false;
        let dead = mgr.cleanup_dead();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].id, id1);
        assert_eq!(mgr.alive_count(), 1);
    }

    #[test]
    fn enemy_follows_path() {
        let def = test_def();
        let mut enemy = EnemyInstance::from_def(&def);
        let path = test_path();

        let mut leaked = false;
        for _ in 0..1000 {
            leaked = enemy.update(0.016, &path);
            if leaked {
                break;
            }
        }
        assert!(leaked);
        assert!(enemy.path_follower.finished);
    }
}
