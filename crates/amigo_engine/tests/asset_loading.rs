//! Asset loading: a packed `game.pak` has to win over loose files.
//!
//! `amigo pack` writes `<assets>/packed/game.pak`, but `load_from_pak` had no
//! caller anywhere, so the archive was written and never read — a packed release
//! still needed the loose `assets/` tree next to the binary. The engine's loader
//! is exercised here through the same `AssetManager` calls it makes.

use amigo_assets::pak::{AssetKind, PakWriter};
use amigo_assets::AssetManager;
use std::path::{Path, PathBuf};

/// The engine's own loader, so this cannot drift from what the engine does.
fn load_assets(assets: &mut AssetManager, assets_path: &Path) -> bool {
    amigo_engine::engine::load_assets(assets, &assets_path.to_string_lossy())
}

/// A 2x1 RGBA PNG: left pixel red, right pixel blue.
fn tiny_png() -> Vec<u8> {
    let mut img = image::RgbaImage::new(2, 1);
    img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
    img.put_pixel(1, 0, image::Rgba([0, 0, 255, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode png");
    bytes.into_inner()
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("amigo_assets_{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sprites")).expect("create sprites dir");
        Self { root }
    }

    /// Write a loose sprite PNG named `<name>.png`.
    fn loose_sprite(&self, name: &str) -> &Self {
        std::fs::write(
            self.root.join("sprites").join(format!("{name}.png")),
            tiny_png(),
        )
        .expect("write loose sprite");
        self
    }

    /// Write a pak containing a one-sprite atlas.
    fn packed_sprite(&self, name: &str) -> &Self {
        let packed_dir = self.root.join("packed");
        std::fs::create_dir_all(&packed_dir).expect("create packed dir");

        // The whole 2x1 atlas is the single sprite, so UVs cover the full image.
        let manifest = format!("[(\"{name}\", (0.0, 0.0, 1.0, 1.0))]");
        let mut writer = PakWriter::new();
        writer.add("atlas.ron", AssetKind::AtlasManifest, manifest.into_bytes());
        writer.add("atlas.png", AssetKind::AtlasImage, tiny_png());
        writer
            .write_to(&packed_dir.join("game.pak"))
            .expect("write pak");
        self
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn loose_sprites_load_when_there_is_no_pak() {
    let fixture = Fixture::new("loose_only");
    fixture.loose_sprite("hero");
    let mut assets = AssetManager::new(fixture.path());

    let packed = load_assets(&mut assets, fixture.path());

    assert!(!packed, "no pak present, so the loose path must be used");
    assert_eq!(assets.sprite_names(), ["hero"]);
}

#[test]
fn a_pak_is_preferred_over_loose_files() {
    let fixture = Fixture::new("pak_wins");
    fixture.loose_sprite("loose_only_sprite");
    fixture.packed_sprite("packed_sprite");
    let mut assets = AssetManager::new(fixture.path());

    let packed = load_assets(&mut assets, fixture.path());

    assert!(packed, "game.pak exists, so it should be used");
    assert_eq!(
        assets.sprite_names(),
        ["packed_sprite"],
        "sprites must come from the archive, not the loose tree"
    );
    let sprite = assets.sprite("packed_sprite").expect("sprite from pak");
    assert_eq!((sprite.width, sprite.height), (2, 1));
}

#[test]
fn a_corrupt_pak_falls_back_to_loose_files() {
    let fixture = Fixture::new("corrupt_pak");
    fixture.loose_sprite("hero");
    let packed_dir = fixture.path().join("packed");
    std::fs::create_dir_all(&packed_dir).expect("create packed dir");
    std::fs::write(packed_dir.join("game.pak"), b"not a pak at all").expect("write junk");
    let mut assets = AssetManager::new(fixture.path());

    let packed = load_assets(&mut assets, fixture.path());

    assert!(!packed, "an unreadable pak must not count as loaded");
    assert_eq!(
        assets.sprite_names(),
        ["hero"],
        "the game should still start from loose files"
    );
}

#[test]
fn a_missing_sprites_dir_is_not_an_error() {
    let root = std::env::temp_dir().join("amigo_assets_empty");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create root");
    let mut assets = AssetManager::new(&root);

    let packed = load_assets(&mut assets, &root);

    assert!(!packed);
    assert!(assets.sprite_names().is_empty());
    let _ = std::fs::remove_dir_all(&root);
}
