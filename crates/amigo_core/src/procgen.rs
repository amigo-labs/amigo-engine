use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Permutation table and gradient noise
// ---------------------------------------------------------------------------

/// Generate a permutation table from a seed for Perlin noise.
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
    (0.7071, 0.7071),
    (-0.7071, 0.7071),
    (0.7071, -0.7071),
    (-0.7071, -0.7071),
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
    pub fn place_decorations(&self, tiles: &mut Vec<u32>) {
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
    fn ridge_noise_positive() {
        let perm = permutation_table(42);
        for y in 0..20 {
            for x in 0..20 {
                let v = ridge2d(x as f64 * 0.1, y as f64 * 0.1, &perm, 4, 2.0, 0.5);
                assert!(v >= 0.0, "ridge noise should be non-negative, got {v}");
            }
        }
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
}
