/// Audio-to-TidalCycles pipeline for the Amigo Engine.
///
/// Orchestrates external Python tools (Demucs, Basic Pitch, midi_to_tidalcycles)
/// to convert audio files into `.amigo.tidal` mini-notation files that the engine
/// can consume at runtime.
pub mod config;
pub mod conversion;
pub mod pipeline;
pub mod postprocess;
pub mod separation;
pub mod transcription;

pub use config::PipelineConfig;
pub use pipeline::{PipelineError, PipelineOrchestrator};
