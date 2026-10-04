//! `amigo pack` collected only PNGs, so a game using `.aseprite` sprites lost
//! them (and their animations) in a release build.

use std::path::Path;
use std::process::Command;

#[test]
fn packed_aseprite_sprites_keep_their_animations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = tempfile::tempdir().unwrap();
    let amigo = env!("CARGO_BIN_EXE_amigo");

    let status = Command::new(amigo)
        .current_dir(scratch.path())
        .args(["new", "packgame", "--template", "custom", "--path"])
        .arg(&root)
        .status()
        .unwrap();
    assert!(status.success());
    let project = scratch.path().join("packgame");
    std::fs::copy(
        root.join("crates/amigo_assets/tests/fixtures/strip.aseprite"),
        project.join("assets/sprites/strip.aseprite"),
    )
    .unwrap();
    image::RgbaImage::new(2, 2)
        .save(project.join("assets/sprites/dot.png"))
        .unwrap();

    let output = Command::new(amigo)
        .current_dir(&project)
        .arg("pack")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "amigo pack failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut assets = amigo_assets::AssetManager::new(project.join("assets"));
    assets
        .load_from_pak(&project.join("assets/packed/game.pak"))
        .expect("pak loads");

    let strip = assets.sprite("strip").expect("aseprite sprite packed");
    assert_eq!((strip.width, strip.height), (16, 2));
    // The first frame survives the atlas round trip pixel for pixel.
    assert_eq!(strip.image.get_pixel(1, 1).0, [255, 0, 0, 255]);
    assert!(assets.sprite("dot").is_some());
    let walk = assets.animation("strip/walk").expect("animations packed");
    assert_eq!(walk.frames.len(), 3);
}
