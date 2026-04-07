use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Permutation table and gradient noise
// ---------------------------------------------------------------------------

/// Generate a permutation table from a seed for Perlin noise.
#[allow(clippy::needless_range_loop)]
pub fn permutation_table(seed: u64) -> [u8; 512] {
    let mut perm = [0u8; 512];
    // Initialize with identity
    for i in 0..256 {
        perm[i] = i as u8;
    }
    // Fisher-Yates shuffle using xorshift
    let mut rng = seed;
    if rng == 0 {
        rng = 0xDEAD_BEEF_CAFE;
    }
    for i in (1..256).rev() {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let j = (rng as usize) % (i + 1);
        perm.swap(i, j);
    }
    // Duplicate for wrapping
    for i in 0..256 {
        perm[i + 256] = perm[i];
    }
    perm
}

// Gradient vectors for 2D Perlin noise (unit circle, 8 directions)
const GRAD2: [(f64, f64); 8] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    ),
    (
        -std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    ),
    (
        std::f64::consts::FRAC_1_SQRT_2,
        -std::f64::consts::FRAC_1_SQRT_2,
    ),
    (
        -std::f64::consts::FRAC_1_SQRT_2,
        -std::f64::consts::FRAC_1_SQRT_2,
    ),
];

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0) // 6t^5 - 15t^4 + 10t^3
}

fn lerp_f64(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

fn grad2d(hash: u8, x: f64, y: f64) -> f64 {
    let g = GRAD2[(hash & 7) as usize];
    g.0 * x + g.1 * y
}

/// 2D Perlin noise. Returns a value roughly in [-1, 1].
pub fn perlin2d(x: f64, y: f64, perm: &[u8; 512]) -> f64 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - x.floor();
    let yf = y - y.floor();

    let u = fade(xf);
    let v = fade(yf);

    let xi = (xi & 255) as usize;
    let yi = (yi & 255) as usize;

    let aa = perm[perm[xi] as usize + yi];
    let ab = perm[perm[xi] as usize + yi + 1];
    let ba = perm[perm[xi + 1] as usize + yi];
    let bb = perm[perm[xi + 1] as usize + yi + 1];

    let x1 = lerp_f64(grad2d(aa, xf, yf), grad2d(ba, xf - 1.0, yf), u);
    let x2 = lerp_f64(grad2d(ab, xf, yf - 1.0), grad2d(bb, xf - 1.0, yf - 1.0), u);

    lerp_f64(x1, x2, v)
}

// ---------------------------------------------------------------------------
// Noise utilities: FBM, ridge noise
// ---------------------------------------------------------------------------

/// Fractal Brownian Motion — layered Perlin noise.
pub fn fbm2d(
    x: f64,
    y: f64,
    perm: &[u8; 512],
    octaves: u32,
    lacunarity: f64,
    persistence: f64,
) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        value += perlin2d(x * frequency, y * frequency, perm) * amplitude;
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    value / max_value
}

/// Ridged noise — absolute value creates ridge-like features (mountains).
pub fn ridge2d(
    x: f64,
    y: f64,
    perm: &[u8; 512],
    octaves: u32,
    lacunarity: f64,
    persistence: f64,
) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        let noise = 1.0 - perlin2d(x * frequency, y * frequency, perm).abs();
        value += noise * noise * amplitude;
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    value / max_value
}

/// Domain warping: distort coordinates using noise.
pub fn warp2d(x: f64, y: f64, perm: &[u8; 512], warp_scale: f64, warp_strength: f64) -> (f64, f64) {
    let wx = perlin2d(x * warp_scale, y * warp_scale, perm) * warp_strength;
    let wy = perlin2d(x * warp_scale + 100.0, y * warp_scale + 100.0, perm) * warp_strength;
    (x + wx, y + wy)
}

// ---------------------------------------------------------------------------
// NoiseMap — 2D grid of noise values
// ---------------------------------------------------------------------------

/// A 2D grid of f64 noise values.
#[derive(Clone, Debug)]
pub struct NoiseMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f64>,
}

impl NoiseMap {
    /// Generate a noise map using FBM.
    pub fn generate(width: u32, height: u32, seed: u64, scale: f64, octaves: u32) -> Self {
        let perm = permutation_table(seed);
        let mut data = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
            for x in 0..width {
                let nx = x as f64 / scale;
                let ny = y as f64 / scale;
                data.push(fbm2d(nx, ny, &perm, octaves, 2.0, 0.5));
            }
        }

        Self {
            width,
            height,
            data,
        }
    }

    /// Generate with ridged noise (good for mountains/terrain).
    pub fn generate_ridged(width: u32, height: u32, seed: u64, scale: f64, octaves: u32) -> Self {
        let perm = permutation_table(seed);
        let mut data = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
            for x in 0..width {
                let nx = x as f64 / scale;
                let ny = y as f64 / scale;
                data.push(ridge2d(nx, ny, &perm, octaves, 2.0, 0.5));
            }
        }

        Self {
            width,
            height,
            data,
        }
    }

    /// Get value at coordinates.
    pub fn get(&self, x: u32, y: u32) -> f64 {
        if x >= self.width || y >= self.height {
            return 0.0;
        }
        self.data[(y * self.width + x) as usize]
    }

    /// Normalize all values to the 0..1 range.
    pub fn normalize(&mut self) {
        let min = self.data.iter().copied().fold(f64::INFINITY, f64::min);
        let max = self.data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let range = max - min;
        if range < 0.0001 {
            self.data.iter_mut().for_each(|v| *v = 0.5);
            return;
        }
        self.data.iter_mut().for_each(|v| *v = (*v - min) / range);
    }

    /// Apply a custom transformation curve to all values.
    pub fn apply_curve(&mut self, f: impl Fn(f64) -> f64) {
        self.data.iter_mut().for_each(|v| *v = f(*v));
    }

    /// Get min and max values.
    pub fn range(&self) -> (f64, f64) {
        let min = self.data.iter().copied().fold(f64::INFINITY, f64::min);
        let max = self.data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (min, max)
    }
}

// ---------------------------------------------------------------------------
// Biome system
// ---------------------------------------------------------------------------

/// A biome definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiomeDef {
    pub id: u32,
    pub name: String,
    /// Temperature range this biome occupies (0..1 normalized).
    pub temperature_range: (f64, f64),
    /// Moisture range this biome occupies (0..1 normalized).
    pub moisture_range: (f64, f64),
    /// Tile ID for the ground layer.
    pub ground_tile: u32,
    /// Optional surface tile (e.g., grass on top of dirt).
    pub surface_tile: Option<u32>,
    /// Decoration tiles with spawn probability.
    pub decoration_tiles: Vec<(u32, f32)>,
}

impl BiomeDef {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            temperature_range: (0.0, 1.0),
            moisture_range: (0.0, 1.0),
            ground_tile: 0,
            surface_tile: None,
            decoration_tiles: Vec::new(),
        }
    }

    pub fn with_temperature(mut self, min: f64, max: f64) -> Self {
        self.temperature_range = (min, max);
        self
    }

    pub fn with_moisture(mut self, min: f64, max: f64) -> Self {
        self.moisture_range = (min, max);
        self
    }

    pub fn with_ground(mut self, tile: u32) -> Self {
        self.ground_tile = tile;
        self
    }

    pub fn with_surface(mut self, tile: u32) -> Self {
        self.surface_tile = Some(tile);
        self
    }

    pub fn with_decoration(mut self, tile: u32, chance: f32) -> Self {
        self.decoration_tiles.push((tile, chance));
        self
    }

    /// Check if a temperature/moisture point falls in this biome.
    pub fn contains(&self, temperature: f64, moisture: f64) -> bool {
        temperature >= self.temperature_range.0
            && temperature <= self.temperature_range.1
            && moisture >= self.moisture_range.0
            && moisture <= self.moisture_range.1
    }
}

/// A 2D map of biome IDs.
#[derive(Clone, Debug)]
pub struct BiomeMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>,
}

impl BiomeMap {
    /// Generate a biome map from temperature and moisture noise maps.
    pub fn from_noise(temperature: &NoiseMap, moisture: &NoiseMap, biomes: &[BiomeDef]) -> Self {
        assert_eq!(temperature.width, moisture.width);
        assert_eq!(temperature.height, moisture.height);

        let width = temperature.width;
        let height = temperature.height;
        let mut data = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
            for x in 0..width {
                let temp = temperature.get(x, y);
                let moist = moisture.get(x, y);

                let biome_id = biomes
                    .iter()
                    .find(|b| b.contains(temp, moist))
                    .map(|b| b.id)
                    .unwrap_or(0);

                data.push(biome_id);
            }
        }

        Self {
            width,
            height,
            data,
        }
    }

    /// Get biome ID at coordinates.
    pub fn get_biome(&self, x: u32, y: u32) -> u32 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.data[(y * self.width + x) as usize]
    }
}

// ---------------------------------------------------------------------------
// World generator
// ---------------------------------------------------------------------------

/// Collision tile types (matches amigo_tilemap::CollisionTile concepts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollisionTile {
    Empty,
    Solid,
}

/// Configurable world generator.
pub struct WorldGenerator {
    pub seed: u64,
    pub width: u32,
    pub height: u32,
    pub biomes: Vec<BiomeDef>,
    pub sea_level: f64,
    pub terrain_scale: f64,
    pub temperature_scale: f64,
    pub moisture_scale: f64,
}

impl WorldGenerator {
    pub fn new(seed: u64, width: u32, height: u32) -> Self {
        Self {
            seed,
            width,
            height,
            biomes: Vec::new(),
            sea_level: 0.35,
            terrain_scale: 50.0,
            temperature_scale: 80.0,
            moisture_scale: 60.0,
        }
    }

    pub fn with_biome(mut self, biome: BiomeDef) -> Self {
        self.biomes.push(biome);
        self
    }

    pub fn with_sea_level(mut self, level: f64) -> Self {
        self.sea_level = level;
        self
    }

    pub fn with_terrain_scale(mut self, scale: f64) -> Self {
        self.terrain_scale = scale;
        self
    }

    /// Generate a heightmap (normalized 0..1).
    pub fn generate_heightmap(&self) -> NoiseMap {
        let mut map = NoiseMap::generate(self.width, self.height, self.seed, self.terrain_scale, 6);
        map.normalize();
        map
    }

    /// Generate a temperature map (normalized 0..1, varies with latitude).
    pub fn generate_temperature_map(&self) -> NoiseMap {
        let perm = permutation_table(self.seed.wrapping_add(1000));
        let mut data = Vec::with_capacity((self.width * self.height) as usize);

        for y in 0..self.height {
            let latitude = y as f64 / self.height as f64;
            // Temperature gradient: hot at equator (0.5), cold at poles (0, 1)
            let base_temp = 1.0 - (latitude - 0.5).abs() * 2.0;

            for x in 0..self.width {
                let noise = perlin2d(
                    x as f64 / self.temperature_scale,
                    y as f64 / self.temperature_scale,
                    &perm,
                ) * 0.3;
                data.push((base_temp + noise).clamp(0.0, 1.0));
            }
        }

        NoiseMap {
            width: self.width,
            height: self.height,
            data,
        }
    }

    /// Generate a moisture map (normalized 0..1).
    pub fn generate_moisture_map(&self) -> NoiseMap {
        let mut map = NoiseMap::generate(
            self.width,
            self.height,
            self.seed.wrapping_add(2000),
            self.moisture_scale,
            4,
        );
        map.normalize();
        map
    }

    /// Generate a biome map.
    pub fn generate_biome_map(&self) -> BiomeMap {
        let temp = self.generate_temperature_map();
        let moisture = self.generate_moisture_map();
        BiomeMap::from_noise(&temp, &moisture, &self.biomes)
    }

    /// Generate tile IDs for a ground layer based on heightmap + biomes.
    pub fn generate_tiles(&self) -> Vec<u32> {
        let heightmap = self.generate_heightmap();
        let biome_map = self.generate_biome_map();

        let mut tiles = Vec::with_capacity((self.width * self.height) as usize);

        for y in 0..self.height {
            for x in 0..self.width {
                let height = heightmap.get(x, y);
                if height < self.sea_level {
                    tiles.push(0); // Water tile (convention: 0 = water)
                } else {
                    let biome_id = biome_map.get_biome(x, y);
                    let tile = self
                        .biomes
                        .iter()
                        .find(|b| b.id == biome_id)
                        .map(|b| b.ground_tile)
                        .unwrap_or(1);
                    tiles.push(tile);
                }
            }
        }

        tiles
    }

    /// Generate collision data (water = solid, land = empty).
    pub fn generate_collision(&self) -> Vec<CollisionTile> {
        let heightmap = self.generate_heightmap();
        let mut collision = Vec::with_capacity((self.width * self.height) as usize);

        for y in 0..self.height {
            for x in 0..self.width {
                if heightmap.get(x, y) < self.sea_level {
                    collision.push(CollisionTile::Solid);
                } else {
                    collision.push(CollisionTile::Empty);
                }
            }
        }

        collision
    }

    /// Place decorations on an existing tile array based on biome definitions.
    pub fn place_decorations(&self, tiles: &mut [u32]) {
        let biome_map = self.generate_biome_map();
        let heightmap = self.generate_heightmap();

        let mut rng = self.seed.wrapping_add(3000);

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) as usize;
                let height = heightmap.get(x, y);

                // Skip water
                if height < self.sea_level {
                    continue;
                }

                let biome_id = biome_map.get_biome(x, y);
                let Some(biome) = self.biomes.iter().find(|b| b.id == biome_id) else {
                    continue;
                };

                for &(deco_tile, chance) in &biome.decoration_tiles {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    let roll = (rng & 0x00FF_FFFF) as f32 / 16_777_216.0;
                    if roll < chance {
                        tiles[idx] = deco_tile;
                        break;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Simplex Noise 2D
// ---------------------------------------------------------------------------

const F2: f64 = 0.3660254037844386; // 0.5 * (sqrt(3) - 1)
const G2: f64 = 0.21132486540518713; // (3 - sqrt(3)) / 6

// Gradients for simplex 2D (12 directions)
const SIMPLEX_GRAD2: [(f64, f64); 12] = [
    (1.0, 1.0),
    (-1.0, 1.0),
    (1.0, -1.0),
    (-1.0, -1.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (1.0, 1.0),
    (-1.0, 1.0),
    (1.0, -1.0),
    (-1.0, -1.0),
];

/// 2D Simplex noise. ~40% faster than Perlin for the same quality.
/// Output range approximately [-1, 1].
pub fn simplex2d(x: f64, y: f64, perm: &[u8; 512]) -> f64 {
    let s = (x + y) * F2;
    let i = (x + s).floor();
    let j = (y + s).floor();

    let t = (i + j) * G2;
    let x0 = x - (i - t);
    let y0 = y - (j - t);

    let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

    let x1 = x0 - i1 as f64 + G2;
    let y1 = y0 - j1 as f64 + G2;
    let x2 = x0 - 1.0 + 2.0 * G2;
    let y2 = y0 - 1.0 + 2.0 * G2;

    let ii = (i as i64 & 255) as usize;
    let jj = (j as i64 & 255) as usize;

    let mut n = 0.0;

    // Corner 0
    let t0 = 0.5 - x0 * x0 - y0 * y0;
    if t0 >= 0.0 {
        let gi = perm[ii + perm[jj] as usize] as usize % 12;
        let t0 = t0 * t0;
        n += t0 * t0 * (SIMPLEX_GRAD2[gi].0 * x0 + SIMPLEX_GRAD2[gi].1 * y0);
    }

    // Corner 1
    let t1 = 0.5 - x1 * x1 - y1 * y1;
    if t1 >= 0.0 {
        let gi = perm[ii + i1 + perm[jj + j1] as usize] as usize % 12;
        let t1 = t1 * t1;
        n += t1 * t1 * (SIMPLEX_GRAD2[gi].0 * x1 + SIMPLEX_GRAD2[gi].1 * y1);
    }

    // Corner 2
    let t2 = 0.5 - x2 * x2 - y2 * y2;
    if t2 >= 0.0 {
        let gi = perm[ii + 1 + perm[jj + 1] as usize] as usize % 12;
        let t2 = t2 * t2;
        n += t2 * t2 * (SIMPLEX_GRAD2[gi].0 * x2 + SIMPLEX_GRAD2[gi].1 * y2);
    }

    // Scale to [-1, 1]
    70.0 * n
}

/// FBM layered on simplex2d.
pub fn simplex_fbm2d(
    x: f64,
    y: f64,
    perm: &[u8; 512],
    octaves: u32,
    lacunarity: f64,
    persistence: f64,
) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_amplitude = 0.0;
    for _ in 0..octaves {
        total += simplex2d(x * frequency, y * frequency, perm) * amplitude;
        max_amplitude += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    total / max_amplitude
}

impl NoiseMap {
    /// Generate a noise map using simplex noise.
    pub fn generate_simplex(w: u32, h: u32, seed: u64, scale: f64, octaves: u32) -> Self {
        let perm = permutation_table(seed);
        let mut data = vec![0.0; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let nx = x as f64 / w as f64 * scale;
                let ny = y as f64 / h as f64 * scale;
                data[(y * w + x) as usize] = simplex_fbm2d(nx, ny, &perm, octaves, 2.0, 0.5);
            }
        }
        let mut map = NoiseMap {
            width: w,
            height: h,
            data,
        };
        map.normalize();
        map
    }
}

// ---------------------------------------------------------------------------
// Wave Function Collapse (WFC)
// ---------------------------------------------------------------------------

/// Tileset rules for WFC: which tiles can be adjacent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WfcRuleset {
    pub tile_count: u32,
    /// Allowed neighbors per tile per direction [Up, Right, Down, Left].
    pub adjacency: Vec<[Vec<u32>; 4]>,
    /// Relative weights per tile (higher = more frequent).
    pub weights: Vec<f32>,
}

/// WFC error type.
#[derive(Debug)]
pub enum WfcError {
    Contradiction { x: u32, y: u32 },
}

/// WFC solver state.
pub struct WfcSolver {
    width: u32,
    height: u32,
    cells: Vec<Vec<bool>>, // cells[y*w+x] = bitset of possible tiles
    result: Vec<Option<u32>>,
    rules: WfcRuleset,
    rng: u64,
}

impl WfcSolver {
    pub fn new(width: u32, height: u32, rules: &WfcRuleset, seed: u64) -> Self {
        let n = (width * height) as usize;
        let tc = rules.tile_count as usize;
        Self {
            width,
            height,
            cells: vec![vec![true; tc]; n],
            result: vec![None; n],
            rules: rules.clone(),
            rng: if seed == 0 { 0xDEAD_BEEF } else { seed },
        }
    }

    fn xorshift(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }

    /// Pin a tile at (x,y) before solving.
    pub fn pin(&mut self, x: u32, y: u32, tile: u32) {
        let idx = (y * self.width + x) as usize;
        let tc = self.rules.tile_count as usize;
        self.cells[idx] = vec![false; tc];
        self.cells[idx][tile as usize] = true;
        self.result[idx] = Some(tile);
    }

    /// Find the cell with lowest entropy (fewest possibilities > 1).
    fn lowest_entropy_cell(&self) -> Option<usize> {
        let mut best_idx = None;
        let mut best_count = usize::MAX;
        for (idx, cell) in self.cells.iter().enumerate() {
            if self.result[idx].is_some() {
                continue;
            }
            let count = cell.iter().filter(|&&b| b).count();
            if count == 0 {
                continue; // Will be caught as contradiction
            }
            if count < best_count {
                best_count = count;
                best_idx = Some(idx);
            }
        }
        best_idx
    }

    /// Collapse a cell to one tile (weighted random).
    fn collapse(&mut self, idx: usize) -> Result<u32, WfcError> {
        let cell = &self.cells[idx];
        let possible: Vec<u32> = cell
            .iter()
            .enumerate()
            .filter(|(_, &b)| b)
            .map(|(i, _)| i as u32)
            .collect();
        if possible.is_empty() {
            let x = (idx as u32) % self.width;
            let y = (idx as u32) / self.width;
            return Err(WfcError::Contradiction { x, y });
        }
        // Weighted random selection
        let total_weight: f32 = possible
            .iter()
            .map(|&t| self.rules.weights[t as usize])
            .sum();
        let r = (self.xorshift() % 10000) as f32 / 10000.0 * total_weight;
        let mut acc = 0.0;
        let mut chosen = possible[0];
        for &t in &possible {
            acc += self.rules.weights[t as usize];
            if acc >= r {
                chosen = t;
                break;
            }
        }
        // Collapse
        let tc = self.rules.tile_count as usize;
        self.cells[idx] = vec![false; tc];
        self.cells[idx][chosen as usize] = true;
        self.result[idx] = Some(chosen);
        Ok(chosen)
    }

    /// Propagate constraints from a collapsed cell.
    fn propagate(&mut self, start_idx: usize) -> Result<(), WfcError> {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start_idx);

        while let Some(idx) = queue.pop_front() {
            let x = (idx as u32) % self.width;
            let y = (idx as u32) / self.width;

            // Directions: Up=0, Right=1, Down=2, Left=3
            let neighbors: [(i32, i32, usize, usize); 4] = [
                (0, -1, 0, 2), // Up: my Up constraint, neighbor's Down
                (1, 0, 1, 3),  // Right: my Right, neighbor's Left
                (0, 1, 2, 0),  // Down: my Down, neighbor's Up
                (-1, 0, 3, 1), // Left: my Left, neighbor's Right
            ];

            for &(dx, dy, my_dir, _their_dir) in &neighbors {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                    continue;
                }
                let nidx = (ny as u32 * self.width + nx as u32) as usize;
                if self.result[nidx].is_some() {
                    continue;
                }

                // Compute allowed tiles for the neighbor based on current cell's possibilities
                let tc = self.rules.tile_count as usize;
                let mut allowed = vec![false; tc];
                for (tile, &possible) in self.cells[idx].iter().enumerate() {
                    if !possible {
                        continue;
                    }
                    for &neighbor_tile in &self.rules.adjacency[tile][my_dir] {
                        if (neighbor_tile as usize) < tc {
                            allowed[neighbor_tile as usize] = true;
                        }
                    }
                }

                // Intersect with neighbor's current possibilities
                let mut changed = false;
                for (t, possible) in self.cells[nidx].iter_mut().enumerate() {
                    if *possible && !allowed[t] {
                        *possible = false;
                        changed = true;
                    }
                }

                if changed {
                    let remaining = self.cells[nidx].iter().filter(|&&b| b).count();
                    if remaining == 0 {
                        let nx = nidx as u32 % self.width;
                        let ny = nidx as u32 / self.width;
                        return Err(WfcError::Contradiction { x: nx, y: ny });
                    }
                    queue.push_back(nidx);
                }
            }
        }
        Ok(())
    }

    /// Step one cell (lowest entropy). Returns false when done.
    pub fn step(&mut self) -> Result<bool, WfcError> {
        let idx = match self.lowest_entropy_cell() {
            Some(i) => i,
            None => return Ok(false), // All cells collapsed
        };
        self.collapse(idx)?;
        self.propagate(idx)?;
        Ok(true)
    }

    /// Run to completion. Returns Ok(tile grid) or Err if contradiction.
    pub fn solve(&mut self) -> Result<Vec<u32>, WfcError> {
        while self.step()? {}
        Ok(self.result.iter().map(|r| r.unwrap_or(0)).collect())
    }
}

// ---------------------------------------------------------------------------
// Room-and-Corridor Dungeon Generator
// ---------------------------------------------------------------------------

/// Configuration for procedural dungeon generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonConfig {
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    pub min_room_size: u32,
    pub max_room_size: u32,
    pub max_rooms: u32,
    pub corridor_width: u32,
    pub room_padding: u32,
    /// Percentage of non-MST edges to re-add for loops (0.0-1.0).
    pub loop_chance: f32,
}

impl Default for DungeonConfig {
    fn default() -> Self {
        Self {
            width: 80,
            height: 50,
            seed: 42,
            min_room_size: 5,
            max_room_size: 15,
            max_rooms: 30,
            corridor_width: 1,
            room_padding: 2,
            loop_chance: 0.15,
        }
    }
}

/// A generated room within a dungeon.
#[derive(Clone, Debug)]
pub struct DungeonRoom {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub center: (u32, u32),
    pub connections: Vec<usize>,
}

/// Result of dungeon generation.
#[derive(Clone, Debug)]
pub struct DungeonResult {
    /// Tile grid: 0=wall, 1=floor, 2=corridor, 3=door.
    pub tiles: Vec<u32>,
    pub width: u32,
    pub height: u32,
    pub rooms: Vec<DungeonRoom>,
    pub start_room: usize,
    pub end_room: usize,
}

/// Generate a dungeon from configuration.
pub fn generate_dungeon(config: &DungeonConfig) -> DungeonResult {
    let mut rng = if config.seed == 0 {
        0xCAFE_BABE
    } else {
        config.seed
    };
    let mut xorshift = move || -> u64 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };

    let w = config.width;
    let h = config.height;
    let mut tiles = vec![0u32; (w * h) as usize]; // 0 = wall

    // 1. Place rooms via rejection sampling
    let mut rooms: Vec<DungeonRoom> = Vec::new();
    for _ in 0..config.max_rooms * 3 {
        // Try 3x attempts
        if rooms.len() >= config.max_rooms as usize {
            break;
        }
        let rw = config.min_room_size
            + (xorshift() as u32 % (config.max_room_size - config.min_room_size + 1));
        let rh = config.min_room_size
            + (xorshift() as u32 % (config.max_room_size - config.min_room_size + 1));
        let rx = 1 + (xorshift() as u32 % (w.saturating_sub(rw + 2)));
        let ry = 1 + (xorshift() as u32 % (h.saturating_sub(rh + 2)));

        // Check overlap with padding
        let overlaps = rooms.iter().any(|r| {
            let pad = config.room_padding;
            rx < r.x + r.width + pad
                && rx + rw + pad > r.x
                && ry < r.y + r.height + pad
                && ry + rh + pad > r.y
        });
        if overlaps {
            continue;
        }

        // Carve room
        for y in ry..ry + rh {
            for x in rx..rx + rw {
                tiles[(y * w + x) as usize] = 1; // floor
            }
        }
        rooms.push(DungeonRoom {
            x: rx,
            y: ry,
            width: rw,
            height: rh,
            center: (rx + rw / 2, ry + rh / 2),
            connections: Vec::new(),
        });
    }

    if rooms.is_empty() {
        return DungeonResult {
            tiles,
            width: w,
            height: h,
            rooms,
            start_room: 0,
            end_room: 0,
        };
    }

    // 2. Build MST (Prim's algorithm) on room centers
    let n = rooms.len();
    let mut in_mst = vec![false; n];
    let mut mst_edges: Vec<(usize, usize)> = Vec::new();
    in_mst[0] = true;

    for _ in 1..n {
        let mut best_edge = (0, 0);
        let mut best_dist = u64::MAX;
        for (i, ri) in rooms.iter().enumerate() {
            if !in_mst[i] {
                continue;
            }
            for (j, rj) in rooms.iter().enumerate() {
                if in_mst[j] {
                    continue;
                }
                let dx = ri.center.0 as i64 - rj.center.0 as i64;
                let dy = ri.center.1 as i64 - rj.center.1 as i64;
                let dist = (dx * dx + dy * dy) as u64;
                if dist < best_dist {
                    best_dist = dist;
                    best_edge = (i, j);
                }
            }
        }
        if best_dist < u64::MAX {
            in_mst[best_edge.1] = true;
            mst_edges.push(best_edge);
        }
    }

    // 3. Add loop edges
    let mut all_edges = mst_edges.clone();
    for i in 0..n {
        for j in (i + 1)..n {
            if all_edges.contains(&(i, j)) || all_edges.contains(&(j, i)) {
                continue;
            }
            let r = (xorshift() % 1000) as f32 / 1000.0;
            if r < config.loop_chance {
                all_edges.push((i, j));
            }
        }
    }

    // 4. Carve L-shaped corridors and record connections
    for &(a, b) in &all_edges {
        let (ax, ay) = rooms[a].center;
        let (bx, by) = rooms[b].center;
        rooms[a].connections.push(b);
        rooms[b].connections.push(a);

        // L-shaped corridor: horizontal then vertical (50%) or vertical then horizontal
        let horizontal_first = xorshift() % 2 == 0;
        if horizontal_first {
            carve_h_corridor(&mut tiles, w, ax, bx, ay);
            carve_v_corridor(&mut tiles, w, ay, by, bx);
        } else {
            carve_v_corridor(&mut tiles, w, ay, by, ax);
            carve_h_corridor(&mut tiles, w, ax, bx, by);
        }
    }

    // 5. Find start (most central) and end (farthest from start)
    let center_x = w / 2;
    let center_y = h / 2;
    let start_room = rooms
        .iter()
        .enumerate()
        .min_by_key(|(_, r)| {
            let dx = r.center.0 as i64 - center_x as i64;
            let dy = r.center.1 as i64 - center_y as i64;
            (dx * dx + dy * dy) as u64
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    let end_room = rooms
        .iter()
        .enumerate()
        .max_by_key(|(_, r)| {
            let sc = rooms[start_room].center;
            let dx = r.center.0 as i64 - sc.0 as i64;
            let dy = r.center.1 as i64 - sc.1 as i64;
            (dx * dx + dy * dy) as u64
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    DungeonResult {
        tiles,
        width: w,
        height: h,
        rooms,
        start_room,
        end_room,
    }
}

fn carve_h_corridor(tiles: &mut [u32], map_width: u32, x1: u32, x2: u32, y: u32) {
    let start = x1.min(x2);
    let end = x1.max(x2);
    for x in start..=end {
        let idx = (y * map_width + x) as usize;
        if idx < tiles.len() && tiles[idx] == 0 {
            tiles[idx] = 2; // corridor
        }
    }
}

fn carve_v_corridor(tiles: &mut [u32], map_width: u32, y1: u32, y2: u32, x: u32) {
    let start = y1.min(y2);
    let end = y1.max(y2);
    for y in start..=end {
        let idx = (y * map_width + x) as usize;
        if idx < tiles.len() && tiles[idx] == 0 {
            tiles[idx] = 2; // corridor
        }
    }
}

/// Convert dungeon tile data to CollisionTile format (0=wall→Solid, 1/2=floor/corridor→Empty).
pub fn dungeon_tiles_to_collision(tiles: &[u32]) -> Vec<u8> {
    tiles
        .iter()
        .map(|&t| if t == 0 { 1 } else { 0 }) // 1=Solid, 0=Empty
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Noise primitives ────────────────────────────────────

    #[test]
    fn perlin_noise_range() {
        let perm = permutation_table(42);
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;

        for y in 0..100 {
            for x in 0..100 {
                let v = perlin2d(x as f64 * 0.1, y as f64 * 0.1, &perm);
                min = min.min(v);
                max = max.max(v);
            }
        }

        // Perlin noise should be in roughly [-1, 1]
        assert!(min >= -1.5, "min was {min}");
        assert!(max <= 1.5, "max was {max}");
        assert!(min < 0.0, "should have negative values");
        assert!(max > 0.0, "should have positive values");
    }

    #[test]
    fn fbm_converges() {
        let perm = permutation_table(123);
        let v = fbm2d(0.5, 0.5, &perm, 6, 2.0, 0.5);
        assert!(v.is_finite());
        assert!(v.abs() < 2.0);
    }

    // ── Noise map ───────────────────────────────────────────

    #[test]
    fn noise_map_normalize() {
        let mut map = NoiseMap::generate(32, 32, 42, 10.0, 4);
        let (min_before, max_before) = map.range();
        assert!(min_before < max_before);

        map.normalize();
        let (min_after, max_after) = map.range();
        assert!((min_after - 0.0).abs() < 0.001);
        assert!((max_after - 1.0).abs() < 0.001);
    }

    #[test]
    fn deterministic_with_same_seed() {
        let map1 = NoiseMap::generate(16, 16, 42, 10.0, 4);
        let map2 = NoiseMap::generate(16, 16, 42, 10.0, 4);
        assert_eq!(map1.data, map2.data);
    }

    #[test]
    fn different_seed_different_output() {
        let map1 = NoiseMap::generate(16, 16, 42, 10.0, 4);
        let map2 = NoiseMap::generate(16, 16, 99, 10.0, 4);
        assert_ne!(map1.data, map2.data);
    }

    // ── Biomes and world generation ─────────────────────────

    #[test]
    fn biome_selection() {
        let biomes = vec![
            BiomeDef::new(1, "Desert")
                .with_temperature(0.6, 1.0)
                .with_moisture(0.0, 0.3),
            BiomeDef::new(2, "Forest")
                .with_temperature(0.3, 0.7)
                .with_moisture(0.4, 1.0),
            BiomeDef::new(3, "Tundra")
                .with_temperature(0.0, 0.3)
                .with_moisture(0.0, 0.5),
        ];

        assert!(biomes[0].contains(0.8, 0.1)); // desert
        assert!(!biomes[0].contains(0.2, 0.1)); // too cold for desert
        assert!(biomes[1].contains(0.5, 0.6)); // forest
        assert!(biomes[2].contains(0.1, 0.2)); // tundra
    }

    #[test]
    fn world_generator_correct_size() {
        let gen = WorldGenerator::new(42, 64, 64)
            .with_biome(BiomeDef::new(1, "Plains").with_ground(1))
            .with_sea_level(0.3);

        let tiles = gen.generate_tiles();
        assert_eq!(tiles.len(), 64 * 64);

        let collision = gen.generate_collision();
        assert_eq!(collision.len(), 64 * 64);
    }

    #[test]
    fn world_has_water_and_land() {
        let gen = WorldGenerator::new(42, 64, 64)
            .with_biome(BiomeDef::new(1, "Plains").with_ground(1))
            .with_sea_level(0.4);

        let tiles = gen.generate_tiles();
        let water_count = tiles.iter().filter(|&&t| t == 0).count();
        let land_count = tiles.iter().filter(|&&t| t != 0).count();

        // Should have both water and land
        assert!(water_count > 0, "should have some water");
        assert!(land_count > 0, "should have some land");
    }

    #[test]
    fn biome_map_from_noise() {
        let biomes = vec![
            BiomeDef::new(1, "Low")
                .with_temperature(0.0, 0.5)
                .with_moisture(0.0, 1.0),
            BiomeDef::new(2, "High")
                .with_temperature(0.5, 1.0)
                .with_moisture(0.0, 1.0),
        ];

        // Create simple noise maps
        let temp = NoiseMap {
            width: 4,
            height: 4,
            data: vec![
                0.1, 0.2, 0.6, 0.8, 0.3, 0.4, 0.7, 0.9, 0.1, 0.3, 0.6, 0.8, 0.2, 0.4, 0.7, 0.9,
            ],
        };
        let moisture = NoiseMap {
            width: 4,
            height: 4,
            data: vec![0.5; 16],
        };

        let biome_map = BiomeMap::from_noise(&temp, &moisture, &biomes);
        assert_eq!(biome_map.get_biome(0, 0), 1); // temp 0.1 → Low
        assert_eq!(biome_map.get_biome(2, 0), 2); // temp 0.6 → High
    }

    // ── Simplex Noise ──────────────────────────────────────

    #[test]
    fn simplex_noise_range() {
        let perm = permutation_table(42);
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for y in 0..100 {
            for x in 0..100 {
                let v = simplex2d(x as f64 * 0.1, y as f64 * 0.1, &perm);
                min = min.min(v);
                max = max.max(v);
            }
        }
        assert!(min >= -1.5, "min was {min}");
        assert!(max <= 1.5, "max was {max}");
        assert!(min < 0.0, "should have negative values");
        assert!(max > 0.0, "should have positive values");
    }

    #[test]
    fn simplex_noisemap() {
        let map = NoiseMap::generate_simplex(32, 32, 42, 4.0, 4);
        assert_eq!(map.data.len(), 32 * 32);
        let (min, max) = map.range();
        assert!(min >= 0.0, "normalized min should be >= 0");
        assert!(max <= 1.0, "normalized max should be <= 1");
    }

    // ── WFC ────────────────────────────────────────────────

    #[test]
    fn wfc_simple_checkerboard() {
        // 2 tiles: black(0) and white(1)
        // Rule: black can only neighbor white and vice versa
        let rules = WfcRuleset {
            tile_count: 2,
            adjacency: vec![
                [vec![1], vec![1], vec![1], vec![1]], // tile 0 → neighbors must be 1
                [vec![0], vec![0], vec![0], vec![0]], // tile 1 → neighbors must be 0
            ],
            weights: vec![1.0, 1.0],
        };
        let mut solver = WfcSolver::new(4, 4, &rules, 42);
        solver.pin(0, 0, 0); // Top-left is black
        let result = solver.solve();
        assert!(result.is_ok());
        let grid = result.unwrap();
        assert_eq!(grid.len(), 16);
        // Check checkerboard pattern
        assert_eq!(grid[0], 0); // (0,0) = black (pinned)
        assert_eq!(grid[1], 1); // (1,0) = white
        assert_eq!(grid[4], 1); // (0,1) = white
    }

    // ── Dungeon Generator ──────────────────────────────────

    #[test]
    fn dungeon_generates_rooms() {
        let config = DungeonConfig {
            width: 80,
            height: 50,
            seed: 42,
            max_rooms: 10,
            ..Default::default()
        };
        let result = generate_dungeon(&config);
        assert_eq!(result.tiles.len(), 80 * 50);
        assert!(!result.rooms.is_empty(), "should have generated rooms");
        assert!(result.rooms.len() <= 10);
        assert!(result.start_room < result.rooms.len());
        assert!(result.end_room < result.rooms.len());
        assert_ne!(result.start_room, result.end_room);
    }

    #[test]
    fn dungeon_has_corridors() {
        let config = DungeonConfig::default();
        let result = generate_dungeon(&config);
        let corridor_count = result.tiles.iter().filter(|&&t| t == 2).count();
        assert!(corridor_count > 0, "should have corridors");
    }

    #[test]
    fn dungeon_deterministic() {
        let config = DungeonConfig {
            seed: 123,
            ..Default::default()
        };
        let a = generate_dungeon(&config);
        let b = generate_dungeon(&config);
        assert_eq!(a.tiles, b.tiles);
        assert_eq!(a.rooms.len(), b.rooms.len());
    }

    #[test]
    fn dungeon_collision_conversion() {
        let config = DungeonConfig::default();
        let result = generate_dungeon(&config);
        let collision = dungeon_tiles_to_collision(&result.tiles);
        assert_eq!(collision.len(), result.tiles.len());
        // Walls should be solid (1), floors/corridors should be empty (0)
        for (i, &tile) in result.tiles.iter().enumerate() {
            if tile == 0 {
                assert_eq!(collision[i], 1, "wall should be solid");
            } else {
                assert_eq!(collision[i], 0, "floor/corridor should be empty");
            }
        }
    }
}
