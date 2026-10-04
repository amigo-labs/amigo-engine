//! `load_aseprite` existed but nothing called it, so `.aseprite` files in
//! `assets/sprites/` were ignored, and its frame "UVs" held a frame index in
//! `uv.x` rather than a rect. The fixture is written by
//! `scripts/gen_aseprite_fixtures.py`: four 4x2 frames of solid red, green,
//! blue and white, and tags that exercise directions, repeats and bad ranges.

use amigo_assets::{AssetManager, load_aseprite, load_sprite_file};
use amigo_core::Rect;
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/strip.aseprite")
}

fn frame_starts(name: &str, anims: &[amigo_animation::Animation]) -> Vec<f32> {
    anims
        .iter()
        .find(|a| a.name == name)
        .unwrap_or_else(|| panic!("no animation {name}"))
        .frames
        .iter()
        .map(|f| f.uv.x)
        .collect()
}

#[test]
fn frames_become_a_strip_and_tags_point_into_it() {
    let data = load_aseprite(&fixture()).expect("fixture parses");
    assert_eq!((data.width, data.height), (4, 2));
    assert_eq!(data.frames.len(), 4);

    let strip = data.strip();
    assert_eq!(strip.dimensions(), (16, 2));
    for (frame, rgba) in [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            strip.get_pixel(frame as u32 * 4 + 1, 1).0,
            *rgba,
            "frame {frame}"
        );
    }

    let walk = data
        .animations
        .iter()
        .find(|a| a.name == "strip/walk")
        .unwrap();
    assert!(walk.looping);
    assert_eq!(walk.frames[0].uv, Rect::new(0.0, 0.0, 0.25, 1.0));
    assert_eq!(walk.frames[2].uv, Rect::new(0.5, 0.0, 0.25, 1.0));
    // 100 ms at 60 Hz is 6 ticks, 200 ms 12.
    assert_eq!(
        walk.frames.iter().map(|f| f.duration).collect::<Vec<_>>(),
        [6, 6, 12]
    );
}

#[test]
fn tag_direction_repeat_and_range_are_honoured() {
    let anims = load_aseprite(&fixture()).unwrap().animations;
    assert_eq!(frame_starts("strip/back", &anims), [0.75, 0.5, 0.25]);
    assert_eq!(frame_starts("strip/bounce", &anims), [0.0, 0.25, 0.5, 0.25]);

    let twice = anims.iter().find(|a| a.name == "strip/twice").unwrap();
    assert!(!twice.looping, "a repeat count means the tag stops");
    assert_eq!(twice.frames.len(), 2);

    // `overflow` runs past the last frame and is clamped; `missing` starts
    // past it and is dropped.
    assert_eq!(frame_starts("strip/overflow", &anims), [0.5, 0.75]);
    assert!(!anims.iter().any(|a| a.name == "strip/missing"));
}

#[test]
fn the_asset_manager_loads_aseprite_sprites_with_their_animations() {
    let dir = tempfile::tempdir().unwrap();
    let sprites = dir.path().join("sprites/hero");
    std::fs::create_dir_all(&sprites).unwrap();
    std::fs::copy(fixture(), sprites.join("strip.aseprite")).unwrap();

    let mut assets = AssetManager::new(dir.path());
    assets.load_sprites().unwrap();

    let sprite = assets.sprite("hero/strip").expect("sprite registered");
    assert_eq!((sprite.width, sprite.height), (16, 2));
    // Animations are named after the sprite, subdirectory included.
    let walk = assets.animation("hero/strip/walk").expect("walk tag");
    assert_eq!(walk.frames.len(), 3);
    assert!(assets.animation("strip/walk").is_none());
}

#[test]
fn hot_reload_replaces_an_aseprite_sprite_and_its_animations() {
    let dir = tempfile::tempdir().unwrap();
    let sprites = dir.path().join("sprites");
    std::fs::create_dir_all(&sprites).unwrap();
    let path = sprites.join("strip.aseprite");
    std::fs::copy(fixture(), &path).unwrap();

    let mut assets = AssetManager::new(dir.path());
    let reloaded = assets.reload_sprite(&path).expect("aseprite reloads");
    assert_eq!(reloaded.width, 16);
    assert!(assets.animation("strip/bounce").is_some());
}

#[test]
fn a_png_is_a_sprite_without_animations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dot.png");
    image::RgbaImage::new(3, 5).save(&path).unwrap();
    let (image, animations) = load_sprite_file(&path, "dot").unwrap();
    assert_eq!(image.dimensions(), (3, 5));
    assert!(animations.is_empty());
}
