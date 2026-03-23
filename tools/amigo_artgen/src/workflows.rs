//! ComfyUI workflow templates for image generation.
//!
//! Each workflow is a factory that builds a ComfyUI prompt graph
//! from an ArtRequest + WorldStyle configuration. The graph shape
//! depends on the selected `ImageBackend`.

use crate::comfyui::ComfyPrompt;
use crate::{ArtRequest, AssetType, ImageBackend, WorldStyle};
use serde_json::json;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build a ComfyUI workflow prompt from an art request.
///
/// Dispatches to the correct backend-specific builder based on
/// `request.backend`. Asset-type adjustments (tileset resolution,
/// portrait cropping, etc.) are applied before the backend builder.
pub fn build_workflow(request: &ArtRequest, style: &WorldStyle) -> ComfyPrompt {
    let adjusted = adjust_for_asset_type(request);
    match adjusted.backend {
        ImageBackend::QwenImage => build_qwen_txt2img(&adjusted, style),
        ImageBackend::Flux2Klein => build_flux_txt2img(&adjusted, style),
        ImageBackend::Custom => build_qwen_txt2img(&adjusted, style), // fallback
    }
}

/// Build a custom-endpoint workflow by loading a template and substituting
/// placeholders. Returns `None` if the template cannot be parsed.
pub fn build_custom_workflow(
    workflow_json: &str,
    request: &ArtRequest,
    style: &WorldStyle,
) -> Option<ComfyPrompt> {
    let full_prompt = format!("{}{}", style.style_prompt_prefix, request.prompt);
    let full_negative = format!("{}, {}", style_negative_suffix(), request.negative_prompt);

    let replaced = workflow_json
        .replace("{{PROMPT}}", &full_prompt)
        .replace("{{NEGATIVE}}", &full_negative)
        .replace("{{WIDTH}}", &request.width.to_string())
        .replace("{{HEIGHT}}", &request.height.to_string())
        .replace("{{SEED}}", &rand_seed().to_string())
        .replace("{{BATCH}}", &request.variants.to_string());

    let nodes: HashMap<String, serde_json::Value> = serde_json::from_str(&replaced).ok()?;
    Some(ComfyPrompt {
        prompt: nodes,
        client_id: Some("amigo_artgen".into()),
    })
}

// ---------------------------------------------------------------------------
// Qwen-Image backend
// ---------------------------------------------------------------------------

/// Build a Qwen-Image txt2img workflow.
///
/// Qwen-Image uses `UNETLoader` + `DualCLIPLoader` + `VAELoader` instead
/// of the monolithic `CheckpointLoaderSimple`. Sampler: `euler`, 28 steps.
fn build_qwen_txt2img(req: &ArtRequest, style: &WorldStyle) -> ComfyPrompt {
    let full_prompt = format!("{}{}", style.style_prompt_prefix, req.prompt);
    let full_negative = format!("{}, {}", style_negative_suffix(), req.negative_prompt);

    let checkpoint = req
        .extra
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(ImageBackend::QwenImage.default_checkpoint());

    let steps = req
        .extra
        .get("steps")
        .and_then(|v| v.as_u64())
        .unwrap_or(28);

    let cfg = req.extra.get("cfg").and_then(|v| v.as_f64()).unwrap_or(7.0);

    let mut nodes = HashMap::new();

    // 1: UNET Loader
    nodes.insert(
        "1".into(),
        json!({
            "class_type": "UNETLoader",
            "inputs": {
                "unet_name": checkpoint,
                "weight_dtype": "default",
            }
        }),
    );

    // 2: DualCLIPLoader
    nodes.insert(
        "2".into(),
        json!({
            "class_type": "DualCLIPLoader",
            "inputs": {
                "clip_name1": "clip_l.safetensors",
                "clip_name2": "clip_g.safetensors",
                "type": "sdxl",
            }
        }),
    );

    // 3: VAELoader
    nodes.insert(
        "3".into(),
        json!({
            "class_type": "VAELoader",
            "inputs": {
                "vae_name": "qwen_image_vae.safetensors",
            }
        }),
    );

    // 4: CLIP Text Encode (positive)
    nodes.insert(
        "4".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": full_prompt,
                "clip": ["2", 0],
            }
        }),
    );

    // 5: CLIP Text Encode (negative)
    nodes.insert(
        "5".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": full_negative,
                "clip": ["2", 0],
            }
        }),
    );

    // 6: Empty Latent Image
    nodes.insert(
        "6".into(),
        json!({
            "class_type": "EmptyLatentImage",
            "inputs": {
                "width": req.width,
                "height": req.height,
                "batch_size": req.variants,
            }
        }),
    );

    // 7: KSampler
    nodes.insert(
        "7".into(),
        json!({
            "class_type": "KSampler",
            "inputs": {
                "model": ["1", 0],
                "positive": ["4", 0],
                "negative": ["5", 0],
                "latent_image": ["6", 0],
                "seed": rand_seed(),
                "steps": steps,
                "cfg": cfg,
                "sampler_name": "euler",
                "scheduler": "normal",
                "denoise": 1.0,
            }
        }),
    );

    // 8: VAE Decode
    nodes.insert(
        "8".into(),
        json!({
            "class_type": "VAEDecode",
            "inputs": {
                "samples": ["7", 0],
                "vae": ["3", 0],
            }
        }),
    );

    // 9: Save Image
    nodes.insert(
        "9".into(),
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "images": ["8", 0],
                "filename_prefix": format!("amigo_{}_{}", style.name, req.asset_type_str()),
            }
        }),
    );

    // LoRA if configured
    if let Some(lora) = &style.lora {
        nodes.insert(
            "10".into(),
            json!({
                "class_type": "LoraLoader",
                "inputs": {
                    "model": ["1", 0],
                    "clip": ["2", 0],
                    "lora_name": lora,
                    "strength_model": 0.8,
                    "strength_clip": 0.8,
                }
            }),
        );
        // Rewire sampler to use LoRA model
        if let Some(sampler) = nodes.get_mut("7") {
            sampler["inputs"]["model"] = json!(["10", 0]);
        }
        // Rewire CLIP encoders to use LoRA CLIP
        if let Some(pos) = nodes.get_mut("4") {
            pos["inputs"]["clip"] = json!(["10", 1]);
        }
        if let Some(neg) = nodes.get_mut("5") {
            neg["inputs"]["clip"] = json!(["10", 1]);
        }
    }

    ComfyPrompt {
        prompt: nodes,
        client_id: Some("amigo_artgen".into()),
    }
}

// ---------------------------------------------------------------------------
// FLUX.2 Klein backend
// ---------------------------------------------------------------------------

/// Build a FLUX.2 Klein txt2img workflow.
///
/// FLUX uses flow-matching: `UNETLoader` + `DualCLIPLoader` (T5 + CLIP-L),
/// `FluxGuidance` node, `BasicScheduler` with `sgm_uniform`, and
/// `SamplerCustomAdvanced` instead of KSampler.
fn build_flux_txt2img(req: &ArtRequest, style: &WorldStyle) -> ComfyPrompt {
    let full_prompt = format!("{}{}", style.style_prompt_prefix, req.prompt);

    let checkpoint = req
        .extra
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(ImageBackend::Flux2Klein.default_checkpoint());

    let steps = req
        .extra
        .get("steps")
        .and_then(|v| v.as_u64())
        .unwrap_or(28);

    let guidance = req
        .extra
        .get("guidance")
        .and_then(|v| v.as_f64())
        .unwrap_or(3.5);

    let mut nodes = HashMap::new();

    // 1: UNET Loader
    nodes.insert(
        "1".into(),
        json!({
            "class_type": "UNETLoader",
            "inputs": {
                "unet_name": checkpoint,
                "weight_dtype": "fp8_e4m3fn",
            }
        }),
    );

    // 2: DualCLIPLoader (T5XXL + CLIP-L for FLUX)
    nodes.insert(
        "2".into(),
        json!({
            "class_type": "DualCLIPLoader",
            "inputs": {
                "clip_name1": "t5xxl_fp8_e4m3fn.safetensors",
                "clip_name2": "clip_l.safetensors",
                "type": "flux",
            }
        }),
    );

    // 3: VAE Loader
    nodes.insert(
        "3".into(),
        json!({
            "class_type": "VAELoader",
            "inputs": {
                "vae_name": "ae.safetensors",
            }
        }),
    );

    // 4: CLIP Text Encode (positive only — FLUX doesn't use negative prompts)
    nodes.insert(
        "4".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": full_prompt,
                "clip": ["2", 0],
            }
        }),
    );

    // 5: FluxGuidance — FLUX-specific guidance node
    nodes.insert(
        "5".into(),
        json!({
            "class_type": "FluxGuidance",
            "inputs": {
                "conditioning": ["4", 0],
                "guidance": guidance,
            }
        }),
    );

    // 6: BasicGuider
    nodes.insert(
        "6".into(),
        json!({
            "class_type": "BasicGuider",
            "inputs": {
                "model": ["1", 0],
                "conditioning": ["5", 0],
            }
        }),
    );

    // 7: KSamplerSelect
    nodes.insert(
        "7".into(),
        json!({
            "class_type": "KSamplerSelect",
            "inputs": {
                "sampler_name": "euler",
            }
        }),
    );

    // 8: BasicScheduler with sgm_uniform
    nodes.insert(
        "8".into(),
        json!({
            "class_type": "BasicScheduler",
            "inputs": {
                "model": ["1", 0],
                "scheduler": "sgm_uniform",
                "steps": steps,
                "denoise": 1.0,
            }
        }),
    );

    // 9: RandomNoise
    nodes.insert(
        "9".into(),
        json!({
            "class_type": "RandomNoise",
            "inputs": {
                "noise_seed": rand_seed(),
            }
        }),
    );

    // 10: EmptySD3LatentImage (FLUX uses SD3-style latent space)
    nodes.insert(
        "10".into(),
        json!({
            "class_type": "EmptySD3LatentImage",
            "inputs": {
                "width": req.width,
                "height": req.height,
                "batch_size": req.variants,
            }
        }),
    );

    // 11: SamplerCustomAdvanced
    nodes.insert(
        "11".into(),
        json!({
            "class_type": "SamplerCustomAdvanced",
            "inputs": {
                "noise": ["9", 0],
                "guider": ["6", 0],
                "sampler": ["7", 0],
                "sigmas": ["8", 0],
                "latent_image": ["10", 0],
            }
        }),
    );

    // 12: VAE Decode
    nodes.insert(
        "12".into(),
        json!({
            "class_type": "VAEDecode",
            "inputs": {
                "samples": ["11", 0],
                "vae": ["3", 0],
            }
        }),
    );

    // 13: Save Image
    nodes.insert(
        "13".into(),
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "images": ["12", 0],
                "filename_prefix": format!("amigo_{}_{}", style.name, req.asset_type_str()),
            }
        }),
    );

    // LoRA if configured
    if let Some(lora) = &style.lora {
        nodes.insert(
            "14".into(),
            json!({
                "class_type": "LoraLoader",
                "inputs": {
                    "model": ["1", 0],
                    "clip": ["2", 0],
                    "lora_name": lora,
                    "strength_model": 0.8,
                    "strength_clip": 0.8,
                }
            }),
        );
        // Rewire guider to use LoRA model
        if let Some(guider) = nodes.get_mut("6") {
            guider["inputs"]["model"] = json!(["14", 0]);
        }
        // Rewire scheduler to use LoRA model
        if let Some(sched) = nodes.get_mut("8") {
            sched["inputs"]["model"] = json!(["14", 0]);
        }
        // Rewire CLIP to use LoRA CLIP
        if let Some(clip_enc) = nodes.get_mut("4") {
            clip_enc["inputs"]["clip"] = json!(["14", 1]);
        }
    }

    ComfyPrompt {
        prompt: nodes,
        client_id: Some("amigo_artgen".into()),
    }
}

// ---------------------------------------------------------------------------
// img2img / inpaint / upscale (backend-aware)
// ---------------------------------------------------------------------------

/// Build an img2img variation workflow.
///
/// Uses the request's backend to select the correct model loader.
pub fn build_img2img_workflow(
    input_path: &str,
    prompt: &str,
    negative_prompt: &str,
    strength: f32,
    style: &WorldStyle,
    backend: &ImageBackend,
) -> ComfyPrompt {
    let full_prompt = format!("{}{}", style.style_prompt_prefix, prompt);
    let full_negative = format!("{}, {}", style_negative_suffix(), negative_prompt);

    let checkpoint = backend.default_checkpoint();
    let mut nodes = HashMap::new();

    // Model loading depends on backend
    let (model_ref, clip_ref, vae_ref) = insert_model_loader_nodes(&mut nodes, checkpoint, backend);

    // CLIP text encode (positive)
    nodes.insert(
        "20".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": full_prompt,
                "clip": clip_ref,
            }
        }),
    );

    // CLIP text encode (negative)
    nodes.insert(
        "21".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": full_negative,
                "clip": clip_ref,
            }
        }),
    );

    // Load input image
    nodes.insert(
        "22".into(),
        json!({
            "class_type": "LoadImage",
            "inputs": { "image": input_path }
        }),
    );

    // VAE Encode the input
    nodes.insert(
        "23".into(),
        json!({
            "class_type": "VAEEncode",
            "inputs": {
                "pixels": ["22", 0],
                "vae": vae_ref,
            }
        }),
    );

    // KSampler with denoise < 1.0
    nodes.insert(
        "24".into(),
        json!({
            "class_type": "KSampler",
            "inputs": {
                "model": model_ref,
                "positive": ["20", 0],
                "negative": ["21", 0],
                "latent_image": ["23", 0],
                "seed": rand_seed(),
                "steps": 20,
                "cfg": 7.0,
                "sampler_name": "euler",
                "scheduler": "normal",
                "denoise": strength.clamp(0.0, 1.0),
            }
        }),
    );

    nodes.insert(
        "25".into(),
        json!({
            "class_type": "VAEDecode",
            "inputs": { "samples": ["24", 0], "vae": vae_ref }
        }),
    );

    nodes.insert(
        "26".into(),
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "images": ["25", 0],
                "filename_prefix": format!("amigo_{}_variation", style.name),
            }
        }),
    );

    ComfyPrompt {
        prompt: nodes,
        client_id: Some("amigo_artgen".into()),
    }
}

/// Build an inpainting workflow.
pub fn build_inpaint_workflow(
    input_path: &str,
    mask_path: &str,
    prompt: &str,
    negative_prompt: &str,
    style: &WorldStyle,
    backend: &ImageBackend,
) -> ComfyPrompt {
    let full_prompt = format!("{}{}", style.style_prompt_prefix, prompt);
    let full_negative = format!("{}, {}", style_negative_suffix(), negative_prompt);

    let checkpoint = backend.default_checkpoint();
    let mut nodes = HashMap::new();

    let (model_ref, clip_ref, vae_ref) = insert_model_loader_nodes(&mut nodes, checkpoint, backend);

    nodes.insert(
        "20".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": { "text": full_prompt, "clip": clip_ref }
        }),
    );

    nodes.insert(
        "21".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": { "text": full_negative, "clip": clip_ref }
        }),
    );

    nodes.insert(
        "22".into(),
        json!({
            "class_type": "LoadImage",
            "inputs": { "image": input_path }
        }),
    );

    nodes.insert(
        "23".into(),
        json!({
            "class_type": "LoadImage",
            "inputs": { "image": mask_path }
        }),
    );

    nodes.insert(
        "24".into(),
        json!({
            "class_type": "VAEEncode",
            "inputs": { "pixels": ["22", 0], "vae": vae_ref }
        }),
    );

    nodes.insert(
        "25".into(),
        json!({
            "class_type": "SetLatentNoiseMask",
            "inputs": {
                "samples": ["24", 0],
                "mask": ["23", 1],
            }
        }),
    );

    nodes.insert(
        "26".into(),
        json!({
            "class_type": "KSampler",
            "inputs": {
                "model": model_ref,
                "positive": ["20", 0],
                "negative": ["21", 0],
                "latent_image": ["25", 0],
                "seed": rand_seed(),
                "steps": 25,
                "cfg": 7.5,
                "sampler_name": "euler",
                "scheduler": "normal",
                "denoise": 0.85,
            }
        }),
    );

    nodes.insert(
        "27".into(),
        json!({
            "class_type": "VAEDecode",
            "inputs": { "samples": ["26", 0], "vae": vae_ref }
        }),
    );

    nodes.insert(
        "28".into(),
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "images": ["27", 0],
                "filename_prefix": format!("amigo_{}_inpaint", style.name),
            }
        }),
    );

    ComfyPrompt {
        prompt: nodes,
        client_id: Some("amigo_artgen".into()),
    }
}

/// Build an upscale workflow (uses nearest-neighbor for pixel art).
pub fn build_upscale_workflow(input_path: &str, factor: u32) -> ComfyPrompt {
    let mut nodes = HashMap::new();

    nodes.insert(
        "1".into(),
        json!({
            "class_type": "LoadImage",
            "inputs": { "image": input_path }
        }),
    );

    nodes.insert(
        "2".into(),
        json!({
            "class_type": "ImageScaleBy",
            "inputs": {
                "image": ["1", 0],
                "upscale_method": "nearest-exact",
                "scale_by": factor,
            }
        }),
    );

    nodes.insert(
        "3".into(),
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "images": ["2", 0],
                "filename_prefix": format!("amigo_upscale_{}x", factor),
            }
        }),
    );

    ComfyPrompt {
        prompt: nodes,
        client_id: Some("amigo_artgen".into()),
    }
}

// ---------------------------------------------------------------------------
// Asset-type adjustments
// ---------------------------------------------------------------------------

fn adjust_for_asset_type(req: &ArtRequest) -> ArtRequest {
    let mut r = req.clone();
    match r.asset_type {
        AssetType::Sprite | AssetType::UiElement => {}
        AssetType::Tileset => {
            r.prompt = format!(
                "{}, seamless tile grid, top-down view, consistent spacing",
                req.prompt
            );
            if r.width < 128 {
                r.width = 256;
                r.height = 256;
            }
        }
        AssetType::Portrait => {
            r.prompt = format!(
                "{}, character portrait, face closeup, expressive",
                req.prompt
            );
            if r.width < 64 {
                r.width = 96;
                r.height = 96;
            }
        }
        AssetType::Background => {
            r.prompt = format!(
                "{}, wide scene, parallax background layer, scenic",
                req.prompt
            );
            if r.width < 320 {
                r.width = 480;
                r.height = 270;
            }
        }
        AssetType::Particle => {
            r.prompt = format!(
                "{}, small particle effect, transparent background, glow",
                req.prompt
            );
            if r.width > 32 {
                r.width = 16;
                r.height = 16;
            }
        }
    }
    r
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Insert model loader nodes (UNET, CLIP, VAE) and return references
/// to their outputs as (model_ref, clip_ref, vae_ref).
fn insert_model_loader_nodes(
    nodes: &mut HashMap<String, serde_json::Value>,
    checkpoint: &str,
    backend: &ImageBackend,
) -> (serde_json::Value, serde_json::Value, serde_json::Value) {
    match backend {
        ImageBackend::Flux2Klein => {
            nodes.insert(
                "1".into(),
                json!({
                    "class_type": "UNETLoader",
                    "inputs": { "unet_name": checkpoint, "weight_dtype": "fp8_e4m3fn" }
                }),
            );
            nodes.insert(
                "2".into(),
                json!({
                    "class_type": "DualCLIPLoader",
                    "inputs": {
                        "clip_name1": "t5xxl_fp8_e4m3fn.safetensors",
                        "clip_name2": "clip_l.safetensors",
                        "type": "flux",
                    }
                }),
            );
            nodes.insert(
                "3".into(),
                json!({
                    "class_type": "VAELoader",
                    "inputs": { "vae_name": "ae.safetensors" }
                }),
            );
            (json!(["1", 0]), json!(["2", 0]), json!(["3", 0]))
        }
        // QwenImage and Custom both use the Qwen-style loader
        _ => {
            nodes.insert(
                "1".into(),
                json!({
                    "class_type": "UNETLoader",
                    "inputs": { "unet_name": checkpoint, "weight_dtype": "default" }
                }),
            );
            nodes.insert(
                "2".into(),
                json!({
                    "class_type": "DualCLIPLoader",
                    "inputs": {
                        "clip_name1": "clip_l.safetensors",
                        "clip_name2": "clip_g.safetensors",
                        "type": "sdxl",
                    }
                }),
            );
            nodes.insert(
                "3".into(),
                json!({
                    "class_type": "VAELoader",
                    "inputs": { "vae_name": "qwen_image_vae.safetensors" }
                }),
            );
            (json!(["1", 0]), json!(["2", 0]), json!(["3", 0]))
        }
    }
}

fn style_negative_suffix() -> &'static str {
    "smooth, gradient, photorealistic"
}

// Helper for ArtRequest
impl ArtRequest {
    pub fn asset_type_str(&self) -> &str {
        match self.asset_type {
            AssetType::Sprite => "sprite",
            AssetType::Tileset => "tileset",
            AssetType::Portrait => "portrait",
            AssetType::Background => "background",
            AssetType::UiElement => "ui",
            AssetType::Particle => "particle",
        }
    }
}

/// Generate a pseudo-random seed for reproducibility logging.
fn rand_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64 % 1_000_000_000)
        .unwrap_or(42)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Qwen-Image (default backend) txt2img ──────────────────

    #[test]
    fn qwen_sprite_workflow_has_all_nodes() {
        let req = ArtRequest {
            prompt: "a pirate captain".into(),
            ..Default::default()
        };
        let style = WorldStyle::find("caribbean").unwrap();
        let prompt = build_workflow(&req, &style);

        assert!(prompt.prompt.contains_key("1")); // UNETLoader
        assert!(prompt.prompt.contains_key("2")); // DualCLIPLoader
        assert!(prompt.prompt.contains_key("3")); // VAELoader
        assert!(prompt.prompt.contains_key("7")); // KSampler
        assert!(prompt.prompt.contains_key("9")); // SaveImage
        assert_eq!(prompt.prompt["1"]["class_type"], "UNETLoader");
        assert_eq!(prompt.prompt["2"]["class_type"], "DualCLIPLoader");
    }

    #[test]
    fn qwen_workflow_uses_correct_checkpoint() {
        let req = ArtRequest::default();
        let style = WorldStyle::find("caribbean").unwrap();
        let prompt = build_workflow(&req, &style);

        let unet = &prompt.prompt["1"];
        assert_eq!(unet["inputs"]["unet_name"], "qwen-image-7b-Q4_K_M.gguf");
    }

    #[test]
    fn tileset_workflow_increases_resolution() {
        let req = ArtRequest {
            asset_type: AssetType::Tileset,
            prompt: "stone floor tiles".into(),
            width: 64,
            height: 64,
            ..Default::default()
        };
        let style = WorldStyle::find("lotr").unwrap();
        let prompt = build_workflow(&req, &style);
        let latent = &prompt.prompt["6"];
        assert_eq!(latent["inputs"]["width"], 256);
    }

    #[test]
    fn lora_rewires_nodes() {
        let req = ArtRequest::default();
        let mut style = WorldStyle::find("dune").unwrap();
        style.lora = Some("pixel_lora_v1.safetensors".into());

        let prompt = build_workflow(&req, &style);

        assert!(prompt.prompt.contains_key("10")); // LoRA loader
                                                   // Sampler should reference LoRA output
        let sampler = &prompt.prompt["7"];
        assert_eq!(sampler["inputs"]["model"], json!(["10", 0]));
    }

    #[test]
    fn background_workflow_wide() {
        let req = ArtRequest {
            asset_type: AssetType::Background,
            ..Default::default()
        };
        let style = WorldStyle::find("matrix").unwrap();
        let prompt = build_workflow(&req, &style);
        let latent = &prompt.prompt["6"];
        assert_eq!(latent["inputs"]["width"], 480);
        assert_eq!(latent["inputs"]["height"], 270);
    }

    // ── FLUX.2 Klein backend ─────────────────────────────────

    #[test]
    fn flux_workflow_has_flux_nodes() {
        let req = ArtRequest {
            prompt: "a space station".into(),
            backend: ImageBackend::Flux2Klein,
            ..Default::default()
        };
        let style = WorldStyle::find("matrix").unwrap();
        let prompt = build_workflow(&req, &style);

        // FLUX-specific nodes
        assert_eq!(prompt.prompt["5"]["class_type"], "FluxGuidance");
        assert_eq!(prompt.prompt["6"]["class_type"], "BasicGuider");
        assert_eq!(prompt.prompt["7"]["class_type"], "KSamplerSelect");
        assert_eq!(prompt.prompt["8"]["class_type"], "BasicScheduler");
        assert_eq!(prompt.prompt["11"]["class_type"], "SamplerCustomAdvanced");

        // Uses SD3 latent image
        assert_eq!(prompt.prompt["10"]["class_type"], "EmptySD3LatentImage");

        // Uses sgm_uniform scheduler
        assert_eq!(prompt.prompt["8"]["inputs"]["scheduler"], "sgm_uniform");
    }

    #[test]
    fn flux_workflow_uses_correct_checkpoint() {
        let req = ArtRequest {
            backend: ImageBackend::Flux2Klein,
            ..Default::default()
        };
        let style = WorldStyle::find("caribbean").unwrap();
        let prompt = build_workflow(&req, &style);

        let unet = &prompt.prompt["1"];
        assert_eq!(
            unet["inputs"]["unet_name"],
            "flux2-klein-4b-fp8.safetensors"
        );
        assert_eq!(unet["inputs"]["weight_dtype"], "fp8_e4m3fn");
    }

    #[test]
    fn flux_workflow_uses_t5_clip() {
        let req = ArtRequest {
            backend: ImageBackend::Flux2Klein,
            ..Default::default()
        };
        let style = WorldStyle::find("dune").unwrap();
        let prompt = build_workflow(&req, &style);

        let clip = &prompt.prompt["2"];
        assert_eq!(clip["inputs"]["clip_name1"], "t5xxl_fp8_e4m3fn.safetensors");
        assert_eq!(clip["inputs"]["type"], "flux");
    }

    #[test]
    fn flux_lora_rewires_guider() {
        let req = ArtRequest {
            backend: ImageBackend::Flux2Klein,
            ..Default::default()
        };
        let mut style = WorldStyle::find("caribbean").unwrap();
        style.lora = Some("pixel_lora_v1.safetensors".into());

        let prompt = build_workflow(&req, &style);

        assert!(prompt.prompt.contains_key("14")); // LoRA loader
        let guider = &prompt.prompt["6"];
        assert_eq!(guider["inputs"]["model"], json!(["14", 0]));
    }

    // ── Custom workflow ──────────────────────────────────────

    #[test]
    fn custom_workflow_substitutes_placeholders() {
        let template = r#"{
            "1": {"class_type": "Test", "inputs": {"prompt": "{{PROMPT}}", "w": {{WIDTH}}, "h": {{HEIGHT}}}}
        }"#;

        let req = ArtRequest {
            prompt: "hello world".into(),
            width: 512,
            height: 512,
            ..Default::default()
        };
        let style = WorldStyle::find("caribbean").unwrap();

        let result = build_custom_workflow(template, &req, &style);
        assert!(result.is_some());
        let prompt = result.unwrap();
        let node = &prompt.prompt["1"];
        let text = node["inputs"]["prompt"].as_str().unwrap();
        assert!(text.contains("hello world"));
        assert_eq!(node["inputs"]["w"], 512);
    }

    #[test]
    fn custom_workflow_invalid_json_returns_none() {
        let result = build_custom_workflow(
            "not valid json",
            &ArtRequest::default(),
            &WorldStyle::find("caribbean").unwrap(),
        );
        assert!(result.is_none());
    }

    // ── img2img (backend-aware) ──────────────────────────────

    #[test]
    fn img2img_workflow_has_load_image_and_denoise() {
        let style = WorldStyle::find("caribbean").unwrap();
        let prompt = build_img2img_workflow(
            "input.png",
            "a pirate ship",
            "blurry",
            0.6,
            &style,
            &ImageBackend::QwenImage,
        );

        assert!(prompt.prompt.contains_key("22"));
        assert_eq!(prompt.prompt["22"]["class_type"], "LoadImage");

        assert!(prompt.prompt.contains_key("23"));
        assert_eq!(prompt.prompt["23"]["class_type"], "VAEEncode");

        let sampler = &prompt.prompt["24"];
        assert_eq!(sampler["class_type"], "KSampler");
        let denoise = sampler["inputs"]["denoise"].as_f64().unwrap();
        assert!((denoise - 0.6).abs() < 0.001);
    }

    #[test]
    fn img2img_clamps_strength() {
        let style = WorldStyle::find("dune").unwrap();
        let prompt = build_img2img_workflow(
            "input.png",
            "test",
            "",
            1.5,
            &style,
            &ImageBackend::QwenImage,
        );
        let denoise = prompt.prompt["24"]["inputs"]["denoise"].as_f64().unwrap();
        assert!((denoise - 1.0).abs() < 0.001);

        let prompt = build_img2img_workflow(
            "input.png",
            "test",
            "",
            -0.5,
            &style,
            &ImageBackend::QwenImage,
        );
        let denoise = prompt.prompt["24"]["inputs"]["denoise"].as_f64().unwrap();
        assert!((denoise - 0.0).abs() < 0.001);
    }

    // ── inpaint (backend-aware) ──────────────────────────────

    #[test]
    fn inpaint_workflow_has_mask_and_noise_mask() {
        let style = WorldStyle::find("lotr").unwrap();
        let prompt = build_inpaint_workflow(
            "input.png",
            "mask.png",
            "a stone wall",
            "blurry",
            &style,
            &ImageBackend::QwenImage,
        );

        assert_eq!(prompt.prompt["22"]["class_type"], "LoadImage");
        assert_eq!(prompt.prompt["22"]["inputs"]["image"], "input.png");
        assert_eq!(prompt.prompt["23"]["class_type"], "LoadImage");
        assert_eq!(prompt.prompt["23"]["inputs"]["image"], "mask.png");

        assert!(prompt.prompt.contains_key("25"));
        assert_eq!(prompt.prompt["25"]["class_type"], "SetLatentNoiseMask");

        let sampler = &prompt.prompt["26"];
        assert_eq!(sampler["class_type"], "KSampler");
        assert_eq!(sampler["inputs"]["latent_image"], json!(["25", 0]));
    }

    // ── upscale ──────────────────────────────────────────────

    #[test]
    fn upscale_workflow_uses_nearest_neighbor() {
        let prompt = build_upscale_workflow("sprite.png", 4);

        assert_eq!(prompt.prompt["1"]["class_type"], "LoadImage");
        assert_eq!(prompt.prompt["1"]["inputs"]["image"], "sprite.png");

        assert_eq!(prompt.prompt["2"]["class_type"], "ImageScaleBy");
        assert_eq!(
            prompt.prompt["2"]["inputs"]["upscale_method"],
            "nearest-exact"
        );
        assert_eq!(prompt.prompt["2"]["inputs"]["scale_by"], 4);

        assert_eq!(prompt.prompt["3"]["class_type"], "SaveImage");
        let prefix = prompt.prompt["3"]["inputs"]["filename_prefix"]
            .as_str()
            .unwrap();
        assert!(prefix.contains("4x"));
    }
}
