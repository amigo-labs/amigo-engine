use crate::ecs::EntityId;
use crate::math::RenderVec2;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Targeting strategies
// ---------------------------------------------------------------------------

/// How a tower selects its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetingStrategy {
    /// Closest enemy.
    Nearest,
    /// Enemy furthest along the path (closest to exit).
    First,
    /// Enemy with highest current HP.
    Strongest,
    /// Enemy with lowest current HP.
    Weakest,
    /// Enemy closest to dying (lowest HP percentage).
    MostDamaged,
    /// Random enemy in range.
    Random,
}

/// Candidate for targeting.
#[derive(Clone, Debug)]
pub struct TargetCandidate {
    pub entity: EntityId,
    pub position: RenderVec2,
    pub distance: f32,
    pub health: i32,
    pub max_health: i32,
    pub path_progress: f32,
}

/// Select the best target from candidates using the given strategy.
pub fn select_target(
    candidates: &[TargetCandidate],
    strategy: TargetingStrategy,
    seed: u64,
) -> Option<EntityId> {
    if candidates.is_empty() {
        return None;
    }

    let best = match strategy {
        TargetingStrategy::Nearest => candidates
            .iter()
            .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap()),
        TargetingStrategy::First => candidates
            .iter()
            .max_by(|a, b| a.path_progress.partial_cmp(&b.path_progress).unwrap()),
        TargetingStrategy::Strongest => candidates.iter().max_by(|a, b| a.health.cmp(&b.health)),
        TargetingStrategy::Weakest => candidates.iter().min_by(|a, b| a.health.cmp(&b.health)),
        TargetingStrategy::MostDamaged => candidates.iter().min_by(|a, b| {
            let frac_a = if a.max_health > 0 {
                a.health as f32 / a.max_health as f32
            } else {
                1.0
            };
            let frac_b = if b.max_health > 0 {
                b.health as f32 / b.max_health as f32
            } else {
                1.0
            };
            frac_a.partial_cmp(&frac_b).unwrap()
        }),
        TargetingStrategy::Random => {
            // Simple deterministic "random"
            let idx = (seed as usize) % candidates.len();
            Some(&candidates[idx])
        }
    };

    best.map(|c| c.entity)
}

// ---------------------------------------------------------------------------
// Tower definition
// ---------------------------------------------------------------------------

/// Attack type for a tower.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TowerAttackType {
    /// Single target projectile.
    SingleTarget,
    /// Area of effect around impact.
    Splash { radius: f32 },
    /// Continuous beam (no projectile).
    Beam,
    /// Slow/debuff aura (no damage, applies effect).
    Aura { radius: f32 },
}

/// Tower tier/upgrade level.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerTier {
    pub damage: i32,
    pub range: f32,
    pub attack_speed: f32,
    pub cost: u32,
    pub attack_type: TowerAttackType,
    pub sprite_name: String,
}

/// A tower definition with upgrade tiers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerDef {
    pub id: u32,
    pub name: String,
    pub tiers: Vec<TowerTier>,
    pub targeting: TargetingStrategy,
}

impl TowerDef {
    pub fn max_tier(&self) -> usize {
        self.tiers.len().saturating_sub(1)
    }

    pub fn tier(&self, level: usize) -> &TowerTier {
        &self.tiers[level.min(self.max_tier())]
    }
}

// ---------------------------------------------------------------------------
// Tower instance (runtime state)
// ---------------------------------------------------------------------------

/// Runtime state for a placed tower.
#[derive(Clone, Debug)]
pub struct TowerInstance {
    pub def_id: u32,
    pub position: RenderVec2,
    pub current_tier: usize,
    pub targeting: TargetingStrategy,
    pub attack_cooldown: f32,
    pub cooldown_timer: f32,
    pub current_target: Option<EntityId>,
    pub total_damage_dealt: u64,
    pub total_kills: u32,
    pub enabled: bool,
}

impl TowerInstance {
    pub fn new(
        def_id: u32,
        position: RenderVec2,
        tier: &TowerTier,
        targeting: TargetingStrategy,
    ) -> Self {
        Self {
            def_id,
            position,
            current_tier: 0,
            targeting,
            attack_cooldown: 1.0 / tier.attack_speed,
            cooldown_timer: 0.0,
            current_target: None,
            total_damage_dealt: 0,
            total_kills: 0,
            enabled: true,
        }
    }

    /// Update cooldown. Returns true if the tower is ready to fire.
    pub fn update(&mut self, dt: f32) -> bool {
        if !self.enabled {
            return false;
        }
        self.cooldown_timer -= dt;
        if self.cooldown_timer <= 0.0 {
            self.cooldown_timer = 0.0;
            true
        } else {
            false
        }
    }

    /// Fire the tower (reset cooldown).
    pub fn fire(&mut self) {
        self.cooldown_timer = self.attack_cooldown;
    }

    /// Upgrade to next tier. Returns the cost, or None if max tier.
    pub fn upgrade(&mut self, def: &TowerDef) -> Option<u32> {
        let next = self.current_tier + 1;
        if next >= def.tiers.len() {
            return None;
        }
        let tier = &def.tiers[next];
        self.current_tier = next;
        self.attack_cooldown = 1.0 / tier.attack_speed;
        Some(tier.cost)
    }

    /// Check if can be upgraded further.
    pub fn can_upgrade(&self, def: &TowerDef) -> bool {
        self.current_tier + 1 < def.tiers.len()
    }

    /// Get sell value (typically 70% of total investment).
    pub fn sell_value(&self, def: &TowerDef) -> u32 {
        let total: u32 = def.tiers[..=self.current_tier].iter().map(|t| t.cost).sum();
        (total as f32 * 0.7) as u32
    }
}

// ---------------------------------------------------------------------------
// Tower placement grid
// ---------------------------------------------------------------------------

/// Grid for tower placement validation.
pub struct PlacementGrid {
    pub width: u32,
    pub height: u32,
    pub tile_size: f32,
    /// True = buildable, False = blocked (path, obstacle, etc).
    cells: Vec<bool>,
    /// Which tower occupies each cell (None = empty).
    occupied: Vec<Option<EntityId>>,
}

impl PlacementGrid {
    pub fn new(width: u32, height: u32, tile_size: f32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            tile_size,
            cells: vec![true; size],
            occupied: vec![None; size],
        }
    }

    fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    /// Mark a cell as blocked (not buildable).
    pub fn set_blocked(&mut self, x: u32, y: u32) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            self.cells[idx] = false;
        }
    }

    /// Mark a cell as buildable.
    pub fn set_buildable(&mut self, x: u32, y: u32) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            self.cells[idx] = true;
        }
    }

    /// Check if a cell can have a tower placed on it.
    pub fn can_place(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let idx = self.idx(x, y);
        self.cells[idx] && self.occupied[idx].is_none()
    }

    /// Place a tower on a cell.
    pub fn place(&mut self, x: u32, y: u32, tower: EntityId) -> bool {
        if !self.can_place(x, y) {
            return false;
        }
        let idx = (y * self.width + x) as usize;
        self.occupied[idx] = Some(tower);
        true
    }

    /// Remove a tower from a cell.
    pub fn remove(&mut self, x: u32, y: u32) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            self.occupied[idx] = None;
        }
    }

    /// Convert world position to grid coordinates.
    pub fn world_to_grid(&self, pos: RenderVec2) -> (u32, u32) {
        (
            (pos.x / self.tile_size).max(0.0) as u32,
            (pos.y / self.tile_size).max(0.0) as u32,
        )
    }

    /// Convert grid coordinates to world center position.
    pub fn grid_to_world(&self, x: u32, y: u32) -> RenderVec2 {
        RenderVec2::new(
            x as f32 * self.tile_size + self.tile_size * 0.5,
            y as f32 * self.tile_size + self.tile_size * 0.5,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Target selection ─────────────────────────────────────

    #[test]
    fn target_selection_nearest() {
        let candidates = vec![
            TargetCandidate {
                entity: EntityId::from_raw(1, 0),
                position: RenderVec2::new(10.0, 0.0),
                distance: 10.0,
                health: 100,
                max_health: 100,
                path_progress: 0.5,
            },
            TargetCandidate {
                entity: EntityId::from_raw(2, 0),
                position: RenderVec2::new(5.0, 0.0),
                distance: 5.0,
                health: 50,
                max_health: 100,
                path_progress: 0.3,
            },
        ];

        let nearest = select_target(&candidates, TargetingStrategy::Nearest, 0);
        assert_eq!(nearest, Some(EntityId::from_raw(2, 0)));

        let first = select_target(&candidates, TargetingStrategy::First, 0);
        assert_eq!(first, Some(EntityId::from_raw(1, 0)));

        let strongest = select_target(&candidates, TargetingStrategy::Strongest, 0);
        assert_eq!(strongest, Some(EntityId::from_raw(1, 0)));
    }

    // ── Placement grid ──────────────────────────────────────

    #[test]
    fn placement_grid() {
        let mut grid = PlacementGrid::new(10, 10, 32.0);
        assert!(grid.can_place(5, 5));

        grid.set_blocked(5, 5);
        assert!(!grid.can_place(5, 5));

        let tower = EntityId::from_raw(1, 0);
        assert!(grid.can_place(3, 3));
        grid.place(3, 3, tower);
        assert!(!grid.can_place(3, 3));

        grid.remove(3, 3);
        assert!(grid.can_place(3, 3));
    }

    // ── Tower upgrades ──────────────────────────────────────

    #[test]
    fn tower_upgrade() {
        let def = TowerDef {
            id: 1,
            name: "Arrow Tower".to_string(),
            tiers: vec![
                TowerTier {
                    damage: 10,
                    range: 100.0,
                    attack_speed: 1.0,
                    cost: 50,
                    attack_type: TowerAttackType::SingleTarget,
                    sprite_name: "tower_1".to_string(),
                },
                TowerTier {
                    damage: 20,
                    range: 120.0,
                    attack_speed: 1.5,
                    cost: 75,
                    attack_type: TowerAttackType::SingleTarget,
                    sprite_name: "tower_2".to_string(),
                },
                TowerTier {
                    damage: 35,
                    range: 150.0,
                    attack_speed: 2.0,
                    cost: 100,
                    attack_type: TowerAttackType::Splash { radius: 30.0 },
                    sprite_name: "tower_3".to_string(),
                },
            ],
            targeting: TargetingStrategy::First,
        };

        let mut tower = TowerInstance::new(1, RenderVec2::ZERO, &def.tiers[0], def.targeting);
        assert!(tower.can_upgrade(&def));
        let cost = tower.upgrade(&def);
        assert_eq!(cost, Some(75));
        assert_eq!(tower.current_tier, 1);

        tower.upgrade(&def); // tier 2
        assert!(!tower.can_upgrade(&def)); // max tier
    }
}
