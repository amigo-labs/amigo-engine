---
status: done
depends_on: []
last_updated: 2026-03-18
---

# Data Formats: RON & TOML

## Purpose

Documents the conventions for RON and TOML usage across the Amigo Engine, when to use which format, and the serialization patterns employed throughout the codebase.

## Public API

```rust
// RON deserialization (game data)
let tower: TowerDef = ron::from_str(&std::fs::read_to_string("assets/data/towers.ron")?)?;

// TOML deserialization (engine config)
let config: EngineConfig = toml::from_str(&std::fs::read_to_string("amigo.toml")?)?;
```

## Behavior

### Format Selection Rules

| Format | Use Case | Hot Reload | Rationale |
|--------|----------|------------|-----------|
| **TOML** | Engine configuration (`amigo.toml`) | No (restart required) | Standard config format, familiar to Rust developers, flat key-value structure ideal for settings |
| **RON** | Game data (tower stats, wave configs, enemy definitions) | Yes (dev mode) | Rust-native, supports enums/structs directly, readable, maps 1:1 to Rust types |
| **RON** | Input mappings (`input.ron`) | Yes | Rebindable by player, complex nested structure |
| **RON** | Level files (`.amigo`) | Yes (dev mode) | Complex nested data (tilemaps, entity lists, paths) |
| **RON** | Style definitions (art + audio) | Yes (dev mode) | Nested structures with enums, palettes, prompt templates |
| **RON** | Adaptive music configs (`.music.ron`, `.sequence.ron`) | Yes (dev mode) | Complex layer rules, transition definitions |
| **RON** | SFX definitions (`sfx.ron`) | Yes (dev mode) | Variant lists, playback parameters |
| **JSON** | ComfyUI workflow templates | No | Required by ComfyUI API, not an engine format |
| **JSON** | MCP server configuration | No | Required by MCP protocol |

### When to Use TOML

- Flat or shallow configuration (key = value)
- Settings that users edit manually
- Files that feel like "config files" (engine settings, build profiles)
- One file per concern: `amigo.toml` for the engine

### When to Use RON

- Data that maps to Rust structs/enums
- Nested structures (tilemaps, entity hierarchies, rule sets)
- Game content that designers iterate on (tower stats, wave configs)
- Anything that benefits from hot reload during development
- Collections of similar items (palette colors, animation frames, stem layers)

### RON Patterns Used in the Engine

#### Struct-style data

```ron
// assets/data/towers.ron
(
    towers: {
        "archer": (
            damage: 10,
            range: 5.0,
            fire_rate: 1.0,
            cost: 100,
        ),
    },
)
```

#### Enum variants

```ron
// Layer rules in adaptive music
("drums", Volume, Threshold(param: "tension", above: 0.3, fade: 0.5)),
```

#### Nested collections

```ron
// Style definitions with maps and arrays
palette: [
    "#1a1a2e",
    "#e8c170",
    "#8b5e3c",
],
stem_instruments: {
    "drums": "war drums, snare, tambourine",
    "bass":  "double bass, pizzicato cello",
},
```

### TOML Patterns Used in the Engine

#### Sections for grouping

```toml
[window]
title = "Pirate TD"
width = 1280

[render]
virtual_width = 480
scale_mode = "pixel_perfect"

[audio]
master_volume = 0.8
```

#### Build profiles

```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
panic = "abort"
opt-level = 2
```

### Serialization Conventions

- All serializable types derive `Serialize, Deserialize` via serde
- `Fix` (I16F16) values are represented as `f32` literals in data files, converted to `Fix` on load
- `RenderVec2` is never serialized (rendering-only type)
- `SimVec2` is serialized for save games and replays
- Entity references in data files use string IDs, resolved to `EntityId` at load time
- All game state is fully serializable for save/load and replay (see `GameState`)

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct GameState {
    pub tick: u64,
    pub rng: SerializableRng,
    pub gold: i32,
    pub lives: i32,
    pub wave: WaveState,
    pub towers: EntityPool<Tower>,
    pub enemies: EntityPool<Enemy>,
    pub projectiles: EntityPool<Projectile>,
    pub tilemap: TileMap,
}
```

### MCP Configuration (JSON)

Three MCP servers configured via JSON:

```json
// ~/.claude/claude_code_config.json
{
  "mcpServers": {
    "amigo": {
      "command": "amigo",
      "args": ["mcp-server", "--port", "9999"]
    },
    "amigo-artgen": {
      "command": "amigo-artgen",
      "args": ["--server", "http://localhost:8188"]
    },
    "amigo-audiogen": {
      "command": "amigo-audiogen",
      "args": ["--acestep", "http://localhost:7860"]
    }
  }
}
```

Three MCP servers side by side: `amigo` for engine control, `amigo-artgen` for pixel art asset generation via ComfyUI, `amigo-audiogen` for music and sound effect generation via ACE-Step/AudioGen. Claude Code sees all three tool sets simultaneously.

### Licensing Context for Generated Assets

All generated audio is royalty-free and commercially usable:

| Model | License | Training Data | Commercial Use |
|-------|---------|--------------|----------------|
| ACE-Step 1.5 | Apache 2.0 | Original training data | Yes |
| AudioGen (AudioCraft) | MIT (code) | Public sound effects | Yes (verify per-model) |
| Demucs (stem split) | MIT | N/A (inference only) | Yes |

Generated output is original -- not copies of training data. Standard disclaimer: verify uniqueness of generated tracks before commercial release.

## Implementation

| Component | File | Details |
|-----------|------|---------|
| TOML config | `crates/amigo_engine/src/config.rs` | `EngineConfig` with `toml::from_str()` |
| Input bindings (RON) | `crates/amigo_input/src/action_map.rs` | `ActionBindings` with `Key`/`MouseButton`/`GamepadButton` enum |
| Level data (RON) | `crates/amigo_editor/src/lib.rs` | `AmigoLevel` with `save_level()`/`load_level()` |
| Hot reload | `crates/amigo_assets/src/hot_reload.rs` | `HotReloader` using `notify` v7, watches `Modify`/`Create` events |

### ActionBindings Format (actual)

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputBinding {
    Key(String),         // e.g., "Space", "KeyW", "ArrowUp"
    MouseButton(u8),     // 0=Left, 1=Right, 2=Middle
    GamepadButton(u8),   // Button index
}
```

### AmigoLevel Format (actual)

```rust
pub struct AmigoLevel {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub layers: Vec<LayerData>,
    pub entities: Vec<EntityPlacement>,
    pub paths: Vec<PathData>,
    pub metadata: HashMap<String, String>,
}
```

## Internal Design

Both RON and TOML parsing use serde for deserialization into strongly-typed Rust structs. Hot-reloadable files are watched via the `notify` crate v7 (see [assets/pipeline](../assets/pipeline.md)). When a watched file changes, it is re-parsed and the corresponding in-memory representation is updated.

Error handling for data files follows the engine's three-layer approach:
- **Dev mode:** Warning with fuzzy-match suggestion (`"playe" -> did you mean "player"?`)
- **Release mode:** Fallback values, silent log

```rust
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Asset not found: {path} (did you mean '{suggestion}'?)")]
    AssetNotFound { path: String, suggestion: Option<String> },
    #[error("Asset parse error: {path}: {reason}")]
    AssetParse { path: String, reason: String },
}
```

## Non-Goals

- Supporting YAML, XML, or other data formats
- Schema generation for external validation tools
- Binary serialization for data files (only for `game.pak` packing)

## Open Questions

- Whether to add RON schema validation at build time via a custom derive macro
- Whether to support RON `#[serde(default)]` consistently across all data types
- Migration strategy when RON struct definitions change between engine versions
