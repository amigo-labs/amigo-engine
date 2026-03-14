use crate::combat::DamageType;
use crate::ecs::EntityId;
use crate::enemy::{DeadEnemy, EnemyDef, EnemyInstance, EnemyManager};
use crate::game_state::TdGameState;
use crate::pathfinding::WaypointPath;
use crate::projectile::{ProjectileHit, ProjectileManager, ProjectileTarget, SpawnProjectile};
use crate::tower::{select_target, TargetCandidate, TowerAttackType, TowerDef, TowerInstance};
use crate::waves::SpawnEvent;

// ---------------------------------------------------------------------------
// Tower firing system
// ---------------------------------------------------------------------------

/// Scan enemies in range and fire projectiles from ready towers.
/// Returns the list of projectile spawn requests generated.
pub fn tower_fire_system(
    towers: &mut [(EntityId, TowerInstance)],
    tower_defs: &[TowerDef],
    enemies: &EnemyManager,
    projectiles: &mut ProjectileManager,
    dt: f32,
    tick: u64,
) {
    for (tower_id, tower) in towers.iter_mut() {
        if !tower.enabled {
            continue;
        }

        let ready = tower.update(dt);
        if !ready {
            continue;
        }

        let def = match tower_defs.iter().find(|d| d.id == tower.def_id) {
            Some(d) => d,
            None => continue,
        };

        let tier = def.tier(tower.current_tier);

        // Build candidate list from alive enemies in range
        let candidates: Vec<TargetCandidate> = enemies
            .iter_alive()
            .filter_map(|(eid, enemy)| {
                let dx = enemy.position.x - tower.position.x;
                let dy = enemy.position.y - tower.position.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= tier.range {
                    Some(TargetCandidate {
                        entity: eid,
                        position: enemy.position,
                        distance: dist,
                        health: enemy.health,
                        max_health: enemy.max_health,
                        path_progress: enemy.path_follower.segment as f32
                            + enemy.path_follower.progress.to_num::<f32>(),
                    })
                } else {
                    None
                }
            })
            .collect();

        if candidates.is_empty() {
            continue;
        }

        let target_eid = match select_target(&candidates, tower.targeting, tick + tower_id.index() as u64) {
            Some(eid) => eid,
            None => continue,
        };

        tower.current_target = Some(target_eid);
        tower.fire();

        // Spawn projectile based on attack type
        match &tier.attack_type {
            TowerAttackType::SingleTarget => {
                projectiles.spawn(SpawnProjectile {
                    owner: Some(*tower_id),
                    start: tower.position,
                    target: ProjectileTarget::Entity(target_eid),
                    speed: 200.0,
                    damage: tier.damage,
                    damage_type: DamageType::Physical,
                    max_range: tier.range * 1.5,
                    aoe_on_hit: None,
                    pierce: 0,
                    sprite_name: tier.sprite_name.clone(),
                });
            }
            TowerAttackType::Splash { radius } => {
                projectiles.spawn(SpawnProjectile {
                    owner: Some(*tower_id),
                    start: tower.position,
                    target: ProjectileTarget::Entity(target_eid),
                    speed: 150.0,
                    damage: tier.damage,
                    damage_type: DamageType::Fire,
                    max_range: tier.range * 1.5,
                    aoe_on_hit: Some(crate::combat::AoeShape::Circle { radius: *radius }),
                    pierce: 0,
                    sprite_name: tier.sprite_name.clone(),
                });
            }
            TowerAttackType::Beam => {
                // Beam: instant damage, no projectile
                // Handled separately — just record damage directly
                projectiles.spawn(SpawnProjectile {
                    owner: Some(*tower_id),
                    start: tower.position,
                    target: ProjectileTarget::Entity(target_eid),
                    speed: 9999.0, // instant
                    damage: tier.damage,
                    damage_type: DamageType::Lightning,
                    max_range: tier.range * 1.5,
                    aoe_on_hit: None,
                    pierce: 0,
                    sprite_name: tier.sprite_name.clone(),
                });
            }
            TowerAttackType::Aura { radius: _ } => {
                // Auras don't fire projectiles — they apply effects
                // Handled by a separate aura_system
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Damage application system
// ---------------------------------------------------------------------------

/// Apply projectile hits to enemies. Returns kill events.
pub fn apply_hits_system(
    hits: &[ProjectileHit],
    enemies: &mut EnemyManager,
    towers: &mut [(EntityId, TowerInstance)],
) {
    for hit in hits {
        if let Some(enemy) = enemies.get_mut(hit.target) {
            let dealt = enemy.take_damage(hit.damage, hit.damage_type);

            // Track damage on the tower that owns this projectile
            if let Some(owner_id) = hit.owner {
                if let Some((_, tower)) = towers.iter_mut().find(|(eid, _)| *eid == owner_id) {
                    tower.total_damage_dealt += dealt as u64;
                    if !enemy.alive {
                        tower.total_kills += 1;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Enemy spawn system
// ---------------------------------------------------------------------------

/// Process wave spawn events and create enemy instances.
pub fn spawn_enemies_system(
    events: &[SpawnEvent],
    enemy_defs: &[EnemyDef],
    enemies: &mut EnemyManager,
) -> Vec<EntityId> {
    let mut spawned = Vec::new();
    for event in events {
        if let Some(def) = enemy_defs.iter().find(|d| d.id == event.enemy_type) {
            let instance = EnemyInstance::from_def(def);
            let id = enemies.spawn(instance, event.position);
            spawned.push(id);
        }
    }
    spawned
}

// ---------------------------------------------------------------------------
// Enemy update system
// ---------------------------------------------------------------------------

/// Update all enemies: move along paths, process status effects.
/// Returns lists of enemies that leaked (reached exit) and died (from DoT).
pub fn update_enemies_system(
    enemies: &mut EnemyManager,
    path: &WaypointPath,
    dt: f32,
) -> EnemyUpdateResult {
    let mut leaked = Vec::new();

    for (eid, enemy) in enemies.iter_alive_mut() {
        let reached_exit = enemy.update(dt, path);
        if reached_exit {
            enemy.alive = false; // mark for cleanup
            leaked.push(eid);
        }
    }

    EnemyUpdateResult { leaked }
}

pub struct EnemyUpdateResult {
    pub leaked: Vec<EntityId>,
}

// ---------------------------------------------------------------------------
// Kill/leak processing
// ---------------------------------------------------------------------------

/// Process dead enemies: award bounties for kills, apply leak damage for leakers.
pub fn process_dead_enemies(
    dead: &[DeadEnemy],
    game_state: &mut TdGameState,
) {
    for enemy in dead {
        if enemy.leaked {
            game_state.on_enemy_leaked(enemy.leak_damage);
        } else {
            game_state.on_enemy_killed(enemy.def_id, enemy.bounty, enemy.score);
        }
    }
}

// ---------------------------------------------------------------------------
// Full tick orchestration
// ---------------------------------------------------------------------------

/// Run one complete tower defense game tick. This is the main game loop body.
pub fn td_tick(
    game_state: &mut TdGameState,
    towers: &mut Vec<(EntityId, TowerInstance)>,
    tower_defs: &[TowerDef],
    enemy_defs: &[EnemyDef],
    enemies: &mut EnemyManager,
    projectiles: &mut ProjectileManager,
    path: &WaypointPath,
    dt: f32,
) {
    // 1. Update game state (phase transitions, defeat check)
    game_state.update(dt);
    if game_state.phase.is_over() || game_state.phase.is_paused() {
        return;
    }

    let scaled_dt = dt * game_state.speed_multiplier;

    // 2. Spawn enemies from wave system (only during combat with waves)
    let spawn_events = if game_state.spawner.total_waves() > 0 {
        game_state.spawner.update(scaled_dt)
    } else {
        Vec::new()
    };
    spawn_enemies_system(&spawn_events, enemy_defs, enemies);

    // 3. Update enemies (movement, status effects, DoT)
    let _enemy_result = update_enemies_system(enemies, path, scaled_dt);

    // Mark leaked enemies for cleanup (they're already marked as !alive)
    // Nothing extra needed — cleanup_dead handles them

    // 4. Tower targeting and firing
    tower_fire_system(towers, tower_defs, enemies, projectiles, scaled_dt, game_state.tick);

    // 5. Update projectiles and detect hits
    let hits = projectiles.update(
        scaled_dt,
        &|eid| enemies.position_of(eid),
        |pos, radius| {
            enemies
                .iter_alive()
                .filter_map(|(eid, enemy)| {
                    let dx = enemy.position.x - pos.x;
                    let dy = enemy.position.y - pos.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist <= radius + 8.0 {
                        Some((eid, enemy.position))
                    } else {
                        None
                    }
                })
                .collect()
        },
    );

    // 6. Apply damage from hits
    apply_hits_system(&hits, enemies, towers);

    // 7. Cleanup dead enemies and process rewards/leaks
    let dead = enemies.cleanup_dead();
    process_dead_enemies(&dead, game_state);

    // 8. Cleanup expired projectiles
    projectiles.cleanup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::RenderVec2;
    use crate::tower::{TowerAttackType, TowerTier, TargetingStrategy};

    fn setup() -> (
        TdGameState,
        Vec<(EntityId, TowerInstance)>,
        Vec<TowerDef>,
        Vec<EnemyDef>,
        EnemyManager,
        ProjectileManager,
        WaypointPath,
    ) {
        let tower_defs = vec![TowerDef {
            id: 1,
            name: "Arrow".to_string(),
            tiers: vec![TowerTier {
                damage: 50,
                range: 200.0,
                attack_speed: 10.0, // fast for testing
                cost: 50,
                attack_type: TowerAttackType::SingleTarget,
                sprite_name: "arrow".to_string(),
            }],
            targeting: TargetingStrategy::Nearest,
        }];

        let enemy_defs = vec![EnemyDef::basic(1, "Goblin", 100, 0.02, 10)];

        let path = WaypointPath::from_f32_pairs(&[
            (0.0, 0.0),
            (500.0, 0.0),
        ]);

        let game_state = TdGameState::new(200, 20, Vec::new(), Vec::new());

        let tower = TowerInstance::new(
            1,
            RenderVec2::new(50.0, 0.0), // near the path
            &tower_defs[0].tiers[0],
            TargetingStrategy::Nearest,
        );
        let tower_id = EntityId::from_raw(100, 0);
        let towers = vec![(tower_id, tower)];

        let enemies = EnemyManager::new();
        let projectiles = ProjectileManager::new();

        (game_state, towers, tower_defs, enemy_defs, enemies, projectiles, path)
    }

    #[test]
    fn tower_fires_at_enemy() {
        let (mut gs, mut towers, tower_defs, enemy_defs, mut enemies, mut projectiles, _path) = setup();

        // Spawn an enemy near the tower
        let enemy = EnemyInstance::from_def(&enemy_defs[0]);
        enemies.spawn(enemy, RenderVec2::new(30.0, 0.0));

        // Run tower firing
        tower_fire_system(&mut towers, &tower_defs, &enemies, &mut projectiles, 1.0, 0);

        assert!(projectiles.count() > 0, "tower should have fired a projectile");
    }

    #[test]
    fn enemy_takes_damage_from_hit() {
        let (_gs, mut towers, _td, enemy_defs, mut enemies, _proj, _path) = setup();

        let enemy = EnemyInstance::from_def(&enemy_defs[0]);
        let eid = enemies.spawn(enemy, RenderVec2::ZERO);

        let hits = vec![ProjectileHit {
            projectile_index: 0,
            owner: Some(towers[0].0),
            target: eid,
            damage: 40,
            damage_type: DamageType::Physical,
            position: RenderVec2::ZERO,
            aoe: None,
        }];

        apply_hits_system(&hits, &mut enemies, &mut towers);

        let enemy = enemies.get(eid).unwrap();
        assert!(enemy.health < 100);
        assert!(enemy.alive);
    }

    #[test]
    fn enemy_death_awards_bounty() {
        let (mut gs, mut towers, _td, enemy_defs, mut enemies, _proj, _path) = setup();

        let enemy = EnemyInstance::from_def(&enemy_defs[0]);
        let eid = enemies.spawn(enemy, RenderVec2::ZERO);

        // Kill the enemy
        enemies.get_mut(eid).unwrap().take_damage(999, DamageType::Pure);
        assert!(!enemies.get(eid).unwrap().alive);

        let dead = enemies.cleanup_dead();
        process_dead_enemies(&dead, &mut gs);

        assert_eq!(gs.economy.gold, 210); // 200 starting + 10 bounty
    }

    #[test]
    fn full_td_tick() {
        let (mut gs, mut towers, tower_defs, enemy_defs, mut enemies, mut projectiles, path) = setup();

        // Spawn enemy very close to tower so projectile reaches it quickly
        let mut enemy = EnemyInstance::from_def(&enemy_defs[0]);
        // Give it very slow speed so it stays in range
        enemy.base_speed = 0.001;
        enemy.path_follower.speed = crate::math::Fix::from_num(0.001f32);
        enemies.spawn(enemy, RenderVec2::new(55.0, 0.0));

        // Run many ticks — tower should fire and kill the enemy
        for _ in 0..500 {
            td_tick(
                &mut gs,
                &mut towers,
                &tower_defs,
                &enemy_defs,
                &mut enemies,
                &mut projectiles,
                &path,
                0.016,
            );
        }

        // Enemy should be dead and bounty awarded
        assert_eq!(enemies.alive_count(), 0);
        assert!(gs.economy.gold > 200); // got bounty
    }
}
