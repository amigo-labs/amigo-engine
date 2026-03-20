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
        let pool = ItemPool::new()
            .with_entry(1, 10.0, 0)
            .with_entry(2, 5.0, 1)
            .with_entry(3, 1.0, 2);

        let counts = std::collections::HashMap::new();
        let result = pool.pick(42, &counts);
        assert!(result.is_some());
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
}
