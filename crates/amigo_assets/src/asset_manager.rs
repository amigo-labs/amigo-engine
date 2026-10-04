use crate::AssetError;
use amigo_animation::{Animation, AnimationLibrary};
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

/// Whether the sprite loader reads files like `path`: PNG, and Aseprite
/// (`.aseprite`/`.ase`).
pub fn is_sprite_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext, "png" | "aseprite" | "ase"))
}

/// Decode one sprite file into the image the engine uploads and the
/// animations it defines.
///
/// A PNG has no animations. An Aseprite file becomes a horizontal strip of
/// its frames ([`AsepriteData::strip`](crate::AsepriteData::strip)), and each
/// tag an animation named `"{name}/{tag}"` whose frame UVs point into that
/// strip. `name` is the sprite name, e.g. `"enemies/bat"` for
/// `sprites/enemies/bat.aseprite`.
pub fn load_sprite_file(
    path: &Path,
    name: &str,
) -> Result<(image::RgbaImage, Vec<Animation>), AssetError> {
    let is_aseprite = path
        .extension()
        .is_some_and(|ext| ext == "aseprite" || ext == "ase");
    if !is_aseprite {
        return Ok((image::open(path)?.to_rgba8(), Vec::new()));
    }

    let data = crate::aseprite::load_aseprite(path)?;
    let stem_prefix = format!("{}/", data.name);
    let animations = data
        .animations
        .iter()
        .cloned()
        .map(|mut animation| {
            let tag = animation
                .name
                .strip_prefix(&stem_prefix)
                .unwrap_or(&animation.name);
            animation.name = format!("{name}/{tag}");
            animation
        })
        .collect();
    Ok((data.strip(), animations))
}

/// Manages all game assets: sprites, data files, etc.
pub struct AssetManager {
    base_path: PathBuf,
    sprites: FxHashMap<String, SpriteData>,
    sprite_names: Vec<String>,
    animations: AnimationLibrary,
}

impl AssetManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            sprites: FxHashMap::default(),
            sprite_names: Vec::new(),
            animations: AnimationLibrary::new(),
        }
    }

    /// Load every sprite file (PNG, Aseprite) from the sprites directory.
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
            } else if is_sprite_file(&path) {
                let stem = path.file_stem().unwrap().to_string_lossy();
                let name = if prefix.is_empty() {
                    stem.to_string()
                } else {
                    format!("{prefix}/{stem}")
                };
                if self.sprites.contains_key(&name) {
                    warn!(
                        "Two sprite files are named '{name}'; {} replaces the one loaded before",
                        path.display()
                    );
                }
                match load_sprite_file(&path, &name) {
                    Ok((image, animations)) => {
                        info!(
                            "Loaded sprite: {} ({}x{}, {} animations)",
                            name,
                            image.width(),
                            image.height(),
                            animations.len()
                        );
                        self.insert_sprite(name, image, animations);
                    }
                    Err(e) => {
                        warn!("Failed to load sprite {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    /// Re-read a single sprite file from disk (hot reload). Accepts an
    /// absolute or relative path; it must point below `<base>/sprites/`.
    /// Returns the refreshed sprite data on success; an Aseprite file's
    /// animations are replaced too.
    pub fn reload_sprite(&mut self, path: &Path) -> Option<&SpriteData> {
        if !is_sprite_file(path) {
            return None;
        }
        let sprites_root = self.base_path.join("sprites");
        // The watcher may report absolute paths while base_path is relative;
        // compare against a canonicalized root in that case.
        let rel = path
            .strip_prefix(&sprites_root)
            .ok()
            .map(|p| p.to_path_buf())
            .or_else(|| {
                let canon_root = sprites_root.canonicalize().ok()?;
                path.strip_prefix(&canon_root).ok().map(|p| p.to_path_buf())
            })?;

        // Same naming scheme as load_sprites_recursive: subdirectories
        // become a `dir/name` prefix, extension dropped.
        let name = rel.with_extension("").to_string_lossy().replace('\\', "/");

        match load_sprite_file(path, &name) {
            Ok((image, animations)) => {
                // Tags removed in the editor must not linger.
                let own_prefix = format!("{name}/");
                self.animations
                    .retain(|anim| !anim.starts_with(&own_prefix));
                self.insert_sprite(name.clone(), image, animations);
                self.sprites.get(&name)
            }
            Err(e) => {
                warn!("Failed to reload sprite {:?}: {}", path, e);
                None
            }
        }
    }

    fn insert_sprite(&mut self, name: String, image: image::RgbaImage, animations: Vec<Animation>) {
        if !self.sprites.contains_key(&name) {
            self.sprite_names.push(name.clone());
        }
        for animation in animations {
            self.animations.add(animation);
        }
        self.sprites.insert(
            name.clone(),
            SpriteData {
                name,
                width: image.width(),
                height: image.height(),
                image,
                uv: Rect::new(0.0, 0.0, 1.0, 1.0),
                texture_index: 0,
            },
        );
    }

    /// An animation by name: `"{sprite}/{tag}"` for an Aseprite file's tags,
    /// e.g. `"hero/walk"` for the `walk` tag of `sprites/hero.aseprite`.
    pub fn animation(&self, name: &str) -> Option<&Animation> {
        self.animations.get(name)
    }

    /// Every loaded animation, for `AnimPlayer::advance`.
    pub fn animations(&self) -> &AnimationLibrary {
        &self.animations
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

        // Animations `amigo pack` collected from Aseprite files. Their frame
        // UVs are relative to each sprite, which is cropped back out of the
        // atlas above, so they apply unchanged.
        if let Some(anims_data) = reader.read_entry(PAK_ANIMATIONS) {
            let animations: Vec<Animation> = std::str::from_utf8(anims_data)
                .map_err(|e| e.to_string())
                .and_then(|text| ron::from_str(text).map_err(|e| e.to_string()))
                .map_err(|reason| AssetError::LoadFailed {
                    path: PAK_ANIMATIONS.into(),
                    reason,
                })?;
            info!("Loaded {} animations from pak", animations.len());
            for animation in animations {
                self.animations.add(animation);
            }
        }

        Ok(reader)
    }
}

/// Pak entry holding the animations of packed Aseprite sprites, as a RON
/// `Vec<Animation>`.
pub const PAK_ANIMATIONS: &str = "anims.ron";

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
