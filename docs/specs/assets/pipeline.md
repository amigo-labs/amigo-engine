---
status: draft
crate: amigo_assets
depends_on: ["assets/format"]
last_updated: 2026-03-16
---

# Asset Pipeline

## Purpose

The asset pipeline handles loading, caching, hot-reloading, and packing of all game assets. In development, assets are loose files with instant hot reload. In release, everything is packed into a single `game.pak` for optimal performance.

## Public API

```rust
// String-based (prototyping)
ctx.draw_sprite("pirates/captain", pos);

// Typed handles (performance, compile-time safe, build-script generated)
ctx.draw_sprite_handle(assets::sprites::CAPTAIN, pos);

// Animated sprites from Aseprite tags
ctx.draw_sprite_animated("player", "walk_right", pos);
```

## Behavior

### Philosophy

Dev: loose files, hot reload, Aseprite native. Release: packed into `game.pak`.

### Supported Formats

| Asset Type | Dev Format | Tool |
|------------|------------|------|
| Sprites | `.aseprite` (native), `.png` | Aseprite |
| Tilemaps | `.amigo` (engine format) | Amigo Editor |
| Audio SFX | `.wav`, `.ogg` | Audacity/sfxr |
| Audio Music | `.ogg` | Any DAW |
| Data | `.ron`, `.toml` | VS Code |
| Shaders | `.wgsl` | VS Code |

### Aseprite Integration

Engine reads `.aseprite` directly via `asefile`. Tags become named animations, layers are composited, slices become 9-patch UI elements.

### Asset Loading Strategy

**Synchronous loading at startup** -- all assets for the current world are loaded into memory before gameplay begins, like a cartridge. No async handle-checking, no "is it loaded yet?" callbacks. `ctx.assets.sprite("player")` always returns immediately. Background async loading only during level/world transitions (loading screen).

### Hot Reload (Dev Mode)

File watcher on assets directory. Sprites, configs, levels, audio, shaders all hot-reloadable.

### Asset Packing (Release)

`amigo pack`: sprites into texture atlases, audio compressed, data validated, all into `game.pak` (memory-mappable).

## Internal Design

### Atlas Pipeline (Dev vs Release)

**Dev mode:** Each Aseprite file / PNG is loaded as an individual texture. More draw calls (20-30 instead of 5), but irrelevant for pixel art performance. Hot reload is trivial -- file changes, texture is replaced instantly.

**Release mode:** `amigo pack` (CLI tool) runs bin-packing to combine all sprites into texture atlases. One atlas = one texture = one draw call. The packing logic lives in the CLI tool, not the engine runtime.

The engine has two loaders behind a common `SpriteHandle` -- game code is identical in both modes:

```rust
// Game code doesn't know or care about Dev vs Release
ctx.draw_sprite("player", pos);
// Dev: SpriteHandle -> individual texture -> draw
// Release: SpriteHandle -> atlas index + UV rect -> draw
```

For detailed atlas packing and spritesheet generation, see [assets/atlas](../assets/atlas.md).

### Workspace Structure

```
amigo-engine/
+-- tools/
|   +-- amigo_artgen/
|   |   +-- Cargo.toml
|   |   +-- src/
|   |   |   +-- main.rs               # MCP server entry point
|   |   |   +-- comfyui_client.rs      # HTTP client for ComfyUI API
|   |   |   +-- workflow_builder.rs    # Builds workflow JSONs from templates
|   |   |   +-- post_processing.rs     # Palette clamp, outline, AA removal
|   |   |   +-- style.rs              # Style definition loader
|   |   |   +-- tools.rs              # MCP tool definitions
|   |   +-- workflows/
|   |       +-- txt2img_sprite.json
|   |       +-- img2img_variation.json
|   |       +-- inpaint.json
|   |       +-- spritesheet.json
|   |       +-- tileset.json
|   |       +-- upscale.json
|   +-- amigo_audiogen/
|       +-- Cargo.toml
|       +-- src/
|       |   +-- main.rs               # MCP server entry point
|       |   +-- acestep_client.rs      # HTTP client for ACE-Step Gradio API
|       |   +-- audiogen_client.rs     # Python bridge for AudioCraft/AudioGen
|       |   +-- stem_splitter.rs       # Stem separation orchestration
|       |   +-- post_processing.rs     # Loop trim, normalize, convert
|       |   +-- style.rs              # Audio style definition loader
|       |   +-- tools.rs              # MCP tool definitions
|       +-- scripts/
|           +-- audiogen_server.py     # AudioGen FastAPI wrapper
+-- styles/
|   +-- caribbean.style.ron            # Art style (visual)
|   +-- lotr.style.ron
|   +-- dune.style.ron
|   +-- matrix.style.ron
|   +-- got.style.ron
|   +-- stranger_things.style.ron
|   +-- audio/
|       +-- caribbean.audio_style.ron  # Audio style (sonic)
|       +-- lotr.audio_style.ron
|       +-- dune.audio_style.ron
|       +-- matrix.audio_style.ron
|       +-- got.audio_style.ron
|       +-- stranger_things.audio_style.ron
+-- assets/
    +-- generated/                     # artgen output lands here
    |   +-- sprites/
    |   +-- tilesets/
    |   +-- spritesheets/
    +-- audio/
        +-- music/                     # adaptive tracks + stems
        |   +-- caribbean/
        |   +-- lotr/
        |   +-- ...
        +-- sfx/                       # sound effects
        +-- ambient/                   # environmental loops
```

## Non-Goals

- No runtime AI model inference for assets
- No dynamic plugin loading for asset types
- No streaming of individual assets during gameplay (cartridge-style loading)

## Open Questions

- Exact `game.pak` binary format specification
- Memory budget thresholds for asset preloading warnings
- Whether to support runtime asset streaming for very large worlds (Phase 2+)
