//! MCP tool definitions for amigo_artgen.
//!
//! Each tool maps to a ComfyUI workflow + post-processing pipeline.

use serde::{Deserialize, Serialize};

// -- Tool parameter structs --

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateSpriteParams {
    pub prompt: String,
    pub style: String,
    pub size: Option<[u32; 2]>,
    pub variants: Option<u32>,
    pub output: Option<String>,
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
pub struct ServerStatusResult {
    pub connected: bool,
    pub gpu: String,
    pub vram: String,
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
            description: "Generate a pixel art sprite using AI via ComfyUI".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "What to generate" },
                    "style": { "type": "string", "description": "Style name (e.g. 'caribbean')" },
                    "size": { "type": "array", "items": { "type": "integer" }, "description": "[width, height]" },
                    "variants": { "type": "integer", "description": "Number of variations" },
                    "output": { "type": "string", "description": "Output filename" }
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
            name: "amigo_artgen_list_checkpoints".into(),
            description: "List available ComfyUI checkpoints".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_artgen_list_loras".into(),
            description: "List available LoRA models".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_artgen_server_status".into(),
            description: "Check ComfyUI server connection status".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
    ]
}

/// Dispatch a tool call by name
pub fn dispatch_tool(
    name: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, ToolError> {
    match name {
        "amigo_artgen_generate_sprite" => {
            let p: GenerateSpriteParams = serde_json::from_value(params)?;
            // In production: build workflow, send to ComfyUI, post-process, save
            Ok(serde_json::to_value(GenerateResult {
                paths: vec![format!(
                    "assets/generated/sprites/{}_v1.png",
                    sanitize(&p.prompt)
                )],
                preview: None,
            })?)
        }
        "amigo_artgen_generate_tileset" => {
            let p: GenerateTilesetParams = serde_json::from_value(params)?;
            Ok(serde_json::to_value(TilesetResult {
                path: format!("assets/generated/tilesets/{}.png", sanitize(&p.theme)),
                tiles: p.tiles,
            })?)
        }
        "amigo_artgen_generate_spritesheet" => {
            let p: GenerateSpritesheetParams = serde_json::from_value(params)?;
            Ok(serde_json::to_value(SpritesheetResult {
                path: format!(
                    "assets/generated/spritesheets/{}_{}.png",
                    std::path::Path::new(&p.base)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    p.animation
                ),
                frames: p.frames,
            })?)
        }
        "amigo_artgen_variation" => {
            let p: VariationParams = serde_json::from_value(params)?;
            Ok(serde_json::to_value(SingleFileResult {
                path: format!("{}_variation.png", p.input.trim_end_matches(".png")),
            })?)
        }
        "amigo_artgen_inpaint" => {
            let p: InpaintParams = serde_json::from_value(params)?;
            Ok(serde_json::to_value(SingleFileResult {
                path: format!("{}_inpainted.png", p.input.trim_end_matches(".png")),
            })?)
        }
        "amigo_artgen_palette_swap" => {
            let _p: PaletteSwapParams = serde_json::from_value(params)?;
            Ok(serde_json::to_value(SingleFileResult {
                path: "output.png".into(),
            })?)
        }
        "amigo_artgen_upscale" => {
            let p: UpscaleParams = serde_json::from_value(params)?;
            Ok(serde_json::to_value(SingleFileResult {
                path: format!("{}_{}x.png", p.input.trim_end_matches(".png"), p.factor),
            })?)
        }
        "amigo_artgen_post_process" => {
            let _p: PostProcessParams = serde_json::from_value(params)?;
            Ok(serde_json::to_value(SingleFileResult {
                path: "output.png".into(),
            })?)
        }
        "amigo_artgen_list_styles" => Ok(serde_json::to_value(ListResult {
            items: vec![
                "caribbean",
                "lotr",
                "dune",
                "matrix",
                "got",
                "stranger_things",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        })?),
        "amigo_artgen_list_checkpoints" | "amigo_artgen_list_loras" => {
            Ok(serde_json::to_value(ListResult { items: vec![] })?)
        }
        "amigo_artgen_server_status" => Ok(serde_json::to_value(ServerStatusResult {
            connected: false,
            gpu: "unknown".into(),
            vram: "unknown".into(),
        })?),
        _ => Err(ToolError::UnknownTool(name.to_string())),
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_tools_returns_12() {
        assert_eq!(list_tools().len(), 12);
    }

    #[test]
    fn dispatch_generate_sprite() {
        let result = dispatch_tool(
            "amigo_artgen_generate_sprite",
            serde_json::json!({
                "prompt": "pirate tower",
                "style": "caribbean"
            }),
        );
        assert!(result.is_ok());
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
    fn dispatch_server_status() {
        let result = dispatch_tool("amigo_artgen_server_status", serde_json::json!({})).unwrap();
        assert_eq!(result["connected"], false);
    }
}
