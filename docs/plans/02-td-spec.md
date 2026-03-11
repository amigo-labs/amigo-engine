# Amigo TD — Tower Defense Game Specification

## Game Design + UI/UX v2.0

---

# Part I — Game Design

---

## 1. Overview

Amigo TD is the first game built on the Amigo Engine. A Tower Defense with 6 thematic worlds, each with unique pixel art, mechanics, towers, enemies, and dynamic atmosphere. 5 levels per world, 10 waves per level, 30 levels total.

---

## 2. Worlds System

Each world has two parts:

### Static Definition (RON)

```ron
// worlds/caribbean.world.ron
(
    name: "Pirates of the Caribbean",
    id: "caribbean",
    tilesets: ["caribbean_ground", "caribbean_deco"],
    tower_types: ["cannon", "pirate_archer", "kraken_tower", "powder_keg"],
    enemy_types: ["skeleton_pirate", "cursed_sailor", "ghost_ship", "parrot_swarm"],
    boss: "captain_blackbeard",
    levels: ["levels/caribbean/level_01.amigo", ...],
    default_atmosphere: "calm",
    special_mechanic: WaterSlow,
)
```

### Atmosphere Presets (RON)

```ron
// data/atmospheres/caribbean.ron
(
    "calm": (ambient_light: (0.9, 0.85, 0.7, 1.0), weather: None, music: "caribbean_calm"),
    "tense": (ambient_light: (0.6, 0.55, 0.5, 1.0), weather: Rain(0.3), music: "caribbean_tense"),
    "storm": (ambient_light: (0.3, 0.25, 0.25, 1.0), weather: Storm(1.0), music: "caribbean_storm"),
    "boss": (ambient_light: (0.2, 0.1, 0.15, 1.0), weather: Storm(1.0), music: "caribbean_boss"),
    "panic": (ambient_light: (0.8, 0.2, 0.2, 1.0), weather: None, music: "caribbean_danger"),
)
```

### Dynamic Atmosphere Logic (Rust)

```rust
fn update_atmosphere(world: &World, res: &mut Resources) {
    let wave = res.wave_manager.current();
    let lives = world.get::<PlayerState>(player).lives;
    let boss_alive = world.query::<With<BossMarker>>().count() > 0;

    if boss_alive {
        res.atmosphere.transition_to("boss", 1.0);
    } else if lives < 3 {
        res.atmosphere.transition_to("panic", 0.5);
    } else if wave >= 5 {
        res.atmosphere.transition_to("storm", 3.0);
    }
}
```

RON defines HOW it looks, Rust decides WHEN it changes. Engine interpolates smoothly.

---

## 3. Economy System

### Core Resources

| Resource | Start | Source | Sink |
|----------|-------|--------|------|
| Gold | 300 (scales per level/world) | Kill enemies, wave bonus, sell towers | Build towers, upgrade towers |
| Lives | 20 | — | Enemy reaches exit (-1 per normal, -3 per boss minion) |

### Gold Income

```ron
// data/economy.ron
(
    kill_gold_formula: "base_gold + (wave * 0.5)",
    wave_bonus: 50,
    wave_bonus_scaling: 10,          // Wave 1: 60g, Wave 5: 100g, Wave 10: 150g
    early_start_bonus: 25,           // start wave before timer expires
    sell_refund_rate: 0.6,           // 60% of total invested
)
```

No interest system – keeps it simple, rewards aggression over banking.

### Starting Gold

Scales with world position. Later worlds give more to compensate for higher tower costs.

```
World 1 (Caribbean):       300g → 375g across levels 1-5
World 2 (LotR):            350g → 425g
World 3 (Dune):            400g → 475g
World 4 (Matrix):          425g → 500g
World 5 (GoT):             450g → 525g
World 6 (Stranger Things): 500g → 575g
```

### Star Rating

```
★      Survive all waves (any lives remaining)
★★     Finish with ≥ 15 lives
★★★    Finish with all 20 lives
```

Cumulative star threshold unlocks next world (e.g., need 10 stars to unlock World 2).

---

## 4. Upgrade System

### Structure: Linear with Late Branch

4 upgrade levels per tower. Levels 1-2 are linear stat boosts. At Level 3, choose Path A or Path B. Locks the other path permanently.

```
Lv.1 ──→ Lv.2 ──→ ┬── Path A (Lv.3) ──→ Lv.4A (MAX)
                    └── Path B (Lv.3) ──→ Lv.4B (MAX)
```

### Cost Scaling

```
Base → Lv.2:   1.0× base cost
Lv.2 → Lv.3:  1.5× base cost
Lv.3 → Lv.4:  2.5× base cost
```

Example: Cannon (75g base) → Lv.2: 75g → Lv.3: 112g → Lv.4: 187g. Total invested: 449g.

### No Respec

Branch choice is permanent. Sell and rebuild for the other path (at 60% refund loss).

### Data Format

```ron
// data/towers/cannon.tower.ron
(
    id: "cannon",
    world: "caribbean",
    base: (
        cost: 75, range: 4.0, damage: 30,
        attack_speed: 1.2, projectile: "cannonball",
        default_target: Strongest,
    ),
    upgrades: [
        (name: "Iron Balls", cost: 75,
         effects: { damage: 45, range: 4.5 },
         description: "Heavier ammunition. +50% damage, +range."),

        (name: "Explosive Shells", cost: 112, path: A,
         effects: { damage: 40, splash_radius: 1.5 },
         description: "Shots explode on impact. Area damage."),

        (name: "Rapid Fire", cost: 112, path: B,
         effects: { attack_speed: 0.7, damage: 35 },
         description: "Faster reload. -40% attack time."),

        (name: "Mortar Barrage", cost: 187, path: A,
         effects: { damage: 60, splash_radius: 2.5 },
         description: "Devastating explosions. Massive splash."),

        (name: "Gatling Cannon", cost: 187, path: B,
         effects: { attack_speed: 0.3, damage: 25 },
         description: "Unleashes a storm of bullets."),
    ],
)
```

---

## 5. Damage & Status System

### Damage Types

```rust
pub enum DamageType {
    Physical,       // cannons, arrows, melee
    Magic,          // mage towers, elemental
    Fire,           // burn damage
    Ice,            // slow + damage
    Electric,       // chain lightning
    Poison,         // DoT
    True,           // ignores all resistance
}
```

### Enemy Resistances

Percentage reduction per damage type. Negative = weakness (bonus damage):

```ron
("skeleton_pirate": (resistances: { Physical: 0.2, Fire: -0.3 }))
// 20% physical resist, 30% fire WEAKNESS
```

### Status Effects

```rust
pub enum StatusEffect {
    Slow { factor: f32, duration: f32 },
    Poison { dps: f32, duration: f32 },
    Burn { dps: f32, duration: f32 },
    Freeze { duration: f32 },
    Stun { duration: f32 },
    ArmorBreak { factor: f32, duration: f32 },
    Marked { bonus_dmg: f32, duration: f32 },
}
```

Same-type doesn't stack (refreshes duration). Different types stack freely.

---

## 6. Tower Targeting

Per tower configurable by the player:

```rust
pub enum TargetPriority { First, Last, Nearest, Strongest, Weakest, Fastest }
```

Player switches via GameCommand. Default per tower type in RON.

---

## 7. Pathfinding (TD)

Predefined waypoint paths, editor-defined:

```rust
pub struct WaypointPath { pub points: Vec<SimVec2> }
pub struct PathFollower { pub path_index: usize, pub progress: Fix, pub segment: usize }
```

---

## 8. The 6 Worlds – Detailed

### World 1: Pirates of the Caribbean

**Special Mechanic: Water Slow** – Water tiles reduce enemy speed by 40%. Creates natural chokepoints. Towers can't be placed on water.

```rust
fn apply_water_slow(world: &World, res: &Resources) {
    for (entity, pos, _) in world.query::<(&Position, &PathFollower)>() {
        if res.tilemap.get_tile(pos.x, pos.y).is_water() {
            world.add(entity, Slow { factor: 0.6, duration: 0.1 });
        }
    }
}
```

**Towers:**

| Tower | Cost | DMG | Speed | Range | Type | Mechanic |
|-------|------|-----|-------|-------|------|----------|
| Cannon | 75 | 30 | 1.2s | 4.0 | Physical | High single-target damage |
| Pirate Archer | 50 | 10 | 0.6s | 6.0 | Physical | Fast, long range |
| Kraken Tower | 120 | 25 | 2.0s | 3.5 | Magic | AoE tentacle slam, slows |
| Powder Keg | 90 | 50 | 3.0s | 3.0 | Fire | Explosive, massive splash |

**Upgrade Branches:**

| Tower | Path A (AoE/Utility) | Path B (Single Target/DPS) |
|-------|----------------------|---------------------------|
| Cannon | Explosive Shells → Mortar Barrage | Rapid Fire → Gatling |
| Pirate Archer | Poison Arrows → Plague Rain (AoE DoT) | Sharpshooter → Sniper |
| Kraken Tower | Whirlpool → Maelstrom (pull + AoE) | Ink Cloud → Black Fog (AoE slow) |
| Powder Keg | Firebomb → Inferno (burn AoE) | Smoke Bomb → Flashbang (stun) |

**Enemies:**

| Enemy | HP | Speed | Armor | Special | Gold |
|-------|-----|-------|-------|---------|------|
| Skeleton Pirate | 60 | 1.0 | 20% phys | — | 8 |
| Cursed Sailor | 80 | 0.8 | — | Regenerates 2 HP/s | 12 |
| Ghost Ship | 150 | 0.6 | 50% phys | Immune to physical until hit by magic first | 20 |
| Parrot Swarm | 30 | 1.8 | — | Fast, spawns in groups of 3 | 5 |
| **Captain Blackbeard** | 2000 | 0.5 | 30% all | Summons skeleton adds every 20s | 100 |

---

### World 2: Lord of the Rings

**Special Mechanic: Elevation Bonus** – Tiles have height values (0-3). Towers on higher ground get +15% range and +10% damage per elevation level above target.

```rust
fn elevation_bonus(tower_elev: u8, target_elev: u8) -> (f32, f32) {
    let diff = (tower_elev as i32 - target_elev as i32).max(0) as f32;
    (1.0 + diff * 0.15, 1.0 + diff * 0.10)  // (range_mult, damage_mult)
}
```

**Towers:**

| Tower | Cost | DMG | Speed | Range | Type | Mechanic |
|-------|------|-----|-------|-------|------|----------|
| Elven Archer | 50 | 12 | 0.5s | 7.0 | Physical | Extreme range |
| Dwarf Axe Thrower | 80 | 35 | 1.0s | 3.5 | Physical | High damage, short range |
| Gandalf Tower | 130 | 40 | 1.5s | 5.0 | Magic | Piercing beam, hits in line |
| Ent Guardian | 100 | 20 | 2.5s | 2.5 | Physical | Melee AoE slam, slows |

**Upgrade Branches:**

| Tower | Path A | Path B |
|-------|--------|--------|
| Elven Archer | Rain of Arrows → Arrow Storm (AoE) | Mithril Tips → Legolas Shot (crit chance) |
| Dwarf Axe | Rune Axe → Thorin's Wrath (cleave) | Throwing Mastery → Gimli Fury (attack speed) |
| Gandalf Tower | You Shall Not Pass → Barrier (stun zone) | White Light → Radiant Beam (pure DPS) |
| Ent Guardian | Root Trap → Fangorn's Wrath (area root) | Stone Skin → Mountain Ent (tank aura) |

**Enemies:**

| Enemy | HP | Speed | Armor | Special | Gold |
|-------|-----|-------|-------|---------|------|
| Orc Soldier | 70 | 0.9 | 25% phys | — | 8 |
| Warg Rider | 50 | 1.6 | — | Fast, leap (skips 2 tiles on path) | 10 |
| Uruk-hai | 120 | 0.7 | 40% phys | Immune to slow | 15 |
| Cave Troll | 300 | 0.4 | 50% phys, 30% magic | Very tanky | 25 |
| **Balrog** | 3000 | 0.3 | 20% all | Fire aura (damages nearby towers), immune to fire | 120 |

---

### World 3: Dune

**Special Mechanic: Sandworms** – Environmental hazard. Every 45-60s a sandworm emerges at random sand tile: 200 true damage to enemies in 3-tile radius, 50 damage to towers in 1-tile radius. 2-second ground-rumble warning before emergence.

```rust
pub struct SandwormSpawner {
    pub timer: f32,
    pub interval: Range<f32>,          // 45.0..60.0
    pub damage_enemies: u32,           // 200 true damage
    pub damage_towers: u32,            // 50
    pub radius_enemies: f32,           // 3.0
    pub radius_towers: f32,            // 1.0
    pub warning_duration: f32,         // 2.0
}
```

**Towers:**

| Tower | Cost | DMG | Speed | Range | Type | Mechanic |
|-------|------|-----|-------|-------|------|----------|
| Fremen Sniper | 60 | 20 | 0.8s | 8.0 | Physical | Extreme range |
| Spice Harvester | 70 | 15 | 1.0s | 4.0 | Poison | Poison DoT |
| Sandstorm Tower | 110 | 10 | 0.5s | 5.0 | Physical | Slow + area blind |
| Worm Caller | 150 | 0 | — | — | True | Biases sandworm toward this area (40s CD) |

**Enemies:**

| Enemy | HP | Speed | Armor | Special | Gold |
|-------|-----|-------|-------|---------|------|
| Harkonnen Soldier | 80 | 0.9 | 30% phys | — | 9 |
| Sardaukar Elite | 150 | 1.0 | 40% phys, 20% magic | Tanky + fast | 18 |
| Ornithopter | 60 | 2.0 | — | Flying (ignores terrain slow) | 12 |
| Sand Crawler | 200 | 0.5 | 60% phys | Immune to sandworm | 22 |
| **Baron Harkonnen** | 2500 | 0.4 | 30% all | Personal shield: 500 HP shield, recharges after 10s if not hit | 130 |

---

### World 4: Matrix

**Special Mechanic: Bullet Time** – Meter fills from kills (20 kills = full). Activate: 5 seconds at 25% enemy speed, towers fire at normal rate (effective 4× DPS). 30s cooldown.

```rust
pub struct BulletTime {
    pub meter: f32,                    // 0.0..1.0
    pub meter_per_kill: f32,           // 0.05
    pub duration: f32,                 // 5.0s
    pub slow_factor: f32,             // 0.25
    pub cooldown: f32,                 // 30.0s
}
```

**Towers:**

| Tower | Cost | DMG | Speed | Range | Type | Mechanic |
|-------|------|-----|-------|-------|------|----------|
| Sentinel Turret | 55 | 15 | 0.4s | 5.0 | Electric | Very fast fire |
| EMP Tower | 90 | 30 | 1.5s | 4.0 | Electric | Stun 0.5s, chains to 3 targets |
| Firewall | 100 | 20 | 1.0s | 3.5 | Fire | Burns in area continuously |
| Virus Injector | 130 | 0 | 3.0s | 6.0 | True | Marks: +50% damage from all sources |

**Enemies:**

| Enemy | HP | Speed | Armor | Special | Gold |
|-------|-----|-------|-------|---------|------|
| Agent Drone | 50 | 1.2 | — | — | 7 |
| Sentinel | 100 | 0.8 | 30% electric | Resists electric | 12 |
| Smith Clone | 70 | 1.0 | — | On death: splits into 2 half-HP copies (once) | 10 |
| Squiddy | 40 | 2.2 | — | Very fast, flying | 8 |
| **Agent Smith** | 2800 | 0.6 | 20% all | Spawns clones every 15s. Adapts: +30% resist to most-used damage type every 10s | 140 |

---

### World 5: Game of Thrones

**Special Mechanic: Fire/Ice Synergy** – Enemy with both Burn and Ice-Slow gets **Shattered** status: 2× damage from all sources for 2s. Adjacent Fire + Ice towers get +20% range (elemental resonance).

```rust
fn check_shatter(world: &World) {
    for (entity, statuses) in world.query::<&StatusEffects>() {
        if statuses.has(Burn) && statuses.has_ice_slow() {
            world.add(entity, Marked { bonus_dmg: 1.0, duration: 2.0 });
        }
    }
}
```

**Towers:**

| Tower | Cost | DMG | Speed | Range | Type | Mechanic |
|-------|------|-----|-------|-------|------|----------|
| Dragon Fire | 90 | 25 | 1.0s | 4.5 | Fire | Burn DoT, cone attack |
| Ice Wall | 80 | 10 | 0.8s | 5.0 | Ice | 40% slow |
| Scorpion Ballista | 70 | 40 | 1.8s | 7.0 | Physical | Pierces 2 enemies |
| Wildfire Tower | 150 | 15 | 0.5s | 3.0 | Fire | Continuous area burn |

**Enemies:**

| Enemy | HP | Speed | Armor | Special | Gold |
|-------|-----|-------|-------|---------|------|
| Wight | 40 | 1.1 | -50% fire | Weak to fire, immune to ice | 6 |
| White Walker | 150 | 0.7 | 50% ice, -30% fire | Tanky, fire weakness | 18 |
| Lannister Knight | 100 | 0.8 | 40% phys | Balanced armor | 12 |
| Direwolf | 60 | 1.8 | — | Fast, packs of 4 | 8 |
| **Night King** | 3500 | 0.3 | 60% ice, 30% phys | Resurrects dead enemies as wights (50% HP). Immune to ice | 150 |

---

### World 6: Stranger Things

**Special Mechanic: Dimension Flip** – Map has two layers: Normal World and Upside Down with different tilemaps and paths. Flips every 3 waves. Towers persist in both. Enemies only exist in the current dimension.

**Technical Implementation:**

```rust
pub struct DimensionFlip {
    pub current: Dimension,             // Normal or UpsideDown
    pub flip_interval: u32,             // every 3 waves
    pub transition_duration: f32,       // 2.0s visual transition
}

pub struct DualTilemap {
    pub normal: Tilemap,
    pub upside_down: Tilemap,
}

pub struct DualPaths {
    pub normal: Vec<WaypointPath>,
    pub upside_down: Vec<WaypointPath>,
}
```

Flip transition: VHS static overlay, lights flicker, 2-second crossfade to other tilemap. Surviving enemies from previous wave leak (life loss). New wave spawns fresh in the new dimension.

Key strategy: place towers in positions that are useful in BOTH tilemaps.

**Towers:**

| Tower | Cost | DMG | Speed | Range | Type | Mechanic |
|-------|------|-----|-------|-------|------|----------|
| Slingshot | 45 | 12 | 0.7s | 5.0 | Physical | Cheap, basic |
| Walkie Tower | 80 | 0 | 3.0s | global | — | Reveals invisible enemies for 5s |
| Christmas Lights | 100 | 20 | 0.8s | 4.0 | Electric | Damage + flicker stun (0.3s) |
| Psionic Tower | 140 | 35 | 1.2s | 5.5 | Magic | Hits in BOTH dimensions |

Psionic Tower is the only tower that damages enemies regardless of current dimension.

**Enemies:**

| Enemy | HP | Speed | Armor | Special | Gold |
|-------|-----|-------|-------|---------|------|
| Demodog | 60 | 1.4 | — | Fast, packs | 8 |
| Vine Monster | 90 | 0.6 | 30% phys | Aura: slows nearby towers 20% | 14 |
| Shadow Agent | 70 | 1.0 | — | Invisible until within 2 tiles of tower | 12 |
| Mind Flayer Tendril | 120 | 0.8 | 20% all | Spawns 2 demodogs on death | 16 |
| **Demogorgon** | 3000 | 0.5 | 25% all | Teleports between dimensions every 10s, heals 5% on flip | 160 |

---

## 9. Wave System

### Structure: 10 Waves per Level

```ron
// data/waves/caribbean/level_01.waves.ron
(
    waves: [
        (groups: [
            (enemy: "skeleton_pirate", count: 6, interval: 1.5, delay: 0.0),
        ]),
        (groups: [
            (enemy: "skeleton_pirate", count: 8, interval: 1.2, delay: 0.0),
            (enemy: "cursed_sailor", count: 2, interval: 2.0, delay: 5.0),
        ]),
        // ... waves 3-9 ...
        (groups: [
            (enemy: "skeleton_pirate", count: 15, interval: 0.6, delay: 0.0),
            (enemy: "ghost_ship", count: 4, interval: 2.0, delay: 3.0),
            (enemy: "captain_blackbeard", count: 1, interval: 0.0, delay: 8.0),
        ]),
    ],
)
```

### Wave Template

| Wave | Role | Count | Types |
|------|------|-------|-------|
| 1 | Warmup | 6-8 | Basic only |
| 2-3 | Ramp | 8-12 | Basic + 1 variant |
| 4 | New intro | 10-14 | Introduce special enemy |
| 5 | Mini-boss | 12-16 | Mix + tough enemy |
| 6-7 | Escalation | 14-18 | Mix, faster spawn |
| 8 | Swarm | 20-25 | Many fast/weak |
| 9 | Pre-boss | 16-20 | All types, tight |
| 10 | Boss | 15-20 + Boss | Adds + boss at end |

### Scaling Across Worlds

| World | HP × | Speed × | Gold × |
|-------|-------|---------|--------|
| 1 Caribbean | 1.0 | 1.0 | 1.0 |
| 2 LotR | 1.3 | 1.0 | 1.15 |
| 3 Dune | 1.6 | 1.05 | 1.3 |
| 4 Matrix | 2.0 | 1.1 | 1.45 |
| 5 GoT | 2.5 | 1.1 | 1.6 |
| 6 Stranger Things | 3.0 | 1.15 | 1.8 |

Within a world: +10% HP per level (1-5).

### AI Balancing Targets

For Claude Code headless simulation:

```ron
// data/balance_targets.ron
(
    optimal_play: (three_star: 0.8, two_star: 0.95, one_star: 1.0),
    random_play: (survive: 0.3),
    gold_at_wave_5: 0.5,
    tower_slots_used_wave_10: 0.8,
    first_life_loss_wave: 4,
    tension_peak_wave: 8,
)
```

---

## 10. Special Mechanics – Summary

| World | Mechanic | Complexity | Player Agency |
|-------|----------|-----------|---------------|
| Caribbean | Water Slow | Low | Route enemies through water |
| LotR | Elevation Bonus | Medium | Fight for high ground |
| Dune | Sandworms | Medium | Avoid clustering, Worm Caller |
| Matrix | Bullet Time | Medium | Skill activation timing |
| GoT | Fire/Ice Synergy | High | Pair fire+ice for shatter combos |
| Stranger Things | Dimension Flip | High | Place for both tilemaps |

Complexity ramps across worlds. Caribbean is pick-up-and-play. Stranger Things demands mastery.

---

## 11. Key TD Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Upgrades | Linear Lv.1-2, branch at Lv.3 A/B | Depth without complexity |
| Economy | Kill gold + wave bonus + early bonus, 60% sell, no interest | Simple, rewards aggression |
| Damage | 7 types + resistances + 7 status effects | Rock-paper-scissors |
| Waves | 10 per level, template-based, world scaling | Predictable structure |
| Stars | 20/15/all lives = 1/2/3 stars | Clear goals, replayability |
| Balance | 80% 3-star optimal, 30% survive random | Fair but challenging |
| Specials | 1 per world, complexity ramps | Teaches incrementally |

---

# Part II — UI/UX Design

---

## 12. Design Philosophy

**Pixel-native, world-themed, minimal.**

All UI renders through the sprite batcher at virtual resolution (480x270 base). No egui, no HTML overlays. Every button, panel, and icon is pixel art that matches the current world's aesthetic. The HUD stays out of the way – the map is the star. Inspired by Kingdom Rush's radial menus (contextual, attached to selection) over Bloons TD's sidebar panels (always visible, eats screen space).

### Principles

- **Context over chrome** – show information where the player is looking, not in a corner
- **World-themed** – UI frames, colors, and icons adapt per world (pirate wood panels for Caribbean, stone for LotR, neon for Matrix)
- **Readable at a glance** – bitmap font, high contrast, consistent icon language
- **Gamepad-friendly** – every interaction works with D-pad + face buttons, cursor snaps to grid
- **Minimal clicks** – tower place = 2 clicks (empty tile → tower icon), upgrade = 2 clicks (tower → upgrade icon)

---

## 13. Virtual Resolution & Safe Zones

```
480 x 270 virtual pixels (16:9, scales to any resolution)

┌────────────────────────────────────────────────────────────┐
│ [HUD BAR - 480 x 16px]                                     │  ← Top bar
│  ♥ 20  |  ⚡ Wave 3/10  |  🪙 450  |  ⏸ ⏩              │
├────────────────────────────────────────────────────────────┤
│                                                              │
│                                                              │
│                    GAME MAP                                  │
│                    438 x 254 px                               │
│                    (full playfield)                           │
│                                                              │
│                                                              │
│                                                              │
│                                                              │
│                                                              │
│                                                              │
│                                                              │
│                                                              │
│                                                              │
│                                                              │
│                                                              │
└────────────────────────────────────────────────────────────┘
```

Top bar is the only permanent UI. Everything else is contextual (appears on interaction, fades when not needed).

---

## 14. Screen Flow

```
┌──────────┐     ┌──────────────┐     ┌──────────────┐
│  Title   │────▶│  World Map   │────▶│ Level Select │
│  Screen  │     │  (6 worlds)  │     │  (per world) │
└──────────┘     └──────────────┘     └──────────────┘
                                              │
                                              ▼
┌──────────┐     ┌──────────────┐     ┌──────────────┐
│  Result  │◀────│   IN-GAME    │◀────│  Pre-Level   │
│  Screen  │     │   (TD HUD)   │     │  (briefing)  │
└──────────┘     └──────────────┘     └──────────────┘
                        │
                        ▼
                 ┌──────────────┐
                 │  Pause Menu  │
                 └──────────────┘
```

---

## 15. Title Screen

Minimal, atmospheric. Full-screen pixel art scene that slowly animates (parallax clouds, flickering torches, etc.). Changes based on last played world or cycles through all six.

```
┌────────────────────────────────────────────────────────┐
│                                                          │
│                                                          │
│              ░█▀█░█▄█░█░█▀▀░█▀█                        │
│              ░█▀█░█░█░█░█░█░█░█                        │
│              ░▀░▀░▀░▀░▀░▀▀▀░▀▀▀                        │
│                     T D                                  │
│                                                          │
│                                                          │
│               ▸ Continue                                 │
│                 New Game                                  │
│                 Settings                                  │
│                                                          │
│                                                          │
│                                          v0.1.0          │
└────────────────────────────────────────────────────────┘
```

Gamepad: D-pad selects, A/Enter confirms. No complex navigation.

---

## 16. World Map

Stylized pixel art overview showing all 6 worlds as islands/regions on a fantasy map. Locked worlds are greyed out with a lock icon. Current world pulses subtly.

```
┌────────────────────────────────────────────────────────┐
│                                                          │
│     🏴‍☠️                          ⚔️                       │
│    Caribbean               Lord of the Rings             │
│    ★★★☆☆                   ★★☆☆☆                        │
│                                                          │
│            🏜️                          💻                 │
│           Dune                      Matrix               │
│           🔒                         🔒                   │
│                                                          │
│        🐉                           👾                    │
│    Game of Thrones           Stranger Things             │
│        🔒                         🔒                      │
│                                                          │
│  [← Back]                                                │
└────────────────────────────────────────────────────────┘
```

Stars show completion progress (out of 5 levels per world). Worlds unlock sequentially or by completing N levels total.

---

## 17. Level Select

Per-world screen. Shows the world's pixel art landscape as background with level nodes connected by a path (like Kingdom Rush's level select). Each node shows star rating (0-3 stars).

```
┌────────────────────────────────────────────────────────┐
│  ◀ Caribbean                                             │
│                                                          │
│     ①───②───③───④───⑤                                  │
│     ★★★  ★★☆  ★☆☆  ·    ·                               │
│                                                          │
│  ┌──────────────────────────┐                            │
│  │  Level 3: Skull Cove     │                            │
│  │  ★☆☆☆  Waves: 8         │                            │
│  │  Difficulty: ██░░░       │                            │
│  │                          │                            │
│  │  New towers: Kraken      │                            │
│  │  New enemies: Ghost Ship │                            │
│  │                          │                            │
│  │  [▸ PLAY]    [Retry ★★★] │                            │
│  └──────────────────────────┘                            │
└────────────────────────────────────────────────────────┘
```

Selecting a level shows an info card with wave count, difficulty, new unlocks, and best star rating. "Retry ★★★" option for perfectionists.

---

## 18. Pre-Level Briefing

Quick screen before gameplay starts. Shows the map preview, wave count, available towers, and any special mechanics for this level.

```
┌────────────────────────────────────────────────────────┐
│  Skull Cove                              Caribbean #3   │
│                                                          │
│  ┌──────────────┐  Waves: 8                              │
│  │              │  Gold: 300                              │
│  │  [Map        │  Lives: 20                              │
│  │   Preview]   │                                        │
│  │              │  Special:                               │
│  └──────────────┘  Water tiles slow enemies               │
│                                                          │
│  Towers:  🔫  🏹  🔮  🐙                                  │
│           Cannon Archer Mage Kraken                      │
│                                                          │
│  ★ Survive all waves                                     │
│  ★★ Lose fewer than 5 lives                              │
│  ★★★ Lose no lives                                       │
│                                                          │
│              [▸ START]        [◀ Back]                    │
└────────────────────────────────────────────────────────┘
```

---

## 19. In-Game HUD

The core gameplay UI. Minimal – top bar + contextual radial menus.

### 19.1 Top Bar (always visible, 480x16 px)

```
┌────────────────────────────────────────────────────────┐
│ ♥20  │ Wave 3/10 ████░░░░░░ │  🪙 450  │  ⏸  ▶▶  ⚙  │
└────────────────────────────────────────────────────────┘
  Lives   Wave + progress bar    Gold    Pause Speed Settings
```

- **Lives**: Heart icon + number. Pulses red when hit, flashes when ≤3.
- **Wave**: "Wave N/M" + progress bar showing enemies remaining in current wave.
- **Gold**: Coin icon + amount. Flashes green on income, flashes red when insufficient for action.
- **Speed**: Toggle 1x / 2x / 3x game speed. Current speed highlighted.
- **Pause**: Opens pause menu.

Between waves, a "NEXT WAVE" button replaces the progress bar area:

```
│ ♥20  │ ▸ NEXT WAVE (5s)     │  🪙 450  │  ⏸  ▶▶  ⚙  │
```

Countdown timer shows auto-start. Click to start immediately (early start = gold bonus).

### 19.2 Wave Banner (transient)

On wave start, a banner slides in from the top for ~2 seconds:

```
         ┌─────────────────────┐
         │   ⚔ WAVE 3 ⚔       │
         │   Skeleton Pirates   │
         └─────────────────────┘
```

Boss waves get a larger, dramatic banner:

```
    ┌─────────────────────────────┐
    │     ☠ BOSS WAVE ☠          │
    │   Captain Blackbeard        │
    │   "Prepare to be boarded!"  │
    └─────────────────────────────┘
```

### 19.3 Empty Tile Interaction (Radial Build Menu)

Click/tap an empty buildable tile → radial menu appears centered on the tile:

```
              🏹
             Archer
             (50g)

    🔫                    🔮
   Cannon                Mage
   (75g)                 (100g)

              🐙
             Kraken
             (120g)
```

- 4-direction radial: Up/Right/Down/Left (maps to D-pad perfectly)
- Each slot shows: tower icon, name, cost
- Greyed out if not enough gold
- Hover/focus shows tower range preview on the map (transparent circle)
- Click icon or press D-pad direction → tower placed instantly
- Click outside or press B/Escape → cancel

**Tower range preview**: While the radial menu is open and a tower is focused, a semi-transparent circle shows the range on the map. This is the single most important visual feedback for placement decisions.

### 19.4 Tower Interaction (Radial Action Menu)

Click/tap an existing tower → radial action menu:

```
              ⬆
            Upgrade
            (150g)

    🎯                    💰
   Target               Sell
   Priority             (37g)
   [First]

              ℹ
             Info
```

- **Upgrade** (top): Shows cost. If tower has branching upgrades, a sub-menu appears.
- **Target** (left): Cycles through targeting modes (First → Last → Nearest → Strongest → Weakest → Fastest). Shows current mode.
- **Sell** (right): Shows refund amount (50% of total invested). Confirm on click.
- **Info** (bottom): Opens tower info panel.

### 19.5 Tower Info Panel

Opens on info action or long-press on tower. Compact panel near the tower:

```
┌─────────────────────────────┐
│  🔫 Cannon Lv.2             │
│                              │
│  DMG: 45    RNG: 4.5        │
│  SPD: 1.2s  TGT: Strongest  │
│                              │
│  Kills: 23                   │
│  Total Damage: 1,840         │
│                              │
│  ⬆ Upgrade Path:            │
│  [✓] Iron Balls → [✓] Grape │
│  → [ ] Explosive (300g)     │
│                              │
│  [Close]                     │
└─────────────────────────────┘
```

Shows stats, kill counter, damage dealt, and upgrade path progress.

### 19.6 Enemy HP Bars

Small HP bar above each enemy. Only visible when damaged:

```
    ████░░  ← green/yellow/red based on %
    👹      ← enemy sprite
```

Boss enemies get a larger bar pinned to the top of the screen:

```
┌────────────────────────────────────────────────────────┐
│ ♥20 │ ☠ Cpt. Blackbeard ████████████░░░░ │ 🪙 450 │...│
└────────────────────────────────────────────────────────┘
```

### 19.7 Damage Numbers (floating)

On hit, small floating numbers rise from the impact point:

```
    -45        ← white for normal
    -120       ← yellow for critical
    POISON     ← green for status effects
    IMMUNE     ← grey for immune
```

Pixel font, fades out over ~0.5s while floating upward.

### 19.8 Gold Income Feedback

When enemies die, floating "+10" gold numbers near the kill point. When selling, "+37" near the tower.

### 19.9 Build Zone Highlighting

When the build radial is open, all valid build tiles glow subtly (pulsing green outline). Invalid tiles (paths, water, occupied) stay normal.

---

## 20. Pause Menu

Overlay with slight darken/blur on the game behind:

```
┌────────────────────────────────────────────────────────┐
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░┌──────────────────────────────┐░░░░░░░░░░░░░░░░░░│
│░░░░░░│          PAUSED              │░░░░░░░░░░░░░░░░░░│
│░░░░░░│                              │░░░░░░░░░░░░░░░░░░│
│░░░░░░│     ▸ Resume                 │░░░░░░░░░░░░░░░░░░│
│░░░░░░│       Restart Level          │░░░░░░░░░░░░░░░░░░│
│░░░░░░│       Settings               │░░░░░░░░░░░░░░░░░░│
│░░░░░░│       Quit to Map            │░░░░░░░░░░░░░░░░░░│
│░░░░░░│                              │░░░░░░░░░░░░░░░░░░│
│░░░░░░│  Wave 3/10  Gold: 450        │░░░░░░░░░░░░░░░░░░│
│░░░░░░│  Time: 4:23  Kills: 47       │░░░░░░░░░░░░░░░░░░│
│░░░░░░└──────────────────────────────┘░░░░░░░░░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
└────────────────────────────────────────────────────────┘
```

---

## 21. Result Screen

### Victory

```
┌────────────────────────────────────────────────────────┐
│                                                          │
│               ⚔ VICTORY ⚔                               │
│                                                          │
│              ★ ★ ★                                       │
│          (all 3 stars earned!)                            │
│                                                          │
│    Lives: 18/20     Gold: 1,240                          │
│    Kills: 87        Time: 6:42                           │
│    Best Tower: Cannon Lv.3 (412 kills)                   │
│                                                          │
│    ┌──────────────────────────────┐                      │
│    │  NEW UNLOCK: Kraken Tower!   │                      │
│    │  Available in next level     │                      │
│    └──────────────────────────────┘                      │
│                                                          │
│         [▸ Next Level]   [Retry]   [Map]                 │
│                                                          │
└────────────────────────────────────────────────────────┘
```

### Defeat

```
┌────────────────────────────────────────────────────────┐
│                                                          │
│              ☠ DEFEAT ☠                                  │
│                                                          │
│        You survived to Wave 6/10                         │
│                                                          │
│    Lives: 0/20      Gold: 890                            │
│    Kills: 54        Time: 4:11                           │
│    Best Tower: Archer Lv.2 (203 kills)                   │
│                                                          │
│    TIP: Try placing towers near                          │
│    the path's curve for maximum                          │
│    coverage.                                             │
│                                                          │
│         [▸ Retry]    [Map]                               │
│                                                          │
└────────────────────────────────────────────────────────┘
```

Defeat screen shows a contextual tip based on what went wrong (e.g., "enemies leaked at the second curve" → suggest tower placement there).

---

## 22. Settings Screen

Accessible from title screen and pause menu:

```
┌────────────────────────────────────────────────────────┐
│  ⚙ Settings                                             │
│                                                          │
│  Audio                                                   │
│    Master     ████████░░  80%                            │
│    Music      ██████░░░░  60%                            │
│    SFX        ████████░░  80%                            │
│    Ambient    ██████░░░░  60%                            │
│                                                          │
│  Display                                                 │
│    Fullscreen        [ON]                                │
│    Resolution        [1920x1080]                         │
│    Pixel Scaling     [Integer]                           │
│    Show FPS          [OFF]                               │
│                                                          │
│  Gameplay                                                │
│    Auto-Start Waves  [OFF]                               │
│    Damage Numbers    [ON]                                │
│    Show HP Bars      [Always / Damaged / Never]          │
│    Confirm Sell      [ON]                                │
│                                                          │
│  Controls                                                │
│    [View Keybinds]   [Rebind]                            │
│                                                          │
│  [◀ Back]                                                │
└────────────────────────────────────────────────────────┘
```

---

## 23. World-Themed UI Frames

Every panel, button, and frame has world-specific art. Same layout, different skin:

| World | Frame Style | Primary Color | Accent | Font Feel |
|-------|------------|---------------|--------|-----------|
| Caribbean | Weathered wood planks, rope borders | Warm brown | Gold | Rugged serif |
| Lord of the Rings | Carved stone, elvish vine borders | Dark grey | Emerald | Elegant |
| Dune | Sandstone, geometric patterns | Sandy beige | Spice orange | Angular |
| Matrix | Black glass, neon green scan lines | Black | Neon green | Monospace |
| Game of Thrones | Iron/steel, riveted borders | Dark iron | Ice blue / fire red | Medieval |
| Stranger Things | 80s wood panel, Christmas lights | Brown/cream | Red flickering | Retro rounded |

The UI frame sprites are swapped when entering a world. Same layout, different atlas. This means every UI element needs to be designed once as a template, then skinned 6 times.

### Frame 9-Slice System

All panels use 9-slice sprites (corners + edges + fill):

```
┌─────────────┐
│ TL │ T │ TR │    TL, TR, BL, BR = fixed corner sprites
├────┼───┼────┤    T, B, L, R = repeating edge sprites
│ L  │ F │ R  │    F = repeating fill sprite
├────┼───┼────┤
│ BL │ B │ BR │    Total: 9 sprites per frame style
└────┴───┴────┘    × 6 worlds = 54 frame sprites
```

---

## 24. Tower Upgrade UI

When a tower has branching upgrades, the upgrade radial expands into a mini upgrade tree:

### Linear Upgrades (Phase 1, simpler)

```
    Tower Lv.1 → Lv.2 (100g) → Lv.3 (200g) → MAX

    Shown in radial as:
              ⬆
         Next Upgrade
           (100g)
           Lv.2
```

### Branching Upgrades (Phase 2+)

Two paths, can only pick one after a branch point:

```
              ⬆ Path A
          Explosive Shots
             (300g)

    ← Back              Path B ▶
                      Rapid Fire
                        (250g)
```

Selecting a path locks the other. The info panel shows both paths with the locked one greyed out.

---

## 25. Wave Preview

Before each wave (during the between-wave pause), a small preview appears in the top bar area:

```
│ ♥20  │ NEXT: 👹×8 🦴×4 💀×2  [▸ START (5s)]  │ 🪙 450 │
```

Icons show enemy types and counts for the upcoming wave. Players can plan tower placement based on what's coming.

For boss waves, a special preview:

```
│ ♥20  │ ⚠ BOSS: ☠ Captain Blackbeard  [▸ START]  │ 🪙 450 │
```

---

## 26. Notifications & Toasts

Small, transient messages for game events:

```
    ┌──────────────────┐
    │ Tower upgraded!   │    ← slides in from right, fades after 2s
    └──────────────────┘

    ┌──────────────────┐
    │ ⚠ Life lost!     │    ← red tint, shakes slightly
    └──────────────────┘

    ┌──────────────────┐
    │ 🪙 Early bonus!  │    ← gold tint
    │ +50 gold         │
    └──────────────────┘
```

Stack from top-right, max 3 visible at once, oldest fades first.

---

## 27. Gamepad Controls

Full gamepad support. Cursor is a highlighted tile selector that snaps to the grid.

| Input | Action |
|-------|--------|
| Left Stick / D-pad | Move tile cursor |
| A (confirm) | Select tile → open radial / confirm action |
| B (back) | Cancel radial / close panel / back |
| X | Quick-sell selected tower |
| Y | Cycle target priority on selected tower |
| RB | Next wave / speed toggle |
| LB | Zoom (if applicable) |
| Start | Pause menu |
| Triggers | Scroll tower list (if more than 4 towers) |

The radial menu maps directly to D-pad: Up = top tower, Right = right tower, etc. No analog aiming needed.

---

## 28. Keyboard + Mouse Controls

| Input | Action |
|-------|--------|
| Left Click | Select tile / confirm action |
| Right Click | Cancel / deselect |
| Mouse Wheel | Zoom |
| 1-9 | Quick-select tower type |
| Space | Start next wave |
| S | Sell selected tower |
| T | Cycle target priority |
| F | Toggle speed (1x / 2x / 3x) |
| Escape | Pause |

Number keys for quick tower placement: press "1" then click a tile to instantly place the first tower without opening the radial menu. Power-user shortcut.

---

## 29. Accessibility

- **Color-blind mode**: Enemy types distinguished by shape + icon, not just color. Status effects use both color and a text label.
- **Font size**: Bitmap font rendered at 1x and 2x for key numbers (lives, gold). Option to toggle large HUD.
- **Screen reader hints**: All UI elements have text labels (for future TTS support).
- **Auto-pause**: Game pauses when window loses focus.
- **Speed control**: 0.5x speed option for players who need more time.

---

## 30. Animation & Juice

### UI Feedback

- **Button press**: 1px down-shift + slight darken on click
- **Tower placement**: Brief flash + small particle burst at tile
- **Tower sell**: Tower sprite shrinks to nothing over 0.3s + gold particles float to HUD
- **Upgrade**: Tower sprite flashes white, then swaps to upgraded version with brief glow
- **Life lost**: Screen edge flashes red, lives counter shakes, enemy that leaked briefly highlighted
- **Wave clear**: Brief "WAVE CLEAR" text, gold tally animation
- **Gold change**: Numbers in HUD count up/down smoothly (not instant jump)

### Screen Transitions

- **Title → World Map**: Fade to black, 0.3s
- **World Map → Level**: Camera zooms into the selected world, crossfade
- **Level Select → Game**: Map swoops in from the level node position
- **Victory/Defeat**: Game dims, result panel slides up from bottom

---

## 31. Mockup: Full In-Game Frame

```
┌────────────────────────────────────────────────────────────┐
│ ♥ 17  │ Wave 5/10 █████░░░░░ │  🪙 320  │  ⏸  ▶▶  ⚙    │
├────────────────────────────────────────────────────────────┤
│                                                              │
│    ~~  ~~  🌴  ~~  ~~  ~~  🌴  ~~  ~~  ~~                  │
│    ~~  ~~  ░░  ░░  ░░  ~~  ~~  ~~  ~~  ~~                  │
│    ~~  ~~  ░░  🏹  ░░  ░░  ~~  ~~  ~~  ~~                  │
│    ~~  ~~  ░░  ░░  ░░  ░░  ░░  ~~  ~~  ~~                  │
│    🌊  ~~  ~~  ░░  🔫  ░░  ░░  ~~  🌴  ~~                  │
│    🌊  🌊  ~~  ░░  ░░  ░░  ░░  ░░  ~~  ~~                  │
│    🌊  🌊  ~~  ~~  ░░  ░░  🔮  ░░  ~~  ~~                  │
│    🌊  ~~  ~~  ~~  ~~  ░░  ░░  ░░  ~~  ~~                  │
│    ~~  ~~  ~~  ~~  ~~  ~~  ░░  ░░  ~~  ~~                  │
│    ~~  ~~  🌴  ~~  ~~  ~~  ~~  ░░  ~~  ~~   💀💀💀→       │
│    ~~  ~~  ~~  ~~  ~~  ~~  ~~  ░░  ~~  ~~    (enemies)     │
│                                                              │
│                  ┌─────┐                                     │
│                  │ -45  │  ← damage number floating          │
│                  └─────┘                                     │
│                                         +10 🪙               │
│                                                              │
│                        [Selected: 🔫 Cannon Lv.2]           │
│                  ┌ 🎯 ─── ⬆ ─── 💰 ┐                       │
│                  │Target  Upg   Sell │  ← radial on tower    │
│                  └ ────── ℹ ──── ───┘                        │
└────────────────────────────────────────────────────────────┘
```

---

## 32. UI Asset Requirements

### Per World (×6)

| Asset | Count | Size | Notes |
|-------|-------|------|-------|
| 9-slice frame | 9 sprites | 8×8 corners, 8×1 edges | Panel backgrounds |
| Button (normal, hover, pressed, disabled) | 4 states | 32×12 | Standard button |
| Tower icons | per tower type | 16×16 | Radial menu + HUD |
| Enemy icons | per enemy type | 12×12 | Wave preview |
| Star (empty, filled) | 2 | 8×8 | Level select |
| Heart icon | 1 | 8×8 | Lives |
| Coin icon | 1 | 8×8 | Gold |
| Wave banner | 1 | 160×24 | World-themed |
| Cursor/selector | 1 | 16×16 | Tile highlight |

### Shared (×1)

| Asset | Count | Size |
|-------|-------|------|
| Bitmap font (Press Start 2P or similar) | 1 atlas | variable |
| Speed icons (1x, 2x, 3x) | 3 | 12×12 |
| Pause icon | 1 | 12×12 |
| Settings gear | 1 | 12×12 |
| Radial menu ring | 1 | 64×64 |
| HP bar (background + fill) | 2 | 16×2 |
| Damage number font | 1 atlas | variable |
| Toast notification frame | 9-slice | 8×8 |

### Total Estimate

~120-150 unique UI sprites per world, ~50 shared sprites. With 6 worlds: ~800 total UI sprites. All 16-color palette per world, consistent with 03-asset-pipeline-spec style definitions.

---

*For the engine, see 01-engine-spec.md. For asset generation (art + audio), see 03-asset-pipeline-spec.md.*
