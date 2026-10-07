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

#[test]
fn atlas_sheets_are_packed_unchanged() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = tempfile::tempdir().unwrap();
    let amigo = env!("CARGO_BIN_EXE_amigo");

    let status = Command::new(amigo)
        .current_dir(scratch.path())
        .args(["new", "atlasgame", "--template", "custom", "--path"])
        .arg(&root)
        .status()
        .unwrap();
    assert!(status.success());
    let project = scratch.path().join("atlasgame");
    let sprites = project.join("assets/sprites");
    let mut sheet = image::RgbaImage::new(16, 8);
    for x in 0..16 {
        sheet.put_pixel(x, 3, image::Rgba([x as u8 * 10, 200, 7, 255]));
    }
    sheet.save(sprites.join("sheet.png")).unwrap();
    std::fs::write(
        sprites.join("hero.atlas.ron"),
        r#"(image: "sheet.png", sprites: {
            "hero/run": (frames: [(x: 0, y: 0, w: 8, h: 8), (x: 8, y: 0, w: 8, h: 8)], origin: (4.0, 8.0), fps: Some(12.0), looping: true),
        })"#,
    )
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

    let pak_path = project.join("assets/packed/game.pak");
    let reader = amigo_assets::PakReader::open(&pak_path).expect("pak opens");
    let packed_png = reader
        .read_entry("atlas/hero.atlas.ron.image")
        .expect("sheet packed");
    assert_eq!(
        packed_png,
        std::fs::read(sprites.join("sheet.png")).unwrap()
    );

    let mut assets = amigo_assets::AssetManager::new(project.join("assets"));
    assets.load_from_pak(&pak_path).expect("pak loads");
    assert!(
        assets.sprite("sheet").is_none(),
        "the sheet is no sprite of its own"
    );
    let run = assets
        .sprite("hero/run")
        .expect("atlas sprite from the pak");
    assert_eq!(run.frames.len(), 2);
    assert_eq!(run.frames[1].origin, [4.0, 8.0]);
    let sheet = assets.sheet("hero.atlas.ron").expect("sheet registered");
    assert_eq!(sheet.image.get_pixel(5, 3).0, [50, 200, 7, 255]);
    assert!(assets.animation("hero/run").is_some());
}
