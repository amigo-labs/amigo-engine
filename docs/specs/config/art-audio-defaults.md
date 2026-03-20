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

## Acceptance Criteria

### API Completeness

#### Config in `amigo.toml` — Art Section
- [ ] `[art]` section is recognized and parsed from `amigo.toml`
- [ ] `art.default_sprite_size` key is supported (integer)
- [ ] `art.default_style` key is supported (string)
- [ ] `art.default_palette` key is supported (string)
- [ ] `art.color_depth` key is supported (integer)
- [ ] `art.background_style` key is supported (string)
- [ ] `art.tileset_tile_size` key is supported (integer)
- [ ] `art.add_outline` key is supported (boolean)
- [ ] `art.outline_color` key is supported (string, hex)

#### Config in `amigo.toml` — Audio Section
- [ ] `[audio]` section is recognized and parsed from `amigo.toml`
- [ ] `audio.default_genre` key is supported (string)
- [ ] `audio.default_bpm` key is supported (integer)
- [ ] `audio.default_key` key is supported (string)
- [ ] `audio.sfx_duration` key is supported (float)
- [ ] `audio.music_duration` key is supported (float)
- [ ] `audio.sample_rate` key is supported (integer)
- [ ] `audio.output_format` key is supported (string)

#### MCP Tool: `amigo_artgen_get_defaults()`
- [ ] Tool exists with name `amigo_artgen_get_defaults`
- [ ] Tool description: "Get project art generation defaults from amigo.toml"
- [ ] Tool accepts empty input schema (`{}`)
- [ ] Tool returns all art defaults from `[art]` section as JSON object

#### MCP Tool: `amigo_artgen_set_defaults(defaults)`
- [ ] Tool exists with name `amigo_artgen_set_defaults`
- [ ] Tool description mentions merging with existing values
- [ ] Tool accepts `default_sprite_size` parameter (integer)
- [ ] Tool accepts `default_style` parameter (string)
- [ ] Tool accepts `default_palette` parameter (string)
- [ ] Tool accepts `color_depth` parameter (integer)
- [ ] Tool accepts `tileset_tile_size` parameter (integer)
- [ ] Tool accepts `add_outline` parameter (boolean)
- [ ] Tool accepts `outline_color` parameter (string)
- [ ] Tool returns `{"saved": true, "path": "amigo.toml"}`

#### MCP Tool: `amigo_audiogen_get_defaults()`
- [ ] Tool exists with name `amigo_audiogen_get_defaults`
- [ ] Tool description: "Get project audio generation defaults from amigo.toml"
- [ ] Tool accepts empty input schema
- [ ] Tool returns all audio defaults from `[audio]` section as JSON object

#### MCP Tool: `amigo_audiogen_set_defaults(defaults)`
- [ ] Tool exists with name `amigo_audiogen_set_defaults`
- [ ] Tool description mentions merging with existing values
- [ ] Tool returns `{"saved": true, "path": "amigo.toml"}`

#### Internal Functions — Art Gen (`tools/amigo_artgen/src/`)
- [ ] `resolve_params(params: &GenerateSpriteParams, project_dir: &Path) -> ResolvedSpriteParams` function exists
- [ ] `load_art_defaults(project_dir: &Path) -> ArtDefaults` function exists
- [ ] `save_art_defaults(project_dir: &Path, updates: &HashMap<String, toml::Value>)` function exists
- [ ] `ArtDefaults` struct exists with `#[serde(default)]` deserialization

#### Internal Functions — Audio Gen (`tools/amigo_audiogen/src/`)
- [ ] `resolve_params(params: &GenerateMusicParams, project_dir: &Path) -> ResolvedMusicParams` function exists
- [ ] `load_audio_defaults(project_dir: &Path) -> AudioDefaults` function exists
- [ ] `save_audio_defaults(project_dir: &Path, updates: &HashMap<String, toml::Value>)` function exists
- [ ] `AudioDefaults` struct exists with `#[serde(default)]` deserialization

### Behavior

#### Default Resolution Order
- [ ] Step 1: Explicit parameter value from the tool call is used if present
- [ ] Step 2: If missing, project default from `amigo.toml` `[art]` or `[audio]` section is used
- [ ] Step 3: If missing, style default (e.g., `StyleDef.default_size`) is used
- [ ] Step 4: If missing, hardcoded fallback is used (always exists, never fails)
- [ ] Resolution proceeds through steps in order, stopping at the first available value

#### Art Parameter Hardcoded Fallbacks
- [ ] Sprite size falls back to `32`
- [ ] Style name falls back to `"caribbean"`
- [ ] Color depth falls back to `32` (no limit)
- [ ] Tileset tile size falls back to `16`
- [ ] Background style falls back to `"static"`
- [ ] Outline falls back to `true`
- [ ] Outline color falls back to `"#1a1a2e"`

#### Audio Parameter Hardcoded Fallbacks
- [ ] BPM falls back to `120`
- [ ] Genre falls back to `""`
- [ ] Music duration falls back to `30.0`
- [ ] SFX duration falls back to `2.0`
- [ ] Key falls back to `"C minor"`
- [ ] Sample rate falls back to `44100`
- [ ] Output format falls back to `"wav"`

#### `defaults_missing` Hint
- [ ] If resolution reaches step 3 (style fallback) or step 4 (hardcoded fallback), the tool response includes a `defaults_missing` hint
- [ ] `defaults_missing` is an array of parameter names that were not found in explicit or project defaults
- [ ] Response includes a `suggestion` field: `"Run amigo_artgen_set_defaults to save project defaults"` (or audiogen equivalent)
- [ ] Hint is included in the `hints` object alongside the `result`

#### "Ask Once" Flow
- [ ] After a generation tool returns `defaults_missing`, Claude can call `set_defaults` to save preferences
- [ ] Saved defaults are used in all subsequent generation calls without re-prompting
- [ ] Explicit parameter values in tool calls always override saved defaults

#### Config File Merging (`set_defaults`)
- [ ] `set_defaults` performs a merge, not an overwrite of the entire `[art]` or `[audio]` section
- [ ] Existing keys not included in the `set_defaults` call are preserved
- [ ] Keys included in the `set_defaults` call are added or updated
- [ ] The resulting `amigo.toml` is written with `toml::to_string_pretty`

#### Config File Read
- [ ] `load_art_defaults` reads from `<project_dir>/amigo.toml`
- [ ] If `amigo.toml` does not exist, `load_art_defaults` returns empty/default `ArtDefaults`
- [ ] If `amigo.toml` exists but has no `[art]` section, returns empty/default `ArtDefaults`
- [ ] `load_audio_defaults` reads from `<project_dir>/amigo.toml`
- [ ] If `amigo.toml` does not exist, `load_audio_defaults` returns empty/default `AudioDefaults`
- [ ] If `amigo.toml` exists but has no `[audio]` section, returns empty/default `AudioDefaults`

#### Edge Cases
- [ ] If `amigo.toml` is malformed TOML, parsing falls back to default (no crash)
- [ ] If `amigo.toml` is not writable, `save_*_defaults` returns an error (not a panic)
- [ ] If `set_defaults` is called with an empty object, the file is unchanged
- [ ] If the `[art]` or `[audio]` section does not exist when `set_defaults` is called, it is created
- [ ] Concurrent reads and writes to `amigo.toml` do not corrupt the file (sequential access)

### Quality Gates
- [ ] `cargo check --workspace` compiles without errors
- [ ] `cargo test --workspace` — all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` — no warnings
- [ ] `cargo fmt --all --check` — correctly formatted
- [ ] New public API has at least one test per method
- [ ] No `unwrap()` in library code
- [ ] No `todo!()` or `unimplemented!()` in committed code

### Convention Compliance
- [ ] MCP tool names use `amigo_` prefix and snake_case (`amigo_artgen_get_defaults`, `amigo_audiogen_set_defaults`, etc.)
- [ ] Config file is `amigo.toml` (kebab-case naming convention)
- [ ] Logging uses `tracing` crate for warnings on missing defaults, parse errors
- [ ] Error handling uses `thiserror` for config read/write errors
- [ ] No `unwrap()` in library code; `fs::read_to_string` failures return defaults gracefully
- [ ] Config structs use `#[serde(default)]` for forward-compatible deserialization
- [ ] TOML parsing uses `toml` crate (consistent with existing `amigo.toml` handling)
