use amigo_engine::prelude::*;
use serde::Deserialize;

/// Player stats loaded from RON (see `assets/data/player.ron`).
#[derive(Debug, Deserialize)]
pub struct PlayerStats {
    pub speed: f32,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self { speed: 80.0 }
    }
}

/// Load a RON file from `assets/data/`, falling back to the default.
///
/// This goes through `ctx.assets`, which resolves the path against the engine's
/// assets directory. Reading it with `std::fs` and a `CARGO_MANIFEST_DIR` path —
/// which is what this example used to do, because `AssetManager` was not
/// reachable from game code — only ever works from a source checkout.
pub fn load_or_default<T>(ctx: &GameContext, relative_path: &str) -> T
where
    T: serde::de::DeserializeOwned + Default,
{
    match ctx.assets.load_ron::<T>(relative_path) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("warning: using defaults for '{relative_path}': {e}");
            T::default()
        }
    }
}
