---
status: done
crate: amigo_artgen
depends_on: ["assets/format"]
last_updated: 2026-03-18
---

# Art Generation Pipeline (amigo_artgen)

## Purpose

Provides an MCP server and Rust library for AI-powered pixel art generation via ComfyUI, with a full post-processing pipeline that enforces visual consistency through palette clamping, anti-aliasing removal, outline addition, and transparency cleanup. Builds ComfyUI workflow graphs programmatically and manages the generate-download-postprocess-save lifecycle.

Existing implementation in `tools/amigo_artgen/src/` (7 files: `lib.rs`, `style.rs`, `comfyui.rs`, `postprocess.rs`, `tools.rs`, `workflows.rs`, `main.rs`).

## Public API

### Core Types (`lib.rs`)

```rust
/// A request to generate pixel art.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtRequest {
    pub asset_type: AssetType,
    pub prompt: String,
    pub negative_prompt: String,      // default: "blurry, 3d, realistic, anti-aliased"
    pub width: u32,                   // default: 64
    pub height: u32,                  // default: 64
    pub world: String,                // default: "default"
    pub variants: u32,                // default: 1
    pub postprocess: Vec<PostProcessStep>,
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetType {
    Sprite,
    Tileset,
    Portrait,
    Background,
    UiElement,
    Particle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PostProcessStep {
    RemoveAntiAliasing,
    PaletteClamp { max_colors: u32 },
    AddOutline { color: [u8; 4] },
    Downscale { factor: u32 },
    ForceDimensions { width: u32, height: u32 },
    ApplyPalette { palette_path: String },
    CleanupTransparency,
    TileEdgeCheck,
}

/// Result of an art generation job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtResult {
    pub output_paths: Vec<String>,
    pub prompt_id: String,
    pub generation_time_ms: u64,
}
```

### WorldStyle (`lib.rs`)

```rust
/// Style configuration for a themed world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldStyle {
    pub name: String,
    pub lora: Option<String>,
    pub style_prompt_prefix: String,
    pub palette_path: Option<String>,
    pub max_colors: u32,
    pub outline_color: Option<[u8; 4]>,
}

impl WorldStyle {
    pub fn builtin_styles() -> Vec<WorldStyle>;  // 6 built-in styles
    pub fn find(name: &str) -> Option<WorldStyle>;
}
```

Six built-in world styles: `caribbean`, `lotr`, `dune`, `matrix`, `got`, `stranger_things`.

### StyleDef (`style.rs`)

```rust
/// A style definition loaded from a `.style.ron` file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StyleDef {
    pub name: String,
    pub checkpoint: String,
    pub lora: Option<(String, f32)>,           // (lora_file, strength)
    pub palette: Vec<String>,                   // hex color strings
    pub prompt_prefix: String,
    pub negative_prompt: String,
    pub default_size: (u32, u32),
    pub steps: u32,
    pub cfg_scale: f32,
    pub post_processing: PostProcessConfig,
    pub reference_images: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostProcessConfig {
    pub palette_clamp: bool,           // default: true
    pub remove_anti_aliasing: bool,    // default: true
    pub add_outline: bool,             // default: true
    pub outline_color: String,         // default: "#1a1a2e"
    pub outline_mode: OutlineMode,     // default: Outer
    pub cleanup_transparency: bool,    // default: true
    pub tile_edge_check: bool,         // default: false
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OutlineMode {
    Outer,
    Inner,
    Both,
}

impl StyleDef {
    pub fn parse_hex_color(hex: &str) -> Option<[u8; 3]>;
    pub fn palette_rgb(&self) -> Vec<[u8; 3]>;
    pub fn outline_rgba(&self) -> [u8; 4];
    pub fn load_from_file(path: &Path) -> Result<Self, StyleError>;
    pub fn load_all(dir: &Path) -> Vec<Self>;
    pub fn builtin_defaults() -> Vec<Self>;    // 6 built-in styles
    pub fn find(name: &str) -> Option<Self>;
}
```

### ComfyUI Client (`comfyui.rs`)

```rust
pub struct ComfyUiConfig {
    pub host: String,      // default: "127.0.0.1"
    pub port: u16,         // default: 8188
}

pub struct ComfyUiClient {
    pub config: ComfyUiConfig,
}

impl ComfyUiClient {
    pub fn new(config: ComfyUiConfig) -> Self;
    pub fn url(&self, path: &str) -> String;
    pub fn queue_prompt(&self, prompt: &ComfyPrompt) -> Result<QueueResponse, ComfyError>;
    pub fn check_status(&self, prompt_id: &str) -> Result<PromptStatus, ComfyError>;
    pub fn get_outputs(&self, prompt_id: &str) -> Result<Vec<OutputImage>, ComfyError>;
    pub fn download_image(&self, image: &OutputImage, output_path: &str) -> Result<(), ComfyError>;
    pub fn list_models(&self) -> Result<Vec<String>, ComfyError>;
    pub fn system_stats(&self) -> Result<Value, ComfyError>;
    pub fn wait_for_completion(
        &self, prompt_id: &str, timeout_ms: u64, poll_interval_ms: u64,
    ) -> Result<PromptStatus, ComfyError>;
}

pub enum PromptStatus {
    Queued,
    Running,
    Completed,
    Failed { error: String },
    Unknown,
}

pub struct ComfyPrompt {
    pub prompt: HashMap<String, Value>,    // node_id -> node_config
    pub client_id: Option<String>,
}

pub struct QueueResponse {
    pub prompt_id: String,
    pub number: u64,
}

pub struct OutputImage {
    pub filename: String,
    pub subfolder: String,
    pub image_type: String,
}
```

### Post-Processing (`postprocess.rs`)

```rust
pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<[u8; 4]>,
}

impl PixelBuffer {
    pub fn new(width: u32, height: u32) -> Self;
    pub fn get(&self, x: u32, y: u32) -> [u8; 4];
    pub fn set(&mut self, x: u32, y: u32, pixel: [u8; 4]);
    pub fn apply_pipeline(&mut self, steps: &[PostProcessStep]);
    pub fn apply_style_pipeline(&mut self, style: &StyleDef);
}

// Standalone functions
pub fn palette_clamp_to_colors(buf: &mut PixelBuffer, palette: &[[u8; 3]]);
pub fn cleanup_transparency(buf: &mut PixelBuffer);
pub fn add_outline_inner(buf: &mut PixelBuffer, color: [u8; 4]);
pub fn tile_edge_check(buf: &PixelBuffer) -> (u32, u32);  // (h_mismatches, v_mismatches)
```

### Workflow Builder (`workflows.rs`)

```rust
pub fn build_workflow(request: &ArtRequest, style: &WorldStyle) -> ComfyPrompt;
pub fn build_img2img_workflow(
    input_path: &str, prompt: &str, negative_prompt: &str,
    strength: f32, style: &WorldStyle,
) -> ComfyPrompt;
pub fn build_inpaint_workflow(
    input_path: &str, mask_path: &str, prompt: &str,
    negative_prompt: &str, style: &WorldStyle,
) -> ComfyPrompt;
pub fn build_upscale_workflow(input_path: &str, factor: u32) -> ComfyPrompt;
```

### MCP Tools (`tools.rs`)

12 MCP tools exposed via `list_tools()` and dispatched via `dispatch_tool()`:

| Tool | Description |
|------|-------------|
| `amigo_artgen_generate_sprite` | Generate a pixel art sprite from text prompt |
| `amigo_artgen_generate_tileset` | Generate a tileset with named tiles |
| `amigo_artgen_generate_spritesheet` | Generate animation frames from a base sprite |
| `amigo_artgen_variation` | Create an img2img variation of an existing sprite |
| `amigo_artgen_inpaint` | Inpaint a masked region of a sprite |
| `amigo_artgen_palette_swap` | Swap palette (pure image processing, no AI) |
| `amigo_artgen_upscale` | Upscale by integer factor (nearest-neighbor) |
| `amigo_artgen_post_process` | Apply a style's post-processing pipeline |
| `amigo_artgen_list_styles` | List available art styles |
| `amigo_artgen_list_checkpoints` | List available ComfyUI checkpoints |
| `amigo_artgen_list_loras` | List available LoRA models |
| `amigo_artgen_server_status` | Check ComfyUI server connection status |

## Behavior

### Generation Pipeline

1. MCP tool call arrives (e.g., `generate_sprite`).
2. `WorldStyle` or `StyleDef` is looked up by name.
3. Workflow builder constructs a ComfyUI prompt graph (`ComfyPrompt`) with nodes for checkpoint loader, CLIP text encode (positive/negative), empty latent image, KSampler, VAE decode, and save image. LoRA nodes are inserted and wired when the style specifies a LoRA.
4. The prompt is queued to ComfyUI via `POST /prompt`.
5. The client polls `GET /history/{prompt_id}` until completion or timeout.
6. Output images are downloaded via `GET /view`.
7. Post-processing pipeline runs on each image.
8. Results are saved to the output directory.

### Workflow Variants

`build_workflow()` dispatches by `AssetType`:
- **Sprite**: Standard txt2img with style prefix.
- **Tileset**: Forces minimum 256x256 resolution, adds "seamless tile grid, top-down view" to the prompt.
- **Portrait**: Forces minimum 96x96, adds "character portrait, face closeup, expressive".
- **Background**: Forces minimum 480x270, adds "wide scene, parallax background layer, scenic".
- **UiElement**: Uses the same pipeline as Sprite.
- **Particle**: Forces 16x16 max, adds "small particle effect, transparent background, glow".

`build_img2img_workflow()` loads an existing image, VAE-encodes it, and runs KSampler with `denoise < 1.0` (clamped to 0.0-1.0) for variation generation.

`build_inpaint_workflow()` loads both the input image and mask, uses `SetLatentNoiseMask` to restrict generation to the masked area, and runs at denoise 0.85.

`build_upscale_workflow()` uses `ImageScaleBy` with `nearest-exact` method to preserve pixel art sharpness.

### Post-Processing Pipeline

`PixelBuffer::apply_style_pipeline()` runs steps in a fixed order per the spec:

1. **Palette Clamp**: Maps each opaque pixel to the nearest color in the style's palette using Euclidean distance in RGB space.
2. **Remove Anti-Aliasing**: Snaps semi-transparent pixels (alpha 1-254) to fully opaque (>= 128) or fully transparent (< 128).
3. **Cleanup Transparency**: Binary alpha -- below 128 becomes [0,0,0,0], at or above becomes alpha 255.
4. **Outline**: Adds a 1px outline around non-transparent regions in the style's outline color. Supports `OutlineMode::Outer` (transparent pixels adjacent to opaque), `OutlineMode::Inner` (opaque pixels adjacent to transparent or edges), or `OutlineMode::Both`.

`PixelBuffer::apply_pipeline()` runs steps in the order they appear in the `PostProcessStep` slice, giving callers control over ordering.

### Tile Edge Checking

`tile_edge_check()` compares left-column vs right-column and top-row vs bottom-row pixels, returning the number of mismatching pixels per axis. Zero mismatches on both axes means the tile is seamlessly tileable.

## Internal Design

### ComfyUI API Integration

The client uses `ureq` for synchronous HTTP requests. Workflow graphs are represented as `HashMap<String, Value>` where keys are node IDs (string numbers like "1", "2") and values are JSON objects with `class_type` and `inputs`. Node connections use the `[node_id, output_index]` array format.

### Style System (Two Levels)

- **WorldStyle** (`lib.rs`): Lightweight style with just prompt prefix, palette path, outline color, and max colors. Used by the workflow builder for prompt construction.
- **StyleDef** (`style.rs`): Full style definition including checkpoint, LoRA with strength, hex palette, diffusion parameters (steps, CFG scale), post-processing config, and reference images. Loadable from `.style.ron` files or built-in defaults.

Both provide `builtin_defaults()` with 6 world styles. `StyleDef` additionally supports loading from RON files on disk.

### Palette Clamping Algorithms

Two clamping approaches:

1. **Budget-based** (`palette_clamp`): Reduces unique colors to a budget by progressively bit-shifting RGB channels until the unique count fits within `max_colors`. Simple but loses precision.
2. **Reference-based** (`palette_clamp_to_colors`): Maps each pixel to the nearest color in a given palette using Euclidean RGB distance. Preserves the exact palette colors.

The style pipeline uses reference-based clamping (approach 2) when the style defines a palette.

### Seed Generation

`rand_seed()` uses `SystemTime` nanoseconds modulo 1 billion for pseudo-random seeds. Deterministic seeds can be overridden via the `extra` parameter in `ArtRequest`.

### Tool Dispatch

`dispatch_tool()` deserializes parameters into typed structs, executes the operation, and returns a `serde_json::Value` result. In the current implementation, ComfyUI integration is placeholder (returns computed output paths); the full pipeline requires a running ComfyUI instance.

### Atlas Integration

Generated sprites must be packed into texture atlases before the engine can render them efficiently. The artgen pipeline integrates with the asset pipeline's atlas packer:

```rust
/// Register a generated sprite in the project's atlas manifest.
/// The next `amigo pack` run will include it in the appropriate atlas.
pub fn register_in_atlas(
    output_path: &str,
    asset_type: &AssetType,
    sprite_name: &str,
    atlas_manifest_path: &str,
) -> Result<(), IoError>;

/// Generate an atlas-ready sprite region definition (.sprite.ron) alongside the PNG.
pub fn write_sprite_ron(
    output_path: &str,
    sprite_name: &str,
    width: u32,
    height: u32,
    frames: Option<&AnimationFrames>,
) -> Result<(), IoError>;
```

**`.sprite.ron` Format** (consumed by atlas packer):

```ron
SpriteDef(
    name: "pirate_captain",
    source: "sprites/pirate_captain.png",
    // Optional: animation frames if this is a spritesheet
    animations: Some([
        (tag: "idle", frames: [(x: 0, y: 0, w: 32, h: 32, duration_ms: 200),
                               (x: 32, y: 0, w: 32, h: 32, duration_ms: 200)]),
        (tag: "walk", frames: [(x: 0, y: 32, w: 32, h: 32, duration_ms: 150),
                               (x: 32, y: 32, w: 32, h: 32, duration_ms: 150),
                               (x: 64, y: 32, w: 32, h: 32, duration_ms: 150),
                               (x: 96, y: 32, w: 32, h: 32, duration_ms: 150)]),
    ]),
)
```

### Spritesheet Animation Metadata

`amigo_artgen_generate_spritesheet` produces a grid of animation frames. The tool now also emits animation metadata:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimationFrames {
    pub tags: Vec<AnimTag>,
    pub frame_width: u32,
    pub frame_height: u32,
    pub columns: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimTag {
    pub name: String,
    pub start_frame: u32,
    pub end_frame: u32,
    pub duration_ms: u32,
}
```

The spritesheet tool accepts an `animations` parameter specifying tag names and frame counts. Output includes both the PNG spritesheet and a `.sprite.ron` file that the engine's `AnimPlayer` can load directly — no Aseprite required for AI-generated assets.

**MCP Tools** (additional):

| Tool | Description |
|------|-------------|
| `amigo_artgen_export_sprite_ron` | Generate `.sprite.ron` metadata for a sprite/spritesheet |
| `amigo_artgen_register_atlas` | Register generated sprite in the atlas manifest |

**Pipeline integration:**
1. `amigo_artgen_generate_spritesheet` → produces PNG + `.sprite.ron`
2. `amigo_artgen_register_atlas` → registers in atlas manifest
3. `amigo pack` → packs into texture atlas
4. Engine hot-reloads atlas, sprite is available via `AnimPlayer`

## Non-Goals

- Running AI inference directly (ComfyUI handles model execution).
- Runtime asset generation (this is a dev-time tool only).
- Audio generation (see [audiogen](audiogen.md)).
- Animated GIF or video output.
- Cloud ComfyUI authentication (manual configuration only).

## Open Questions

- Which ComfyUI custom nodes are required vs. optional.
- Whether the post-processing pipeline should support user-defined steps.
- Whether to add a sprite preview tool that renders the generated asset in-engine.
- Whether to support ControlNet workflows for pose/structure conditioning.
- Cloud ComfyUI provider selection and authentication flow.

## Referenzen

- [assets/format](../assets/format.md) -- Asset format for generated sprites
- [assets/atlas](../assets/atlas.md) -- Atlas packing for generated tilesets
- [ai-pipelines/audiogen](audiogen.md) -- Audio generation counterpart
- [ai-pipelines/agent-api](agent-api.md) -- MCP server architecture
