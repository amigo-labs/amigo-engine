//! ComfyUI workflow builder for ACE-Step music generation.
//!
//! Builds a prompt graph that:
//! 1. Loads the ACE-Step model (`ACEStepLoader`)
//! 2. Generates music from conditioning (`ACEStepGenerate`)
//! 3. Saves the output (`SaveAudio`)

use crate::MusicRequest;
use amigo_comfyui::ComfyPrompt;
use serde_json::json;
use std::collections::HashMap;

/// Build a ComfyUI workflow prompt for ACE-Step music generation.
///
/// The workflow graph:
/// ```text
/// [1: ACEStepLoader] -> [2: ACEStepGenerate] -> [3: SaveAudio]
/// ```
pub fn build_music_workflow(request: &MusicRequest) -> ComfyPrompt {
    let mut prompt = HashMap::new();

    // Node 1: Load the ACE-Step model
    prompt.insert(
        "1".into(),
        json!({
            "class_type": "ACEStepLoader",
            "inputs": {
                "model_name": "ace-step-v1"
            }
        }),
    );

    // Build genre/style conditioning string
    let genre = if request.genre.is_empty() {
        crate::WorldAudioStyle::find(&request.world, None)
            .map(|s| s.genre.clone())
            .unwrap_or_else(|| "ambient".into())
    } else {
        request.genre.clone()
    };

    let section_str = match &request.section {
        crate::MusicSection::Calm => "calm",
        crate::MusicSection::Tense => "tense",
        crate::MusicSection::Battle => "battle",
        crate::MusicSection::Boss => "boss",
        crate::MusicSection::Victory => "victory",
        crate::MusicSection::Menu => "menu",
        crate::MusicSection::Custom(s) => s.as_str(),
    };

    // Node 2: Generate music
    let mut generate_inputs = json!({
        "model": ["1", 0],
        "genre": genre,
        "bpm": request.bpm,
        "duration": request.duration_secs,
        "mood": section_str,
    });

    if let Some(ref lyrics) = request.lyrics {
        generate_inputs["lyrics"] = json!(lyrics);
    }

    // Pass through extra fields (e.g. conditioning_strength from from_reference)
    for (key, value) in &request.extra {
        generate_inputs[key] = value.clone();
    }

    prompt.insert(
        "2".into(),
        json!({
            "class_type": "ACEStepGenerate",
            "inputs": generate_inputs
        }),
    );

    // Node 3: Save the generated audio
    let filename_prefix = format!("music_{}_{}", request.world, section_str);
    prompt.insert(
        "3".into(),
        json!({
            "class_type": "SaveAudio",
            "inputs": {
                "audio": ["2", 0],
                "filename_prefix": filename_prefix,
                "format": "wav"
            }
        }),
    );

    ComfyPrompt {
        prompt,
        client_id: Some("amigo_audiogen_music".into()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MusicRequest;

    #[test]
    fn music_workflow_basic() {
        let req = MusicRequest::default();
        let wf = build_music_workflow(&req);

        assert!(wf.prompt.contains_key("1")); // Loader
        assert!(wf.prompt.contains_key("2")); // Generate
        assert!(wf.prompt.contains_key("3")); // Save
        assert_eq!(wf.client_id.as_deref(), Some("amigo_audiogen_music"));
    }

    #[test]
    fn music_workflow_uses_genre_from_world() {
        let req = MusicRequest {
            world: "caribbean".into(),
            genre: String::new(), // empty -> should use world style
            ..MusicRequest::default()
        };
        let wf = build_music_workflow(&req);
        let gen_node = &wf.prompt["2"];
        // Caribbean world style has "pirate shanty" genre
        assert!(gen_node["inputs"]["genre"]
            .as_str()
            .unwrap()
            .contains("shanty"));
    }

    #[test]
    fn music_workflow_with_lyrics() {
        let req = MusicRequest {
            lyrics: Some("Yo ho ho".into()),
            ..MusicRequest::default()
        };
        let wf = build_music_workflow(&req);
        let gen_node = &wf.prompt["2"];
        assert_eq!(gen_node["inputs"]["lyrics"], "Yo ho ho");
    }

    #[test]
    fn music_workflow_node_class_types() {
        let req = MusicRequest::default();
        let wf = build_music_workflow(&req);

        assert_eq!(wf.prompt["1"]["class_type"], "ACEStepLoader");
        assert_eq!(wf.prompt["2"]["class_type"], "ACEStepGenerate");
        assert_eq!(wf.prompt["3"]["class_type"], "SaveAudio");
    }
}
