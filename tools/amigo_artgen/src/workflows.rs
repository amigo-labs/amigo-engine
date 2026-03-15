//! ComfyUI workflow templates for pixel art generation.
//!
//! Each workflow is a factory that builds a ComfyUI prompt graph
//! from an ArtRequest + WorldStyle configuration.

use crate::comfyui::ComfyPrompt;
use crate::{ArtRequest, AssetType, WorldStyle};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Build a ComfyUI workflow prompt from an art request.
pub fn build_workflow(request: &ArtRequest, style: &WorldStyle) -> ComfyPrompt {
    match request.asset_type {
        AssetType::Sprite => build_sprite_workflow(request, style),
        AssetType::Tileset => build_tileset_workflow(request, style),
        AssetType::Portrait => build_portrait_workflow(request, style),
        AssetType::Background => build_background_workflow(request, style),
        AssetType::UiElement => build_sprite_workflow(request, style),
        AssetType::Particle => build_particle_workflow(request, style),
    }
}

fn build_sprite_workflow(req: &ArtRequest, style: &WorldStyle) -> ComfyPrompt {
    let full_prompt = format!("{}{}", style.style_prompt_prefix, req.prompt);
    let full_negative = format!("{}, smooth, gradient, photorealistic", req.negative_prompt);

    let model = req
        .extra
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("pixel_art_v1.safetensors");

    let steps = req
        .extra
        .get("steps")
        .and_then(|v| v.as_u64())
        .unwrap_or(25);

    let cfg = req.extra.get("cfg").and_then(|v| v.as_f64()).unwrap_or(7.0);

    let mut nodes = HashMap::new();

    // Checkpoint loader
    nodes.insert(
        "1".into(),
        json!({
            "class_type": "CheckpointLoaderSimple",
            "inputs": {
                "ckpt_name": model,
            }
        }),
    );

    // CLIP text encode (positive)
    nodes.insert(
        "2".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": full_prompt,
                "clip": ["1", 1],
            }
        }),
    );

    // CLIP text encode (negative)
    nodes.insert(
        "3".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": full_negative,
                "clip": ["1", 1],
            }
        }),
    );

    // Empty latent image
    nodes.insert(
        "4".into(),
        json!({
            "class_type": "EmptyLatentImage",
            "inputs": {
                "width": req.width,
                "height": req.height,
                "batch_size": req.variants,
            }
        }),
    );

    // KSampler
    nodes.insert(
        "5".into(),
        json!({
            "class_type": "KSampler",
            "inputs": {
                "model": ["1", 0],
                "positive": ["2", 0],
                "negative": ["3", 0],
                "latent_image": ["4", 0],
                "seed": rand_seed(),
                "steps": steps,
                "cfg": cfg,
                "sampler_name": "euler_ancestral",
                "scheduler": "normal",
                "denoise": 1.0,
            }
        }),
    );

    // VAE Decode
    nodes.insert(
        "6".into(),
        json!({
            "class_type": "VAEDecode",
            "inputs": {
                "samples": ["5", 0],
                "vae": ["1", 2],
            }
        }),
    );

    // Save Image
    nodes.insert(
        "7".into(),
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "images": ["6", 0],
                "filename_prefix": format!("amigo_{}_{}", style.name, req.asset_type_str()),
            }
        }),
    );

    // LoRA if configured
    if let Some(lora) = &style.lora {
        nodes.insert(
            "8".into(),
            json!({
                "class_type": "LoraLoader",
                "inputs": {
                    "model": ["1", 0],
                    "clip": ["1", 1],
                    "lora_name": lora,
                    "strength_model": 0.8,
                    "strength_clip": 0.8,
                }
            }),
        );
        // Rewire sampler to use LoRA model
        if let Some(sampler) = nodes.get_mut("5") {
            sampler["inputs"]["model"] = json!(["8", 0]);
        }
        // Rewire CLIP encoders to use LoRA CLIP
        if let Some(pos) = nodes.get_mut("2") {
            pos["inputs"]["clip"] = json!(["8", 1]);
        }
        if let Some(neg) = nodes.get_mut("3") {
            neg["inputs"]["clip"] = json!(["8", 1]);
        }
    }

    ComfyPrompt {
        prompt: nodes,
        client_id: Some("amigo_artgen".into()),
    }
}

fn build_tileset_workflow(req: &ArtRequest, style: &WorldStyle) -> ComfyPrompt {
    // Tilesets use larger resolution and a grid-specific prompt suffix
    let mut modified = req.clone();
    modified.prompt = format!(
        "{}, seamless tile grid, top-down view, consistent spacing",
        req.prompt
    );
    if modified.width < 128 {
        modified.width = 256;
        modified.height = 256;
    }
    build_sprite_workflow(&modified, style)
}

fn build_portrait_workflow(req: &ArtRequest, style: &WorldStyle) -> ComfyPrompt {
    let mut modified = req.clone();
    modified.prompt = format!(
        "{}, character portrait, face closeup, expressive",
        req.prompt
    );
    if modified.width < 64 {
        modified.width = 96;
        modified.height = 96;
    }
    build_sprite_workflow(&modified, style)
}

fn build_background_workflow(req: &ArtRequest, style: &WorldStyle) -> ComfyPrompt {
    let mut modified = req.clone();
    modified.prompt = format!(
        "{}, wide scene, parallax background layer, scenic",
        req.prompt
    );
    if modified.width < 320 {
        modified.width = 480;
        modified.height = 270;
    }
    build_sprite_workflow(&modified, style)
}

fn build_particle_workflow(req: &ArtRequest, style: &WorldStyle) -> ComfyPrompt {
    let mut modified = req.clone();
    modified.prompt = format!(
        "{}, small particle effect, transparent background, glow",
        req.prompt
    );
    if modified.width > 32 {
        modified.width = 16;
        modified.height = 16;
    }
    build_sprite_workflow(&modified, style)
}

/// Generate a pseudo-random seed for reproducibility logging.
fn rand_seed() -> u64 {
    // Use system time as seed — deterministic workflows can override via extra params
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64 % 1_000_000_000)
        .unwrap_or(42)
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

/// Build an img2img variation workflow
pub fn build_img2img_workflow(
    input_path: &str,
    prompt: &str,
    negative_prompt: &str,
    strength: f32,
    style: &WorldStyle,
) -> ComfyPrompt {
    // Similar to sprite but with LoadImage + denoise < 1.0
    let mut nodes = HashMap::new();

    let model = "pixel_art_v1.safetensors";

    nodes.insert(
        "1".into(),
        json!({
            "class_type": "CheckpointLoaderSimple",
            "inputs": { "ckpt_name": model }
        }),
    );

    nodes.insert(
        "2".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": format!("{}{}", style.style_prompt_prefix, prompt),
                "clip": ["1", 1]
            }
        }),
    );

    nodes.insert(
        "3".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": format!("{}, smooth, gradient, photorealistic", negative_prompt),
                "clip": ["1", 1]
            }
        }),
    );

    // Load input image
    nodes.insert(
        "4".into(),
        json!({
            "class_type": "LoadImage",
            "inputs": { "image": input_path }
        }),
    );

    // VAE Encode the input
    nodes.insert(
        "5".into(),
        json!({
            "class_type": "VAEEncode",
            "inputs": {
                "pixels": ["4", 0],
                "vae": ["1", 2]
            }
        }),
    );

    // KSampler with denoise < 1.0
    nodes.insert(
        "6".into(),
        json!({
            "class_type": "KSampler",
            "inputs": {
                "model": ["1", 0],
                "positive": ["2", 0],
                "negative": ["3", 0],
                "latent_image": ["5", 0],
                "seed": rand_seed(),
                "steps": 20,
                "cfg": 7.0,
                "sampler_name": "euler_ancestral",
                "scheduler": "normal",
                "denoise": strength.clamp(0.0, 1.0),
            }
        }),
    );

    nodes.insert(
        "7".into(),
        json!({
            "class_type": "VAEDecode",
            "inputs": { "samples": ["6", 0], "vae": ["1", 2] }
        }),
    );

    nodes.insert(
        "8".into(),
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "images": ["7", 0],
                "filename_prefix": format!("amigo_{}_variation", style.name),
            }
        }),
    );

    ComfyPrompt {
        prompt: nodes,
        client_id: Some("amigo_artgen".into()),
    }
}

/// Build an inpainting workflow
pub fn build_inpaint_workflow(
    input_path: &str,
    mask_path: &str,
    prompt: &str,
    negative_prompt: &str,
    style: &WorldStyle,
) -> ComfyPrompt {
    let mut nodes = HashMap::new();
    let model = "pixel_art_v1.safetensors";

    nodes.insert(
        "1".into(),
        json!({
            "class_type": "CheckpointLoaderSimple",
            "inputs": { "ckpt_name": model }
        }),
    );

    nodes.insert(
        "2".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": format!("{}{}", style.style_prompt_prefix, prompt),
                "clip": ["1", 1]
            }
        }),
    );

    nodes.insert(
        "3".into(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": format!("{}, smooth, gradient, photorealistic", negative_prompt),
                "clip": ["1", 1]
            }
        }),
    );

    nodes.insert(
        "4".into(),
        json!({
            "class_type": "LoadImage",
            "inputs": { "image": input_path }
        }),
    );

    nodes.insert(
        "5".into(),
        json!({
            "class_type": "LoadImage",
            "inputs": { "image": mask_path }
        }),
    );

    // Set latent noise mask
    nodes.insert(
        "6".into(),
        json!({
            "class_type": "VAEEncode",
            "inputs": { "pixels": ["4", 0], "vae": ["1", 2] }
        }),
    );

    nodes.insert(
        "7".into(),
        json!({
            "class_type": "SetLatentNoiseMask",
            "inputs": {
                "samples": ["6", 0],
                "mask": ["5", 1]
            }
        }),
    );

    nodes.insert(
        "8".into(),
        json!({
            "class_type": "KSampler",
            "inputs": {
                "model": ["1", 0],
                "positive": ["2", 0],
                "negative": ["3", 0],
                "latent_image": ["7", 0],
                "seed": rand_seed(),
                "steps": 25,
                "cfg": 7.5,
                "sampler_name": "euler_ancestral",
                "scheduler": "normal",
                "denoise": 0.85,
            }
        }),
    );

    nodes.insert(
        "9".into(),
        json!({
            "class_type": "VAEDecode",
            "inputs": { "samples": ["8", 0], "vae": ["1", 2] }
        }),
    );

    nodes.insert(
        "10".into(),
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "images": ["9", 0],
                "filename_prefix": format!("amigo_{}_inpaint", style.name),
            }
        }),
    );

    ComfyPrompt {
        prompt: nodes,
        client_id: Some("amigo_artgen".into()),
    }
}

/// Build an upscale workflow (uses nearest-neighbor for pixel art)
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_workflow_has_all_nodes() {
        let req = ArtRequest {
            prompt: "a pirate captain".into(),
            ..Default::default()
        };
        let style = WorldStyle::find("caribbean").unwrap();
        let prompt = build_workflow(&req, &style);

        assert!(prompt.prompt.contains_key("1")); // checkpoint
        assert!(prompt.prompt.contains_key("2")); // positive
        assert!(prompt.prompt.contains_key("3")); // negative
        assert!(prompt.prompt.contains_key("5")); // sampler
        assert!(prompt.prompt.contains_key("7")); // save
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
        let latent = &prompt.prompt["4"];
        assert_eq!(latent["inputs"]["width"], 256);
    }

    #[test]
    fn lora_rewires_nodes() {
        let req = ArtRequest::default();
        let mut style = WorldStyle::find("dune").unwrap();
        style.lora = Some("pixel_lora_v1.safetensors".into());

        let prompt = build_workflow(&req, &style);

        assert!(prompt.prompt.contains_key("8")); // LoRA loader
                                                  // Sampler should reference LoRA output
        let sampler = &prompt.prompt["5"];
        assert_eq!(sampler["inputs"]["model"], json!(["8", 0]));
    }

    #[test]
    fn background_workflow_wide() {
        let req = ArtRequest {
            asset_type: AssetType::Background,
            ..Default::default()
        };
        let style = WorldStyle::find("matrix").unwrap();
        let prompt = build_workflow(&req, &style);
        let latent = &prompt.prompt["4"];
        assert_eq!(latent["inputs"]["width"], 480);
        assert_eq!(latent["inputs"]["height"], 270);
    }

    #[test]
    fn img2img_workflow_has_load_image_and_denoise() {
        let style = WorldStyle::find("caribbean").unwrap();
        let prompt = build_img2img_workflow("input.png", "a pirate ship", "blurry", 0.6, &style);

        // Should have LoadImage node
        assert!(prompt.prompt.contains_key("4"));
        assert_eq!(prompt.prompt["4"]["class_type"], "LoadImage");

        // Should have VAEEncode for input
        assert!(prompt.prompt.contains_key("5"));
        assert_eq!(prompt.prompt["5"]["class_type"], "VAEEncode");

        // KSampler denoise should be clamped strength
        let sampler = &prompt.prompt["6"];
        assert_eq!(sampler["class_type"], "KSampler");
        let denoise = sampler["inputs"]["denoise"].as_f64().unwrap();
        assert!((denoise - 0.6).abs() < 0.001);

        // Should have SaveImage
        assert!(prompt.prompt.contains_key("8"));
        assert_eq!(prompt.prompt["8"]["class_type"], "SaveImage");
    }

    #[test]
    fn img2img_clamps_strength() {
        let style = WorldStyle::find("dune").unwrap();
        let prompt = build_img2img_workflow("input.png", "test", "", 1.5, &style);
        let denoise = prompt.prompt["6"]["inputs"]["denoise"].as_f64().unwrap();
        assert!((denoise - 1.0).abs() < 0.001);

        let prompt = build_img2img_workflow("input.png", "test", "", -0.5, &style);
        let denoise = prompt.prompt["6"]["inputs"]["denoise"].as_f64().unwrap();
        assert!((denoise - 0.0).abs() < 0.001);
    }

    #[test]
    fn inpaint_workflow_has_mask_and_noise_mask() {
        let style = WorldStyle::find("lotr").unwrap();
        let prompt =
            build_inpaint_workflow("input.png", "mask.png", "a stone wall", "blurry", &style);

        // Should load both input and mask images
        assert_eq!(prompt.prompt["4"]["class_type"], "LoadImage");
        assert_eq!(prompt.prompt["4"]["inputs"]["image"], "input.png");
        assert_eq!(prompt.prompt["5"]["class_type"], "LoadImage");
        assert_eq!(prompt.prompt["5"]["inputs"]["image"], "mask.png");

        // Should have SetLatentNoiseMask
        assert!(prompt.prompt.contains_key("7"));
        assert_eq!(prompt.prompt["7"]["class_type"], "SetLatentNoiseMask");

        // KSampler should use the noise-masked latent
        let sampler = &prompt.prompt["8"];
        assert_eq!(sampler["class_type"], "KSampler");
        assert_eq!(sampler["inputs"]["latent_image"], json!(["7", 0]));

        // Should have SaveImage
        assert!(prompt.prompt.contains_key("10"));
        assert_eq!(prompt.prompt["10"]["class_type"], "SaveImage");
    }

    #[test]
    fn upscale_workflow_uses_nearest_neighbor() {
        let prompt = build_upscale_workflow("sprite.png", 4);

        // Should load image
        assert_eq!(prompt.prompt["1"]["class_type"], "LoadImage");
        assert_eq!(prompt.prompt["1"]["inputs"]["image"], "sprite.png");

        // Should scale with nearest-exact
        assert_eq!(prompt.prompt["2"]["class_type"], "ImageScaleBy");
        assert_eq!(
            prompt.prompt["2"]["inputs"]["upscale_method"],
            "nearest-exact"
        );
        assert_eq!(prompt.prompt["2"]["inputs"]["scale_by"], 4);

        // Should save
        assert_eq!(prompt.prompt["3"]["class_type"], "SaveImage");
        let prefix = prompt.prompt["3"]["inputs"]["filename_prefix"]
            .as_str()
            .unwrap();
        assert!(prefix.contains("4x"));
    }
}
