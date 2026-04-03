use crate::AssetError;
use amigo_core::Rect;
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Data for a loaded sprite.
#[derive(Clone, Debug)]
pub struct SpriteData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub image: image::RgbaImage,
    /// UV rect within the texture atlas (for release mode). In dev mode, this is (0,0,1,1).
    pub uv: Rect,
    /// Index into the texture atlas (or individual texture).
    pub texture_index: u32,
}

/// Manages all game assets: sprites, data files, etc.
pub struct AssetManager {
    base_path: PathBuf,
    sprites: FxHashMap<String, SpriteData>,
    sprite_names: Vec<String>,
}

impl AssetManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            sprites: FxHashMap::default(),
            sprite_names: Vec::new(),
        }
    }

    /// Load all PNG files from the sprites directory.
    pub fn load_sprites(&mut self) -> Result<(), AssetError> {
        let sprites_dir = self.base_path.join("sprites");
        if !sprites_dir.exists() {
            info!("No sprites directory found at {:?}, skipping", sprites_dir);
            return Ok(());
        }
        self.load_sprites_recursive(&sprites_dir, "")
    }

    fn load_sprites_recursive(&mut self, dir: &Path, prefix: &str) -> Result<(), AssetError> {
        let entries = std::fs::read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().unwrap().to_string_lossy();
                let new_prefix = if prefix.is_empty() {
                    dir_name.to_string()
                } else {
                    format!("{prefix}/{dir_name}")
                };
                self.load_sprites_recursive(&path, &new_prefix)?;
            } else if path.extension().is_some_and(|ext| ext == "png") {
                let stem = path.file_stem().unwrap().to_string_lossy();
                let name = if prefix.is_empty() {
                    stem.to_string()
                } else {
                    format!("{prefix}/{stem}")
                };
                match image::open(&path) {
                    Ok(img) => {
                        let rgba = img.to_rgba8();
                        let width = rgba.width();
                        let height = rgba.height();
                        info!("Loaded sprite: {} ({}x{})", name, width, height);
                        self.sprite_names.push(name.clone());
                        self.sprites.insert(
                            name.clone(),
                            SpriteData {
                                name,
                                width,
                                height,
                                image: rgba,
                                uv: Rect::new(0.0, 0.0, 1.0, 1.0),
                                texture_index: 0,
                            },
                        );
                    }
                    Err(e) => {
                        warn!("Failed to load sprite {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    /// Get a sprite by name. In dev mode, provides fuzzy match suggestions.
    pub fn sprite(&self, name: &str) -> Option<&SpriteData> {
        match self.sprites.get(name) {
            Some(s) => Some(s),
            None => {
                if let Some(suggestion) = self.fuzzy_match(name) {
                    warn!(
                        "Sprite '{}' not found. Did you mean '{}'?",
                        name, suggestion
                    );
                } else {
                    warn!("Sprite '{}' not found", name);
                }
                None
            }
        }
    }

    /// Get sprite data, consuming the image (for texture upload).
    pub fn take_sprite_image(&mut self, name: &str) -> Option<image::RgbaImage> {
        self.sprites.get(name).map(|s| s.image.clone())
    }

    /// List all loaded sprite names.
    pub fn sprite_names(&self) -> &[String] {
        &self.sprite_names
    }

    /// Load a RON data file.
    pub fn load_ron<T: serde::de::DeserializeOwned>(
        &self,
        relative_path: &str,
    ) -> Result<T, AssetError> {
        let path = self.base_path.join("data").join(relative_path);
        let contents = std::fs::read_to_string(&path).map_err(|_| AssetError::NotFound {
            path: relative_path.to_string(),
        })?;
        ron::from_str(&contents).map_err(|e| AssetError::LoadFailed {
            path: relative_path.to_string(),
            reason: e.to_string(),
        })
    }

    /// Simple fuzzy matching for dev mode suggestions.
    fn fuzzy_match(&self, query: &str) -> Option<String> {
        let query_lower = query.to_lowercase();
        let mut best: Option<(&str, usize)> = None;

        for name in &self.sprite_names {
            let name_lower = name.to_lowercase();
            let dist = levenshtein(&query_lower, &name_lower);
            if dist <= 3 && (best.is_none() || dist < best.unwrap().1) {
                best = Some((name, dist));
            }
        }

        best.map(|(name, _)| name.to_string())
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Load assets from a `game.pak` file. Returns the PakReader for
    /// further queries (audio, data, levels, fonts).
    ///
    /// Sprites are loaded from the embedded atlas + manifest. Other asset
    /// types remain accessible via the returned reader.
    pub fn load_from_pak(&mut self, pak_path: &Path) -> Result<crate::pak::PakReader, AssetError> {
        use crate::pak::PakReader;

        let reader = PakReader::open(pak_path).map_err(|e| AssetError::LoadFailed {
            path: pak_path.to_string_lossy().to_string(),
            reason: e.to_string(),
        })?;

        // Load atlas manifest to get sprite UV coordinates
        if let Some(manifest_data) = reader.read_entry("atlas.ron") {
            let manifest_str =
                std::str::from_utf8(manifest_data).map_err(|e| AssetError::LoadFailed {
                    path: "atlas.ron".into(),
                    reason: e.to_string(),
                })?;

            let entries: Vec<(String, [f32; 4])> =
                ron::from_str(manifest_str).map_err(|e| AssetError::LoadFailed {
                    path: "atlas.ron".into(),
                    reason: e.to_string(),
                })?;

            // Load atlas image
            if let Some(atlas_data) = reader.read_entry("atlas.png") {
                let atlas_img = image::load_from_memory(atlas_data)
                    .map_err(|e| AssetError::LoadFailed {
                        path: "atlas.png".into(),
                        reason: e.to_string(),
                    })?
                    .to_rgba8();

                let atlas_w = atlas_img.width();
                let atlas_h = atlas_img.height();

                // Register each sprite with its UV rect from the atlas
                for (name, [u, v, w, h]) in &entries {
                    let pixel_x = (u * atlas_w as f32) as u32;
                    let pixel_y = (v * atlas_h as f32) as u32;
                    let pixel_w = (w * atlas_w as f32) as u32;
                    let pixel_h = (h * atlas_h as f32) as u32;

                    // Extract sub-image for this sprite
                    let sub_img =
                        image::imageops::crop_imm(&atlas_img, pixel_x, pixel_y, pixel_w, pixel_h)
                            .to_image();

                    self.sprite_names.push(name.clone());
                    self.sprites.insert(
                        name.clone(),
                        SpriteData {
                            name: name.clone(),
                            width: pixel_w,
                            height: pixel_h,
                            image: sub_img,
                            uv: amigo_core::Rect::new(*u, *v, *w, *h),
                            texture_index: 0,
                        },
                    );
                }

                info!(
                    "Loaded {} sprites from pak atlas ({}x{})",
                    entries.len(),
                    atlas_w,
                    atlas_h,
                );
            }
        }

        Ok(reader)
    }
}

/// Simple Levenshtein distance for fuzzy matching.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut matrix = vec![vec![0usize; b.len() + 1]; a.len() + 1];

    for (i, row) in matrix.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in matrix[0].iter_mut().enumerate().take(b.len() + 1) {
        *cell = j;
    }

    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a.len()][b.len()]
}
