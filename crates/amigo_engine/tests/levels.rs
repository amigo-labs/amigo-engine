//! Reading `.amigo` levels from game code, and the reload event.

use amigo_engine::prelude::*;

fn assets_with_level(name: &str, contents: &str) -> std::path::PathBuf {
    let assets = std::env::temp_dir().join(format!("amigo_levels_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&assets);
    std::fs::create_dir_all(assets.join("levels")).unwrap();
    std::fs::write(
        assets.join("levels").join(format!("{name}.amigo")),
        contents,
    )
    .unwrap();
    assets
}

/// What `amigo new` and the editor write: no per-layer size, no zones.
#[test]
fn games_load_the_levels_the_editor_writes() {
    let mut level = amigo_core::level::AmigoLevel::new("Level 1", 3, 2, 16);
    level.layers[0].tiles = vec![1, 0, 1, 0, 0, 1];
    level.entities.push(amigo_core::level::EntityPlacement {
        entity_type: "player_spawn".into(),
        x: 16.0,
        y: 0.0,
        properties: Default::default(),
    });
    let text = ron::ser::to_string_pretty(&level, ron::ser::PrettyConfig::default()).unwrap();
    let assets = assets_with_level("level_01", &text);

    let ctx = GameContext::new(320.0, 180.0, assets.to_str().unwrap());
    let loaded: LoadedLevel = ctx.load_level("level_01").expect("level loads");
    assert_eq!(loaded.tile_at_layer("ground", 2, 1), 1);
    assert_eq!(loaded.find_entity("player_spawn").map(|e| e.x), Some(16.0));
    let tiles = TileLayer::from_level(&loaded.layers[0]);
    assert_eq!(tiles.get(0, 0), TileId(1));

    let err = ctx.load_level("level_99").unwrap_err();
    assert!(err.contains("level_99.amigo"), "{err}");
}

#[test]
fn level_reloaded_is_a_registered_event() {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    ctx.events.emit(LevelReloaded {
        name: "level_01".into(),
        path: "assets/levels/level_01.amigo".into(),
    });
    assert!(!ctx.level_reloaded("level_01"), "readable after the flush");
    ctx.events.flush();
    assert!(ctx.level_reloaded("level_01"));
    ctx.events.flush();
    assert!(!ctx.level_reloaded("level_01"), "one tick only");
}
