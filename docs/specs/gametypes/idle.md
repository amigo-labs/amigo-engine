---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/save-load"]
last_updated: 2026-06-09
---

# Idle / Incremental

## Purpose

Games about numbers that grow on their own. Players accumulate resources, buy generators that produce resources per second, buy upgrades that multiply production, and eventually perform a prestige reset that trades all progress for a persistent meta-currency. The loop is: idle/wait, spend, unlock, prestige, repeat faster. Production continues while the game is closed (offline progress).

Examples: Cookie Clicker (generators + exponential costs), Adventure Capitalist (managers/multipliers), Antimatter Dimensions (layered prestige).

Implemented in `crates/amigo_core/src/idle.rs`.

## Public API

### Definitions

```rust
// Newtype identifiers:
pub struct ResourceId(pub u32);
pub struct GeneratorId(pub u32);
pub struct UpgradeId(pub u32);
pub struct PrestigeId(pub u32);

/// Definition of a resource.
pub struct ResourceDef {
    pub id: ResourceId,
    pub name: String,
    pub initial: f64,
    pub cap: Option<f64>,    // maximum amount (None = unlimited)
}

/// Definition of a generator (produces resources over time).
pub struct GeneratorDef {
    pub id: GeneratorId,
    pub name: String,
    pub produces: ResourceId,
    pub base_rate: f64,           // production per second per unit
    pub cost_resource: ResourceId,
    pub base_cost: f64,           // cost of the first unit
    pub cost_scaling: f64,        // cost = base_cost * scaling^owned
}

/// Effect of an upgrade.
pub enum UpgradeEffect {
    MultiplyGenerator { gen: GeneratorId, multiplier: f64 },
    MultiplyResource { res: ResourceId, multiplier: f64 },
    AddGeneratorFlat { gen: GeneratorId, amount: f64 },
    UnlockGenerator(GeneratorId),
    Custom(String),
}

/// Definition of an upgrade.
pub struct UpgradeDef {
    pub id: UpgradeId,
    pub name: String,
    pub cost: Vec<(ResourceId, f64)>,   // multi-resource cost
    pub effect: UpgradeEffect,
    pub max_level: u32,
}

/// Formula for prestige currency calculation.
pub enum PrestigeFormula {
    Linear(f64),   // currency = amount * factor
    Sqrt(f64),     // currency = sqrt(amount) * factor
    Log(f64),      // currency = log2(amount) * factor
}

/// Definition of a prestige layer.
pub struct PrestigeDef {
    pub id: PrestigeId,
    pub name: String,
    pub resource_required: ResourceId,  // must reach threshold to prestige
    pub threshold: f64,
    pub currency: ResourceId,           // resource granted on prestige
    pub formula: PrestigeFormula,
}

/// All idle game definitions.
pub struct IdleRegistry {
    pub resources: FxHashMap<ResourceId, ResourceDef>,
    pub generators: FxHashMap<GeneratorId, GeneratorDef>,
    pub upgrades: FxHashMap<UpgradeId, UpgradeDef>,
    pub prestiges: FxHashMap<PrestigeId, PrestigeDef>,
}

impl IdleRegistry {
    pub fn new() -> Self;
}
```

### Runtime State

```rust
/// Runtime state for an idle game.
pub struct IdleState {
    pub resources: FxHashMap<ResourceId, f64>,
    pub generators: FxHashMap<GeneratorId, u32>,   // owned count
    pub upgrades: FxHashMap<UpgradeId, u32>,       // level
    pub prestige_count: u32,
    pub total_time_played: f64,
}

impl IdleState {
    /// Initialize all resources to their ResourceDef::initial values.
    pub fn new(registry: &IdleRegistry) -> Self;
    pub fn resource(&self, id: ResourceId) -> f64;
    pub fn generator_count(&self, id: GeneratorId) -> u32;
    pub fn upgrade_level(&self, id: UpgradeId) -> u32;
}
```

### System Functions

```rust
/// Calculate the production rate for a resource (per second), applying
/// generator counts, MultiplyGenerator/AddGeneratorFlat per-generator
/// upgrades, then MultiplyResource upgrades on the total.
pub fn production_rate(state: &IdleState, resource: ResourceId, registry: &IdleRegistry) -> f64;

/// Tick the idle game, producing resources (respects ResourceDef::cap).
/// Call every frame.
pub fn idle_tick(state: &mut IdleState, dt: f64, registry: &IdleRegistry) -> Vec<IdleEvent>;

/// Cost of the next generator purchase: base_cost * cost_scaling^owned.
pub fn generator_cost(state: &IdleState, gen_id: GeneratorId, registry: &IdleRegistry) -> Option<f64>;

/// Buy a generator. Returns false if not enough resources.
pub fn buy_generator(state: &mut IdleState, gen_id: GeneratorId, registry: &IdleRegistry) -> bool;

/// Buy one level of an upgrade. Returns false if a cost is unaffordable
/// or max_level is reached.
pub fn buy_upgrade(state: &mut IdleState, upgrade_id: UpgradeId, registry: &IdleRegistry) -> bool;

/// Perform a prestige reset. Returns the prestige currency gained, or None
/// if below the threshold. Grants currency, resets all other resources to
/// their initial values, clears generators and upgrades, increments
/// prestige_count.
pub fn prestige(state: &mut IdleState, prestige_id: PrestigeId, registry: &IdleRegistry) -> Option<f64>;

/// Calculate offline progress for elapsed wall-clock seconds
/// (currently a single idle_tick with the full duration).
pub fn calculate_offline_progress(state: &mut IdleState, seconds_elapsed: f64, registry: &IdleRegistry) -> Vec<IdleEvent>;
```

### Events

```rust
pub enum IdleEvent {
    ResourceGained { resource: ResourceId, amount: f64 },
    GeneratorBought { generator: GeneratorId, count: u32 },
    UpgradeBought { upgrade: UpgradeId, level: u32 },
    PrestigePerformed { prestige: PrestigeId, currency_gained: f64 },
    MilestoneReached { resource: ResourceId, amount: f64 },
}
```

## Behavior

- **Production**: `idle_tick()` adds `production_rate * dt` to each resource, clamped to `ResourceDef::cap`. Rates compose as: `base_rate * count`, then per-generator upgrades (`MultiplyGenerator` is exponential in level: `multiplier^level`; `AddGeneratorFlat` is linear: `amount * level`), then resource-wide `MultiplyResource` multipliers.
- **Exponential Costs**: The classic incremental curve — buying generator N+1 costs `base_cost * cost_scaling^owned` (e.g. 1.15x per Cursor in Cookie Clicker). `buy_generator()` checks and deducts the cost resource atomically.
- **Upgrades**: Multi-resource costs are checked fully before any deduction; each purchase raises the level by 1 up to `max_level`.
- **Prestige**: When `resource_required >= threshold`, `prestige()` converts the resource amount into currency via `PrestigeFormula` (Linear/Sqrt/Log), credits the currency resource, resets every other resource to `initial`, and wipes generators and upgrades. The prestige currency itself survives because it is excluded from the reset, so it can gate permanent meta-upgrades.
- **Offline Progress**: On load, compute elapsed real time and call `calculate_offline_progress()`. Because production is linear in `dt` for a fixed state, one large tick is exact as long as no purchases happen offline; caps still apply.
- **Numerics**: All quantities are `f64` — idle games routinely exceed `u64` range, and float drift is acceptable in the genre.

## Internal Design

- Data-driven split: `IdleRegistry` holds immutable definitions (loadable from RON/serde since everything is `Serialize`/`Deserialize`); `IdleState` holds only the mutable counters. Saves serialize `IdleState` alone.
- All logic lives in pure free functions over `(state, registry)`; there is no internal RNG and no ECS coupling, so the system can run headless (e.g. in a background save-validation pass).
- `idle_tick()` currently returns an empty event vec; purchase/prestige feedback is conveyed via return values (`bool` / `Option<f64>`), with `IdleEvent` available for game-layer emission.

## Future Work

- `UpgradeEffect::UnlockGenerator` and `Custom` are defined but not interpreted by `production_rate()`/`buy_generator()`; unlock gating is game-layer logic today.
- `IdleEvent` emission from the system functions themselves (`ResourceGained`, `MilestoneReached` thresholds) — events exist but are not yet produced internally.
- Bulk-buy helpers ("buy 10 / buy max") via geometric-series cost closed form.
- Subdivided offline simulation for games where caps or unlocks change rates mid-offline-period.
- Multiple prestige layers interacting (the registry supports several `PrestigeDef`s, but cross-layer multipliers are not modeled).

## Referenzen

- Cookie Clicker: generator cost curve (1.15^n), upgrade multipliers
- Adventure Capitalist: offline progress expectations
- Antimatter Dimensions: layered prestige formulas (Sqrt/Log)
- [engine/core](../engine/core.md) → event flow conventions
- [engine/save-load](../engine/save-load.md) → IdleState persistence + offline elapsed-time calculation
