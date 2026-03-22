//! ComfyUI workflow builders for audio generation.
//!
//! Each module builds a ComfyUI prompt graph for a specific backend:
//! - `tts`: Qwen3-TTS speech synthesis
//! - `music`: ACE-Step music generation
//! - `sfx`: Stable Audio Open SFX generation

pub mod music;
pub mod sfx;
pub mod tts;
