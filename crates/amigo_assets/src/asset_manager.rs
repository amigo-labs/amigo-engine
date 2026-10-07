use crate::AssetError;
use crate::atlas_manifest::{self, AtlasError, LoadedAtlas, SpriteFrame, is_atlas_manifest};
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
    /// The sprite's frames: one covering the image for a PNG, one per cell
    /// of an Aseprite strip, the manifest's frames for a sheet sprite.
    pub frames: Vec<SpriteFrame>,
    /// For a sprite on an atlas sheet, the sheet's key (see
    /// [`AssetManager::sheets`]). Its pixels live in the sheet, so `image`
    /// is empty and `width`/`height` are those of its first frame.
    pub sheet: Option<String>,
}

/// A sheet image from an atlas manifest, shared by the sprites on it.
#[derive(Clone, Debug)]
pub struct SheetData {
    /// The manifest's path below `sprites/`, e.g. `"hamster.atlas.ron"`.
    pub key: String,
    pub image: image::RgbaImage,
    /// Mip levels below full size the sheet may use (0 = none).
    pub mip_levels: u32,
    /// Names of the sprites on the sheet.
    pub sprites: Vec<String>,
    pub manifest_path: PathBuf,
    pub image_path: PathBuf,
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
    load_sprite_file_with_frames(path, name).map(|(image, animations, _)| (image, animations))
}

/// [`load_sprite_file`], plus the number of frames in the image: 1 for a PNG,
/// the strip's cell count for an Aseprite file.
pub fn load_sprite_file_with_frames(
    path: &Path,
    name: &str,
) -> Result<(image::RgbaImage, Vec<Animation>, u32), AssetError> {
    let is_aseprite = path
        .extension()
        .is_some_and(|ext| ext == "aseprite" || ext == "ase");
    if !is_aseprite {
        return Ok((image::open(path)?.to_rgba8(), Vec::new(), 1));
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
    let frames = data.frames.len().max(1) as u32;
    Ok((data.strip(), animations, frames))
}

/// The frames of a strip of `count` equal cells side by side.
fn strip_frames(width: u32, height: u32, count: u32) -> Vec<SpriteFrame> {
    let count = count.max(1);
    let cell = width / count;
    (0..count)
        .map(|i| SpriteFrame::on_texture(i * cell, 0, cell, height, (width, height), [0.0, 0.0]))
        .collect()
}

/// Manages all game assets: sprites, data files, etc.
pub struct AssetManager {
    base_path: PathBuf,
    sprites: FxHashMap<String, SpriteData>,
    sprite_names: Vec<String>,
    animations: AnimationLibrary,
    sheets: Vec<SheetData>,
}

impl AssetManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            sprites: FxHashMap::default(),
            sprite_names: Vec::new(),
            animations: AnimationLibrary::new(),
            sheets: Vec::new(),
        }
    }

    /// Load every sprite file (PNG, Aseprite) and atlas manifest
    /// (`*.atlas.ron`) from the sprites directory.
    ///
    /// A manifest that fails to load is logged and its sprites are skipped;
    /// the other assets still load. Its sheet image is never loaded as a
    /// sprite of its own.
    pub fn load_sprites(&mut self) -> Result<(), AssetError> {
        let sprites_dir = self.base_path.join("sprites");
        if !sprites_dir.exists() {
            info!("No sprites directory found at {:?}, skipping", sprites_dir);
            return Ok(());
        }
        let mut files = Vec::new();
        let mut manifests = Vec::new();
        collect_sprite_files(&sprites_dir, "", &mut files, &mut manifests)?;

        // Sheet images belong to their manifest, not to the plain sprites.
        let sheet_images: Vec<PathBuf> = manifests
            .iter()
            .filter_map(|m| {
                let text = std::fs::read_to_string(m).ok()?;
                let manifest = atlas_manifest::parse_manifest(m, &text).ok()?;
                Some(normalize(&atlas_manifest::image_path(m, &manifest)))
            })
            .collect();

        for (name, path) in files {
            if sheet_images.contains(&normalize(&path)) {
                continue;
            }
            if self.sprites.contains_key(&name) {
                warn!(
                    "Two sprite files are named '{name}'; {} replaces the one loaded before",
                    path.display()
                );
            }
            match load_sprite_file_with_frames(&path, &name) {
                Ok((image, animations, frames)) => {
                    info!(
                        "Loaded sprite: {} ({}x{}, {} animations)",
                        name,
                        image.width(),
                        image.height(),
                        animations.len()
                    );
                    self.insert_sprite(name, image, animations, frames);
                }
                Err(e) => {
                    warn!("Failed to load sprite {:?}: {}", path, e);
                }
            }
        }

        for manifest in manifests {
            if let Err(e) = self.load_atlas_file(&manifest) {
                warn!("Skipping atlas: {e}");
            }
        }
        Ok(())
    }

    /// Load (or reload) one atlas manifest: register its sheet and its
    /// sprites, and an animation for each sprite with `fps`. On error nothing
    /// changes. A `MipPadding` problem is logged and the sheet loads without
    /// mipmaps. Returns the sheet's key.
    pub fn load_atlas_file(&mut self, manifest_path: &Path) -> Result<String, AtlasError> {
        let atlas = atlas_manifest::load_atlas(manifest_path)?;
        let key = self.sheet_key(manifest_path);
        self.register_atlas(key, atlas)
    }

    fn sheet_key(&self, manifest_path: &Path) -> String {
        let root = self.base_path.join("sprites");
        let rel = manifest_path
            .strip_prefix(&root)
            .ok()
            .map(Path::to_path_buf)
            .or_else(|| {
                let root = root.canonicalize().ok()?;
                let path = manifest_path.canonicalize().ok()?;
                path.strip_prefix(root).ok().map(Path::to_path_buf)
            })
            .unwrap_or_else(|| manifest_path.to_path_buf());
        rel.to_string_lossy().replace('\\', "/")
    }

    /// Register a validated atlas under `key`, replacing the sheet of that key
    /// and its sprites.
    pub fn register_atlas(
        &mut self,
        key: String,
        atlas: LoadedAtlas,
    ) -> Result<String, AtlasError> {
        let previous: Vec<String> = self
            .sheets
            .iter()
            .find(|s| s.key == key)
            .map(|s| s.sprites.clone())
            .unwrap_or_default();
        for sprite in &atlas.sprites {
            if previous.contains(&sprite.name) {
                continue;
            }
            if let Some(other) = self.sprites.get(&sprite.name) {
                let other = match &other.sheet {
                    Some(sheet) => format!("atlas '{sheet}'"),
                    None => format!("sprite file '{}'", sprite.name),
                };
                return Err(AtlasError::DuplicateName {
                    path: atlas.manifest_path.clone(),
                    sprite: sprite.name.clone(),
                    other,
                });
            }
        }
        if let Some(e) = &atlas.mip_error {
            warn!("{e}; loading the sheet without mipmaps");
        }

        // Out with the old sheet's sprites, in with the new.
        for name in &previous {
            self.sprites.remove(name);
            self.animations.retain(|anim| anim != name);
        }
        self.sprite_names.retain(|n| !previous.contains(n));
        let (sheet_w, sheet_h) = atlas.image.dimensions();
        let mut names = Vec::with_capacity(atlas.sprites.len());
        for sprite in atlas.sprites {
            let first = sprite.frames[0];
            if let Some(animation) = sprite.animation {
                self.animations.add(animation);
            }
            names.push(sprite.name.clone());
            self.sprite_names.push(sprite.name.clone());
            self.sprites.insert(
                sprite.name.clone(),
                SpriteData {
                    name: sprite.name,
                    width: first.w,
                    height: first.h,
                    image: image::RgbaImage::new(0, 0),
                    uv: first.uv,
                    texture_index: 0,
                    frames: sprite.frames,
                    sheet: Some(key.clone()),
                },
            );
        }
        info!(
            "Loaded atlas {key}: {} sprite(s) on a {sheet_w}x{sheet_h} sheet",
            names.len()
        );
        let sheet = SheetData {
            key: key.clone(),
            image: atlas.image,
            mip_levels: atlas.mip_levels,
            sprites: names,
            manifest_path: atlas.manifest_path,
            image_path: atlas.image_path,
        };
        match self.sheets.iter_mut().find(|s| s.key == key) {
            Some(existing) => *existing = sheet,
            None => self.sheets.push(sheet),
        }
        Ok(key)
    }

    /// Every loaded sheet.
    pub fn sheets(&self) -> &[SheetData] {
        &self.sheets
    }

    /// A sheet by key.
    pub fn sheet(&self, key: &str) -> Option<&SheetData> {
        self.sheets.iter().find(|s| s.key == key)
    }

    /// The manifest a changed file belongs to: the manifest itself, or the
    /// sheet image of a loaded one. For hot reload.
    pub fn atlas_for_path(&self, path: &Path) -> Option<PathBuf> {
        if is_atlas_manifest(path) {
            return Some(path.to_path_buf());
        }
        let path = normalize(path);
        self.sheets
            .iter()
            .find(|s| normalize(&s.image_path) == path)
            .map(|s| s.manifest_path.clone())
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

        match load_sprite_file_with_frames(path, &name) {
            Ok((image, animations, frames)) => {
                // Tags removed in the editor must not linger.
                let own_prefix = format!("{name}/");
                self.animations
                    .retain(|anim| !anim.starts_with(&own_prefix));
                self.insert_sprite(name.clone(), image, animations, frames);
                self.sprites.get(&name)
            }
            Err(e) => {
                warn!("Failed to reload sprite {:?}: {}", path, e);
                None
            }
        }
    }

    fn insert_sprite(
        &mut self,
        name: String,
        image: image::RgbaImage,
        animations: Vec<Animation>,
        frames: u32,
    ) {
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
                uv: Rect::new(0.0, 0.0, 1.0, 1.0),
                texture_index: 0,
                frames: strip_frames(image.width(), image.height(), frames),
                sheet: None,
                image,
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
                            frames: vec![SpriteFrame::whole(pixel_w, pixel_h)],
                            sheet: None,
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

        // Atlas sheets, packed unchanged with their manifests.
        let manifests: Vec<String> = reader
            .entries()
            .iter()
            .map(|e| e.name.clone())
            .filter(|n| {
                n.starts_with(PAK_ATLAS_PREFIX) && n.ends_with(atlas_manifest::ATLAS_SUFFIX)
            })
            .collect();
        for entry in manifests {
            let key = entry[PAK_ATLAS_PREFIX.len()..].to_string();
            let path = PathBuf::from(&entry);
            let loaded = (|| {
                let text = reader
                    .read_entry(&entry)
                    .and_then(|b| std::str::from_utf8(b).ok());
                let manifest = atlas_manifest::parse_manifest(&path, text.unwrap_or(""))?;
                let image_entry = format!("{entry}.image");
                let image = reader
                    .read_entry(&image_entry)
                    .and_then(|bytes| image::load_from_memory(bytes).ok())
                    .ok_or_else(|| AtlasError::Image {
                        path: path.clone(),
                        image: manifest.image.clone(),
                    })?
                    .to_rgba8();
                let atlas = atlas_manifest::build_atlas(
                    &path,
                    &manifest,
                    PathBuf::from(image_entry),
                    image,
                )?;
                self.register_atlas(key, atlas)
            })();
            if let Err(e) = loaded {
                warn!("Skipping packed atlas: {e}");
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

/// Collect sprite files (`(name, path)`) and atlas manifests below `dir`.
fn collect_sprite_files(
    dir: &Path,
    prefix: &str,
    files: &mut Vec<(String, PathBuf)>,
    manifests: &mut Vec<PathBuf>,
) -> Result<(), AssetError> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for path in entries {
        let Some(file_name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if path.is_dir() {
            let new_prefix = if prefix.is_empty() {
                file_name
            } else {
                format!("{prefix}/{file_name}")
            };
            collect_sprite_files(&path, &new_prefix, files, manifests)?;
        } else if is_atlas_manifest(&path) {
            manifests.push(path);
        } else if is_sprite_file(&path) {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = if prefix.is_empty() {
                stem
            } else {
                format!("{prefix}/{stem}")
            };
            files.push((name, path));
        }
    }
    Ok(())
}

/// `path` with `.` and `..` resolved lexically and separators unified, for
/// comparing paths that may be spelled differently.
fn normalize(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Prefix of the pak entries that hold atlas manifests (`atlas/<key>`) and
/// their sheet images (`atlas/<key>.image`), copied unchanged.
pub const PAK_ATLAS_PREFIX: &str = "atlas/";

/// What [`pack_atlases`] did.
#[derive(Debug, Default)]
pub struct PackedAtlases {
    /// Manifests packed.
    pub manifests: usize,
    /// Sheet images packed with them; they are not packed again as sprites.
    pub sheet_images: Vec<PathBuf>,
    /// Manifests left out, and why.
    pub errors: Vec<AtlasError>,
}

/// Copy every valid atlas manifest below `sprites_dir` and its sheet image
/// into `pak` unchanged, for `amigo pack`. Their frames are not re-packed.
pub fn pack_atlases(sprites_dir: &Path, pak: &mut crate::pak::PakWriter) -> PackedAtlases {
    use crate::pak::AssetKind;
    let mut report = PackedAtlases::default();
    let mut files = Vec::new();
    let mut manifests = Vec::new();
    if collect_sprite_files(sprites_dir, "", &mut files, &mut manifests).is_err() {
        return report;
    }
    for manifest_path in manifests {
        let atlas = match atlas_manifest::load_atlas(&manifest_path) {
            Ok(atlas) => atlas,
            Err(e) => {
                report.errors.push(e);
                continue;
            }
        };
        let key = manifest_path
            .strip_prefix(sprites_dir)
            .unwrap_or(&manifest_path)
            .to_string_lossy()
            .replace('\\', "/");
        let (Ok(manifest), Ok(image)) = (
            std::fs::read(&manifest_path),
            std::fs::read(&atlas.image_path),
        ) else {
            continue;
        };
        pak.add(
            format!("{PAK_ATLAS_PREFIX}{key}"),
            AssetKind::AtlasManifest,
            manifest,
        );
        pak.add(
            format!("{PAK_ATLAS_PREFIX}{key}.image"),
            AssetKind::AtlasImage,
            image,
        );
        report.manifests += 1;
        report.sheet_images.push(normalize(&atlas.image_path));
    }
    report
}

/// Whether `path` is one of the sheet images [`pack_atlases`] packed.
pub fn is_packed_sheet(report: &PackedAtlases, path: &Path) -> bool {
    report.sheet_images.contains(&normalize(path))
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
