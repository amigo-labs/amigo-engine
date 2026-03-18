---
status: done
crate: amigo_assets
depends_on: ["assets/format"]
last_updated: 2026-03-16
---

# Atlas Pipeline

## Purpose

Handles texture atlas packing and spritesheet generation. In dev mode, sprites are individual textures for instant hot reload. In release mode, sprites are bin-packed into atlases for minimal draw calls.

## Public API

```rust
// Game code is identical in both modes -- the SpriteHandle abstracts the difference
ctx.draw_sprite("player", pos);
// Dev: SpriteHandle -> individual texture -> draw
// Release: SpriteHandle -> atlas index + UV rect -> draw

// String-based (prototyping)
ctx.draw_sprite("pirates/captain", pos);

// Typed handles (performance, compile-time safe, build-script generated)
ctx.draw_sprite_handle(assets::sprites::CAPTAIN, pos);

// Extended options via _ex variant
ctx.draw_sprite_ex("x", pos, |s| s.flip_x().tint(RED));
```

## Behavior

### Dev Mode

Each Aseprite file / PNG is loaded as an individual texture. More draw calls (20-30 instead of 5), but irrelevant for pixel art performance. Hot reload is trivial -- file changes, texture is replaced instantly.

### Release Mode

`amigo pack` (CLI tool, see [tooling/cli](../tooling/cli.md)) runs bin-packing to combine all sprites into texture atlases. One atlas = one texture = one draw call. The packing logic lives in the CLI tool, not the engine runtime.

### Sprite Batcher

Collect all sprites per frame, sort by texture atlas, one draw call per atlas. Target: 5-10 draw calls for a full TD scene.

### Spritesheet Generation (AI-Assisted)

Spritesheets can be generated from a base sprite using the art generation pipeline:

```
amigo_artgen_generate_spritesheet(
    base: string,            # path to base sprite
    animation: string,       # "walk", "attack", "death", "idle"
    frames: u32,             # number of animation frames
    directions: u32?,        # 1, 4, or 8 (default: 1)
) -> { path: string, frames: u32 }
```

### Art Post-Processing Pipeline

Runs in Rust after ComfyUI returns a raw image. No AI involved -- pure image manipulation. Ensures every generated asset matches the pixel art style.

#### Pipeline Steps

```
Raw AI Output (may have anti-aliasing, wrong colors, soft edges)
    |
    v
+- 1. Downscale ------------------------------------------+
|  If generated at higher resolution (e.g., 128x128       |
|  for a 32x32 sprite), downscale with nearest-           |
|  neighbor to target size.                                |
+---------------------------------------------------------+
    |
    v
+- 2. Palette Clamping -----------------------------------+
|  For each pixel: find nearest color in the style's      |
|  palette (Euclidean distance in RGB space).              |
|  Result: image uses only defined palette colors.         |
+---------------------------------------------------------+
    |
    v
+- 3. Anti-Aliasing Removal ------------------------------+
|  Detect pixels that are blends between two palette       |
|  colors (intermediate values). Snap to the nearest       |
|  palette color. Eliminates soft edges.                   |
+---------------------------------------------------------+
    |
    v
+- 4. Transparency Cleanup -------------------------------+
|  Pixels with alpha < 128 -> fully transparent (0).       |
|  Pixels with alpha >= 128 -> fully opaque (255).         |
|  No semi-transparent pixels in pixel art.                |
+---------------------------------------------------------+
    |
    v
+- 5. Outline Addition -----------------------------------+
|  For each opaque pixel adjacent to a transparent         |
|  pixel: add 1px outline in outline_color.                |
|  Configurable: inner outline, outer outline, or off.     |
+---------------------------------------------------------+
    |
    v
+- 6. Tile Edge Check (tilesets only) --------------------+
|  Verify that tile edges match for seamless tiling.       |
|  Left edge pixels must match right edge of neighbor.     |
|  If mismatch > threshold: flag for manual review         |
|  or re-run with inpainting on edges.                     |
+---------------------------------------------------------+
    |
    v
  Clean pixel art asset -> saved to assets/ -> engine hot reload
```

## Internal Design

The atlas packer uses bin-packing algorithms (e.g., rectangle packing) to fit all sprites into power-of-2 texture sizes. Each atlas stores a mapping of sprite names to UV rectangles. The `SpriteHandle` type is resolved at load time to either a direct texture reference (dev) or an atlas index + UV rect (release).

## Non-Goals

- Runtime atlas repacking
- GPU-side atlas management
- Mipmapping (pixel art uses nearest-neighbor filtering only)

## Open Questions

- Maximum atlas texture size (2048x2048 vs 4096x4096)
- Whether to support multiple atlas pages per category (sprites, tiles, UI)
- Compression format for atlas textures in `game.pak`
