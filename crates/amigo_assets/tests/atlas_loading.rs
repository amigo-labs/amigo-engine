//! Atlas manifests (`*.atlas.ron`) through `AssetManager::load_sprites`
//! (rendering-extensions R8).

use amigo_assets::{AssetManager, AtlasError};
use std::path::Path;

fn write(dir: &Path, rel: &str, contents: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("mkdir");
    std::fs::write(path, contents).expect("write");
}

fn sheet(dir: &Path, rel: &str, w: u32, h: u32) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("mkdir");
    image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]))
        .save(path)
        .expect("png");
}

const TWO_SPRITES: &str = r#"(
    image: "sheet.png",
    sprites: {
        "hamster/fly": (
            frames: [(x: 0, y: 0, w: 8, h: 8), (x: 8, y: 0, w: 8, h: 8, origin: Some((1.0, 1.0)))],
            origin: (4.0, 4.0),
            fps: Some(19.0),
            looping: true,
        ),
        "star": (frames: [(x: 0, y: 8, w: 4, h: 4)]),
    },
)"#;

#[test]
fn a_manifest_loads_one_sheet_and_registers_its_sprites() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sprites = dir.path().join("sprites");
    sheet(&sprites, "sheet.png", 16, 16);
    write(&sprites, "hamster.atlas.ron", TWO_SPRITES);
    sheet(&sprites, "plain.png", 2, 2);

    let mut assets = AssetManager::new(dir.path());
    assets.load_sprites().expect("loads");

    assert_eq!(assets.sheets().len(), 1);
    let sheet = &assets.sheets()[0];
    assert_eq!(sheet.key, "hamster.atlas.ron");
    assert_eq!(sheet.image.dimensions(), (16, 16));
    // The sheet image is not a sprite of its own.
    assert!(assets.sprite("sheet").is_none());
    assert!(assets.sprite("plain").is_some());

    let fly = assets.sprite("hamster/fly").expect("registered");
    assert_eq!(fly.sheet.as_deref(), Some("hamster.atlas.ron"));
    assert_eq!(fly.frames.len(), 2);
    assert_eq!(fly.frames[0].origin, [4.0, 4.0]);
    assert_eq!(fly.frames[1].origin, [1.0, 1.0]);
    assert_eq!((fly.width, fly.height), (8, 8));
    let star = assets.sprite("star").expect("registered");
    assert_eq!(star.frames.len(), 1);

    let anim = assets
        .animation("hamster/fly")
        .expect("fps registers an animation");
    assert_eq!(anim.frames.len(), 2);
    assert!(anim.looping);
    assert!(assets.animation("star").is_none());
}

#[test]
fn a_broken_manifest_is_skipped_and_the_rest_still_loads() {
    let cases = [
        ("not ron at all", "Parse"),
        (r#"(image: "missing.png", sprites: {})"#, "Image"),
        (
            r#"(image: "sheet.png", sprites: { "x": (frames: [(x: 12, y: 0, w: 8, h: 8)]) })"#,
            "FrameOutOfBounds",
        ),
        (
            r#"(image: "sheet.png", sprites: { "x": (frames: []) })"#,
            "NoFrames",
        ),
        (
            r#"(image: "sheet.png", sprites: { "x": (frames: [(x: 0, y: 0, w: 2, h: 2)], fps: Some(0.0)) })"#,
            "InvalidFps",
        ),
        (
            r#"(image: "sheet.png", sprites: { "plain": (frames: [(x: 0, y: 0, w: 2, h: 2)]) })"#,
            "DuplicateName",
        ),
    ];
    for (manifest, expected) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let sprites = dir.path().join("sprites");
        sheet(&sprites, "sheet.png", 16, 16);
        sheet(&sprites, "plain.png", 2, 2);
        write(&sprites, "bad.atlas.ron", manifest);

        let mut assets = AssetManager::new(dir.path());
        assets
            .load_sprites()
            .expect("one bad manifest fails nothing else");
        assert!(assets.sprite("plain").is_some(), "{expected}");
        assert!(assets.sheets().is_empty(), "{expected}");
        let err = assets
            .load_atlas_file(&sprites.join("bad.atlas.ron"))
            .expect_err(expected);
        let variant = match err {
            AtlasError::Parse { .. } => "Parse",
            AtlasError::Image { .. } => "Image",
            AtlasError::FrameOutOfBounds { .. } => "FrameOutOfBounds",
            AtlasError::NoFrames { .. } => "NoFrames",
            AtlasError::DuplicateName { .. } => "DuplicateName",
            AtlasError::InvalidFps { .. } => "InvalidFps",
            AtlasError::MipPadding { .. } => "MipPadding",
        };
        assert_eq!(variant, expected);
    }
}

#[test]
fn two_manifests_cannot_share_a_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sprites = dir.path().join("sprites");
    sheet(&sprites, "a.png", 4, 4);
    sheet(&sprites, "b.png", 4, 4);
    write(
        &sprites,
        "a.atlas.ron",
        r#"(image: "a.png", sprites: { "x": (frames: [(x: 0, y: 0, w: 4, h: 4)]) })"#,
    );
    write(
        &sprites,
        "b.atlas.ron",
        r#"(image: "b.png", sprites: { "x": (frames: [(x: 0, y: 0, w: 4, h: 4)]) })"#,
    );
    let mut assets = AssetManager::new(dir.path());
    assets.load_sprites().expect("loads");
    assert_eq!(assets.sheets().len(), 1);
    assert_eq!(
        assets.sprite("x").and_then(|s| s.sheet.clone()).as_deref(),
        Some("a.atlas.ron")
    );
}

#[test]
fn reloading_a_manifest_replaces_its_sprites() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sprites = dir.path().join("sprites");
    sheet(&sprites, "sheet.png", 16, 16);
    write(&sprites, "hamster.atlas.ron", TWO_SPRITES);
    let mut assets = AssetManager::new(dir.path());
    assets.load_sprites().expect("loads");

    write(
        &sprites,
        "hamster.atlas.ron",
        r#"(image: "sheet.png", sprites: { "star": (frames: [(x: 0, y: 0, w: 16, h: 16)]) })"#,
    );
    let manifest = assets
        .atlas_for_path(&sprites.join("sheet.png"))
        .expect("the sheet image belongs to the manifest");
    assets.load_atlas_file(&manifest).expect("reloads");
    assert!(assets.sprite("hamster/fly").is_none());
    assert!(assets.animation("hamster/fly").is_none());
    assert_eq!(assets.sprite("star").map(|s| s.width), Some(16));
    assert_eq!(assets.sprite_names(), ["star"]);
}

#[test]
fn plain_and_aseprite_sprites_report_their_frames() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sprites = dir.path().join("sprites");
    sheet(&sprites, "dot.png", 3, 3);
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/strip.aseprite"),
        sprites.join("strip.aseprite"),
    )
    .expect("copy fixture");
    let mut assets = AssetManager::new(dir.path());
    assets.load_sprites().expect("loads");
    assert_eq!(assets.sprite("dot").map(|s| s.frames.len()), Some(1));
    let strip = assets.sprite("strip").expect("aseprite");
    assert_eq!(strip.frames.len(), 4);
    assert_eq!(strip.frames[1].x, 4);
}
