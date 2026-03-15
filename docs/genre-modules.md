# Genre Modules

`amigo_core` ships with 18 ready-to-use game systems. Each module is
self-contained — pick only what your game needs.

## Platformer (`amigo_core::platformer`)

2D platformer physics with production-grade feel.

| Type | Purpose |
|------|---------|
| `PlatformerController` | Tick-based movement controller |
| `PlatformerConfig` | Master config (bundles sub-configs below) |
| `JumpBufferConfig` | Input buffering for jump presses |
| `CoyoteConfig` | Ledge-forgiveness timing |
| `VariableJumpConfig` | Variable jump height (hold to jump higher) |
| `WallConfig` | Wall slide and wall-jump |
| `DashConfig` | Dash/dodge ability |
| `PlatformerInput` | Per-tick input state |
| `PlatformerOutput` | Computed velocity result |
| `PlatformerEvent` | Events: jumped, dashed, landed, left ground |
| `MovingPlatform` | Path-following platform entity |

```rust
let config = PlatformerConfig::default();
let mut ctrl = PlatformerController::new(config);

// Each tick:
let output = ctrl.tick(&input, is_grounded, is_touching_wall);
entity_velocity = output.velocity;
for event in &output.events {
    // play sounds, spawn particles, etc.
}
```

## Roguelike (`amigo_core::roguelike`)

Dungeon generation, runs, item pools, and meta-progression.

| Type | Purpose |
|------|---------|
| `Dungeon` | Generated dungeon (rooms + corridors + tile grid) |
| `DungeonConfig` | Generation parameters |
| `Room` / `RoomType` | Rectangular room with type (boss, shop, etc.) |
| `Corridor` | Passage connecting rooms |
| `DungeonTile` | Wall, Floor, Door, Corridor |
| `Run` | Single permadeath playthrough state |
| `ItemPool` / `PoolEntry` | Weighted random item selection |
| `MetaProgress` | Persistent unlocks and stats across runs |

```rust
let config = DungeonConfig { rooms: 12, ..Default::default() };
let dungeon = generate_dungeon(42, &config);

let mut run = Run::new(42);
run.next_floor(); // advance to floor 2
```

## Fighting (`amigo_core::fighting`)

Frame-based fighting game mechanics with motion inputs.

| Type | Purpose |
|------|---------|
| `Fighter` | Runtime fighter state machine |
| `FighterState` | Idle, Attacking, Blocking, HitStun, etc. |
| `MoveDef` / `FrameData` | Move definition with frame-by-frame hitboxes |
| `HitBox` / `HitType` | Attack hitbox with damage and launch properties |
| `GuardType` | Mid, High, Low, Unblockable |
| `InputBuffer` | Motion detection (QCF, DP, SPD) |
| `InputFrame` / `InputDir` | Single frame of directional + button input |
| `ComboTracker` | Combo counting and damage scaling |

```rust
let mut fighter = Fighter::new(health);
fighter.start_move(&hadouken_move);

// Each tick:
fighter.advance_move();
if check_hit(&attacker_hitbox, &defender_hurtbox) {
    combo.add_hit(ComboHit { damage, hit_type, .. });
}
```

## Farming (`amigo_core::farming`)

Calendar system, crop growth, and farm grid management.

| Type | Purpose |
|------|---------|
| `Calendar` | Day/season/year cycle (ticks → days) |
| `Season` | Spring, Summer, Autumn, Winter |
| `GrowthDef` / `GrowthStage` | Multi-stage crop/tree growth definition |
| `GrowthInstance` | Active growing crop |
| `FarmGrid` / `FarmTile` | Interactable farm tile grid |
| `SoilState` | Empty, Tilled, Planted |

```rust
let mut calendar = Calendar::new(28, 60 * 60); // 28 days/season, 3600 ticks/day
calendar.tick(); // advance one tick

let mut farm = FarmGrid::new(16, 16);
farm.till(3, 4);
farm.water(3, 4);
farm.plant(3, 4, &wheat_def);
```

## Bullet Pattern (`amigo_core::bullet_pattern`)

Bullet pool and pattern generation for shmups and bullet-hell.

| Type | Purpose |
|------|---------|
| `BulletPool` | Object pool with automatic despawn |
| `Bullet` | Single bullet entity |
| `BulletEmitter` | Fires patterns at configurable intervals |
| `PatternShape` | Radial, Spiral, Aimed, Wave, Random |
| `PatternSequence` / `PatternPhase` | Boss multi-phase pattern sequencer |

```rust
let mut pool = BulletPool::new(2000);
let velocities = compute_pattern(&PatternShape::Radial { count: 24 }, speed, angle);
for vel in velocities {
    pool.spawn(origin, vel, lifetime);
}
pool.tick(dt);
```

## Puzzle (`amigo_core::puzzle`)

Generic puzzle grid, match-finding, and Tetris-like blocks.

| Type | Purpose |
|------|---------|
| `PuzzleGrid<T>` | Generic 2D grid with gravity |
| `MatchGroup` | Group of matched cells |
| `MoveHistory` | Undo/redo stack for moves |
| `BlockShape` | Tetromino-like piece with rotation |
| `BlockBag` | Fair 7-bag randomization |

```rust
let mut grid = PuzzleGrid::<u8>::new(6, 12, 0);
grid.set(2, 0, 3); // place a gem of color 3
let matches = find_horizontal_matches(&grid, 3); // find 3+ in a row
grid.apply_gravity(); // drop floating pieces
```

## Combat (`amigo_core::combat`)

Real-time damage calculation, abilities, and AoE.

| Type | Purpose |
|------|---------|
| `CombatStats` | Attack, defense, crit rate/multiplier |
| `Resistances` | Per-element damage resistance |
| `DamageType` | Physical, Fire, Ice, Lightning, Poison, etc. |
| `DamageEvent` / `HealEvent` / `KillEvent` | Combat log events |
| `Ability` / `Cooldown` | Skill with cooldown and AoE |
| `AoeShape` | Circle, Cone, Line, Rect |
| `Projectile` | In-flight projectile with homing support |

```rust
let damage = calculate_damage(base_dmg, DamageType::Fire, &attacker, &defender);
```

## Loot (`amigo_core::loot`)

Item rarity, definitions, and drop tables.

| Type | Purpose |
|------|---------|
| `Rarity` | Common → Legendary (5 tiers) |
| `ItemDef` | Item template/prototype |
| `ItemInstance` | Concrete item with modifiers and stack count |
| `ItemModifier` | Stat modifier (+5 Attack, etc.) |
| `DropTable` / `DropEntry` | Enemy/chest drop definition |
| `GroundItem` | Dropped loot on the ground with pickup timer |

```rust
let drops = roll_drops(&boss_drop_table, rng);
for drop in drops {
    spawn_ground_item(drop.item_id, drop.count, position);
}
```

## Inventory (`amigo_core::inventory`)

Grid-based inventory and equipment slots.

| Type | Purpose |
|------|---------|
| `Inventory` | Fixed-size item storage |
| `InventorySlot` | Single slot (item ID + count) |
| `Equipment` | Equipped items by slot |
| `EquipSlot` | MainHand, Head, Chest, Legs, Feet, etc. |
| `ItemRegistry` | Central item definition lookup |

```rust
let mut inv = Inventory::new(20);
inv.add(sword_id, 1);
let mut equip = Equipment::new();
equip.equip(EquipSlot::MainHand, sword_instance);
let total_atk = equip.total_modifier("attack");
```

## Turn-Based Combat (`amigo_core::turn_combat`)

Full turn-based RPG battle system.

| Type | Purpose |
|------|---------|
| `Battle` | Battle state machine |
| `BattlePhase` | Setup, TurnOrder, WaitingForAction, Victory, etc. |
| `Combatant` / `CombatantStats` | Fighter with HP, MP, stats, level |
| `TurnAction` | Attack, Skill, UseItem, Defend, Flee, Switch |
| `ActionResult` / `BattleEffect` | Action outcome |
| `SkillDef` / `SkillTarget` | Skill definition and targeting |
| `Element` | 11 elements with effectiveness chart |
| `StatusType` / `StatusEffect` | Buff/debuff system |
| `EncounterTable` | Random encounter definition |

```rust
let mut battle = Battle::new(party, enemies);
battle.start();
battle.submit_action(TurnAction::Skill { skill_id: 0, target: 1 });
let results = battle.step(); // execute turn
```

## Dialog (`amigo_core::dialog`)

Branching dialog trees with conditions and effects.

| Type | Purpose |
|------|---------|
| `DialogTree` | Complete dialog tree (nodes + choices) |
| `DialogNode` | Single dialog screen |
| `DialogChoice` | Player choice with optional condition |
| `DialogCondition` | FlagSet, HasItem, And/Or/Not |
| `DialogEffect` | SetFlag, GiveItem, TakeItem, StartBattle |
| `DialogRunner` | Runtime state machine |
| `DialogState` | Persistent dialog flags |
| `DialogRegistry` | Central dialog tree registry |

```rust
let mut runner = DialogRunner::new();
runner.start("merchant_intro", &registry, &dialog_state);
// Show text: runner.current_text()
// Show choices: runner.current_choices()
runner.choose(0, &mut dialog_state); // pick first choice
```

## Crafting (`amigo_core::crafting`)

Recipe system with stations, discovery, and timed jobs.

| Type | Purpose |
|------|---------|
| `Recipe` / `RecipeId` | Recipe with ingredients and results |
| `RecipeIngredient` | Single ingredient or output |
| `RecipeRegistry` | Central recipe database |
| `UnlockCondition` | Always, HasItem, PlayerLevel, QuestFlag |
| `CraftingJob` | Active crafting with progress bar |
| `CraftingState` | Discovered recipes + active job |
| `CraftError` | NotEnoughMaterials, WrongStation, etc. |

```rust
let available = available_recipes(&registry, &inventory, &state);
let result = craft(&recipe, &mut inventory)?;
```

## Procedural Generation (`amigo_core::procgen`)

Perlin noise, biome mapping, and world generation.

| Type | Purpose |
|------|---------|
| `NoiseMap` | 2D grid of noise values |
| `BiomeDef` / `BiomeMap` | Biome definitions and mapping |
| `WorldGenerator` | Configurable multi-layer world generator |
| `CollisionTile` | Empty or Solid (for physics) |

```rust
let heightmap = NoiseMap::generate(128, 128, seed, 4, 0.5, 64.0);
let gen = WorldGenerator::new(seed, 256, 256);
let tiles = gen.generate_tiles();
```

**Noise functions:** `perlin2d()`, `fbm2d()`, `ridge2d()`, `warp2d()`

## Economy (`amigo_core::economy`)

Gold, lives, score tracking with transaction history.

| Type | Purpose |
|------|---------|
| `Economy` | Gold, lives, and score container |
| `Transaction` | Recorded transaction with kind and amount |
| `TransactionKind` | EnemyBounty, WaveBonus, TowerPlace, Interest, etc. |

```rust
let mut eco = Economy::new(200, 20, 0); // 200 gold, 20 lives, 0 score
eco.try_spend(50, TransactionKind::TowerPlace);
eco.add_gold(10, TransactionKind::EnemyBounty);
eco.apply_interest(5); // 5% interest per wave
```

## AI (`amigo_core::ai`)

Finite state machines and steering behaviors.

| Type | Purpose |
|------|---------|
| `StateMachine` | FSM with condition-based transitions |
| `StateId` | State identifier |
| `Transition` | State → State with condition function |
| `AiContext` | Context passed to AI conditions |
| `Steering` | Seek, flee, arrive, separation behaviors |

```rust
let mut ai = monster_ai(100.0, 150.0); // chase range, attack range
ai.update(&context);

let force = Steering::seek(position, target, max_speed);
```

**Pre-built AI:** `monster_ai()`, `patrol_ai()`

## Navigation (`amigo_core::navigation`)

Click-to-move agents and direction helpers.

| Type | Purpose |
|------|---------|
| `NavAgent` | Pathfinding agent with smooth movement |
| `Direction` | 8-directional enum with vector conversion |

```rust
let mut agent = NavAgent::new(tile_size, speed);
agent.move_to(target_world_pos, &walkable_map);
agent.update(dt); // smooth movement along path
```

## Status Effects (`amigo_core::status_effect`)

Real-time buff/debuff system with stacking.

| Type | Purpose |
|------|---------|
| `StatusEffects` | Container for all active effects |
| `StatusEffect` | Single timed effect |
| `EffectType` | Slow, Stun, Burn, Poison, ArmorBreak, Vulnerable |

```rust
let mut effects = StatusEffects::new();
effects.apply(StatusEffect::new(EffectType::Slow(Fix::from_num(0.5)), 180));
effects.update(); // tick durations
let speed_mult = effects.speed_multiplier(); // 0.5 while slowed
```

## Projectile (`amigo_core::projectile`)

Projectile spawning and lifecycle management.

| Type | Purpose |
|------|---------|
| `ProjectileManager` | Pool of active projectiles |
| `SpawnProjectile` | Spawn request |
| `ProjectileTarget` | Entity (homing), Direction, or Position |
| `ProjectileHit` | Hit result |

```rust
let mut pm = ProjectileManager::new();
pm.spawn(SpawnProjectile {
    origin, target: ProjectileTarget::Entity(enemy_id),
    speed, damage, ..
});
pm.update(dt);
for hit in pm.iter() { /* apply damage */ }
```

## Combining modules

Modules are designed to compose. A typical RPG might use:
`combat` + `loot` + `inventory` + `turn_combat` + `dialog` + `status_effect`

A tower defense game uses:
`economy` + `pathfinding` + `projectile` + `status_effect` + the `td` feature modules

A metroidvania combines:
`platformer` + `combat` + `inventory` + `navigation` + `procgen`
