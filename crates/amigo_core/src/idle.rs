use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique resource identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub u32);

/// Unique generator identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GeneratorId(pub u32);

/// Unique upgrade identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UpgradeId(pub u32);

/// Unique prestige layer identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrestigeId(pub u32);

// ---------------------------------------------------------------------------
// Definitions
// ---------------------------------------------------------------------------

/// Definition of a resource.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceDef {
    pub id: ResourceId,
    pub name: String,
    pub initial: f64,
    /// Maximum amount (None = unlimited).
    pub cap: Option<f64>,
}

/// Definition of a generator (produces resources over time).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratorDef {
    pub id: GeneratorId,
    pub name: String,
    pub produces: ResourceId,
    /// Base production rate per second per unit.
    pub base_rate: f64,
    /// Resource used to buy this generator.
    pub cost_resource: ResourceId,
    /// Base cost of the first unit.
    pub base_cost: f64,
    /// Multiplicative cost scaling per unit owned. Cost = base_cost * scaling^owned.
    pub cost_scaling: f64,
}

/// Effect of an upgrade.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UpgradeEffect {
    MultiplyGenerator { gen: GeneratorId, multiplier: f64 },
    MultiplyResource { res: ResourceId, multiplier: f64 },
    AddGeneratorFlat { gen: GeneratorId, amount: f64 },
    UnlockGenerator(GeneratorId),
    Custom(String),
}

/// Definition of an upgrade.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpgradeDef {
    pub id: UpgradeId,
    pub name: String,
    pub cost: Vec<(ResourceId, f64)>,
    pub effect: UpgradeEffect,
    pub max_level: u32,
}

/// Formula for prestige currency calculation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PrestigeFormula {
    /// currency = amount * factor
    Linear(f64),
    /// currency = sqrt(amount) * factor
    Sqrt(f64),
    /// currency = log2(amount) * factor
    Log(f64),
}

/// Definition of a prestige layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrestigeDef {
    pub id: PrestigeId,
    pub name: String,
    /// Resource that must reach the threshold to prestige.
    pub resource_required: ResourceId,
    /// Minimum amount of resource to prestige.
    pub threshold: f64,
    /// Resource ID for the prestige currency.
    pub currency: ResourceId,
    /// How much prestige currency you get.
    pub formula: PrestigeFormula,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// All idle game definitions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IdleRegistry {
    pub resources: FxHashMap<ResourceId, ResourceDef>,
    pub generators: FxHashMap<GeneratorId, GeneratorDef>,
    pub upgrades: FxHashMap<UpgradeId, UpgradeDef>,
    pub prestiges: FxHashMap<PrestigeId, PrestigeDef>,
}

impl IdleRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Runtime state for an idle game.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IdleState {
    pub resources: FxHashMap<ResourceId, f64>,
    pub generators: FxHashMap<GeneratorId, u32>,
    pub upgrades: FxHashMap<UpgradeId, u32>,
    pub prestige_count: u32,
    pub total_time_played: f64,
}

impl IdleState {
    pub fn new(registry: &IdleRegistry) -> Self {
        let mut resources = FxHashMap::default();
        for (id, def) in &registry.resources {
            resources.insert(*id, def.initial);
        }
        Self {
            resources,
            generators: FxHashMap::default(),
            upgrades: FxHashMap::default(),
            prestige_count: 0,
            total_time_played: 0.0,
        }
    }

    /// Get the current amount of a resource.
    pub fn resource(&self, id: ResourceId) -> f64 {
        self.resources.get(&id).copied().unwrap_or(0.0)
    }

    /// Get the count of owned generators.
    pub fn generator_count(&self, id: GeneratorId) -> u32 {
        self.generators.get(&id).copied().unwrap_or(0)
    }

    /// Get the level of an upgrade.
    pub fn upgrade_level(&self, id: UpgradeId) -> u32 {
        self.upgrades.get(&id).copied().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events from the idle system.
#[derive(Clone, Debug)]
pub enum IdleEvent {
    ResourceGained {
        resource: ResourceId,
        amount: f64,
    },
    GeneratorBought {
        generator: GeneratorId,
        count: u32,
    },
    UpgradeBought {
        upgrade: UpgradeId,
        level: u32,
    },
    PrestigePerformed {
        prestige: PrestigeId,
        currency_gained: f64,
    },
    MilestoneReached {
        resource: ResourceId,
        amount: f64,
    },
}

// ---------------------------------------------------------------------------
// System functions
// ---------------------------------------------------------------------------

/// Calculate the production rate for a resource (per second).
pub fn production_rate(state: &IdleState, resource: ResourceId, registry: &IdleRegistry) -> f64 {
    let mut rate = 0.0;

    for (gen_id, gen_def) in &registry.generators {
        if gen_def.produces != resource {
            continue;
        }
        let count = state.generator_count(*gen_id) as f64;
        if count == 0.0 {
            continue;
        }

        let mut gen_rate = gen_def.base_rate * count;

        // Apply upgrade multipliers.
        for upgrade_def in registry.upgrades.values() {
            let level = state.upgrade_level(upgrade_def.id) as f64;
            if level == 0.0 {
                continue;
            }
            match &upgrade_def.effect {
                UpgradeEffect::MultiplyGenerator { gen, multiplier } if gen == gen_id => {
                    gen_rate *= multiplier.powf(level);
                }
                UpgradeEffect::AddGeneratorFlat { gen, amount } if gen == gen_id => {
                    gen_rate += amount * level;
                }
                _ => {}
            }
        }

        rate += gen_rate;
    }

    // Apply resource multipliers from upgrades.
    for upgrade_def in registry.upgrades.values() {
        let level = state.upgrade_level(upgrade_def.id) as f64;
        if level == 0.0 {
            continue;
        }
        if let UpgradeEffect::MultiplyResource { res, multiplier } = &upgrade_def.effect {
            if *res == resource {
                rate *= multiplier.powf(level);
            }
        }
    }

    rate
}

/// Tick the idle game, producing resources. Call every frame.
pub fn idle_tick(state: &mut IdleState, dt: f64, registry: &IdleRegistry) -> Vec<IdleEvent> {
    let events = Vec::new();
    state.total_time_played += dt;

    // Produce resources.
    let resource_ids: Vec<ResourceId> = registry.resources.keys().copied().collect();
    for res_id in resource_ids {
        let rate = production_rate(state, res_id, registry);
        if rate > 0.0 {
            let gained = rate * dt;
            let current = state.resources.entry(res_id).or_insert(0.0);
            *current += gained;

            // Apply cap.
            if let Some(def) = registry.resources.get(&res_id) {
                if let Some(cap) = def.cap {
                    if *current > cap {
                        *current = cap;
                    }
                }
            }
        }
    }

    events
}

/// Calculate the cost of the next generator purchase.
pub fn generator_cost(
    state: &IdleState,
    gen_id: GeneratorId,
    registry: &IdleRegistry,
) -> Option<f64> {
    let def = registry.generators.get(&gen_id)?;
    let owned = state.generator_count(gen_id) as f64;
    Some(def.base_cost * def.cost_scaling.powf(owned))
}

/// Buy a generator. Returns false if not enough resources.
pub fn buy_generator(state: &mut IdleState, gen_id: GeneratorId, registry: &IdleRegistry) -> bool {
    let cost = match generator_cost(state, gen_id, registry) {
        Some(c) => c,
        None => return false,
    };
    let def = match registry.generators.get(&gen_id) {
        Some(d) => d,
        None => return false,
    };

    let current = state.resource(def.cost_resource);
    if current < cost {
        return false;
    }

    *state.resources.entry(def.cost_resource).or_insert(0.0) -= cost;
    *state.generators.entry(gen_id).or_insert(0) += 1;
    true
}

/// Buy an upgrade. Returns false if not enough resources or max level reached.
pub fn buy_upgrade(state: &mut IdleState, upgrade_id: UpgradeId, registry: &IdleRegistry) -> bool {
    let def = match registry.upgrades.get(&upgrade_id) {
        Some(d) => d,
        None => return false,
    };

    let current_level = state.upgrade_level(upgrade_id);
    if current_level >= def.max_level {
        return false;
    }

    // Check costs.
    for (res_id, amount) in &def.cost {
        if state.resource(*res_id) < *amount {
            return false;
        }
    }

    // Deduct costs.
    for (res_id, amount) in &def.cost {
        *state.resources.entry(*res_id).or_insert(0.0) -= amount;
    }

    *state.upgrades.entry(upgrade_id).or_insert(0) += 1;
    true
}

/// Perform a prestige reset. Returns the amount of prestige currency gained.
pub fn prestige(
    state: &mut IdleState,
    prestige_id: PrestigeId,
    registry: &IdleRegistry,
) -> Option<f64> {
    let def = registry.prestiges.get(&prestige_id)?;
    let amount = state.resource(def.resource_required);
    if amount < def.threshold {
        return None;
    }

    let currency = match &def.formula {
        PrestigeFormula::Linear(factor) => amount * factor,
        PrestigeFormula::Sqrt(factor) => amount.sqrt() * factor,
        PrestigeFormula::Log(factor) => amount.log2().max(0.0) * factor,
    };

    // Grant prestige currency.
    *state.resources.entry(def.currency).or_insert(0.0) += currency;

    // Reset non-prestige resources and generators.
    for (res_id, res_def) in &registry.resources {
        if *res_id != def.currency {
            state.resources.insert(*res_id, res_def.initial);
        }
    }
    state.generators.clear();
    state.upgrades.clear();
    state.prestige_count += 1;

    Some(currency)
}

/// Calculate offline progress for a given number of elapsed seconds.
pub fn calculate_offline_progress(
    state: &mut IdleState,
    seconds_elapsed: f64,
    registry: &IdleRegistry,
) -> Vec<IdleEvent> {
    // Simple approach: apply tick with full elapsed time.
    // For more accuracy, could subdivide into smaller steps.
    idle_tick(state, seconds_elapsed, registry)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> IdleRegistry {
        let mut reg = IdleRegistry::new();
        reg.resources.insert(
            ResourceId(0),
            ResourceDef {
                id: ResourceId(0),
                name: "Cookies".into(),
                initial: 0.0,
                cap: None,
            },
        );
        reg.resources.insert(
            ResourceId(1),
            ResourceDef {
                id: ResourceId(1),
                name: "Prestige Points".into(),
                initial: 0.0,
                cap: None,
            },
        );
        reg.generators.insert(
            GeneratorId(0),
            GeneratorDef {
                id: GeneratorId(0),
                name: "Cursor".into(),
                produces: ResourceId(0),
                base_rate: 1.0,
                cost_resource: ResourceId(0),
                base_cost: 10.0,
                cost_scaling: 1.15,
            },
        );
        reg.upgrades.insert(
            UpgradeId(0),
            UpgradeDef {
                id: UpgradeId(0),
                name: "Double Cursors".into(),
                cost: vec![(ResourceId(0), 100.0)],
                effect: UpgradeEffect::MultiplyGenerator {
                    gen: GeneratorId(0),
                    multiplier: 2.0,
                },
                max_level: 3,
            },
        );
        reg.prestiges.insert(
            PrestigeId(0),
            PrestigeDef {
                id: PrestigeId(0),
                name: "Ascend".into(),
                resource_required: ResourceId(0),
                threshold: 1000.0,
                currency: ResourceId(1),
                formula: PrestigeFormula::Sqrt(1.0),
            },
        );
        reg
    }

    #[test]
    fn production_with_generators() {
        let reg = test_registry();
        let mut state = IdleState::new(&reg);

        // No generators = no production.
        assert_eq!(production_rate(&state, ResourceId(0), &reg), 0.0);

        // Buy a cursor (need cookies first).
        *state.resources.entry(ResourceId(0)).or_insert(0.0) = 100.0;
        assert!(buy_generator(&mut state, GeneratorId(0), &reg));
        assert_eq!(state.generator_count(GeneratorId(0)), 1);

        // 1 cursor * 1.0/s = 1.0/s
        assert_eq!(production_rate(&state, ResourceId(0), &reg), 1.0);
    }

    #[test]
    fn generator_scaling_cost() {
        let reg = test_registry();
        let state = IdleState::new(&reg);

        let cost0 = generator_cost(&state, GeneratorId(0), &reg).unwrap();
        assert_eq!(cost0, 10.0); // base_cost * 1.15^0

        let mut state2 = state.clone();
        *state2.generators.entry(GeneratorId(0)).or_insert(0) = 5;
        let cost5 = generator_cost(&state2, GeneratorId(0), &reg).unwrap();
        assert!((cost5 - 10.0 * 1.15_f64.powi(5)).abs() < 0.01);
    }

    #[test]
    fn upgrade_multiplies_production() {
        let reg = test_registry();
        let mut state = IdleState::new(&reg);
        *state.resources.entry(ResourceId(0)).or_insert(0.0) = 200.0;

        buy_generator(&mut state, GeneratorId(0), &reg);
        let rate_before = production_rate(&state, ResourceId(0), &reg);

        assert!(buy_upgrade(&mut state, UpgradeId(0), &reg));
        let rate_after = production_rate(&state, ResourceId(0), &reg);
        assert!((rate_after - rate_before * 2.0).abs() < 0.01);
    }

    #[test]
    fn prestige_resets_and_grants_currency() {
        let reg = test_registry();
        let mut state = IdleState::new(&reg);
        *state.resources.entry(ResourceId(0)).or_insert(0.0) = 2000.0;
        *state.generators.entry(GeneratorId(0)).or_insert(0) = 10;

        let currency = prestige(&mut state, PrestigeId(0), &reg);
        assert!(currency.is_some());
        assert!(currency.unwrap() > 0.0);

        // Resources and generators reset.
        assert_eq!(state.resource(ResourceId(0)), 0.0);
        assert_eq!(state.generator_count(GeneratorId(0)), 0);
        // Prestige currency persists.
        assert!(state.resource(ResourceId(1)) > 0.0);
        assert_eq!(state.prestige_count, 1);
    }

    #[test]
    fn prestige_below_threshold_fails() {
        let reg = test_registry();
        let mut state = IdleState::new(&reg);
        *state.resources.entry(ResourceId(0)).or_insert(0.0) = 500.0;

        assert!(prestige(&mut state, PrestigeId(0), &reg).is_none());
    }

    #[test]
    fn idle_tick_produces_resources() {
        let reg = test_registry();
        let mut state = IdleState::new(&reg);
        *state.resources.entry(ResourceId(0)).or_insert(0.0) = 100.0;
        buy_generator(&mut state, GeneratorId(0), &reg);

        let before = state.resource(ResourceId(0));
        idle_tick(&mut state, 10.0, &reg);
        let after = state.resource(ResourceId(0));
        assert!((after - before - 10.0).abs() < 0.01); // 1.0/s * 10s = 10.0
    }
}
