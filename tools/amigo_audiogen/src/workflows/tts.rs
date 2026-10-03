//! ComfyUI workflow builder for Qwen3-TTS speech synthesis.
//!
//! Builds a prompt graph that:
//! 1. Loads the Qwen3-TTS model (`QwenTTSLoader`)
//! 2. Generates speech from text (`QwenTTSGenerate`)
//! 3. Optionally loads reference audio for voice cloning (`LoadAudio`)
//! 4. Saves the output (`SaveAudio`)

use crate::TtsRequest;
use amigo_comfyui::ComfyPrompt;
use serde_json::json;
use std::collections::HashMap;

/// Build a ComfyUI workflow prompt for Qwen3-TTS generation.
///
/// The workflow graph:
/// ```text
/// [1: QwenTTSLoader] -> [2: QwenTTSGenerate] -> [3: SaveAudio]
///                              ^
///                     [4: LoadAudio] (optional, for voice cloning)
/// ```
pub fn build_tts_workflow(request: &TtsRequest) -> ComfyPrompt {
    let mut prompt = HashMap::new();

    // Node 1: Load the Qwen3-TTS model
    prompt.insert(
        "1".into(),
        json!({
            "class_type": "QwenTTSLoader",
            "inputs": {
                "model_name": "qwen3-tts-1.7b"
            }
        }),
    );

    // Node 2: Generate speech from text
    let mut generate_inputs = json!({
        "text": request.text,
        "language": &request.language,
        "model": ["1", 0],
    });

    if let Some(ref delivery) = request.delivery {
        generate_inputs["delivery_instruction"] = json!(delivery);
    }

    // If reference audio is provided (voice cloning), connect the LoadAudio node
    if request.reference_audio.is_some() {
        generate_inputs["reference_audio"] = json!(["4", 0]);
    }

    prompt.insert(
        "2".into(),
        json!({
            "class_type": "QwenTTSGenerate",
            "inputs": generate_inputs
        }),
    );

    // Node 3: Save the generated audio
    let format_str = match request.format {
        crate::AudioFormat::Wav => "wav",
        crate::AudioFormat::Ogg => "ogg",
    };

    prompt.insert(
        "3".into(),
        json!({
            "class_type": "SaveAudio",
            "inputs": {
                "audio": ["2", 0],
                "filename_prefix": "tts_output",
                "format": format_str
            }
        }),
    );

    // Node 4 (optional): Load reference audio for voice cloning
    if let Some(ref ref_audio) = request.reference_audio {
        prompt.insert(
            "4".into(),
            json!({
                "class_type": "LoadAudio",
                "inputs": {
                    "audio": ref_audio
                }
            }),
        );
    }

    ComfyPrompt {
        prompt,
        client_id: Some("amigo_audiogen_tts".into()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioFormat, TtsRequest};

    #[test]
    fn tts_workflow_basic() {
        let req = TtsRequest {
            text: "Hallo Welt".into(),
            language: "de-DE".into(),
            delivery: None,
            reference_audio: None,
            speaker_id: None,
            format: AudioFormat::Wav,
        };

        let wf = build_tts_workflow(&req);
        assert!(wf.prompt.contains_key("1")); // Loader
        assert!(wf.prompt.contains_key("2")); // Generate
        assert!(wf.prompt.contains_key("3")); // Save
        assert!(!wf.prompt.contains_key("4")); // No reference audio
        assert_eq!(wf.client_id.as_deref(), Some("amigo_audiogen_tts"));
    }

    #[test]
    fn tts_workflow_with_voice_cloning() {
        let req = TtsRequest {
            text: "Hello world".into(),
            language: "en-US".into(),
            delivery: Some("speak with excitement".into()),
            reference_audio: Some("assets/voices/narrator.wav".into()),
            speaker_id: Some("narrator".into()),
            format: AudioFormat::Ogg,
        };

        let wf = build_tts_workflow(&req);
        assert!(wf.prompt.contains_key("4")); // Reference audio loader
        // Verify generate node references the loader
        let gen_node = &wf.prompt["2"];
        assert!(gen_node["inputs"]["reference_audio"].is_array());
        assert!(gen_node["inputs"]["delivery_instruction"].is_string());
    }

    #[test]
    fn tts_workflow_nodes_have_correct_class_types() {
        let req = TtsRequest::default();
        let wf = build_tts_workflow(&TtsRequest {
            text: "test".into(),
            ..req
        });

        assert_eq!(wf.prompt["1"]["class_type"], "QwenTTSLoader");
        assert_eq!(wf.prompt["2"]["class_type"], "QwenTTSGenerate");
        assert_eq!(wf.prompt["3"]["class_type"], "SaveAudio");
    }
}
