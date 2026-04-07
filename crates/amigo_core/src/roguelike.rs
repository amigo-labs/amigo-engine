use crate::math::{Fix, RenderVec2};
use crate::save::{SaveError, SaveManager};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Dungeon generation — rooms + corridors
// ---------------------------------------------------------------------------

/// A rectangular room in a dungeon.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Room {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub room_type: RoomType,
    pub connected_to: Vec<usize>,
}

impl Room {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            room_type: RoomType::Normal,
            connected_to: Vec::new(),
        }
    }

    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    pub fn intersects(&self, other: &Room, padding: i32) -> bool {
        self.x - padding < other.x + other.width + padding
            && self.x + self.width + padding > other.x - padding
            && self.y - padding < other.y + other.height + padding
            && self.y + self.height + padding > other.y - padding
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    pub fn world_center(&self, tile_size: f32) -> RenderVec2 {
        let (cx, cy) = self.center();
        RenderVec2::new(
            cx as f32 * tile_size + tile_size * 0.5,
            cy as f32 * tile_size + tile_size * 0.5,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomType {
    Normal,
    Spawn,
    Elite,
    Boss,
    Treasure,
    Shop,
    Secret,
}

/// A corridor connecting two rooms.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corridor {
    pub points: Vec<(i32, i32)>,
}

/// Result of dungeon generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dungeon {
    pub width: i32,
    pub height: i32,
    pub rooms: Vec<Room>,
    pub corridors: Vec<Corridor>,
    pub tiles: Vec<DungeonTile>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DungeonTile {
    Wall,
    Floor,
    Door,
    Corridor,
}

impl Dungeon {
    pub fn get_tile(&self, x: i32, y: i32) -> DungeonTile {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return DungeonTile::Wall;
        }
        self.tiles[(y * self.width + x) as usize]
    }

    pub fn spawn_room(&self) -> Option<&Room> {
        self.rooms.iter().find(|r| r.room_type == RoomType::Spawn)
    }

    pub fn boss_room(&self) -> Option<&Room> {
        self.rooms.iter().find(|r| r.room_type == RoomType::Boss)
    }

    /// Get rooms by type.
    pub fn rooms_of_type(&self, room_type: RoomType) -> Vec<&Room> {
        self.rooms
            .iter()
            .filter(|r| r.room_type == room_type)
            .collect()
    }
}

/// Configuration for dungeon generation.
#[derive(Clone, Debug)]
pub struct DungeonConfig {
    pub width: i32,
    pub height: i32,
    pub min_room_size: i32,
    pub max_room_size: i32,
    pub max_rooms: usize,
    pub room_padding: i32,
    pub extra_connections: f32,
}

impl Default for DungeonConfig {
    fn default() -> Self {
        Self {
            width: 64,
            height: 64,
            min_room_size: 5,
            max_room_size: 12,
            max_rooms: 15,
            room_padding: 1,
            extra_connections: 0.15,
        }
    }
}

struct DungeonRng(u64);

impl DungeonRng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        min + (self.next() % (max - min + 1) as u64) as i32
    }

    fn f32(&mut self) -> f32 {
        (self.next() & 0x00FF_FFFF) as f32 / 16_777_216.0
    }
}

/// Generate a dungeon with rooms and corridors.
pub fn generate_dungeon(config: &DungeonConfig, seed: u64) -> Dungeon {
    let mut rng = DungeonRng::new(seed);
    let mut rooms = Vec::new();
    let mut tiles = vec![DungeonTile::Wall; (config.width * config.height) as usize];

    // Place rooms
    for _ in 0..config.max_rooms * 5 {
        if rooms.len() >= config.max_rooms {
            break;
        }

        let w = rng.range_i32(config.min_room_size, config.max_room_size);
        let h = rng.range_i32(config.min_room_size, config.max_room_size);
        let x = rng.range_i32(1, config.width - w - 1);
        let y = rng.range_i32(1, config.height - h - 1);

        let room = Room::new(x, y, w, h);

        let overlaps = rooms
            .iter()
            .any(|r: &Room| r.intersects(&room, config.room_padding));
        if !overlaps {
            rooms.push(room);
        }
    }

    if rooms.is_empty() {
        return Dungeon {
            width: config.width,
            height: config.height,
            rooms,
            corridors: Vec::new(),
            tiles,
        };
    }

    // Carve room floors
    for room in &rooms {
        for ry in room.y..room.y + room.height {
            for rx in room.x..room.x + room.width {
                if rx > 0 && rx < config.width && ry > 0 && ry < config.height {
                    tiles[(ry * config.width + rx) as usize] = DungeonTile::Floor;
                }
            }
        }
    }

    // Connect rooms with L-shaped corridors (minimum spanning tree style)
    let mut connected: FxHashSet<usize> = FxHashSet::default();
    let mut corridors = Vec::new();
    connected.insert(0);

    #[allow(clippy::needless_range_loop)]
    while connected.len() < rooms.len() {
        let mut best_from = 0;
        let mut best_to = 0;
        let mut best_dist = i64::MAX;

        for &from in &connected {
            let (fx, fy) = rooms[from].center();
            for to in 0..rooms.len() {
                if connected.contains(&to) {
                    continue;
                }
                let (tx, ty) = rooms[to].center();
                let dist = ((tx - fx) as i64).abs() + ((ty - fy) as i64).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_from = from;
                    best_to = to;
                }
            }
        }

        let corr = carve_corridor(
            &rooms[best_from],
            &rooms[best_to],
            &mut tiles,
            config.width,
            &mut rng,
        );
        corridors.push(corr);
        rooms[best_from].connected_to.push(best_to);
        rooms[best_to].connected_to.push(best_from);
        connected.insert(best_to);
    }

    // Extra random connections
    for i in 0..rooms.len() {
        for j in i + 1..rooms.len() {
            if rooms[i].connected_to.contains(&j) {
                continue;
            }
            if rng.f32() < config.extra_connections {
                let corr = carve_corridor(&rooms[i], &rooms[j], &mut tiles, config.width, &mut rng);
                corridors.push(corr);
                rooms[i].connected_to.push(j);
                rooms[j].connected_to.push(i);
            }
        }
    }

    // Assign room types
    if !rooms.is_empty() {
        rooms[0].room_type = RoomType::Spawn;

        // Boss room = furthest from spawn
        let (sx, sy) = rooms[0].center();
        let boss_idx = rooms
            .iter()
            .enumerate()
            .skip(1)
            .max_by_key(|(_, r)| {
                let (cx, cy) = r.center();
                ((cx - sx).abs() + (cy - sy).abs()) as i64
            })
            .map(|(i, _)| i)
            .unwrap_or(rooms.len() - 1);
        rooms[boss_idx].room_type = RoomType::Boss;

        // Random treasure rooms
        #[allow(clippy::needless_range_loop)]
        for i in 1..rooms.len() {
            if i == boss_idx {
                continue;
            }
            if rng.f32() < 0.2 {
                rooms[i].room_type = RoomType::Treasure;
            } else if rng.f32() < 0.1 {
                rooms[i].room_type = RoomType::Shop;
            }
        }
    }

    Dungeon {
        width: config.width,
        height: config.height,
        rooms,
        corridors,
        tiles,
    }
}

fn carve_corridor(
    from: &Room,
    to: &Room,
    tiles: &mut [DungeonTile],
    width: i32,
    rng: &mut DungeonRng,
) -> Corridor {
    let (fx, fy) = from.center();
    let (tx, ty) = to.center();
    let mut points = Vec::new();

    let (mid_x, mid_y) = if rng.f32() < 0.5 {
        // Horizontal first
        (tx, fy)
    } else {
        // Vertical first
        (fx, ty)
    };

    // Carve horizontal segment from (fx, fy) to (mid_x, fy)
    let (start_x, end_x) = if fx < mid_x { (fx, mid_x) } else { (mid_x, fx) };
    for x in start_x..=end_x {
        set_corridor(tiles, x, fy, width);
        points.push((x, fy));
    }

    // Carve vertical segment from (mid_x, fy) to (mid_x, mid_y)
    let (start_y, end_y) = if fy < mid_y { (fy, mid_y) } else { (mid_y, fy) };
    for y in start_y..=end_y {
        set_corridor(tiles, mid_x, y, width);
        points.push((mid_x, y));
    }

    // Carve remaining segment to target
    if mid_x != tx {
        let (sx, ex) = if mid_x < tx { (mid_x, tx) } else { (tx, mid_x) };
        for x in sx..=ex {
            set_corridor(tiles, x, mid_y, width);
            points.push((x, mid_y));
        }
    }
    if mid_y != ty {
        let (sy, ey) = if mid_y < ty { (mid_y, ty) } else { (ty, mid_y) };
        for y in sy..=ey {
            set_corridor(tiles, tx, y, width);
            points.push((tx, y));
        }
    }

    Corridor { points }
}

fn set_corridor(tiles: &mut [DungeonTile], x: i32, y: i32, width: i32) {
    let idx = (y * width + x) as usize;
    if idx < tiles.len() && tiles[idx] == DungeonTile::Wall {
        tiles[idx] = DungeonTile::Corridor;
    }
}

// ---------------------------------------------------------------------------
// Run / Permadeath system
// ---------------------------------------------------------------------------

/// A roguelike run (one playthrough attempt).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub seed: u64,
    pub floor: u32,
    pub max_floor: u32,
    pub gold: u32,
    pub score: u32,
    pub kills: u32,
    pub items_collected: u32,
    pub run_time_secs: f64,
    pub active: bool,
}

impl Run {
    pub fn new(seed: u64, max_floor: u32) -> Self {
        Self {
            seed,
            floor: 1,
            max_floor,
            gold: 0,
            score: 0,
            kills: 0,
            items_collected: 0,
            run_time_secs: 0.0,
            active: true,
        }
    }

    /// Advance to the next floor. Returns true if there are more floors.
    pub fn next_floor(&mut self) -> bool {
        if self.floor < self.max_floor {
            self.floor += 1;
            true
        } else {
            false
        }
    }

    /// Generate a floor-specific seed.
    pub fn floor_seed(&self) -> u64 {
        let mut s = self
            .seed
            .wrapping_add(self.floor as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        s ^= s >> 30;
        s = s.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        s ^= s >> 27;
        s = s.wrapping_mul(0x94D0_49BB_1331_11EB);
        s ^= s >> 31;
        if s == 0 {
            s = 1;
        }
        s
    }

    pub fn end_run(&mut self) {
        self.active = false;
    }

    pub fn is_boss_floor(&self) -> bool {
        self.floor == self.max_floor || self.floor.is_multiple_of(5)
    }
}

// ---------------------------------------------------------------------------
// Item pool / weighted random selection
// ---------------------------------------------------------------------------

/// An entry in an item pool.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolEntry {
    pub id: u32,
    pub weight: f32,
    pub rarity_tier: u32,
    /// Maximum times this can appear per run (0 = unlimited).
    pub max_per_run: u32,
}

/// A pool of items for random selection (chests, rewards, shops).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemPool {
    pub entries: Vec<PoolEntry>,
}

impl ItemPool {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn with_entry(mut self, id: u32, weight: f32, rarity: u32) -> Self {
        self.entries.push(PoolEntry {
            id,
            weight,
            rarity_tier: rarity,
            max_per_run: 0,
        });
        self
    }

    pub fn with_limited_entry(mut self, id: u32, weight: f32, rarity: u32, max: u32) -> Self {
        self.entries.push(PoolEntry {
            id,
            weight,
            rarity_tier: rarity,
            max_per_run: max,
        });
        self
    }

    /// Pick a random item from the pool, excluding already-picked items that hit their limit.
    pub fn pick(
        &self,
        seed: u64,
        picked_counts: &std::collections::HashMap<u32, u32>,
    ) -> Option<u32> {
        let available: Vec<&PoolEntry> = self
            .entries
            .iter()
            .filter(|e| {
                if e.max_per_run == 0 {
                    return true;
                }
                let count = picked_counts.get(&e.id).copied().unwrap_or(0);
                count < e.max_per_run
            })
            .collect();

        if available.is_empty() {
            return None;
        }

        let total: f32 = available.iter().map(|e| e.weight).sum();
        if total <= 0.0 {
            return None;
        }

        let mut rng = seed;
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let mut roll = (rng & 0x00FF_FFFF) as f32 / 16_777_216.0 * total;

        for entry in &available {
            roll -= entry.weight;
            if roll <= 0.0 {
                return Some(entry.id);
            }
        }

        available.last().map(|e| e.id)
    }

    /// Pick multiple unique items.
    pub fn pick_multiple(&self, count: usize, seed: u64) -> Vec<u32> {
        let mut results = Vec::new();
        let mut counts = std::collections::HashMap::new();
        let mut current_seed = seed;

        for _ in 0..count {
            current_seed ^= current_seed << 13;
            current_seed ^= current_seed >> 7;
            current_seed ^= current_seed << 17;

            if let Some(id) = self.pick(current_seed, &counts) {
                *counts.entry(id).or_insert(0) += 1;
                results.push(id);
            }
        }

        results
    }
}

impl Default for ItemPool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Meta-progression (persistent unlocks across runs)
// ---------------------------------------------------------------------------

/// Persistent meta-progression state (saved between runs).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MetaProgress {
    /// Currency earned across runs for permanent upgrades.
    pub meta_currency: u32,
    /// Unlocked item IDs (available in future item pools).
    pub unlocked_items: Vec<u32>,
    /// Unlocked characters/classes.
    pub unlocked_characters: Vec<u32>,
    /// Total runs completed.
    pub total_runs: u32,
    /// Total wins.
    pub total_wins: u32,
    /// Best score.
    pub best_score: u32,
    /// Best floor reached.
    pub best_floor: u32,
    /// Achievement flags.
    pub achievements: Vec<String>,
}

impl MetaProgress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_run(&mut self, run: &Run, won: bool) {
        self.total_runs += 1;
        if won {
            self.total_wins += 1;
        }
        self.best_score = self.best_score.max(run.score);
        self.best_floor = self.best_floor.max(run.floor);
    }

    pub fn unlock_item(&mut self, id: u32) {
        if !self.unlocked_items.contains(&id) {
            self.unlocked_items.push(id);
        }
    }

    pub fn unlock_character(&mut self, id: u32) {
        if !self.unlocked_characters.contains(&id) {
            self.unlocked_characters.push(id);
        }
    }

    pub fn add_achievement(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.achievements.contains(&name) {
            self.achievements.push(name);
        }
    }
}

// ---------------------------------------------------------------------------
// XorShift64 PRNG — deterministic, forkable
// ---------------------------------------------------------------------------

/// XorShift64 PRNG used for all roguelike randomness.
/// Shifts: 13, 7, 17 — matching the bullet-pattern RNG.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct XorShift64 {
    pub state: u64,
}

impl XorShift64 {
    /// Create a new PRNG from the given seed. Seed of 0 is replaced with a
    /// non-zero constant (the algorithm requires a non-zero state).
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xDEAD_BEEF } else { seed },
        }
    }

    /// Advance and return the next raw u64 value.
    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// Return a float in `[0.0, 1.0)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() & 0x00FF_FFFF) as f32 / 16_777_216.0
    }

    /// Return an integer in `[min, max]` (inclusive).
    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        min + (self.next_u64() % (max - min + 1) as u64) as i32
    }

    /// Create a child RNG with a deterministic seed derived from the parent.
    /// Uses the parent's current state XORed with a domain tag so that
    /// different sub-systems get different sequences.
    /// **Does NOT advance the parent RNG's state.**
    pub fn fork(&self, domain: u64) -> XorShift64 {
        XorShift64::new(self.state ^ domain)
    }

    /// Peek at the next `count` random values without advancing state.
    pub fn peek(&self, count: usize) -> Vec<u64> {
        let mut clone = self.clone();
        (0..count).map(|_| clone.next_u64()).collect()
    }
}

// ---------------------------------------------------------------------------
// DeathMode / DeathCause
// ---------------------------------------------------------------------------

/// Permadeath mode for a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathMode {
    /// Full reset: all run progress lost on death.
    Hard,
    /// Partial reset: keep a percentage of currency on death.
    Soft {
        /// Percentage of currency retained (0-100).
        currency_retain_pct: u8,
    },
}

/// How the player died.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeathCause {
    Enemy { enemy_id: String },
    Boss { boss_id: String },
    Trap { trap_type: String },
    CursedItem { item_id: String },
    Environmental,
}

// ---------------------------------------------------------------------------
// PlayerStats / StatModifier
// ---------------------------------------------------------------------------

/// Player stats that can be modified by items and meta-progression.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerStats {
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: Fix,
    pub crit_chance: Fix,
    pub crit_multiplier: Fix,
    pub luck: i32,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            max_hp: 100,
            attack: 10,
            defense: 5,
            speed: Fix::from_num(1),
            crit_chance: Fix::from_num(0.05f32),
            crit_multiplier: Fix::from_num(1.5f32),
            luck: 0,
        }
    }
}

impl PlayerStats {
    /// Apply a stat modifier (additive).
    pub fn apply_modifier(&mut self, modifier: &StatModifier) {
        self.max_hp += modifier.max_hp;
        self.attack += modifier.attack;
        self.defense += modifier.defense;
        self.speed += modifier.speed;
        self.crit_chance += modifier.crit_chance;
        self.crit_multiplier += modifier.crit_multiplier;
        self.luck += modifier.luck;
    }

    /// Remove a stat modifier (reverse of apply).
    pub fn remove_modifier(&mut self, modifier: &StatModifier) {
        self.max_hp -= modifier.max_hp;
        self.attack -= modifier.attack;
        self.defense -= modifier.defense;
        self.speed -= modifier.speed;
        self.crit_chance -= modifier.crit_chance;
        self.crit_multiplier -= modifier.crit_multiplier;
        self.luck -= modifier.luck;
    }
}

/// Additive stat modifier applied by items and synergies.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatModifier {
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: Fix,
    pub crit_chance: Fix,
    pub crit_multiplier: Fix,
    pub luck: i32,
}

impl Default for StatModifier {
    fn default() -> Self {
        Self {
            max_hp: 0,
            attack: 0,
            defense: 0,
            speed: Fix::ZERO,
            crit_chance: Fix::ZERO,
            crit_multiplier: Fix::ZERO,
            luck: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Item System — Rarity, ItemDef, Item, SynergyDef, ItemSystem
// ---------------------------------------------------------------------------

/// Rarity tiers for items. Higher rarity = rarer drops, stronger effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// Base drop weight for each rarity tier.
    fn base_weight(self) -> f32 {
        match self {
            Self::Common => 50.0,
            Self::Uncommon => 30.0,
            Self::Rare => 15.0,
            Self::Epic => 4.0,
            Self::Legendary => 1.0,
        }
    }

    const ALL: [Rarity; 5] = [
        Self::Common,
        Self::Uncommon,
        Self::Rare,
        Self::Epic,
        Self::Legendary,
    ];
}

/// An item definition (data-driven, loaded from RON).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rarity: Rarity,
    pub sprite: String,
    /// Stat modifiers applied while the item is held.
    pub modifiers: StatModifier,
    /// Tags used for synergy matching (e.g., "fire", "poison", "speed").
    pub tags: Vec<String>,
    /// Whether this item is cursed (has negative effects alongside positive).
    pub cursed: bool,
    /// If true, the item is consumed on use rather than held passively.
    pub consumable: bool,
}

/// A concrete item instance in the player's inventory.
#[derive(Clone, Debug)]
pub struct Item {
    pub def: ItemDef,
    /// Unique instance ID for removal tracking.
    pub instance_id: u32,
    /// Stacks remaining (for consumables).
    pub stacks: u8,
}

/// Synergy bonus activated when the player holds items with matching tags.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SynergyDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Tags required. All must be present in the player's item tags.
    pub required_tags: Vec<String>,
    /// Minimum number of items with matching tags needed.
    pub min_items: u8,
    /// Bonus stat modifiers when synergy is active.
    pub bonus: StatModifier,
}

/// Manages item drops, rarity rolls, and synergy checking.
pub struct ItemSystem {
    items: Vec<ItemDef>,
    synergies: Vec<SynergyDef>,
    next_instance_id: u32,
}

impl ItemSystem {
    /// Create an ItemSystem from pre-loaded item and synergy definitions.
    pub fn new(items: Vec<ItemDef>, synergies: Vec<SynergyDef>) -> Self {
        Self {
            items,
            synergies,
            next_instance_id: 1,
        }
    }

    /// Return a reference to all registered item definitions.
    pub fn item_defs(&self) -> &[ItemDef] {
        &self.items
    }

    /// Return a reference to all registered synergy definitions.
    pub fn synergy_defs(&self) -> &[SynergyDef] {
        &self.synergies
    }

    /// Roll a random item drop. `luck` shifts weight from Common toward
    /// higher rarities: each point of luck transfers 1 weight from Common
    /// to Uncommon, 0.5 to Rare, etc.
    pub fn roll_drop(&self, rng: &mut XorShift64, luck: i32) -> Option<ItemDef> {
        if self.items.is_empty() {
            return None;
        }

        // Compute luck-adjusted rarity weights.
        let luck_f = luck as f32;
        let mut weights = [0.0f32; 5];
        for (i, &rarity) in Rarity::ALL.iter().enumerate() {
            weights[i] = rarity.base_weight();
        }
        // Shift weight from Common to higher tiers.
        let shift_uncommon = luck_f.max(0.0).min(weights[0]);
        weights[0] -= shift_uncommon;
        weights[1] += shift_uncommon;

        let shift_rare = (luck_f * 0.5).max(0.0).min(weights[0]);
        weights[0] -= shift_rare;
        weights[2] += shift_rare;

        let shift_epic = (luck_f * 0.25).max(0.0).min(weights[0]);
        weights[0] -= shift_epic;
        weights[3] += shift_epic;

        let shift_legendary = (luck_f * 0.1).max(0.0).min(weights[0]);
        weights[0] -= shift_legendary;
        weights[4] += shift_legendary;

        // Ensure no negative weights.
        for w in &mut weights {
            if *w < 0.0 {
                *w = 0.0;
            }
        }

        // Pick a rarity tier.
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            return None;
        }
        let mut roll = rng.next_f32() * total;
        let mut chosen_rarity = Rarity::Common;
        for (i, &rarity) in Rarity::ALL.iter().enumerate() {
            roll -= weights[i];
            if roll <= 0.0 {
                chosen_rarity = rarity;
                break;
            }
        }

        // Pick a random item within that tier.
        let candidates: Vec<&ItemDef> = self
            .items
            .iter()
            .filter(|item| item.rarity == chosen_rarity)
            .collect();
        if candidates.is_empty() {
            // Fallback: pick any item.
            let idx = rng.next_u64() as usize % self.items.len();
            return Some(self.items[idx].clone());
        }
        let idx = rng.next_u64() as usize % candidates.len();
        Some(candidates[idx].clone())
    }

    /// Roll N items for a shop or treasure room.
    pub fn roll_shop(&self, rng: &mut XorShift64, count: usize, luck: i32) -> Vec<ItemDef> {
        (0..count)
            .filter_map(|_| self.roll_drop(rng, luck))
            .collect()
    }

    /// Check which synergies are active given the player's current items.
    pub fn active_synergies<'a>(&'a self, inventory: &[Item]) -> Vec<&'a SynergyDef> {
        self.synergies
            .iter()
            .filter(|synergy| {
                // Count how many items have ALL required tags.
                let matching = inventory
                    .iter()
                    .filter(|item| {
                        synergy
                            .required_tags
                            .iter()
                            .all(|tag| item.def.tags.contains(tag))
                    })
                    .count();
                matching >= synergy.min_items as usize
            })
            .collect()
    }

    /// Instantiate an item definition into a concrete item with a unique ID.
    pub fn instantiate(&mut self, def: ItemDef) -> Item {
        let id = self.next_instance_id;
        self.next_instance_id += 1;
        Item {
            stacks: if def.consumable { 1 } else { 0 },
            def,
            instance_id: id,
        }
    }
}

// ---------------------------------------------------------------------------
// DungeonGenerator — spec-conformant interface over existing generate_dungeon
// ---------------------------------------------------------------------------

/// A generated room within a dungeon floor (spec-conformant view).
#[derive(Clone, Debug)]
pub struct DungeonRoom {
    pub id: u16,
    /// (x, y, width, height) in tiles.
    pub rect: (i32, i32, u16, u16),
    pub room_type: RoomType,
    /// IDs of connected rooms.
    pub connections: Vec<u16>,
    pub cleared: bool,
}

/// Result of dungeon generation (spec-conformant).
#[derive(Clone, Debug)]
pub struct DungeonFloor {
    pub rooms: Vec<DungeonRoom>,
    /// Pairs of connected room IDs.
    pub corridors: Vec<(u16, u16)>,
    pub tilemap_width: u32,
    pub tilemap_height: u32,
    /// Tilemap indices: 0=Wall, 1=Floor, 2=Corridor, 3=Door(closed).
    pub tile_data: Vec<u8>,
    pub start_room_id: u16,
    pub boss_room_id: u16,
}

impl DungeonFloor {
    /// Build from an existing low-level `Dungeon`.
    pub fn from_dungeon(dungeon: &Dungeon) -> Self {
        let rooms: Vec<DungeonRoom> = dungeon
            .rooms
            .iter()
            .enumerate()
            .map(|(i, r)| DungeonRoom {
                id: i as u16,
                rect: (r.x, r.y, r.width as u16, r.height as u16),
                room_type: r.room_type,
                connections: r.connected_to.iter().map(|&c| c as u16).collect(),
                cleared: false,
            })
            .collect();

        // Build corridor pairs from room connections (deduplicated).
        let mut corridor_pairs: Vec<(u16, u16)> = Vec::new();
        for room in &rooms {
            for &conn in &room.connections {
                let pair = if room.id < conn {
                    (room.id, conn)
                } else {
                    (conn, room.id)
                };
                if !corridor_pairs.contains(&pair) {
                    corridor_pairs.push(pair);
                }
            }
        }

        let start_room_id = dungeon
            .spawn_room()
            .map(|_| {
                dungeon
                    .rooms
                    .iter()
                    .position(|r| r.room_type == RoomType::Spawn)
                    .unwrap_or(0) as u16
            })
            .unwrap_or(0);

        let boss_room_id = dungeon
            .boss_room()
            .map(|_| {
                dungeon
                    .rooms
                    .iter()
                    .position(|r| r.room_type == RoomType::Boss)
                    .unwrap_or(0) as u16
            })
            .unwrap_or(0);

        let tile_data = dungeon
            .tiles
            .iter()
            .map(|t| match t {
                DungeonTile::Wall => 0,
                DungeonTile::Floor => 1,
                DungeonTile::Corridor => 2,
                DungeonTile::Door => 3,
            })
            .collect();

        DungeonFloor {
            rooms,
            corridors: corridor_pairs,
            tilemap_width: dungeon.width as u32,
            tilemap_height: dungeon.height as u32,
            tile_data,
            start_room_id,
            boss_room_id,
        }
    }
}

/// Generates dungeon floors from seed + config.
pub struct DungeonGenerator;

impl DungeonGenerator {
    /// Generate a dungeon floor from the given seed and config.
    /// The `floor` number affects difficulty scaling (more rooms, more elites).
    pub fn generate(seed: u64, floor: u8, config: &DungeonConfig) -> DungeonFloor {
        // Scale room count with floor number for increasing complexity.
        let scaled_config = DungeonConfig {
            max_rooms: (config.max_rooms + floor as usize / 3).min(25),
            extra_connections: config.extra_connections + floor as f32 * 0.02,
            ..config.clone()
        };
        let dungeon = generate_dungeon(&scaled_config, seed ^ (floor as u64));
        DungeonFloor::from_dungeon(&dungeon)
    }
}

// ---------------------------------------------------------------------------
// FloorEscalation — difficulty scaling per floor
// ---------------------------------------------------------------------------

/// Difficulty scaling parameters per floor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FloorEscalation {
    /// Multiplier to enemy HP per floor (compounding). E.g., 1.15 = +15% per floor.
    pub hp_multiplier: f32,
    /// Multiplier to enemy damage per floor.
    pub damage_multiplier: f32,
    /// Additional enemy count per room per floor.
    pub extra_enemies_per_floor: u8,
    /// Floors on which boss encounters occur (e.g., [5, 10, 15]).
    pub boss_floors: Vec<u8>,
    /// Floor at which elite enemies start appearing.
    pub elite_start_floor: u8,
    /// Chance of an elite spawn per room, increasing per floor.
    pub elite_chance_per_floor: f32,
}

impl Default for FloorEscalation {
    fn default() -> Self {
        Self {
            hp_multiplier: 1.15,
            damage_multiplier: 1.10,
            extra_enemies_per_floor: 1,
            boss_floors: vec![5, 10, 15],
            elite_start_floor: 3,
            elite_chance_per_floor: 0.05,
        }
    }
}

impl FloorEscalation {
    /// Get the effective HP multiplier for a given floor number.
    /// Formula: `hp_multiplier ^ (floor - 1)`.
    pub fn hp_at_floor(&self, floor: u8) -> f32 {
        if floor == 0 {
            return 1.0;
        }
        self.hp_multiplier.powi((floor - 1) as i32)
    }

    /// Get the effective damage multiplier for a given floor number.
    pub fn damage_at_floor(&self, floor: u8) -> f32 {
        if floor == 0 {
            return 1.0;
        }
        self.damage_multiplier.powi((floor - 1) as i32)
    }

    /// Check if the given floor is a boss floor.
    pub fn is_boss_floor(&self, floor: u8) -> bool {
        self.boss_floors.contains(&floor)
    }

    /// Get the elite spawn chance for the given floor.
    /// Returns 0.0 before `elite_start_floor`.
    pub fn elite_chance_at_floor(&self, floor: u8) -> f32 {
        if floor < self.elite_start_floor {
            return 0.0;
        }
        (floor - self.elite_start_floor) as f32 * self.elite_chance_per_floor
    }
}

// ---------------------------------------------------------------------------
// RunStats — post-mortem statistics
// ---------------------------------------------------------------------------

/// Statistics for a completed (or ended) run. Displayed on the post-mortem screen.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunStats {
    pub seed: u64,
    pub floor_reached: u8,
    pub death_cause: Option<DeathCause>,
    pub items_collected: Vec<String>,
    pub enemies_killed: u32,
    pub damage_dealt: u64,
    pub damage_taken: u64,
    pub time_played_secs: f64,
    pub currency_earned: FxHashMap<String, u64>,
    pub synergies_activated: Vec<String>,
    pub completed: bool,
}

// ---------------------------------------------------------------------------
// RunConfig
// ---------------------------------------------------------------------------

/// Configuration for a roguelike run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig {
    /// Permadeath mode.
    pub death_mode: DeathMode,
    /// Number of floors before the final boss.
    pub floor_count: u8,
    /// Starting seed. If `None`, a random seed is generated.
    pub seed: Option<u64>,
    /// Starting items granted at run start (unlocked via meta-progression).
    pub starting_items: Vec<String>,
    /// Base player stats before item modifiers.
    pub base_stats: PlayerStats,
}

// ---------------------------------------------------------------------------
// RunManager — manages the state of a single roguelike run
// ---------------------------------------------------------------------------

/// Manages the state of a single roguelike run.
#[derive(Clone, Debug)]
pub struct RunManager {
    pub seed: u64,
    pub current_floor: u8,
    pub config: RunConfig,
    pub stats: PlayerStats,
    pub inventory: Vec<Item>,
    pub run_stats: RunStats,
    pub rng: XorShift64,
    pub floor_cleared: bool,
    pub run_active: bool,
}

impl RunManager {
    /// Start a new run with the given configuration.
    pub fn new(config: RunConfig) -> Self {
        let seed = config.seed.unwrap_or_else(|| {
            // Derive a seed from system time when none provided.
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            if t == 0 {
                0xCAFE_BABE
            } else {
                t
            }
        });

        let stats = config.base_stats.clone();
        let rng = XorShift64::new(seed);

        Self {
            seed,
            current_floor: 0,
            stats,
            inventory: Vec::new(),
            run_stats: RunStats {
                seed,
                ..Default::default()
            },
            rng,
            floor_cleared: false,
            run_active: true,
            config,
        }
    }

    /// Advance to the next floor. Triggers dungeon generation.
    /// Returns the floor number entered, or `None` if the run is complete.
    pub fn advance_floor(&mut self) -> Option<u8> {
        if !self.run_active {
            return None;
        }
        // Cannot advance if current floor is not cleared (except floor 0 = pre-start).
        if self.current_floor > 0 && !self.floor_cleared {
            return None;
        }
        if self.current_floor >= self.config.floor_count {
            return None;
        }
        self.current_floor += 1;
        self.floor_cleared = false;
        self.run_stats.floor_reached = self.current_floor;
        Some(self.current_floor)
    }

    /// Mark the current floor as cleared (boss or exit reached).
    pub fn clear_floor(&mut self) {
        self.floor_cleared = true;
        // If this was the last floor, mark the run as completed.
        if self.current_floor >= self.config.floor_count {
            self.run_stats.completed = true;
            self.run_active = false;
        }
    }

    /// Record player death. Returns `RunStats` for the post-mortem screen.
    pub fn die(&mut self, cause: DeathCause) -> RunStats {
        self.run_active = false;
        self.run_stats.death_cause = Some(cause);
        self.run_stats.items_collected = self
            .inventory
            .iter()
            .map(|item| item.def.id.clone())
            .collect();
        self.run_stats.clone()
    }

    /// Add an item to the player's inventory. Applies stat modifiers.
    pub fn add_item(&mut self, item: Item) {
        self.stats.apply_modifier(&item.def.modifiers);
        self.inventory.push(item);
    }

    /// Remove an item (dropped, consumed, or cursed-item removal).
    pub fn remove_item(&mut self, item_id: &str) -> Option<Item> {
        if let Some(pos) = self.inventory.iter().position(|i| i.def.id == item_id) {
            let item = self.inventory.remove(pos);
            self.stats.remove_modifier(&item.def.modifiers);
            Some(item)
        } else {
            None
        }
    }

    /// Get the current effective stats (base + all item modifiers).
    /// This recomputes from base stats + all held items to avoid drift.
    pub fn effective_stats(&self) -> PlayerStats {
        let mut stats = self.config.base_stats.clone();
        for item in &self.inventory {
            stats.apply_modifier(&item.def.modifiers);
        }
        stats
    }

    /// Peek at the next N random values without advancing the RNG
    /// (used for item preview in shops).
    pub fn peek_rng(&self, count: usize) -> Vec<u64> {
        self.rng.peek(count)
    }

    /// Check if current floor is a boss floor.
    pub fn is_boss_floor(&self) -> bool {
        self.current_floor == self.config.floor_count
            || (self.current_floor > 0 && self.current_floor.is_multiple_of(5))
    }

    /// Get a seeded sub-RNG for dungeon generation on the current floor.
    /// Uses `seed ^ floor` so that floor N is always deterministic regardless
    /// of how many items were rolled on previous floors.
    pub fn floor_rng(&self) -> XorShift64 {
        self.rng.fork(self.current_floor as u64)
    }
}

// ---------------------------------------------------------------------------
// MetaProgression (spec version) — permanent unlock state
// ---------------------------------------------------------------------------

/// Permanent unlock state that survives death. Persisted via `SaveManager`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MetaProgression {
    /// Permanent currencies accumulated across runs.
    pub currencies: FxHashMap<String, u64>,
    /// IDs of permanently unlocked items (added to the drop pool).
    pub unlocked_items: Vec<String>,
    /// IDs of permanently unlocked abilities.
    pub unlocked_abilities: Vec<String>,
    /// Permanent stat bonuses purchased at the hub.
    pub permanent_upgrades: Vec<String>,
    /// Total number of runs attempted.
    pub total_runs: u32,
    /// Total number of runs that reached the final boss.
    pub total_completions: u32,
    /// Best run stats for the records screen.
    pub best_run: Option<RunStats>,
}

/// Manages loading, saving, and modifying meta-progression.
pub struct MetaManager {
    pub progression: MetaProgression,
    save_manager: SaveManager,
}

/// The save slot reserved for meta-progression data.
const META_SAVE_SLOT: u32 = 9999;

impl MetaManager {
    /// Load meta-progression from the save system.
    /// If no save exists, starts with defaults.
    pub fn load(save_manager: SaveManager) -> Self {
        let progression = save_manager
            .load::<MetaProgression>(META_SAVE_SLOT)
            .unwrap_or_default();
        Self {
            progression,
            save_manager,
        }
    }

    /// Persist current meta-progression to disk.
    pub fn save(&self) -> Result<(), SaveError> {
        self.save_manager
            .save(META_SAVE_SLOT, "meta_progression", &self.progression, 0.0)
    }

    /// Add currency earned from a run (respects `DeathMode` retention).
    pub fn add_currency(&mut self, currency: &str, amount: u64) {
        *self
            .progression
            .currencies
            .entry(currency.to_string())
            .or_insert(0) += amount;
    }

    /// Attempt to purchase an unlock. Returns `false` if insufficient currency.
    pub fn purchase_unlock(&mut self, unlock_id: &str, cost: u64, currency: &str) -> bool {
        let balance = self
            .progression
            .currencies
            .get(currency)
            .copied()
            .unwrap_or(0);
        if balance < cost {
            return false;
        }
        *self
            .progression
            .currencies
            .entry(currency.to_string())
            .or_insert(0) -= cost;
        if !self
            .progression
            .unlocked_items
            .contains(&unlock_id.to_string())
        {
            self.progression.unlocked_items.push(unlock_id.to_string());
        }
        true
    }

    /// Check if an item is unlocked (and thus eligible for drop rolls).
    pub fn is_unlocked(&self, item_id: &str) -> bool {
        self.progression
            .unlocked_items
            .iter()
            .any(|id| id == item_id)
    }

    /// Record the end of a run into the meta-progression stats.
    pub fn record_run(&mut self, stats: &RunStats) {
        self.progression.total_runs += 1;
        if stats.completed {
            self.progression.total_completions += 1;
        }
        // Update best run if this one reached a higher floor, or completed.
        let dominated = match &self.progression.best_run {
            None => true,
            Some(prev) => {
                stats.floor_reached > prev.floor_reached || (stats.completed && !prev.completed)
            }
        };
        if dominated {
            self.progression.best_run = Some(stats.clone());
        }
    }

    /// Get a reference to the underlying `MetaProgression`.
    pub fn progression(&self) -> &MetaProgression {
        &self.progression
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Dungeon generation ──────────────────────────────────

    #[test]
    fn dungeon_generates_rooms() {
        let config = DungeonConfig {
            width: 64,
            height: 64,
            max_rooms: 10,
            ..Default::default()
        };

        let dungeon = generate_dungeon(&config, 42);
        assert!(!dungeon.rooms.is_empty());
        assert!(dungeon.rooms.len() <= 10);

        // Should have spawn and boss rooms
        assert!(dungeon.spawn_room().is_some());
        assert!(dungeon.boss_room().is_some());
    }

    #[test]
    fn dungeon_rooms_dont_overlap() {
        let config = DungeonConfig::default();
        let dungeon = generate_dungeon(&config, 123);

        for i in 0..dungeon.rooms.len() {
            for j in i + 1..dungeon.rooms.len() {
                assert!(
                    !dungeon.rooms[i].intersects(&dungeon.rooms[j], 0),
                    "rooms {i} and {j} overlap"
                );
            }
        }
    }

    #[test]
    fn dungeon_has_floors_and_corridors() {
        let config = DungeonConfig::default();
        let dungeon = generate_dungeon(&config, 42);

        let floors = dungeon
            .tiles
            .iter()
            .filter(|&&t| t == DungeonTile::Floor)
            .count();
        let corridors = dungeon
            .tiles
            .iter()
            .filter(|&&t| t == DungeonTile::Corridor)
            .count();

        assert!(floors > 0, "should have floor tiles");
        assert!(corridors > 0, "should have corridor tiles");
    }

    #[test]
    fn dungeon_deterministic() {
        let config = DungeonConfig::default();
        let d1 = generate_dungeon(&config, 42);
        let d2 = generate_dungeon(&config, 42);
        assert_eq!(d1.rooms.len(), d2.rooms.len());
        assert_eq!(d1.tiles, d2.tiles);
    }

    // ── Run progression ─────────────────────────────────────

    #[test]
    fn run_floor_progression() {
        let mut run = Run::new(42, 5);
        assert_eq!(run.floor, 1);
        assert!(run.next_floor());
        assert_eq!(run.floor, 2);

        // Go to max
        for _ in 0..3 {
            run.next_floor();
        }
        assert_eq!(run.floor, 5);
        assert!(!run.next_floor()); // at max
    }

    #[test]
    fn run_floor_seeds_differ() {
        let run = Run::new(42, 10);
        let seeds: Vec<u64> = (1..=5)
            .map(|f| {
                let mut r = run.clone();
                r.floor = f;
                r.floor_seed()
            })
            .collect();

        // All different
        for i in 0..seeds.len() {
            for j in i + 1..seeds.len() {
                assert_ne!(seeds[i], seeds[j]);
            }
        }
    }

    // ── Item pool ───────────────────────────────────────────

    #[test]
    fn item_pool_pick() {
        // Pool with Common (weight 10), Rare (weight 5), and Epic (weight 1)
        let pool = ItemPool::new()
            .with_entry(1, 10.0, 0) // Common-ish, high weight
            .with_entry(2, 5.0, 1) // Rare-ish, medium weight
            .with_entry(3, 1.0, 2); // Epic-ish, low weight

        let counts = std::collections::HashMap::new();

        // Pick 200 times with varying seeds and count items by id
        let mut item_counts = std::collections::HashMap::<u32, u32>::new();
        for i in 0..200u64 {
            // Derive a distinct seed per pick
            let seed = 42u64.wrapping_mul(i.wrapping_add(1)).wrapping_add(7);
            let result = pool.pick(seed, &counts);
            assert!(
                result.is_some(),
                "pick should always return Some from a non-empty pool"
            );
            *item_counts.entry(result.unwrap()).or_insert(0) += 1;
        }

        let common_count = item_counts.get(&1).copied().unwrap_or(0);
        let rare_count = item_counts.get(&2).copied().unwrap_or(0);
        let epic_count = item_counts.get(&3).copied().unwrap_or(0);

        // Item 1 (weight 10) should appear more than item 2 (weight 5)
        assert!(
            common_count > rare_count,
            "Common (id=1, weight=10) should appear more often than Rare (id=2, weight=5): got {} vs {}",
            common_count, rare_count
        );
        // Item 2 (weight 5) should appear more than item 3 (weight 1)
        assert!(
            rare_count > epic_count,
            "Rare (id=2, weight=5) should appear more often than Epic (id=3, weight=1): got {} vs {}",
            rare_count, epic_count
        );
    }

    #[test]
    fn item_pool_respects_limits() {
        let pool = ItemPool::new()
            .with_limited_entry(1, 10.0, 0, 1)
            .with_limited_entry(2, 10.0, 0, 1);

        let mut counts = std::collections::HashMap::new();
        counts.insert(1, 1);
        counts.insert(2, 1);

        let result = pool.pick(42, &counts);
        assert!(result.is_none()); // all at limit
    }

    // ── Meta-progression ────────────────────────────────────

    #[test]
    fn meta_progress_tracking() {
        let mut meta = MetaProgress::new();
        let run = Run {
            seed: 42,
            floor: 5,
            max_floor: 10,
            gold: 100,
            score: 500,
            kills: 20,
            items_collected: 5,
            run_time_secs: 300.0,
            active: false,
        };

        meta.record_run(&run, false);
        assert_eq!(meta.total_runs, 1);
        assert_eq!(meta.total_wins, 0);
        assert_eq!(meta.best_score, 500);

        meta.record_run(&run, true);
        assert_eq!(meta.total_runs, 2);
        assert_eq!(meta.total_wins, 1);
    }

    // ── XorShift64 ─────────────────────────────────────────

    #[test]
    fn xorshift_deterministic() {
        let mut a = XorShift64::new(42);
        let mut b = XorShift64::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn xorshift_fork_does_not_advance_parent() {
        let rng = XorShift64::new(42);
        let state_before = rng.state;
        let _child = rng.fork(1);
        assert_eq!(rng.state, state_before);
    }

    #[test]
    fn xorshift_fork_different_domains() {
        let rng = XorShift64::new(42);
        let mut a = rng.fork(1);
        let mut b = rng.fork(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn xorshift_peek_does_not_advance() {
        let mut rng = XorShift64::new(42);
        let peeked = rng.peek(3);
        // The actual next values should match the peeked values.
        for expected in peeked {
            assert_eq!(rng.next_u64(), expected);
        }
    }

    #[test]
    fn xorshift_zero_seed_handled() {
        let rng = XorShift64::new(0);
        assert_ne!(rng.state, 0, "zero seed should be replaced");
    }

    // ── PlayerStats / StatModifier ─────────────────────────

    #[test]
    fn player_stats_apply_remove_modifier() {
        let mut stats = PlayerStats::default();
        let original_hp = stats.max_hp;
        let original_attack = stats.attack;

        let modifier = StatModifier {
            max_hp: 20,
            attack: 5,
            defense: 3,
            ..Default::default()
        };

        stats.apply_modifier(&modifier);
        assert_eq!(stats.max_hp, original_hp + 20);
        assert_eq!(stats.attack, original_attack + 5);

        stats.remove_modifier(&modifier);
        assert_eq!(stats.max_hp, original_hp);
        assert_eq!(stats.attack, original_attack);
    }

    // ── ItemSystem ─────────────────────────────────────────

    fn make_test_item(id: &str, rarity: Rarity, tags: Vec<&str>) -> ItemDef {
        ItemDef {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            rarity,
            sprite: String::new(),
            modifiers: StatModifier::default(),
            tags: tags.into_iter().map(String::from).collect(),
            cursed: false,
            consumable: false,
        }
    }

    #[test]
    fn item_system_roll_drop() {
        let items = vec![
            make_test_item("sword", Rarity::Common, vec!["melee"]),
            make_test_item("bow", Rarity::Uncommon, vec!["ranged"]),
            make_test_item("staff", Rarity::Rare, vec!["magic"]),
        ];
        let system = ItemSystem::new(items, vec![]);
        let mut rng = XorShift64::new(42);

        let mut total_drops = 0u32;
        let mut common_count = 0u32;
        for _ in 0..100 {
            if let Some(drop) = system.roll_drop(&mut rng, 0) {
                total_drops += 1;
                if drop.rarity == Rarity::Common {
                    common_count += 1;
                }
            }
        }

        // Drops aren't always guaranteed, but we should get a majority
        assert!(
            total_drops > 50,
            "Expected more than 50 drops out of 100 rolls, got {}",
            total_drops
        );
        // Common has the highest base weight (50), so at least 1 should appear
        assert!(
            common_count >= 1,
            "Expected at least 1 Common drop out of {} total, got 0",
            total_drops
        );
    }

    #[test]
    fn item_system_roll_shop() {
        let items = vec![
            make_test_item("a", Rarity::Common, vec![]),
            make_test_item("b", Rarity::Common, vec![]),
            make_test_item("c", Rarity::Uncommon, vec![]),
        ];
        let system = ItemSystem::new(items, vec![]);
        let mut rng = XorShift64::new(99);

        let shop = system.roll_shop(&mut rng, 3, 0);
        assert_eq!(shop.len(), 3);

        // All shop items should have valid (non-empty) IDs
        for item in &shop {
            assert!(
                !item.id.is_empty(),
                "Shop item should have a valid non-empty ID"
            );
        }

        // At least 2 distinct item IDs should appear (not all the same item)
        let distinct_ids: std::collections::HashSet<&str> =
            shop.iter().map(|item| item.id.as_str()).collect();
        assert!(
            distinct_ids.len() >= 2,
            "Expected at least 2 distinct item IDs in the shop, got {:?}",
            distinct_ids
        );
    }

    #[test]
    fn item_system_synergies() {
        let items = vec![
            make_test_item("fire_sword", Rarity::Common, vec!["fire", "melee"]),
            make_test_item("fire_ring", Rarity::Common, vec!["fire"]),
        ];
        let synergies = vec![SynergyDef {
            id: "fire_mastery".into(),
            name: "Fire Mastery".into(),
            description: "Bonus for fire items".into(),
            required_tags: vec!["fire".into()],
            min_items: 2,
            bonus: StatModifier {
                attack: 10,
                ..Default::default()
            },
        }];

        let mut system = ItemSystem::new(items.clone(), synergies);

        let item_a = system.instantiate(items[0].clone());
        let item_b = system.instantiate(items[1].clone());

        // With both fire items, synergy should be active.
        let active = system.active_synergies(&[item_a.clone(), item_b.clone()]);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "fire_mastery");

        // With only one, synergy should NOT be active.
        let active = system.active_synergies(&[item_a]);
        assert!(active.is_empty());
    }

    #[test]
    fn item_system_instantiate_unique_ids() {
        let def = make_test_item("test", Rarity::Common, vec![]);
        let mut system = ItemSystem::new(vec![def.clone()], vec![]);

        let a = system.instantiate(def.clone());
        let b = system.instantiate(def);
        assert_ne!(a.instance_id, b.instance_id);
    }

    // ── FloorEscalation ────────────────────────────────────

    #[test]
    fn floor_escalation_hp_scales() {
        let esc = FloorEscalation::default();
        let hp_1 = esc.hp_at_floor(1);
        let hp_5 = esc.hp_at_floor(5);
        assert!((hp_1 - 1.0).abs() < 0.001);
        assert!(hp_5 > hp_1, "later floors should have higher HP scaling");
    }

    #[test]
    fn floor_escalation_boss_floors() {
        let esc = FloorEscalation::default();
        assert!(esc.is_boss_floor(5));
        assert!(esc.is_boss_floor(10));
        assert!(!esc.is_boss_floor(3));
    }

    #[test]
    fn floor_escalation_elite_chance() {
        let esc = FloorEscalation::default();
        assert_eq!(esc.elite_chance_at_floor(1), 0.0);
        assert_eq!(esc.elite_chance_at_floor(2), 0.0);
        assert!(esc.elite_chance_at_floor(5) > 0.0);
    }

    // ── DungeonGenerator / DungeonFloor ────────────────────

    #[test]
    fn dungeon_generator_produces_floor() {
        let config = DungeonConfig::default();
        let floor = DungeonGenerator::generate(42, 1, &config);

        assert!(!floor.rooms.is_empty());
        assert!(floor.tilemap_width > 0);
        assert!(floor.tilemap_height > 0);
        assert!(!floor.tile_data.is_empty());
    }

    #[test]
    fn dungeon_floor_has_start_and_boss() {
        let config = DungeonConfig::default();
        let floor = DungeonGenerator::generate(42, 1, &config);

        let has_spawn = floor.rooms.iter().any(|r| r.room_type == RoomType::Spawn);
        let has_boss = floor.rooms.iter().any(|r| r.room_type == RoomType::Boss);
        assert!(has_spawn, "floor should have a spawn room");
        assert!(has_boss, "floor should have a boss room");
    }

    #[test]
    fn dungeon_generator_deterministic() {
        let config = DungeonConfig::default();
        let a = DungeonGenerator::generate(42, 3, &config);
        let b = DungeonGenerator::generate(42, 3, &config);
        assert_eq!(a.rooms.len(), b.rooms.len());
        assert_eq!(a.tile_data, b.tile_data);
    }

    // ── RunManager ─────────────────────────────────────────

    fn make_run_config() -> RunConfig {
        RunConfig {
            death_mode: DeathMode::Hard,
            floor_count: 5,
            seed: Some(42),
            starting_items: vec![],
            base_stats: PlayerStats::default(),
        }
    }

    #[test]
    fn run_manager_floor_progression() {
        let mut rm = RunManager::new(make_run_config());
        assert!(rm.run_active);
        assert_eq!(rm.current_floor, 0);

        // Advance to floor 1.
        assert_eq!(rm.advance_floor(), Some(1));
        assert_eq!(rm.current_floor, 1);

        // Cannot advance without clearing.
        assert_eq!(rm.advance_floor(), None);

        // Clear and advance.
        rm.clear_floor();
        assert_eq!(rm.advance_floor(), Some(2));
    }

    #[test]
    fn run_manager_completion() {
        let mut rm = RunManager::new(RunConfig {
            floor_count: 2,
            seed: Some(42),
            death_mode: DeathMode::Hard,
            starting_items: vec![],
            base_stats: PlayerStats::default(),
        });

        rm.advance_floor(); // floor 1
        rm.clear_floor();
        rm.advance_floor(); // floor 2
        rm.clear_floor(); // completes the run

        assert!(!rm.run_active);
        assert!(rm.run_stats.completed);
    }

    #[test]
    fn run_manager_death() {
        let mut rm = RunManager::new(make_run_config());
        rm.advance_floor();

        let stats = rm.die(DeathCause::Environmental);
        assert!(!rm.run_active);
        assert!(stats.death_cause.is_some());
        assert_eq!(stats.floor_reached, 1);
    }

    #[test]
    fn run_manager_add_remove_item() {
        let mut rm = RunManager::new(make_run_config());
        let original_attack = rm.stats.attack;

        let def = ItemDef {
            id: "test_sword".into(),
            name: "Test Sword".into(),
            description: String::new(),
            rarity: Rarity::Common,
            sprite: String::new(),
            modifiers: StatModifier {
                attack: 5,
                ..Default::default()
            },
            tags: vec![],
            cursed: false,
            consumable: false,
        };

        let item = Item {
            def: def.clone(),
            instance_id: 1,
            stacks: 0,
        };

        rm.add_item(item);
        assert_eq!(rm.stats.attack, original_attack + 5);
        assert_eq!(rm.inventory.len(), 1);

        let removed = rm.remove_item("test_sword");
        assert!(removed.is_some());
        assert_eq!(rm.stats.attack, original_attack);
        assert!(rm.inventory.is_empty());
    }

    #[test]
    fn run_manager_effective_stats_consistent() {
        let mut rm = RunManager::new(make_run_config());
        let def = ItemDef {
            id: "ring".into(),
            name: "Ring".into(),
            description: String::new(),
            rarity: Rarity::Uncommon,
            sprite: String::new(),
            modifiers: StatModifier {
                max_hp: 10,
                luck: 3,
                ..Default::default()
            },
            tags: vec![],
            cursed: false,
            consumable: false,
        };
        let item = Item {
            def,
            instance_id: 1,
            stacks: 0,
        };
        rm.add_item(item);

        let eff = rm.effective_stats();
        assert_eq!(eff.max_hp, rm.config.base_stats.max_hp + 10);
        assert_eq!(eff.luck, rm.config.base_stats.luck + 3);
    }

    #[test]
    fn run_manager_floor_rng_deterministic() {
        let rm = RunManager::new(make_run_config());
        let a = rm.floor_rng();
        let b = rm.floor_rng();
        assert_eq!(a.state, b.state);
    }

    #[test]
    fn run_manager_peek_rng() {
        let rm = RunManager::new(make_run_config());
        let peeked = rm.peek_rng(5);
        assert_eq!(peeked.len(), 5);
        // Peeking again should give the same result.
        let peeked2 = rm.peek_rng(5);
        assert_eq!(peeked, peeked2);
    }

    #[test]
    fn run_manager_boss_floor() {
        let mut rm = RunManager::new(RunConfig {
            floor_count: 10,
            seed: Some(42),
            death_mode: DeathMode::Hard,
            starting_items: vec![],
            base_stats: PlayerStats::default(),
        });

        rm.advance_floor(); // floor 1
        assert!(!rm.is_boss_floor());

        // Advance to floor 5.
        rm.clear_floor();
        rm.advance_floor(); // 2
        rm.clear_floor();
        rm.advance_floor(); // 3
        rm.clear_floor();
        rm.advance_floor(); // 4
        rm.clear_floor();
        rm.advance_floor(); // 5
        assert!(rm.is_boss_floor());
    }

    // ── RunStats ───────────────────────────────────────────

    // ── MetaProgression (spec version) ─────────────────────

    #[test]
    fn meta_progression_currencies() {
        let mut meta = MetaProgression::default();
        *meta.currencies.entry("souls".into()).or_insert(0) += 100;
        assert_eq!(meta.currencies["souls"], 100);
    }

    #[test]
    fn meta_progression_best_run_tracking() {
        let mut meta = MetaProgression::default();

        let stats_a = RunStats {
            floor_reached: 3,
            completed: false,
            ..Default::default()
        };
        // Simulate MetaManager::record_run logic.
        meta.total_runs += 1;
        meta.best_run = Some(stats_a);

        let stats_b = RunStats {
            floor_reached: 5,
            completed: true,
            ..Default::default()
        };
        meta.total_runs += 1;
        meta.best_run = Some(stats_b);

        assert_eq!(meta.total_runs, 2);
        assert_eq!(meta.best_run.as_ref().unwrap().floor_reached, 5);
        assert!(meta.best_run.as_ref().unwrap().completed);
    }

    // ── DeathMode ──────────────────────────────────────────
}
