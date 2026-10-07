//! MCP tool definitions for amigo_artgen.
//!
//! Each tool maps to a backend-specific ComfyUI workflow + post-processing
//! pipeline. The backend (Qwen-Image, FLUX.2 Klein, Custom) is resolved
//! from project config or passed explicitly.

use crate::comfyui::{ComfyError, ComfyPrompt, ComfyUiClient, ComfyUiConfig, ComfyUiLifecycle};
use crate::config::{load_art_defaults, save_art_defaults};
use crate::image_io;
use crate::postprocess::{PixelBuffer, palette_clamp_to_colors, tile_edge_check};
use crate::workflows::{
    build_img2img_workflow, build_inpaint_workflow, build_upscale_workflow, build_workflow,
};
use crate::{ArtRequest, AssetType, ImageBackend, StyleDef, WorldStyle};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

// -- Tool parameter structs --

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateSpriteParams {
    pub prompt: String,
    pub style: String,
    pub size: Option<[u32; 2]>,
    pub variants: Option<u32>,
    pub output: Option<String>,
    /// Override the backend for this request.
    pub backend: Option<String>,
    /// Override the art mode: "pixel" or "raster".
    pub art_mode: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateTilesetParams {
    pub theme: String,
    pub style: String,
    pub tile_size: Option<u32>,
    pub tiles: Vec<String>,
    pub seamless: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateSpritesheetParams {
    pub base: String,
    pub animation: String,
    pub frames: u32,
    pub directions: Option<u32>,
    /// Playback speed written to the atlas manifest (default 8).
    #[serde(default)]
    pub fps: Option<f32>,
    /// How far each frame may stray from the base, 0..1 (default 0.4).
    #[serde(default)]
    pub strength: Option<f32>,
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariationParams {
    pub input: String,
    pub prompt: String,
    pub strength: Option<f32>,
    pub style: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InpaintParams {
    pub input: String,
    pub mask: String,
    pub prompt: String,
    pub style: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaletteSwapParams {
    pub input: String,
    pub palette: String,
    /// Where to write, relative to the project. Default
    /// `assets/generated/palette_swaps/<input>_<palette>.png`.
    #[serde(default)]
    pub output: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpscaleParams {
    pub input: String,
    pub factor: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostProcessParams {
    pub input: String,
    pub style: String,
    /// Where to write, relative to the project. Default
    /// `assets/generated/processed/<input>_<style>.png`.
    #[serde(default)]
    pub output: Option<String>,
}

// -- Tool result structs --

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateResult {
    pub paths: Vec<String>,
    pub preview: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TilesetResult {
    pub path: String,
    pub tiles: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpritesheetResult {
    /// The sheet image.
    pub path: String,
    /// Its `.atlas.ron` manifest, one sprite per direction.
    pub manifest: String,
    /// Frames per direction.
    pub frames: u32,
    pub directions: u32,
    /// The sprite names in the manifest.
    pub sprites: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SingleFileResult {
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListResult {
    pub items: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendInfo {
    pub name: String,
    pub display_name: String,
    pub checkpoint: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerStatusResult {
    pub connected: bool,
    pub gpu: String,
    pub vram: String,
    pub backend: String,
    pub art_mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetDefaultsParams {
    pub project_dir: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetDefaultsParams {
    pub project_dir: String,
    pub defaults: HashMap<String, serde_json::Value>,
}

// -- Tool registry for MCP --

/// Describes a tool for MCP tool listing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Returns all available artgen MCP tools
pub fn list_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "amigo_artgen_generate_sprite".into(),
            description: "Generate a pixel art or raster sprite using AI. Backend (qwen-image, flux2-klein, custom) is resolved from project config.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "What to generate" },
                    "style": { "type": "string", "description": "Style name (e.g. 'caribbean')" },
                    "size": { "type": "array", "items": { "type": "integer" }, "description": "[width, height]" },
                    "variants": { "type": "integer", "description": "Number of variations" },
                    "output": { "type": "string", "description": "Output filename" },
                    "backend": { "type": "string", "description": "Override backend: 'qwen-image', 'flux2-klein', or 'custom'" },
                    "art_mode": { "type": "string", "description": "Override art mode: 'pixel' or 'raster'" }
                },
                "required": ["prompt", "style"]
            }),
        },
        ToolDef {
            name: "amigo_artgen_generate_tileset".into(),
            description: "Generate a pixel art tileset".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "theme": { "type": "string" },
                    "style": { "type": "string" },
                    "tile_size": { "type": "integer" },
                    "tiles": { "type": "array", "items": { "type": "string" } },
                    "seamless": { "type": "boolean" }
                },
                "required": ["theme", "style", "tiles"]
            }),
        },
        ToolDef {
            name: "amigo_artgen_generate_spritesheet".into(),
            description: "Generate animation frames from a base sprite (img2img per frame) and write them as one sheet with an .atlas.ron manifest".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "base": { "type": "string", "description": "Path to base sprite" },
                    "animation": { "type": "string", "description": "Animation type: walk, attack, death, idle" },
                    "frames": { "type": "integer", "description": "Frames per direction, 1 to 32" },
                    "directions": { "type": "integer", "description": "1, 4, or 8" },
                    "fps": { "type": "number", "description": "Playback speed in the atlas manifest (default 8)" },
                    "strength": { "type": "number", "description": "How far frames may stray from the base, 0..1 (default 0.4)" },
                    "style": { "type": "string" }
                },
                "required": ["base", "animation", "frames"]
            }),
        },
        ToolDef {
            name: "amigo_artgen_variation".into(),
            description: "Create a variation of an existing sprite".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" },
                    "prompt": { "type": "string" },
                    "strength": { "type": "number" },
                    "style": { "type": "string" }
                },
                "required": ["input", "prompt"]
            }),
        },
        ToolDef {
            name: "amigo_artgen_inpaint".into(),
            description: "Inpaint a region of a sprite".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" },
                    "mask": { "type": "string" },
                    "prompt": { "type": "string" },
                    "style": { "type": "string" }
                },
                "required": ["input", "mask", "prompt"]
            }),
        },
        ToolDef {
            name: "amigo_artgen_palette_swap".into(),
            description: "Map every opaque pixel of an image to the nearest colour of a palette (no AI)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Image path, relative to the project" },
                    "palette": { "type": "string", "description": "pico8, gameboy, a style name, '#rrggbb,#rrggbb…', or a .hex, .gpl or image file" },
                    "output": { "type": "string", "description": "Output path, relative to the project" }
                },
                "required": ["input", "palette"]
            }),
        },
        ToolDef {
            name: "amigo_artgen_upscale".into(),
            description: "Upscale a sprite by integer factor".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" },
                    "factor": { "type": "integer" }
                },
                "required": ["input", "factor"]
            }),
        },
        ToolDef {
            name: "amigo_artgen_post_process".into(),
            description: "Apply a style's post-processing (palette clamp, anti-aliasing removal, transparency cleanup, outline) to any image".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Image path, relative to the project" },
                    "style": { "type": "string", "description": "A built-in style or assets/styles/<name>.style.ron" },
                    "output": { "type": "string", "description": "Output path, relative to the project" }
                },
                "required": ["input", "style"]
            }),
        },
        ToolDef {
            name: "amigo_artgen_list_styles".into(),
            description: "List available art styles".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_artgen_list_backends".into(),
            description: "List available image generation backends (qwen-image, flux2-klein, custom)".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_artgen_list_checkpoints".into(),
            description: "List available model checkpoints".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_artgen_list_loras".into(),
            description: "List available LoRA models".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_artgen_server_status".into(),
            description: "Check image generation server status and active backend".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_artgen_get_defaults".into(),
            description: "Get project art generation defaults from amigo.toml".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Path to the project directory" }
                },
                "required": ["project_dir"]
            }),
        },
        ToolDef {
            name: "amigo_artgen_set_defaults".into(),
            description: "Save art generation defaults to `amigo.toml` `[art]` section. \
                Merges with existing values. Supports backend, art_mode, and all style defaults."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Path to the project directory" },
                    "defaults": {
                        "type": "object",
                        "description": "Key-value pairs to merge into [art] section",
                        "properties": {
                            "backend": { "type": "string", "description": "Image backend: 'qwen-image', 'flux2-klein', or 'custom'" },
                            "art_mode": { "type": "string", "description": "Art mode: 'pixel' or 'raster'" },
                            "default_sprite_size": { "type": "integer", "description": "Default sprite size in pixels (e.g., 16, 32, 64)" },
                            "default_style": { "type": "string", "description": "Default art style name" },
                            "default_palette": { "type": "string", "description": "Default color palette (e.g., 'nes', 'snes', 'gameboy')" },
                            "color_depth": { "type": "integer", "description": "Color depth (8, 16, 24, 32)" },
                            "tileset_tile_size": { "type": "integer", "description": "Default tileset tile size" },
                            "background_style": { "type": "string", "description": "Default background style (e.g., 'static', 'parallax')" },
                            "add_outline": { "type": "boolean", "description": "Add pixel outline to sprites" },
                            "outline_color": { "type": "string", "description": "Outline color as hex (#RRGGBB)" },
                            "custom_endpoint": { "type": "string", "description": "Custom ComfyUI endpoint URL" },
                            "custom_workflow_url": { "type": "string", "description": "URL to custom workflow JSON" }
                        }
                    }
                },
                "required": ["project_dir", "defaults"]
            }),
        },
    ]
}

/// How long one generation may take before the tool gives up.
const GENERATION_TIMEOUT: Duration = Duration::from_secs(600);

/// Runs artgen tool calls against a ComfyUI server.
///
/// The generation tools used to return made-up paths such as
/// `assets/generated/sprites/<prompt>_v1.png` without contacting ComfyUI or
/// writing anything, so an agent "succeeded" and then failed to find the
/// file. Every tool now either does its work and returns the files it wrote,
/// or returns an error ([`ToolError::Backend`] when ComfyUI is unreachable
/// or the prompt failed, [`ToolError::BadInput`] for unusable arguments).
pub struct ArtgenServer {
    project_dir: PathBuf,
    comfy: ComfyUiConfig,
    /// When set, ComfyUI is started on the first generation request and
    /// stopped when the server is dropped (the MCP binary). When `None`,
    /// requests go to whatever listens at `comfy` (tests, a ComfyUI the user
    /// started).
    lifecycle: Option<ComfyUiLifecycle>,
}

impl ArtgenServer {
    /// A server for the project in `project_dir` (where `amigo.toml` and
    /// `assets/` live), talking to ComfyUI at `comfy`.
    pub fn new(project_dir: impl Into<PathBuf>, comfy: ComfyUiConfig) -> Self {
        Self {
            project_dir: project_dir.into(),
            comfy,
            lifecycle: None,
        }
    }

    /// Start ComfyUI on demand if nothing answers at the configured address.
    pub fn with_autostart(mut self) -> Self {
        self.lifecycle = Some(ComfyUiLifecycle::new(self.comfy.clone()));
        self
    }

    /// A client for a ComfyUI that is running, starting it first if this
    /// server manages one.
    fn client(&mut self) -> Result<ComfyUiClient, ToolError> {
        if let Some(lifecycle) = &mut self.lifecycle {
            lifecycle
                .ensure_running()
                .map_err(|e| ToolError::Backend(format!("ComfyUI is not available: {e}")))?;
        }
        Ok(ComfyUiClient::new(self.comfy.clone()))
    }

    /// Resolve a tool's file argument against the project directory and
    /// check it exists.
    fn input_file(&self, path: &str) -> Result<PathBuf, ToolError> {
        let resolved = self.project_dir.join(path);
        if resolved.is_file() {
            Ok(resolved)
        } else {
            Err(ToolError::BadInput(format!(
                "{} does not exist",
                resolved.display()
            )))
        }
    }

    /// Run `prompt`, write its images into `assets/generated/<kind>/`, and
    /// return their paths relative to the project.
    fn generate(
        &mut self,
        prompt: &ComfyPrompt,
        kind: &str,
        stem: &str,
    ) -> Result<Vec<String>, ToolError> {
        let client = self.client()?;
        let out_dir = self.project_dir.join("assets").join("generated").join(kind);
        let written = client
            .run_image_workflow(prompt, &out_dir, stem, GENERATION_TIMEOUT)
            .map_err(|e| comfy_error(&self.comfy, e))?;
        Ok(written
            .iter()
            .map(|p| {
                p.strip_prefix(&self.project_dir)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect())
    }

    /// Upload a project file to ComfyUI's input folder for a `LoadImage` node.
    fn upload(&mut self, path: &str) -> Result<String, ToolError> {
        let local = self.input_file(path)?;
        self.client()?
            .upload_image(&local)
            .map_err(|e| comfy_error(&self.comfy, e))
    }

    /// A style by name: `assets/styles/<name>.style.ron` (or `.ron`) in the
    /// project, else a built-in one.
    fn style(&self, name: &str) -> Result<StyleDef, ToolError> {
        let dir = self.project_dir.join("assets").join("styles");
        for file in [format!("{name}.style.ron"), format!("{name}.ron")] {
            let path = dir.join(file);
            if path.is_file() {
                return StyleDef::load_from_file(&path)
                    .map_err(|e| ToolError::BadInput(format!("{}: {e}", path.display())));
            }
        }
        StyleDef::find(name).ok_or_else(|| {
            let names: Vec<String> = StyleDef::builtin_defaults()
                .into_iter()
                .map(|s| s.name)
                .collect();
            ToolError::BadInput(format!(
                "unknown style '{name}': put it in assets/styles/{name}.style.ron or use one of {}",
                names.join(", ")
            ))
        })
    }

    /// Write `image` to `rel` inside the project and return `rel`. Absolute
    /// paths and `..` are refused: the tools only write into the project.
    fn write_image(&self, image: &PixelBuffer, rel: &str) -> Result<String, ToolError> {
        let path = std::path::Path::new(rel);
        if path.is_absolute()
            || path.components().any(|c| {
                !matches!(
                    c,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            })
        {
            return Err(ToolError::BadInput(format!(
                "output '{rel}' must be a relative path inside the project"
            )));
        }
        image_io::save_png(image, &self.project_dir.join(path)).map_err(ToolError::Backend)?;
        Ok(rel.replace('\\', "/"))
    }

    /// Frames of `p.animation` for each direction, generated from the base
    /// sprite with img2img, laid out one direction per row on a sheet with
    /// an atlas manifest next to it.
    fn generate_spritesheet(
        &mut self,
        p: GenerateSpritesheetParams,
    ) -> Result<serde_json::Value, ToolError> {
        if !(1..=32).contains(&p.frames) {
            return Err(ToolError::BadInput(format!(
                "frames must be 1 to 32, got {}",
                p.frames
            )));
        }
        let directions: &[&str] = match p.directions.unwrap_or(1) {
            1 => &[""],
            4 => &["down", "left", "right", "up"],
            8 => &[
                "down",
                "down_left",
                "left",
                "up_left",
                "up",
                "up_right",
                "right",
                "down_right",
            ],
            n => {
                return Err(ToolError::BadInput(format!(
                    "directions must be 1, 4 or 8, got {n}"
                )));
            }
        };
        let fps = p.fps.unwrap_or(8.0);
        if !(fps.is_finite() && fps > 0.0 && fps <= 60.0) {
            return Err(ToolError::BadInput(format!(
                "fps must be in (0, 60], got {fps}"
            )));
        }
        // Read the base first: a broken file should not cost a GPU run.
        let base = image_io::load_image(&self.input_file(&p.base)?).map_err(ToolError::BadInput)?;
        let (w, h) = (base.width, base.height);
        let uploaded = self.upload(&p.base)?;
        let defaults = load_art_defaults(&self.project_dir);
        let style = world_style(
            p.style.as_deref().unwrap_or("default"),
            &defaults.resolve_art_mode(),
        );
        let backend = defaults.resolve_backend();
        let strength = p.strength.unwrap_or(0.4).clamp(0.0, 1.0);
        let animation = sanitize(&p.animation);
        let stem = format!("{}_{animation}", file_stem(&p.base));

        let mut rows = Vec::with_capacity(directions.len());
        for (d, direction) in directions.iter().enumerate() {
            let mut row = Vec::with_capacity(p.frames as usize);
            for f in 0..p.frames {
                let facing = if direction.is_empty() {
                    String::new()
                } else {
                    format!(", facing {}", direction.replace('_', "-"))
                };
                let prompt = format!(
                    "{} animation, frame {} of {}{facing}, same character, same palette, same size",
                    p.animation,
                    f + 1,
                    p.frames
                );
                let workflow = build_img2img_workflow(
                    &uploaded,
                    &prompt,
                    &ArtRequest::default().negative_prompt,
                    strength,
                    &style,
                    &backend,
                );
                let written =
                    self.generate(&workflow, "spritesheets/frames", &format!("{stem}_{d}_{f}"))?;
                let first = written.first().ok_or_else(|| {
                    ToolError::Backend("ComfyUI returned no image for a frame".into())
                })?;
                let frame = image_io::load_image(&self.project_dir.join(first))
                    .map_err(|e| ToolError::Backend(format!("frame {f}: {e}")))?;
                row.push(resize_nearest(&frame, w, h));
            }
            rows.push(row);
        }

        let sheet = image_io::compose_sheet(&rows)
            .ok_or_else(|| ToolError::Backend("no frames were generated".into()))?;
        let sheet_rel = format!("assets/generated/spritesheets/{stem}.png");
        self.write_image(&sheet, &sheet_rel)?;

        let looping = !matches!(p.animation.as_str(), "death" | "die" | "attack");
        let mut sprites = std::collections::BTreeMap::new();
        for (d, direction) in directions.iter().enumerate() {
            let name = if direction.is_empty() {
                stem.clone()
            } else {
                format!("{stem}/{direction}")
            };
            let frames = (0..p.frames)
                .map(|f| ManifestFrame {
                    x: f * w,
                    y: d as u32 * h,
                    w,
                    h,
                })
                .collect();
            sprites.insert(
                name,
                ManifestSprite {
                    frames,
                    origin: (w as f32 / 2.0, h as f32),
                    fps: Some(fps),
                    looping,
                },
            );
        }
        let manifest = Manifest {
            image: format!("{stem}.png"),
            sprites,
        };
        let text = ron::ser::to_string_pretty(&manifest, ron::ser::PrettyConfig::default())
            .map_err(|e| ToolError::Backend(format!("could not write the manifest: {e}")))?;
        let manifest_rel = format!("assets/generated/spritesheets/{stem}.atlas.ron");
        std::fs::write(self.project_dir.join(&manifest_rel), text)
            .map_err(|e| ToolError::Backend(format!("could not write {manifest_rel}: {e}")))?;

        Ok(serde_json::to_value(SpritesheetResult {
            path: sheet_rel,
            manifest: manifest_rel,
            frames: p.frames,
            directions: directions.len() as u32,
            sprites: manifest.sprites.keys().cloned().collect(),
        })?)
    }

    /// Dispatch a tool call by name.
    pub fn call(
        &mut self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        match name {
            "amigo_artgen_generate_sprite" => {
                let p: GenerateSpriteParams = serde_json::from_value(params)?;

                // Resolve defaults: explicit param → amigo.toml → style → hardcoded
                let defaults = load_art_defaults(&self.project_dir);
                let style_def = crate::StyleDef::find(&p.style);
                let mut missing: Vec<String> = Vec::new();

                let backend = p
                    .backend
                    .as_deref()
                    .and_then(ImageBackend::parse)
                    .unwrap_or_else(|| defaults.resolve_backend());
                let art_mode = p
                    .art_mode
                    .as_deref()
                    .and_then(crate::ArtMode::parse)
                    .unwrap_or_else(|| defaults.resolve_art_mode());

                let size = if let Some(s) = p.size {
                    s
                } else if let Some(s) = defaults.default_sprite_size {
                    [s, s]
                } else {
                    missing.push("sprite_size".into());
                    style_def
                        .as_ref()
                        .map(|sd| [sd.default_size.0, sd.default_size.1])
                        .unwrap_or([32, 32])
                };
                if defaults.default_palette.is_none() {
                    missing.push("palette".into());
                }

                let request = ArtRequest {
                    asset_type: AssetType::Sprite,
                    prompt: p.prompt.clone(),
                    width: size[0],
                    height: size[1],
                    world: p.style.clone(),
                    variants: p.variants.unwrap_or(1).max(1),
                    backend: backend.clone(),
                    art_mode: art_mode.clone(),
                    ..ArtRequest::default()
                };
                let workflow = build_workflow(&request, &world_style(&p.style, &art_mode));
                let stem = p
                    .output
                    .as_deref()
                    .map(|o| sanitize(o.trim_end_matches(".png")))
                    .unwrap_or_else(|| sanitize(&p.prompt));
                let paths = self.generate(&workflow, "sprites", &stem)?;

                let mut response = serde_json::to_value(GenerateResult {
                    paths,
                    preview: None,
                })?;
                response["backend"] = serde_json::json!(format!("{:?}", backend));
                response["art_mode"] = serde_json::json!(format!("{:?}", art_mode));
                if art_mode == crate::ArtMode::Pixel {
                    response["note"] = serde_json::json!(
                        "Raw model output: run amigo_artgen_post_process with the style for the pixel-art clean-up (palette clamp, outline)"
                    );
                }
                if !missing.is_empty() {
                    response["hints"] = serde_json::json!({
                        "defaults_missing": missing,
                        "suggestion": "Run amigo_artgen_set_defaults to save project defaults"
                    });
                }
                Ok(response)
            }
            "amigo_artgen_generate_tileset" => {
                let p: GenerateTilesetParams = serde_json::from_value(params)?;
                let defaults = load_art_defaults(&self.project_dir);
                let mut missing: Vec<String> = Vec::new();
                let tile_size = match p.tile_size.or(defaults.tileset_tile_size) {
                    Some(ts) => ts,
                    None => {
                        missing.push("tileset_tile_size".into());
                        16
                    }
                };

                // A square grid with room for every requested tile.
                let per_row = (p.tiles.len().max(1) as f64).sqrt().ceil() as u32;
                let art_mode = defaults.resolve_art_mode();
                let request = ArtRequest {
                    asset_type: AssetType::Tileset,
                    prompt: format!("{} tileset: {}", p.theme, p.tiles.join(", ")),
                    width: tile_size * per_row,
                    height: tile_size * per_row,
                    world: p.style.clone(),
                    backend: defaults.resolve_backend(),
                    art_mode: art_mode.clone(),
                    ..ArtRequest::default()
                };
                let workflow = build_workflow(&request, &world_style(&p.style, &art_mode));
                let paths = self.generate(&workflow, "tilesets", &sanitize(&p.theme))?;

                let mut response = serde_json::to_value(TilesetResult {
                    path: paths.into_iter().next().unwrap_or_default(),
                    tiles: p.tiles,
                })?;
                if !missing.is_empty() {
                    response["hints"] = serde_json::json!({
                        "defaults_missing": missing,
                        "suggestion": "Run amigo_artgen_set_defaults to save project defaults"
                    });
                }
                Ok(response)
            }
            "amigo_artgen_variation" => {
                let p: VariationParams = serde_json::from_value(params)?;
                let uploaded = self.upload(&p.input)?;
                let defaults = load_art_defaults(&self.project_dir);
                let style = world_style(
                    p.style.as_deref().unwrap_or("default"),
                    &defaults.resolve_art_mode(),
                );
                let workflow = build_img2img_workflow(
                    &uploaded,
                    &p.prompt,
                    &ArtRequest::default().negative_prompt,
                    p.strength.unwrap_or(0.5).clamp(0.0, 1.0),
                    &style,
                    &defaults.resolve_backend(),
                );
                let stem = format!("{}_variation", file_stem(&p.input));
                let paths = self.generate(&workflow, "variations", &stem)?;
                Ok(serde_json::to_value(SingleFileResult {
                    path: paths.into_iter().next().unwrap_or_default(),
                })?)
            }
            "amigo_artgen_inpaint" => {
                let p: InpaintParams = serde_json::from_value(params)?;
                let image = self.upload(&p.input)?;
                let mask = self.upload(&p.mask)?;
                let defaults = load_art_defaults(&self.project_dir);
                let style = world_style(
                    p.style.as_deref().unwrap_or("default"),
                    &defaults.resolve_art_mode(),
                );
                let workflow = build_inpaint_workflow(
                    &image,
                    &mask,
                    &p.prompt,
                    &ArtRequest::default().negative_prompt,
                    &style,
                    &defaults.resolve_backend(),
                );
                let stem = format!("{}_inpainted", file_stem(&p.input));
                let paths = self.generate(&workflow, "inpainted", &stem)?;
                Ok(serde_json::to_value(SingleFileResult {
                    path: paths.into_iter().next().unwrap_or_default(),
                })?)
            }
            "amigo_artgen_upscale" => {
                let p: UpscaleParams = serde_json::from_value(params)?;
                if !(2..=8).contains(&p.factor) {
                    return Err(ToolError::BadInput(format!(
                        "factor must be 2 to 8, got {}",
                        p.factor
                    )));
                }
                let uploaded = self.upload(&p.input)?;
                let workflow = build_upscale_workflow(&uploaded, p.factor);
                let stem = format!("{}_{}x", file_stem(&p.input), p.factor);
                let paths = self.generate(&workflow, "upscaled", &stem)?;
                Ok(serde_json::to_value(SingleFileResult {
                    path: paths.into_iter().next().unwrap_or_default(),
                })?)
            }
            "amigo_artgen_generate_spritesheet" => {
                let p: GenerateSpritesheetParams = serde_json::from_value(params)?;
                self.generate_spritesheet(p)
            }
            "amigo_artgen_palette_swap" => {
                let p: PaletteSwapParams = serde_json::from_value(params)?;
                let input = self.input_file(&p.input)?;
                let palette = image_io::parse_palette(&p.palette, &self.project_dir)
                    .map_err(ToolError::BadInput)?;
                let mut image = image_io::load_image(&input).map_err(ToolError::BadInput)?;
                palette_clamp_to_colors(&mut image, &palette);
                let default = format!(
                    "assets/generated/palette_swaps/{}_{}.png",
                    file_stem(&p.input),
                    slug(&p.palette)
                );
                let path = self.write_image(&image, p.output.as_deref().unwrap_or(&default))?;
                Ok(serde_json::json!({ "path": path, "colors": palette.len() }))
            }
            "amigo_artgen_post_process" => {
                let p: PostProcessParams = serde_json::from_value(params)?;
                let input = self.input_file(&p.input)?;
                let style = self.style(&p.style)?;
                let mode = load_art_defaults(&self.project_dir).resolve_art_mode();
                let mut image = image_io::load_image(&input).map_err(ToolError::BadInput)?;
                image.apply_style_pipeline_for_mode(&style, &mode);
                let default = format!(
                    "assets/generated/processed/{}_{}.png",
                    file_stem(&p.input),
                    sanitize(&p.style)
                );
                let path = self.write_image(&image, p.output.as_deref().unwrap_or(&default))?;
                let mut response = serde_json::json!({
                    "path": path,
                    "art_mode": format!("{mode:?}"),
                    "width": image.width,
                    "height": image.height,
                });
                if style.post_processing.tile_edge_check && image.width > 0 && image.height > 0 {
                    let (h, v) = tile_edge_check(&image);
                    response["tile_edge_mismatches"] = serde_json::json!({
                        "left_right": h,
                        "top_bottom": v,
                    });
                }
                Ok(response)
            }
            "amigo_artgen_list_styles" => Ok(serde_json::to_value(ListResult {
                items: WorldStyle::builtin_styles()
                    .into_iter()
                    .map(|s| s.name)
                    .collect(),
            })?),
            "amigo_artgen_list_backends" => {
                let default = load_art_defaults(&self.project_dir).resolve_backend();
                let backends: Vec<BackendInfo> = [
                    ImageBackend::QwenImage,
                    ImageBackend::Flux2Klein,
                    ImageBackend::Custom,
                ]
                .into_iter()
                .map(|b| BackendInfo {
                    name: match b {
                        ImageBackend::QwenImage => "qwen-image",
                        ImageBackend::Flux2Klein => "flux2-klein",
                        ImageBackend::Custom => "custom",
                    }
                    .into(),
                    display_name: b.display_name().into(),
                    checkpoint: b.default_checkpoint().into(),
                    is_default: b == default,
                })
                .collect();
                Ok(serde_json::to_value(backends)?)
            }
            "amigo_artgen_list_checkpoints" => {
                let items = self
                    .client()?
                    .list_models()
                    .map_err(|e| comfy_error(&self.comfy, e))?;
                Ok(serde_json::to_value(ListResult { items })?)
            }
            "amigo_artgen_list_loras" => {
                let items = self
                    .client()?
                    .list_loras()
                    .map_err(|e| comfy_error(&self.comfy, e))?;
                Ok(serde_json::to_value(ListResult { items })?)
            }
            "amigo_artgen_server_status" => {
                let defaults = load_art_defaults(&self.project_dir);
                // A status query must not start ComfyUI as a side effect.
                let stats = ComfyUiClient::new(self.comfy.clone()).system_stats().ok();
                let device = stats.as_ref().and_then(|s| s["devices"].get(0)).cloned();
                Ok(serde_json::to_value(ServerStatusResult {
                    connected: stats.is_some(),
                    gpu: device
                        .as_ref()
                        .and_then(|d| d["name"].as_str())
                        .unwrap_or("unknown")
                        .into(),
                    vram: device
                        .as_ref()
                        .and_then(|d| d["vram_total"].as_u64())
                        .map(|b| format!("{:.1} GB", b as f64 / 1_073_741_824.0))
                        .unwrap_or_else(|| "unknown".into()),
                    backend: defaults.resolve_backend().display_name().into(),
                    art_mode: format!("{:?}", defaults.resolve_art_mode()),
                })?)
            }
            "amigo_artgen_get_defaults" => {
                let p: GetDefaultsParams = serde_json::from_value(params)?;
                let defaults = load_art_defaults(std::path::Path::new(&p.project_dir));
                Ok(serde_json::to_value(defaults).unwrap_or_default())
            }
            "amigo_artgen_set_defaults" => {
                let p: SetDefaultsParams = serde_json::from_value(params)?;
                let project_path = std::path::Path::new(&p.project_dir);
                save_art_defaults(project_path, &p.defaults).map_err(ToolError::Backend)?;
                Ok(serde_json::json!({ "saved": true, "path": "amigo.toml" }))
            }
            _ => Err(ToolError::UnknownTool(name.to_string())),
        }
    }
}

/// Dispatch a tool call against ComfyUI at its default address, with the
/// working directory as the project.
pub fn dispatch_tool(
    name: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, ToolError> {
    let project = std::env::current_dir().unwrap_or_default();
    ArtgenServer::new(project, ComfyUiConfig::default()).call(name, params)
}

/// Like [`dispatch_tool`], with an explicit project directory for `[art]`
/// defaults and output paths.
pub fn dispatch_tool_with_defaults(
    name: &str,
    params: serde_json::Value,
    project_dir: Option<&std::path::Path>,
) -> Result<serde_json::Value, ToolError> {
    let project = project_dir
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    ArtgenServer::new(project, ComfyUiConfig::default()).call(name, params)
}

/// `amigo_assets`' atlas manifest, as far as the spritesheet tool writes it.
#[derive(Serialize)]
struct Manifest {
    image: String,
    sprites: std::collections::BTreeMap<String, ManifestSprite>,
}

#[derive(Serialize)]
struct ManifestSprite {
    frames: Vec<ManifestFrame>,
    origin: (f32, f32),
    fps: Option<f32>,
    looping: bool,
}

#[derive(Serialize)]
struct ManifestFrame {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// `buf` scaled to `w × h` without blending, so pixel art stays crisp.
fn resize_nearest(buf: &PixelBuffer, w: u32, h: u32) -> PixelBuffer {
    if (buf.width, buf.height) == (w, h) || buf.width == 0 || buf.height == 0 {
        return buf.clone();
    }
    let mut out = PixelBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let sx = (u64::from(x) * u64::from(buf.width) / u64::from(w)) as u32;
            let sy = (u64::from(y) * u64::from(buf.height) / u64::from(h)) as u32;
            out.set(x, y, buf.get(sx, sy));
        }
    }
    out
}

/// The built-in style of that name, or a plain one carrying the name.
fn world_style(name: &str, art_mode: &crate::ArtMode) -> WorldStyle {
    WorldStyle::find(name).unwrap_or_else(|| WorldStyle {
        name: name.to_string(),
        lora: None,
        style_prompt_prefix: match art_mode {
            crate::ArtMode::Pixel => "pixel art, ".into(),
            crate::ArtMode::Raster => String::new(),
        },
        palette_path: None,
        max_colors: 32,
        outline_color: None,
    })
}

fn file_stem(path: &str) -> String {
    sanitize(
        &std::path::Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}

/// A ComfyUI failure as a tool error, naming the address, with the fix when
/// the connection itself failed.
fn comfy_error(config: &ComfyUiConfig, err: ComfyError) -> ToolError {
    let hint = match err {
        ComfyError::Http(_) | ComfyError::Io(_) => {
            ". Is ComfyUI running? `amigo setup --only artgen` installs it"
        }
        _ => "",
    };
    ToolError::Backend(format!("ComfyUI at {}: {err}{hint}", config.base_url()))
}

/// [`sanitize`] without runs of `_` or `_` at either end: `#ff0000, #00ff00`
/// becomes `ff0000_00ff00`.
fn slug(s: &str) -> String {
    sanitize(s)
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .chars()
        .take(40)
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Unknown tool: {0}")]
    UnknownTool(String),
    #[error("Invalid parameters: {0}")]
    InvalidParams(#[from] serde_json::Error),
    /// An argument is well-formed but unusable (a missing input file, a
    /// factor out of range).
    #[error("Invalid input: {0}")]
    BadInput(String),
    /// ComfyUI or the file system failed.
    #[error("{0}")]
    Backend(String),
}

impl ToolError {
    /// Whether this is a protocol error (unknown tool, malformed arguments,
    /// a JSON-RPC error in MCP) rather than a tool that ran and failed (an
    /// `isError` result the model can read and react to).
    pub fn is_protocol_error(&self) -> bool {
        matches!(self, Self::UnknownTool(_) | Self::InvalidParams(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_tools_returns_15() {
        assert_eq!(list_tools().len(), 15);
    }

    use amigo_comfyui::fake::{FAKE_IMAGE, FakeComfyUi};

    fn server_for(project: &std::path::Path, fake: &FakeComfyUi) -> ArtgenServer {
        ArtgenServer::new(project, fake.config())
    }

    #[test]
    fn generate_sprite_writes_the_image_comfyui_returns() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        let v = server_for(project.path(), &fake)
            .call(
                "amigo_artgen_generate_sprite",
                serde_json::json!({ "prompt": "pirate tower", "style": "caribbean" }),
            )
            .unwrap();

        // Default backend and mode, as before.
        assert_eq!(v["backend"], "QwenImage");
        assert_eq!(v["art_mode"], "Pixel");
        let path = v["paths"][0].as_str().unwrap();
        assert_eq!(path, "assets/generated/sprites/pirate_tower.png");
        // The file is real now, with ComfyUI's bytes in it.
        assert_eq!(
            std::fs::read(project.path().join(path)).unwrap(),
            FAKE_IMAGE
        );
        // And the workflow that produced it carried the prompt.
        assert!(fake.prompts()[0].to_string().contains("pirate tower"));
    }

    #[test]
    fn generate_sprite_with_backend_override() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        let v = server_for(project.path(), &fake)
            .call(
                "amigo_artgen_generate_sprite",
                serde_json::json!({
                    "prompt": "space ship",
                    "style": "matrix",
                    "backend": "flux2-klein",
                    "art_mode": "raster",
                    "output": "ship.png"
                }),
            )
            .unwrap();
        assert_eq!(v["backend"], "Flux2Klein");
        assert_eq!(v["art_mode"], "Raster");
        assert_eq!(v["paths"][0], "assets/generated/sprites/ship.png");
    }

    #[test]
    fn generation_without_comfyui_is_an_error_not_a_made_up_path() {
        // Nothing listens on this port: bind and drop to get a free one.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let project = tempfile::tempdir().unwrap();
        let mut server = ArtgenServer::new(
            project.path(),
            ComfyUiConfig {
                host: "127.0.0.1".into(),
                port,
            },
        );
        let err = server
            .call(
                "amigo_artgen_generate_sprite",
                serde_json::json!({ "prompt": "tower", "style": "caribbean" }),
            )
            .unwrap_err();
        assert!(matches!(err, ToolError::Backend(_)), "{err}");
        assert!(err.to_string().contains("amigo setup"), "{err}");
        assert!(!err.is_protocol_error());
        assert!(!project.path().join("assets").exists());
    }

    #[test]
    fn a_failed_prompt_reports_comfyuis_message() {
        let fake = FakeComfyUi::start();
        fake.fail_prompts_with("CUDA out of memory");
        let project = tempfile::tempdir().unwrap();
        let err = server_for(project.path(), &fake)
            .call(
                "amigo_artgen_generate_tileset",
                serde_json::json!({ "theme": "beach", "style": "caribbean", "tiles": ["sand", "water"] }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("CUDA out of memory"), "{err}");
    }

    #[test]
    fn variation_uploads_its_input_and_writes_the_result() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        std::fs::write(project.path().join("knight.png"), b"knight").unwrap();
        let mut server = server_for(project.path(), &fake);

        let v = server
            .call(
                "amigo_artgen_variation",
                serde_json::json!({ "input": "knight.png", "prompt": "red armour" }),
            )
            .unwrap();
        assert_eq!(
            v["path"],
            "assets/generated/variations/knight_variation.png"
        );
        assert_eq!(fake.uploads(), ["knight.png"]);
        // LoadImage gets the uploaded name, not a local path.
        assert!(
            fake.prompts()[0]
                .to_string()
                .contains("\"image\":\"knight.png\"")
        );

        // A missing input is the caller's error, before anything is sent.
        let err = server
            .call(
                "amigo_artgen_upscale",
                serde_json::json!({ "input": "nope.png", "factor": 2 }),
            )
            .unwrap_err();
        assert!(matches!(err, ToolError::BadInput(_)), "{err}");
    }

    fn png(project: &std::path::Path, rel: &str, w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) {
        let mut buf = PixelBuffer::new(w, h);
        for y in 0..h {
            for x in 0..w {
                buf.set(x, y, f(x, y));
            }
        }
        image_io::save_png(&buf, &project.join(rel)).unwrap();
    }

    #[test]
    fn palette_swap_maps_pixels_to_the_palette_without_comfyui() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        png(project.path(), "hero.png", 2, 1, |x, _| {
            if x == 0 {
                [250, 10, 10, 255]
            } else {
                [0, 0, 0, 0]
            }
        });
        let mut server = server_for(project.path(), &fake);
        let v = server
            .call(
                "amigo_artgen_palette_swap",
                serde_json::json!({ "input": "hero.png", "palette": "#ff0000,#0000ff" }),
            )
            .unwrap();
        assert_eq!(
            v["path"],
            "assets/generated/palette_swaps/hero_ff0000_0000ff.png"
        );
        assert_eq!(v["colors"], 2);
        let out = image_io::load_image(&project.path().join(v["path"].as_str().unwrap())).unwrap();
        assert_eq!(out.get(0, 0), [255, 0, 0, 255]);
        assert_eq!(out.get(1, 0)[3], 0, "transparent stays transparent");

        let v = server
            .call(
                "amigo_artgen_palette_swap",
                serde_json::json!({ "input": "hero.png", "palette": "gameboy", "output": "out/gb.png" }),
            )
            .unwrap();
        assert_eq!(v["path"], "out/gb.png");
        assert!(project.path().join("out/gb.png").is_file());

        for (args, what) in [
            (
                serde_json::json!({ "input": "hero.png", "palette": "nope" }),
                "unknown palette",
            ),
            (
                serde_json::json!({ "input": "gone.png", "palette": "pico8" }),
                "does not exist",
            ),
            (
                serde_json::json!({ "input": "hero.png", "palette": "pico8", "output": "../x.png" }),
                "inside the project",
            ),
        ] {
            let err = server.call("amigo_artgen_palette_swap", args).unwrap_err();
            assert!(matches!(err, ToolError::BadInput(_)), "{err}");
            assert!(err.to_string().contains(what), "{err}");
        }
        assert!(fake.prompts().is_empty(), "no AI involved");
    }

    #[test]
    fn post_process_applies_the_style_pipeline() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        // A 4x4 sprite: a soft-edged 2x2 blob in the middle.
        png(project.path(), "blob.png", 4, 4, |x, y| match (x, y) {
            (1..=2, 1..=2) => [250, 250, 250, 255],
            (0, 0) => [250, 250, 250, 60],
            _ => [0, 0, 0, 0],
        });
        let mut server = server_for(project.path(), &fake);
        let v = server
            .call(
                "amigo_artgen_post_process",
                serde_json::json!({ "input": "blob.png", "style": "caribbean" }),
            )
            .unwrap();
        assert_eq!(v["path"], "assets/generated/processed/blob_caribbean.png");
        let out = image_io::load_image(&project.path().join(v["path"].as_str().unwrap())).unwrap();
        let style = StyleDef::find("caribbean").unwrap();
        let palette = style.palette_rgb();
        let outline = style.outline_rgba();
        for p in out.data.iter().filter(|p| p[3] > 0) {
            assert_eq!(p[3], 255, "binary alpha");
            let rgb = [p[0], p[1], p[2]];
            assert!(palette.contains(&rgb) || *p == outline, "{p:?}");
        }
        assert_eq!(out.get(0, 0)[3], 0, "the faint pixel is gone");
        assert_eq!(out.get(1, 0), outline, "outlined");

        // A project style overrides the built-ins.
        let mut custom = style.clone();
        custom.name = "mine".into();
        custom.post_processing.add_outline = false;
        custom.post_processing.tile_edge_check = true;
        std::fs::create_dir_all(project.path().join("assets/styles")).unwrap();
        std::fs::write(
            project.path().join("assets/styles/mine.style.ron"),
            ron::ser::to_string(&custom).unwrap(),
        )
        .unwrap();
        let v = server
            .call(
                "amigo_artgen_post_process",
                serde_json::json!({ "input": "blob.png", "style": "mine" }),
            )
            .unwrap();
        assert!(v["tile_edge_mismatches"].is_object(), "{v}");
        let err = server
            .call(
                "amigo_artgen_post_process",
                serde_json::json!({ "input": "blob.png", "style": "nope" }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("unknown style 'nope'"), "{err}");
    }

    #[test]
    fn spritesheet_generates_every_frame_and_writes_a_loadable_atlas() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        png(project.path(), "knight.png", 4, 4, |_, _| [9, 9, 9, 255]);
        // ComfyUI answers with an 8x8 frame; the sheet scales it to 4x4.
        let frame_dir = tempfile::tempdir().unwrap();
        png(frame_dir.path(), "f.png", 8, 8, |x, _| {
            if x < 4 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        });
        fake.set_output_image(std::fs::read(frame_dir.path().join("f.png")).unwrap());

        let mut server = server_for(project.path(), &fake);
        let v = server
            .call(
                "amigo_artgen_generate_spritesheet",
                serde_json::json!({ "base": "knight.png", "animation": "walk", "frames": 3, "directions": 4 }),
            )
            .unwrap();
        assert_eq!(fake.prompts().len(), 12, "one img2img run per frame");
        assert_eq!(fake.uploads().len(), 1, "the base is uploaded once");
        assert_eq!(v["path"], "assets/generated/spritesheets/knight_walk.png");
        assert_eq!(v["directions"], 4);
        let prompts = serde_json::to_string(&fake.prompts()).unwrap();
        assert!(prompts.contains("frame 3 of 3, facing up"), "{prompts}");

        let sheet =
            image_io::load_image(&project.path().join(v["path"].as_str().unwrap())).unwrap();
        assert_eq!((sheet.width, sheet.height), (12, 16));
        assert_eq!(sheet.get(5, 13), [255, 0, 0, 255]);
        assert_eq!(sheet.get(6, 13), [0, 0, 255, 255]);

        let manifest_path = project.path().join(v["manifest"].as_str().unwrap());
        let atlas = amigo_assets::atlas_manifest::load_atlas(&manifest_path).unwrap();
        assert!(atlas.mip_error.is_none());
        let manifest = amigo_assets::atlas_manifest::parse_manifest(
            &manifest_path,
            &std::fs::read_to_string(&manifest_path).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.sprites.len(), 4);
        let walk_up = &manifest.sprites["knight_walk/up"];
        assert_eq!(walk_up.frames.len(), 3);
        assert_eq!((walk_up.frames[2].x, walk_up.frames[2].y), (8, 12));
        assert_eq!(walk_up.fps, Some(8.0));
        assert!(walk_up.looping);
        assert_eq!(walk_up.origin, (2.0, 4.0));
    }

    #[test]
    fn spritesheet_checks_its_arguments_before_generating() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        png(project.path(), "a.png", 2, 2, |_, _| [1, 1, 1, 255]);
        std::fs::write(project.path().join("broken.png"), b"not a png").unwrap();
        let mut server = server_for(project.path(), &fake);
        for args in [
            serde_json::json!({ "base": "a.png", "animation": "walk", "frames": 0 }),
            serde_json::json!({ "base": "a.png", "animation": "walk", "frames": 2, "directions": 3 }),
            serde_json::json!({ "base": "a.png", "animation": "walk", "frames": 2, "fps": 0.0 }),
            serde_json::json!({ "base": "broken.png", "animation": "walk", "frames": 2 }),
            serde_json::json!({ "base": "gone.png", "animation": "walk", "frames": 2 }),
        ] {
            let err = server
                .call("amigo_artgen_generate_spritesheet", args.clone())
                .unwrap_err();
            assert!(matches!(err, ToolError::BadInput(_)), "{args}: {err}");
        }
        assert!(fake.prompts().is_empty());
        assert!(fake.uploads().is_empty());

        // ComfyUI's output must decode.
        let err = server
            .call(
                "amigo_artgen_generate_spritesheet",
                serde_json::json!({ "base": "a.png", "animation": "idle", "frames": 1 }),
            )
            .unwrap_err();
        assert!(matches!(err, ToolError::Backend(_)), "{err}");
    }

    #[test]
    fn server_status_and_listings_ask_comfyui() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        let mut server = server_for(project.path(), &fake);
        let status = server
            .call("amigo_artgen_server_status", serde_json::json!({}))
            .unwrap();
        assert_eq!(status["connected"], true);
        assert_eq!(status["gpu"], "Fake GPU");
        assert_eq!(status["vram"], "8.0 GB");

        let loras = server
            .call("amigo_artgen_list_loras", serde_json::json!({}))
            .unwrap();
        assert_eq!(loras["items"][0], "pixel-art.safetensors");
    }

    #[test]
    fn dispatch_unknown_tool() {
        let result = dispatch_tool("nonexistent", serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn dispatch_list_styles() {
        let result = dispatch_tool("amigo_artgen_list_styles", serde_json::json!({})).unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 6);
    }

    #[test]
    fn dispatch_list_backends() {
        let result = dispatch_tool("amigo_artgen_list_backends", serde_json::json!({})).unwrap();
        let backends = result.as_array().unwrap();
        assert_eq!(backends.len(), 3);
        assert_eq!(backends[0]["name"], "qwen-image");
        assert_eq!(backends[0]["is_default"], true);
        assert_eq!(backends[1]["name"], "flux2-klein");
        assert_eq!(backends[2]["name"], "custom");
    }

    #[test]
    fn dispatch_server_status() {
        let result = dispatch_tool("amigo_artgen_server_status", serde_json::json!({})).unwrap();
        assert_eq!(result["connected"], false);
        assert!(result["backend"].as_str().unwrap().contains("Qwen"));
    }

    #[test]
    fn dispatch_get_defaults_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = dispatch_tool(
            "amigo_artgen_get_defaults",
            serde_json::json!({ "project_dir": dir.path().to_str().unwrap() }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["default_sprite_size"].is_null());
    }

    #[test]
    fn dispatch_set_and_get_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("amigo.toml"),
            "[window]\ntitle = \"Test\"\n",
        )
        .unwrap();

        let result = dispatch_tool(
            "amigo_artgen_set_defaults",
            serde_json::json!({
                "project_dir": dir.path().to_str().unwrap(),
                "defaults": {
                    "default_sprite_size": 32,
                    "default_style": "caribbean",
                    "backend": "flux2-klein",
                    "art_mode": "raster"
                }
            }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["saved"], true);

        // Verify they were actually saved
        let get_result = dispatch_tool(
            "amigo_artgen_get_defaults",
            serde_json::json!({ "project_dir": dir.path().to_str().unwrap() }),
        )
        .unwrap();
        assert_eq!(get_result["default_sprite_size"], 32);
        assert_eq!(get_result["default_style"], "caribbean");
        assert_eq!(get_result["backend"], "flux2-klein");
        assert_eq!(get_result["art_mode"], "raster");
    }

    #[test]
    fn missing_defaults_are_hinted_and_present_ones_are_not() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        let args = serde_json::json!({ "prompt": "test sprite", "style": "caribbean" });

        let v = server_for(project.path(), &fake)
            .call("amigo_artgen_generate_sprite", args.clone())
            .unwrap();
        assert!(v["hints"]["defaults_missing"].is_array());
        assert!(
            v["hints"]["suggestion"]
                .as_str()
                .unwrap()
                .contains("amigo_artgen_set_defaults")
        );

        std::fs::write(
            project.path().join("amigo.toml"),
            "[art]\ndefault_sprite_size = 32\ndefault_palette = \"nes\"\n",
        )
        .unwrap();
        let v = server_for(project.path(), &fake)
            .call("amigo_artgen_generate_sprite", args)
            .unwrap();
        assert!(v.get("hints").is_none());
    }
}
