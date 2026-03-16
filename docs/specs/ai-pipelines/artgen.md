# Art Generation Pipeline (amigo_artgen)

> Status: draft
> Crate: amigo_artgen
> Depends on: [assets/format](../assets/format.md)
> Last updated: 2026-03-16

## Zweck

amigo_artgen is an MCP server that connects to external ComfyUI instances for pixel art sprite, tileset, and animation generation with a full post-processing pipeline enforcing visual consistency. It never runs AI models itself -- it builds ComfyUI workflow JSONs, sends them to the ComfyUI HTTP API, receives generated images, runs post-processing in Rust, and saves to the assets folder.

## Public API

See [Art MCP Tools](#5-art-mcp-tools) for the full tool interface exposed to Claude Code.

## Verhalten

See [Connection](#3-connection) for server modes and [Workflow Builder](#7-workflow-builder) for the generation pipeline.

## Internes Design

See [Art Architecture](#2-art-architecture) and [Art Post-Processing Pipeline](#6-art-post-processing-pipeline).

## Nicht-Ziele

- Running AI inference directly (ComfyUI handles this)
- Runtime asset manipulation (this is a dev-time tool only)
- Audio generation (see [audiogen](audiogen.md))

## Offene Fragen

- Which ComfyUI custom nodes are required vs. optional?
- Should the post-processing pipeline support user-defined steps?
- Cloud ComfyUI provider selection and authentication flow

---

## 2. Art Architecture

```
+-------------------------------------------------+
|              Claude Code (MCP)                   |
|                                                  |
|  amigo_artgen_generate_sprite(...)               |
|  amigo_artgen_generate_tileset(...)              |
|  amigo_artgen_inpaint(...)                       |
|  amigo_artgen_palette_swap(...)                  |
|                                                  |
+--------------------------------------------------+
|              amigo_artgen (MCP Server)            |
|                                                  |
|  +------------+  +------------+  +------------+  |
|  | Workflow    |  | ComfyUI    |  | Post-      |  |
|  | Builder    |->| Client     |->| Processing |  |
|  |            |  | (HTTP)     |  | Pipeline   |  |
|  +------------+  +------------+  +------------+  |
|                                        |         |
|                                        v         |
|                                  assets/ folder   |
|                                  (hot reload)     |
+--------------------------------------------------+
|              ComfyUI (external)                   |
|                                                  |
|  - Local (localhost:8188)                        |
|  - Remote (LAN / Cloud)                          |
|  - Pixel Art Checkpoint + LoRA                   |
|  - ControlNet models                             |
|  - Custom nodes as needed                        |
+--------------------------------------------------+
```

amigo_artgen never runs AI models itself. It builds ComfyUI workflow JSONs, sends them to the ComfyUI HTTP API, receives generated images, runs post-processing in Rust, and saves to the assets folder.

---

## 3. Connection

Three modes, like Krita AI Diffusion:

| Mode   | Server URL                         | Use Case                           |
| ------ | ---------------------------------- | ---------------------------------- |
| Local  | `http://localhost:8188`            | ComfyUI on same machine (default)  |
| Remote | `http://192.168.x.x:8188`          | ComfyUI on another PC / GPU server |
| Cloud  | `https://api.rundiffusion.com/...` | Hosted ComfyUI service             |

Configuration in `amigo.toml`:

```toml
[artgen]
server = "http://localhost:8188"
timeout = 120                        # seconds per generation
output_dir = "assets/generated"      # where results land
```

On startup, amigo_artgen queries the ComfyUI server for available models, LoRAs, and custom nodes. Missing requirements are reported as warnings.

---

## 4. Style Definitions

Each world has a style file that constrains AI generation for visual consistency:

```ron
// styles/caribbean.style.ron
(
    name: "Caribbean",

    // Model selection
    checkpoint: "pixel_art_xl_v1.safetensors",
    lora: Some(("pixel_art_16bit.safetensors", 0.7)),

    // Color palette (enforced in post-processing)
    palette: [
        "#1a1a2e",  // outline/dark
        "#e8c170",  // sand
        "#8b5e3c",  // wood
        "#3b7dd8",  // water
        "#4caf50",  // leaf
        "#f5f5dc",  // bone/sail
        "#c0392b",  // red accent
        "#f39c12",  // gold
        "#2c3e50",  // shadow
        "#ecf0f1",  // highlight
    ],

    // Prompt engineering
    prompt_prefix: "pixel art, 16-bit style, tropical pirate theme,",
    negative_prompt: "realistic, 3d, smooth, anti-aliased, gradient, blurry, modern, photo",

    // Generation defaults
    default_size: (32, 32),
    steps: 20,
    cfg_scale: 7.0,

    // Post-processing flags
    post_processing: (
        palette_clamp: true,
        remove_anti_aliasing: true,
        add_outline: true,
        outline_color: "#1a1a2e",
        cleanup_transparency: true,
    ),

    // Reference images for img2img consistency
    reference_images: [
        "styles/ref/caribbean_tower.png",
        "styles/ref/caribbean_enemy.png",
        "styles/ref/caribbean_tiles.png",
    ],
)
```

---

## 5. Art MCP Tools

### Generation

```
amigo_artgen_generate_sprite(
    prompt: string,          # "pirate archer tower with skull flag"
    style: string,           # "caribbean" -> loads style file
    size: [u32, u32]?,       # default from style (e.g., [32, 32])
    variants: u32?,          # number of variations (default: 3)
    output: string?,         # output filename (auto-generated if omitted)
) -> { paths: [string], preview: string }

amigo_artgen_generate_tileset(
    theme: string,           # "caribbean_ground"
    style: string,           # "caribbean"
    tile_size: u32?,         # default from style
    tiles: [string],         # ["grass", "dirt", "water", "sand", "path"]
    seamless: bool?,         # ensure edges tile correctly (default: true)
) -> { path: string, tiles: [string] }

amigo_artgen_generate_spritesheet(
    base: string,            # path to base sprite
    animation: string,       # "walk", "attack", "death", "idle"
    frames: u32,             # number of animation frames
    directions: u32?,        # 1, 4, or 8 (default: 1)
) -> { path: string, frames: u32 }
```

### Modification

```
amigo_artgen_variation(
    input: string,           # path to existing sprite
    prompt: string,          # "smaller flag, more wood texture"
    strength: f32?,          # 0.0 = identical, 1.0 = completely new (default: 0.4)
    style: string?,          # style for post-processing
) -> { path: string }

amigo_artgen_inpaint(
    input: string,           # path to sprite
    mask: string,            # path to mask (white = replace, black = keep)
    prompt: string,          # what to fill in
    style: string?,
) -> { path: string }
```

### Post-Processing Only (no AI)

```
amigo_artgen_palette_swap(
    input: string,           # path to sprite
    palette: string,         # palette name or style name
) -> { path: string }

amigo_artgen_upscale(
    input: string,           # path to sprite
    factor: u32,             # 2 or 4
) -> { path: string }

amigo_artgen_post_process(
    input: string,           # path to any image
    style: string,           # apply style's post-processing
) -> { path: string }
```

### Utility

```
amigo_artgen_list_styles() -> { styles: [string] }
amigo_artgen_list_checkpoints() -> { checkpoints: [string] }
amigo_artgen_list_loras() -> { loras: [string] }
amigo_artgen_server_status() -> { connected: bool, gpu: string, vram: string }
```

---

## 6. Art Post-Processing Pipeline

Runs in Rust after ComfyUI returns a raw image. No AI involved -- pure image manipulation. Ensures every generated asset matches the pixel art style.

### Pipeline Steps

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

### Configuration

Each step is toggleable per style:

```ron
post_processing: (
    palette_clamp: true,
    remove_anti_aliasing: true,
    add_outline: true,
    outline_color: "#1a1a2e",
    outline_mode: "outer",       // "outer", "inner", or "both"
    cleanup_transparency: true,
    tile_edge_check: false,      // only for tilesets
)
```

---

## 7. Workflow Builder

amigo_artgen builds ComfyUI workflow JSONs programmatically. No need for Claude Code or the user to understand ComfyUI node graphs.

### How It Works

1. Tool call comes in (e.g., `generate_sprite`)
2. Workflow Builder selects a template based on the operation
3. Fills in parameters from the tool call + style definition
4. Sends completed workflow JSON to ComfyUI HTTP API (`/prompt`)
5. Polls for completion (`/history`)
6. Downloads result image
7. Runs post-processing pipeline
8. Saves to output directory

### Template Workflows

Stored as JSON templates with placeholder values:

```
tools/amigo_artgen/workflows/
+-- txt2img_sprite.json          # text -> new sprite
+-- img2img_variation.json       # existing sprite -> variation
+-- inpaint.json                 # sprite + mask -> modified sprite
+-- spritesheet.json             # base sprite -> animation frames
+-- tileset.json                 # theme -> tileset grid
+-- upscale.json                 # small -> larger with detail
+-- custom/                      # user-provided workflows
```

### ComfyUI API Integration

```rust
pub struct ComfyUIClient {
    base_url: String,
    client: reqwest::Client,
}

impl ComfyUIClient {
    // Queue a workflow for execution
    pub async fn queue_prompt(&self, workflow: Value) -> Result<String>;

    // Poll for completion
    pub async fn get_history(&self, prompt_id: &str) -> Result<PromptResult>;

    // Download generated image
    pub async fn get_image(&self, filename: &str) -> Result<Vec<u8>>;

    // Query available models
    pub async fn get_checkpoints(&self) -> Result<Vec<String>>;
    pub async fn get_loras(&self) -> Result<Vec<String>>;

    // Server health
    pub async fn system_stats(&self) -> Result<SystemStats>;
}
```

---

## Example Workflow: Creating a Tower Sprite

```
Claude Code: "Create a cannon tower for the Caribbean world"

1. amigo_artgen_generate_sprite(
     prompt="pirate cannon tower, wooden platform, black cannon, skull decoration",
     style="caribbean",
     size=[32, 32],
     variants=3
   )
   -> Generates 3 variants, post-processed with Caribbean palette

2. Claude sees the 3 PNGs, picks the best one

3. amigo_artgen_variation(
     input="assets/generated/sprites/cannon_tower_v2.png",
     prompt="add small pirate flag on top",
     strength=0.3
   )
   -> Refined version with flag

4. amigo_artgen_generate_spritesheet(
     base="assets/generated/sprites/cannon_tower_v2_refined.png",
     animation="idle",
     frames=4
   )
   -> 4-frame idle animation (flag waving, cannon rotating slightly)

5. // Asset is now in assets/generated/sprites/
   // Engine hot-reloads it automatically
   // Claude can immediately test it in the running game:
   amigo_place_tower(x=5, y=3, tower_type="cannon")
   amigo_screenshot(path="/tmp/tower_placed.png")
   -> Claude sees the tower in-game, evaluates the look
```
