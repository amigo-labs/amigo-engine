//! MCP tool definitions for amigo_artgen.
//!
//! Each tool maps to a backend-specific ComfyUI workflow + post-processing
//! pipeline. The backend (Qwen-Image, FLUX.2 Klein, Custom) is resolved
//! from project config or passed explicitly.

use crate::comfyui::{ComfyError, ComfyPrompt, ComfyUiClient, ComfyUiConfig, ComfyUiLifecycle};
use crate::config::{load_art_defaults, save_art_defaults};
use crate::workflows::{
    build_img2img_workflow, build_inpaint_workflow, build_upscale_workflow, build_workflow,
};
use crate::{ArtRequest, AssetType, ImageBackend, WorldStyle};
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
    pub path: String,
    pub frames: u32,
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
            description: "Generate animation frames from a base sprite".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "base": { "type": "string", "description": "Path to base sprite" },
                    "animation": { "type": "string", "description": "Animation type: walk, attack, death, idle" },
                    "frames": { "type": "integer" },
                    "directions": { "type": "integer", "description": "1, 4, or 8" }
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
            description: "Swap palette of a sprite (no AI, pure image processing)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" },
                    "palette": { "type": "string" }
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
            description: "Apply a style's post-processing to any image".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" },
                    "style": { "type": "string" }
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
/// or the prompt failed, [`ToolError::NotImplemented`] for the tools that
/// have no backend yet).
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
                        "Raw model output: the pixel-art post-processing (palette clamp, outline) is not applied yet"
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
                let _: GenerateSpritesheetParams = serde_json::from_value(params)?;
                Err(ToolError::NotImplemented(
                    "spritesheet generation has no ComfyUI workflow yet; generate frames one by one with amigo_artgen_variation".into(),
                ))
            }
            "amigo_artgen_palette_swap" => {
                let _: PaletteSwapParams = serde_json::from_value(params)?;
                Err(ToolError::NotImplemented(
                    "palette swap is not wired to image files yet (the PixelBuffer operations exist, PNG reading and writing does not)".into(),
                ))
            }
            "amigo_artgen_post_process" => {
                let _: PostProcessParams = serde_json::from_value(params)?;
                Err(ToolError::NotImplemented(
                    "post-processing is not wired to image files yet (the PixelBuffer operations exist, PNG reading and writing does not)".into(),
                ))
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
    /// The tool exists but has no implementation behind it yet. Returned
    /// instead of a made-up success.
    #[error("Not implemented: {0}")]
    NotImplemented(String),
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

    #[test]
    fn tools_without_a_backend_say_so() {
        let fake = FakeComfyUi::start();
        let project = tempfile::tempdir().unwrap();
        let mut server = server_for(project.path(), &fake);
        for (tool, args) in [
            (
                "amigo_artgen_palette_swap",
                serde_json::json!({ "input": "a.png", "palette": "nes" }),
            ),
            (
                "amigo_artgen_post_process",
                serde_json::json!({ "input": "a.png", "style": "caribbean" }),
            ),
            (
                "amigo_artgen_generate_spritesheet",
                serde_json::json!({ "base": "a.png", "animation": "walk", "frames": 4 }),
            ),
        ] {
            let err = server.call(tool, args).unwrap_err();
            assert!(matches!(err, ToolError::NotImplemented(_)), "{tool}: {err}");
        }
        assert!(fake.prompts().is_empty());
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
