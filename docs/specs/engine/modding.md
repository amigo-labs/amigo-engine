---
status: done
crate: amigo_assets
depends_on: ["assets/format"]
last_updated: 2026-03-18
---

# Modding

## Purpose

Provides a data-driven modding framework that allows community content (asset packs, data overrides, new levels, tuning parameters) without recompilation. Mods are distributed as directories with a `mod.toml` manifest. The system integrates with `AssetManager` as a layered loading pipeline where mod assets override base game assets by path.

Sandboxed to data and assets only -- no Rust code execution, no scripting runtime.

## Public API

### ModManifest

```rust
/// Parsed from `mod.toml` in each mod directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    /// SemVer range the mod is compatible with, e.g. ">=0.3.0, <1.0.0".
    pub engine_version: String,
    /// Optional human-readable description shown in the mod manager UI.
    pub description: Option<String>,
    /// Other mod names this mod requires to be loaded first.
    pub dependencies: Vec<String>,
    /// Load priority override. Higher values load later (override more).
    /// Default: 0. Mods with the same priority are ordered alphabetically.
    pub priority: i32,
}
```

### ModInfo

```rust
/// Runtime state for a discovered mod.
#[derive(Clone, Debug)]
pub struct ModInfo {
    pub manifest: ModManifest,
    /// Absolute path to the mod directory on disk.
    pub path: PathBuf,
    /// Whether the mod is currently active.
    pub active: bool,
    /// Validation errors encountered during discovery (missing files, bad TOML, etc.).
    pub errors: Vec<String>,
}
```

### ModError

```rust
#[derive(Debug, thiserror::Error)]
pub enum ModError {
    #[error("Mod directory not found: {0}")]
    DirectoryNotFound(PathBuf),
    #[error("Invalid mod.toml in {mod_name}: {reason}")]
    InvalidManifest { mod_name: String, reason: String },
    #[error("Engine version mismatch for {mod_name}: requires {required}, got {actual}")]
    VersionMismatch { mod_name: String, required: String, actual: String },
    #[error("Missing dependency: {mod_name} requires {dependency}")]
    MissingDependency { mod_name: String, dependency: String },
    #[error("Circular dependency detected: {cycle:?}")]
    CircularDependency { cycle: Vec<String> },
    #[error("Asset override failed: {path}: {reason}")]
    AssetOverride { path: String, reason: String },
    #[error("Data merge failed: {path}: {reason}")]
    DataMerge { path: String, reason: String },
}
```

### DataOverrideMode

```rust
/// How a mod's RON data file interacts with the base data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DataOverrideMode {
    /// Completely replace the base file. The mod file is used as-is.
    Replace,
    /// Merge fields into the base file. Missing fields keep base values.
    /// Nested structs are merged recursively. Vec fields are appended.
    Extend,
}
```

### ModManager

```rust
/// Central mod management system. Discovers, validates, and activates mods.
pub struct ModManager {
    mods_dir: PathBuf,
    discovered: Vec<ModInfo>,
    load_order: Vec<usize>,
    engine_version: String,
}

impl ModManager {
    /// Create a new ModManager that scans the given directory.
    pub fn new(mods_dir: PathBuf, engine_version: &str) -> Self;

    /// Scan the mods directory for mod.toml manifests.
    /// Populates `discovered` with all found mods (valid or invalid).
    pub fn discover(&mut self) -> Result<usize, ModError>;

    /// Return all discovered mods (including inactive/invalid ones).
    pub fn discovered(&self) -> &[ModInfo];

    /// Activate a mod by name. Validates engine version and dependencies.
    pub fn activate(&mut self, name: &str) -> Result<(), ModError>;

    /// Deactivate a mod by name. Also deactivates mods that depend on it.
    pub fn deactivate(&mut self, name: &str) -> Result<Vec<String>, ModError>;

    /// Recompute load order from active mods. Performs topological sort
    /// on dependencies, then sorts by priority within each dependency level.
    pub fn compute_load_order(&mut self) -> Result<(), ModError>;

    /// Return the computed load order as mod names.
    pub fn load_order(&self) -> Vec<&str>;

    /// Apply all active mods to an AssetManager. Processes in load order:
    /// base game assets first, then each mod overlays its assets.
    pub fn apply_to_assets(&self, assets: &mut AssetManager) -> Result<ModReport, ModError>;

    /// Check if a specific mod is active.
    pub fn is_active(&self, name: &str) -> bool;

    /// Set the priority of a mod (persisted in user config, not in mod.toml).
    pub fn set_priority(&mut self, name: &str, priority: i32);
}
```

### ModReport

```rust
/// Summary of what a mod application changed.
#[derive(Clone, Debug, Default)]
pub struct ModReport {
    /// Asset paths that were overridden by mod assets.
    pub overridden_assets: Vec<String>,
    /// Data files that were replaced.
    pub replaced_data: Vec<String>,
    /// Data files that were extended/merged.
    pub extended_data: Vec<String>,
    /// Warnings (non-fatal issues encountered during application).
    pub warnings: Vec<String>,
}
```

## Behavior

### Mod Discovery

`ModManager::discover()` scans the `mods/` directory for immediate subdirectories. Each subdirectory is checked for a `mod.toml` file. If found, the TOML is parsed into a `ModManifest`. If parsing fails or `mod.toml` is absent, the mod is still registered in `discovered` but with `errors` populated and `active` set to `false`.

Directory structure for a mod:
```
mods/
  my_mod/
    mod.toml
    sprites/
      player/idle.png     # overrides base sprites/player/idle.png
    data/
      enemies.ron          # overrides or extends base data/enemies.ron
    levels/
      bonus_level.ron      # new content (no base equivalent)
```

### Activation and Validation

When `activate()` is called:

1. Engine version compatibility is checked against `ModManifest.engine_version` using SemVer range parsing. Mismatch returns `ModError::VersionMismatch`.
2. All declared dependencies must be present in `discovered` and also active. Missing dependencies return `ModError::MissingDependency`.
3. The mod is marked `active = true`.

When `deactivate()` is called, any active mods that list the deactivated mod as a dependency are also deactivated. The names of all cascade-deactivated mods are returned.

### Load Order

`compute_load_order()` builds a directed dependency graph and performs a topological sort. Circular dependencies are detected and reported via `ModError::CircularDependency`. Within the same dependency tier, mods are sorted by `priority` (ascending -- lower priority loads first, higher priority loads later and overrides earlier). Equal-priority mods sort alphabetically by name for determinism.

The final load order is: **base game -> mod_a -> mod_b -> ...** where later entries override earlier ones.

### Asset Override

During `apply_to_assets()`, each active mod's directory is walked in load order. For each file found under a mod's `sprites/`, `audio/`, or `levels/` directory:

- If a base game asset exists at the same relative path, the mod's version replaces it.
- If no base asset exists, the mod asset is added as new content.

Path matching is case-sensitive and uses forward-slash separators internally (normalized on Windows). The path `sprites/player/idle.png` in a mod maps to the asset key `player/idle` in `AssetManager`.

### Data Override (RON Merging)

RON data files under a mod's `data/` directory interact with base data based on a header annotation:

```ron
// data/enemies.ron
#![override(extend)]
(
    goblin: (
        hp: 150,        // overrides base goblin hp
        // attack is not specified -> keeps base value
    ),
    new_enemy: (         // new entry added to the map
        hp: 200,
        attack: 30,
    ),
)
```

- `#![override(replace)]` -- the entire base file is discarded, mod file is used as-is. This is the default when no annotation is present.
- `#![override(extend)]` -- the mod file is merged into the base. Struct fields are overwritten per-field. Map entries are upserted. Vec fields are appended. Nested structs are merged recursively.

If multiple mods extend the same file, they are applied in load order. Each mod's extensions build on the result of the previous mod's merge.

### Integration with AssetManager

`AssetManager` gains a layered loading mode. Internally, it maintains a stack of asset sources:

1. **Base layer**: the game's `assets/` directory (or `game.pak`).
2. **Mod layers**: one per active mod, in load order.

When an asset is requested, the stack is searched top-down (latest mod first). The first source containing the requested path wins. This is transparent to game code -- `assets.sprite("player/idle")` returns the modded version if a mod overrides it.

### Hot Reload Integration

When `HotReloader` detects changes in a mod directory, the affected mod's assets are reloaded. The layered resolution is recomputed so that editing a mod file in dev mode takes effect immediately.

## Internal Design

### Manifest Parsing

`mod.toml` is parsed with the `toml` crate (already a dependency of the engine config system). SemVer range checking uses a minimal inline implementation -- no external `semver` crate. The range syntax supports `>=`, `<`, `=`, and comma-separated constraints.

### Dependency Graph

The dependency graph is stored as an adjacency list (`Vec<Vec<usize>>`) indexed by mod index in the `discovered` list. Topological sort uses Kahn's algorithm (iterative BFS with in-degree tracking). Cycle detection falls out naturally: if the sorted list is shorter than the input, remaining nodes form cycles.

### RON Merging

The extend merge operates on `ron::Value` trees. Both the base and mod files are parsed to `ron::Value`, then merged recursively:

- `Value::Map`: keys from the mod override matching base keys; new keys are inserted.
- `Value::Seq`: mod sequence elements are appended to the base sequence.
- All other types: mod value replaces base value.

The merged `Value` is then deserialized into the target Rust type by the caller.

### Asset Layer Stack

Implemented as a `Vec<AssetLayer>` where each layer holds a path root and an optional `PakReader`. Resolution iterates from the last layer (highest priority mod) to the first (base game), returning the first hit. This adds no overhead when no mods are active (single-layer fast path).

```rust
struct AssetLayer {
    name: String,         // "base" or mod name
    root: PathBuf,        // directory root for loose files
    pak: Option<PakReader>, // optional pak archive
}
```

## Non-Goals

- **Rust code execution.** Mods cannot contain compiled code, shared libraries, or WASM modules. This is a data/asset-only modding system.
- **Scripting runtime.** No Lua, Rhai, or other scripting language is included. Behavior modification is limited to data tuning.
- **Workshop integration.** Steam Workshop, mod.io, or similar platform integration is out of scope. Mods are distributed as directories.
- **Mod compilation.** Mods are not compiled or packed into `.pak` files. They remain as loose files for easy editing.
- **UI implementation.** The `ModManager` provides the data layer. The actual mod manager UI (list, enable, disable, reorder) is left to the game's UI layer.
- **Partial hot reload.** When a mod asset changes, the entire mod layer is reloaded rather than surgically patching individual assets.

## Open Questions

- Should mods be distributable as `.pak` archives in addition to loose directories? This would reduce file count but complicate hot reload.
- Should there be a mod validation tool (`amigo mod check`) that reports compatibility issues before distribution?
- How should mod conflicts be surfaced when two mods override the same asset? Currently last-in-load-order wins silently.
- Should the extend merge support deletion semantics (e.g., `#![remove]` to delete a base entry)?
- Is a scripting layer (Rhai or Lua) desirable for advanced mods in a future iteration?

## Referenzen

- [engine/assets](../assets/pipeline.md) -- Asset loading pipeline and `AssetManager`
- [config/amigo-toml](../config/amigo-toml.md) -- Engine configuration system (TOML parsing)
- [engine/save-load](save-load.md) -- Persistence (mod activation state could be saved here)
- Stardew Valley / SMAPI -- Reference modding framework (data override pattern)
- Minecraft Forge -- Layered resource packs as mod override model
