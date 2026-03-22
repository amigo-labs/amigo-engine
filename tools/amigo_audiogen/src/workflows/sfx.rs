//! ComfyUI workflow builder for Stable Audio Open SFX generation.
//!
//! Builds a prompt graph that:
//! 1. Loads the Stable Audio model (`StableAudioLoader`)
//! 2. Generates SFX from a text prompt (`StableAudioSampler`)
//! 3. Saves the output (`SaveAudio`)
//!
//! Replaces the previous Gradio-based AudioGen integration.

use crate::SfxRequest;
use amigo_comfyui::ComfyPrompt;
use serde_json::json;
use std::collections::HashMap;

/// Build a ComfyUI workflow prompt for Stable Audio Open SFX generation.
///
/// The workflow graph:
/// ```text
/// [1: StableAudioLoader] -> [2: StableAudioSampler] -> [3: SaveAudio]
/// ```
pub fn build_sfx_workflow(request: &SfxRequest) -> ComfyPrompt {
    let mut prompt = HashMap::new();

    // Node 1: Load the Stable Audio model
    prompt.insert(
        "1".into(),
        json!({
            "class_type": "StableAudioLoader",
            "inputs": {
                "model_name": "stable-audio-open-1.0"
            }
        }),
    );

    // Build the category prefix for better conditioning
    let category_prefix = match &request.category {
        crate::SfxCategory::Gameplay => "game sound effect, ",
        crate::SfxCategory::UI => "user interface sound, clean, ",
        crate::SfxCategory::Ambient => "ambient background sound, ",
        crate::SfxCategory::Impact => "impact sound, ",
        crate::SfxCategory::Explosion => "explosion sound, ",
        crate::SfxCategory::Magic => "magical sound effect, fantasy, ",
        crate::SfxCategory::Voice => "vocal sound effect, ",
        crate::SfxCategory::Custom(_) => "",
    };

    let full_prompt = format!("{}{}", category_prefix, request.prompt);

    // Node 2: Generate SFX
    prompt.insert(
        "2".into(),
        json!({
            "class_type": "StableAudioSampler",
            "inputs": {
                "model": ["1", 0],
                "prompt": full_prompt,
                "duration": request.duration_secs,
                "num_variants": request.variants,
            }
        }),
    );

    // Node 3: Save the generated audio
    prompt.insert(
        "3".into(),
        json!({
            "class_type": "SaveAudio",
            "inputs": {
                "audio": ["2", 0],
                "filename_prefix": "sfx_output",
                "format": "wav"
            }
        }),
    );

    ComfyPrompt {
        prompt,
        client_id: Some("amigo_audiogen_sfx".into()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SfxRequest;

    #[test]
    fn sfx_workflow_basic() {
        let req = SfxRequest {
            prompt: "sword clash".into(),
            ..SfxRequest::default()
        };
        let wf = build_sfx_workflow(&req);

        assert!(wf.prompt.contains_key("1")); // Loader
        assert!(wf.prompt.contains_key("2")); // Sampler
        assert!(wf.prompt.contains_key("3")); // Save
        assert_eq!(wf.client_id.as_deref(), Some("amigo_audiogen_sfx"));
    }

    #[test]
    fn sfx_workflow_adds_category_prefix() {
        let req = SfxRequest {
            prompt: "fireball whoosh".into(),
            category: crate::SfxCategory::Magic,
            ..SfxRequest::default()
        };
        let wf = build_sfx_workflow(&req);
        let sampler = &wf.prompt["2"];
        let prompt_text = sampler["inputs"]["prompt"].as_str().unwrap();
        assert!(prompt_text.starts_with("magical sound effect"));
        assert!(prompt_text.contains("fireball whoosh"));
    }

    #[test]
    fn sfx_workflow_node_class_types() {
        let req = SfxRequest {
            prompt: "click".into(),
            ..SfxRequest::default()
        };
        let wf = build_sfx_workflow(&req);

        assert_eq!(wf.prompt["1"]["class_type"], "StableAudioLoader");
        assert_eq!(wf.prompt["2"]["class_type"], "StableAudioSampler");
        assert_eq!(wf.prompt["3"]["class_type"], "SaveAudio");
    }
}
