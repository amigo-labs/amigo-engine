---
status: spec
depends_on: ["config/amigo-toml", "ai-pipelines/artgen", "ai-pipelines/audiogen"]
last_updated: 2026-03-19
---

# Art & Audio Generation Defaults ("Ask Once, Save Forever")

## Purpose

When Claude calls an art or audio generation MCP tool and a parameter is missing (sprite size, palette, style, BPM, etc.), the system should: (1) check `amigo.toml` for a project-level default, (2) if not found, prompt the user once, (3) save the answer to `amigo.toml` so it never asks again.

This eliminates repetitive questions while ensuring generated assets are consistent across a project.

## Public API

### Config in `amigo.toml`

```toml
[art]
default_sprite_size = 32
default_style = "caribbean"
default_palette = "nes"
color_depth = 8
background_style = "parallax"
tileset_tile_size = 16

[audio]
default_genre = "chiptune"
default_bpm = 120
default_key = "C minor"
sfx_duration = 2.0
music_duration = 30.0
sample_rate = 44100
output_format = "wav"
```

### MCP Tools

```
amigo_artgen_get_defaults()
  -> { "default_sprite_size": 32, "default_style": "caribbean", ... }

amigo_artgen_set_defaults(defaults)
  -> { "saved": true, "path": "amigo.toml" }
  # Merges into existing [art] section

amigo_audiogen_get_defaults()
  -> { "default_bpm": 120, "default_genre": "chiptune", ... }

amigo_audiogen_set_defaults(defaults)
  -> { "saved": true, "path": "amigo.toml" }
  # Merges into existing [audio] section
```

## Behavior

### Default Resolution Order

For every optional parameter in an art/audio gen tool:

```
1. Explicit parameter value from Claude's tool call
   |
   v (if missing)
2. Project default from amigo.toml [art] or [audio]
   |
   v (if missing)
3. Style default (e.g., caribbean style has default_size = 32x32)
   |
   v (if missing)
4. Hardcoded fallback (last resort, always exists)
```

If resolution reaches step 3 or 4, the MCP tool response includes a `defaults_missing` hint:

```json
{
    "result": { "output": "assets/generated/sprites/player.png" },
    "hints": {
        "defaults_missing": ["sprite_size", "palette"],
        "suggestion": "Run amigo_artgen_set_defaults to save project defaults"
    }
}
```

Claude sees this hint and can ask the user once, then call `set_defaults` to save.

### Parameter Mapping

#### Art Parameters

| Parameter | amigo.toml key | Style fallback | Hardcoded fallback |
|-----------|---------------|----------------|-------------------|
| Sprite size | `art.default_sprite_size` | `StyleDef.default_size` | 32 |
| Style name | `art.default_style` | -- | "caribbean" |
| Palette | `art.default_palette` | `StyleDef.palette` | (no clamp) |
| Color depth | `art.color_depth` | -- | 32 (no limit) |
| Tileset tile size | `art.tileset_tile_size` | -- | 16 |
| Background style | `art.background_style` | -- | "static" |
| Outline | `art.add_outline` | `PostProcessConfig.add_outline` | true |
| Outline color | `art.outline_color` | `PostProcessConfig.outline_color` | "#1a1a2e" |

#### Audio Parameters

| Parameter | amigo.toml key | Style fallback | Hardcoded fallback |
|-----------|---------------|----------------|-------------------|
| BPM | `audio.default_bpm` | `WorldAudioStyle.default_bpm` | 120 |
| Genre | `audio.default_genre` | `WorldAudioStyle.genre` | "" |
| Music duration | `audio.music_duration` | -- | 30.0 |
| SFX duration | `audio.sfx_duration` | -- | 2.0 |
| Key | `audio.default_key` | -- | "C minor" |
| Sample rate | `audio.sample_rate` | -- | 44100 |
| Output format | `audio.output_format` | -- | "wav" |

### The "Ask Once" Flow (Claude's Perspective)

1. User says: "Generate a player sprite"
2. Claude calls `amigo_artgen_generate_sprite(prompt: "player idle", style: "caribbean")`
3. Tool succeeds but response includes `defaults_missing: ["sprite_size"]`
4. Claude asks user: "What sprite size should I use? 16x16, 32x32, or 64x64?"
5. User answers: "32"
6. Claude calls `amigo_artgen_set_defaults({"default_sprite_size": 32})`
7. From now on, all sprite generations use 32x32 unless explicitly overridden

### Config File Merging

`set_defaults` performs a **merge**, not overwrite:

```toml
# Before: amigo.toml has
[art]
default_style = "caribbean"

# Claude calls: amigo_artgen_set_defaults({"default_sprite_size": 32, "color_depth": 8})

# After: merged
[art]
default_style = "caribbean"
default_sprite_size = 32
color_depth = 8
```

Existing keys are preserved unless explicitly included in the `set_defaults` call.

## Internal Design

### Art Gen Integration (`tools/amigo_artgen/src/tools.rs`)

Before executing a generation tool, load defaults:

```rust
fn resolve_params(params: &GenerateSpriteParams, project_dir: &Path) -> ResolvedSpriteParams {
    let config_defaults = load_art_defaults(project_dir); // reads amigo.toml [art]
    let style_defaults = StyleDef::find(&params.style);

    ResolvedSpriteParams {
        size: params.size
            .or(config_defaults.default_sprite_size.map(|s| [s, s]))
            .or(style_defaults.map(|s| [s.default_size.0, s.default_size.1]))
            .unwrap_or([32, 32]),
        // ... same pattern for other params
    }
}
```

### Audio Gen Integration (`tools/amigo_audiogen/src/tools.rs`)

Same pattern:

```rust
fn resolve_params(params: &GenerateMusicParams, project_dir: &Path) -> ResolvedMusicParams {
    let config_defaults = load_audio_defaults(project_dir);
    let style_defaults = WorldAudioStyle::find(&params.world);

    ResolvedMusicParams {
        bpm: if params.bpm > 0 { params.bpm }
            else { config_defaults.default_bpm
                .or(style_defaults.map(|s| s.default_bpm))
                .unwrap_or(120) },
        // ...
    }
}
```

### Config Read/Write (`tools/amigo_artgen/src/config.rs`, `tools/amigo_audiogen/src/config.rs`)

```rust
fn load_art_defaults(project_dir: &Path) -> ArtDefaults {
    let path = project_dir.join("amigo.toml");
    let content = fs::read_to_string(&path).unwrap_or_default();
    let config: toml::Value = toml::from_str(&content).unwrap_or_default();
    // Extract [art] section, deserialize with #[serde(default)]
}

fn save_art_defaults(project_dir: &Path, updates: &HashMap<String, toml::Value>) {
    let path = project_dir.join("amigo.toml");
    let mut config: toml::Value = /* load existing */;
    // Merge updates into [art] section
    // Write back with toml::to_string_pretty
}
```

### MCP Tool Definitions

Added to existing tool lists in `tools.rs`:

```rust
ToolDef {
    name: "amigo_artgen_get_defaults",
    description: "Get project art generation defaults from amigo.toml",
    input_schema: json!({ "type": "object", "properties": {} }),
},
ToolDef {
    name: "amigo_artgen_set_defaults",
    description: "Save art generation defaults to amigo.toml [art] section. \
        Merges with existing values. Use after asking the user for preferences.",
    input_schema: json!({
        "type": "object",
        "properties": {
            "default_sprite_size": { "type": "integer", "description": "Default sprite size in pixels (e.g., 16, 32, 64)" },
            "default_style": { "type": "string", "description": "Default art style name" },
            "default_palette": { "type": "string", "description": "Default color palette (e.g., 'nes', 'snes', 'gameboy')" },
            "color_depth": { "type": "integer", "description": "Color depth (8, 16, 24, 32)" },
            "tileset_tile_size": { "type": "integer", "description": "Default tileset tile size" },
            "add_outline": { "type": "boolean", "description": "Add pixel outline to sprites" },
            "outline_color": { "type": "string", "description": "Outline color as hex (#RRGGBB)" }
        }
    }),
},
```

## Non-Goals

- Per-asset overrides (use explicit tool parameters instead)
- Version history of defaults (just a flat config section)
- Validating parameter combinations (e.g., palette vs color_depth conflicts)
- UI for editing defaults (text editor or Claude)

## Open Questions

- Whether to support per-world defaults (e.g., `[art.caribbean]` with different palette than `[art.dune]`)
- Whether to add a `amigo_artgen_reset_defaults()` tool
- Whether defaults should be inherited from a parent template
