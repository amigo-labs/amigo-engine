//! Clean Mode stem workflow — generates each stem individually for maximum
//! quality, then mixes them into a final track.
//!
//! Flow:
//! 1. `generate_core_melody(prompt, key, bpm)` → melody.wav
//! 2. `generate_stem("bass", melody_ref)` → bass.wav
//! 3. `generate_stem("drums", melody_ref)` → drums.wav
//! 4. `generate_stem("harmony", melody_ref)` → harmony.wav
//! 5. Mix all stems → final_track.wav

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The steps in the clean mode pipeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanModeStep {
    /// Generate the core melody reference.
    GenerateMelody,
    /// Generate an individual stem.
    GenerateStem(String),
    /// Mix all stems into the final track.
    Mix,
    /// Post-process the final track (normalize, BPM-verify, loop-find).
    PostProcess,
}

/// Current state of the pipeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineState {
    /// Not started.
    Idle,
    /// Currently running a step.
    Running(CleanModeStep),
    /// Completed successfully.
    Completed,
    /// Failed with an error message.
    Failed(String),
}

/// Configuration for a clean mode generation run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanModeConfig {
    /// World/theme name for style conditioning.
    pub world: String,
    /// Textual prompt for the music.
    pub prompt: String,
    /// Musical key (e.g. "C minor", "A major").
    pub key: String,
    /// Target BPM.
    pub bpm: u32,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// Which stems to generate (defaults: bass, drums, harmony, melody).
    pub stems: Vec<String>,
    /// Output directory.
    pub output_dir: String,
}

impl Default for CleanModeConfig {
    fn default() -> Self {
        Self {
            world: "default".into(),
            prompt: String::new(),
            key: "C minor".into(),
            bpm: 120,
            duration_secs: 30.0,
            stems: vec![
                "melody".into(),
                "bass".into(),
                "drums".into(),
                "harmony".into(),
            ],
            output_dir: "assets/audio/generated".into(),
        }
    }
}

/// Tracks progress of the clean mode pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanModePipeline {
    pub config: CleanModeConfig,
    pub state: PipelineState,
    /// Completed steps and their output paths.
    pub completed_steps: Vec<(CleanModeStep, String)>,
    /// The melody reference path (set after step 1).
    pub melody_ref: Option<String>,
    /// All generated stem paths.
    pub stem_paths: HashMap<String, String>,
    /// Final mixed output path.
    pub final_output: Option<String>,
    /// Total elapsed time in milliseconds.
    pub elapsed_ms: u64,
}

impl CleanModePipeline {
    /// Create a new pipeline with the given config.
    pub fn new(config: CleanModeConfig) -> Self {
        Self {
            config,
            state: PipelineState::Idle,
            completed_steps: Vec::new(),
            melody_ref: None,
            stem_paths: HashMap::new(),
            final_output: None,
            elapsed_ms: 0,
        }
    }

    /// Get the next step to execute, or `None` if done/failed.
    pub fn next_step(&self) -> Option<CleanModeStep> {
        match &self.state {
            PipelineState::Completed | PipelineState::Failed(_) => None,
            _ => {
                // First: generate melody if not done
                if self.melody_ref.is_none() {
                    return Some(CleanModeStep::GenerateMelody);
                }

                // Then: generate each stem that isn't done yet
                for stem_name in &self.config.stems {
                    if stem_name == "melody" {
                        continue; // melody is the reference, already done
                    }
                    if !self.stem_paths.contains_key(stem_name) {
                        return Some(CleanModeStep::GenerateStem(stem_name.clone()));
                    }
                }

                // All stems done: mix
                if self.final_output.is_none() {
                    return Some(CleanModeStep::Mix);
                }

                // Mix done: post-process
                let post_done = self
                    .completed_steps
                    .iter()
                    .any(|(s, _)| *s == CleanModeStep::PostProcess);
                if !post_done {
                    return Some(CleanModeStep::PostProcess);
                }

                None
            }
        }
    }

    /// Record that a step completed, advancing the pipeline.
    pub fn complete_step(&mut self, step: CleanModeStep, output_path: String) {
        match &step {
            CleanModeStep::GenerateMelody => {
                self.melody_ref = Some(output_path.clone());
                self.stem_paths
                    .insert("melody".into(), output_path.clone());
            }
            CleanModeStep::GenerateStem(name) => {
                self.stem_paths.insert(name.clone(), output_path.clone());
            }
            CleanModeStep::Mix => {
                self.final_output = Some(output_path.clone());
            }
            CleanModeStep::PostProcess => {}
        }

        self.completed_steps.push((step, output_path));

        // Check if we're done
        if self.next_step().is_none() {
            self.state = PipelineState::Completed;
        } else {
            self.state = PipelineState::Idle;
        }
    }

    /// Mark the pipeline as failed.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.state = PipelineState::Failed(error.into());
    }

    /// Mark a step as currently running.
    pub fn begin_step(&mut self, step: CleanModeStep) {
        self.state = PipelineState::Running(step);
    }

    /// How far along we are (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        // Total steps: 1 melody + N-1 stems + 1 mix + 1 postprocess
        let total = self.config.stems.len() as f32 + 2.0; // stems (incl melody) + mix + postprocess
        let done = self.completed_steps.len() as f32;
        (done / total).min(1.0)
    }

    /// Whether the pipeline finished successfully.
    pub fn is_completed(&self) -> bool {
        matches!(self.state, PipelineState::Completed)
    }

    /// Whether the pipeline failed.
    pub fn is_failed(&self) -> bool {
        matches!(self.state, PipelineState::Failed(_))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_step_order() {
        let config = CleanModeConfig {
            stems: vec![
                "melody".into(),
                "bass".into(),
                "drums".into(),
                "harmony".into(),
            ],
            ..Default::default()
        };
        let mut pipe = CleanModePipeline::new(config);

        // First step should be melody
        assert_eq!(pipe.next_step(), Some(CleanModeStep::GenerateMelody));

        // Complete melody
        pipe.complete_step(
            CleanModeStep::GenerateMelody,
            "out/melody.wav".into(),
        );
        assert_eq!(pipe.melody_ref, Some("out/melody.wav".into()));

        // Next should be bass (first non-melody stem)
        let next = pipe.next_step().unwrap();
        assert!(matches!(next, CleanModeStep::GenerateStem(ref s) if s == "bass"));

        // Complete bass
        pipe.complete_step(
            CleanModeStep::GenerateStem("bass".into()),
            "out/bass.wav".into(),
        );

        // Complete drums
        pipe.complete_step(
            CleanModeStep::GenerateStem("drums".into()),
            "out/drums.wav".into(),
        );

        // Complete harmony
        pipe.complete_step(
            CleanModeStep::GenerateStem("harmony".into()),
            "out/harmony.wav".into(),
        );

        // Next should be Mix
        assert_eq!(pipe.next_step(), Some(CleanModeStep::Mix));
        pipe.complete_step(CleanModeStep::Mix, "out/final.wav".into());

        // Next should be PostProcess
        assert_eq!(pipe.next_step(), Some(CleanModeStep::PostProcess));
        pipe.complete_step(CleanModeStep::PostProcess, "out/final.wav".into());

        // Done
        assert!(pipe.is_completed());
        assert!(pipe.next_step().is_none());
        assert_eq!(pipe.stem_paths.len(), 4);
    }

    #[test]
    fn pipeline_progress() {
        let config = CleanModeConfig::default();
        let mut pipe = CleanModePipeline::new(config);

        assert_eq!(pipe.progress(), 0.0);

        pipe.complete_step(
            CleanModeStep::GenerateMelody,
            "melody.wav".into(),
        );
        assert!(pipe.progress() > 0.0);
        assert!(pipe.progress() < 1.0);
    }

    #[test]
    fn pipeline_failure() {
        let config = CleanModeConfig::default();
        let mut pipe = CleanModePipeline::new(config);

        pipe.fail("ACE-Step server unreachable");
        assert!(pipe.is_failed());
        assert!(pipe.next_step().is_none());
    }

    #[test]
    fn default_config_has_four_stems() {
        let config = CleanModeConfig::default();
        assert_eq!(config.stems.len(), 4);
        assert!(config.stems.contains(&"melody".to_string()));
        assert!(config.stems.contains(&"bass".to_string()));
        assert!(config.stems.contains(&"drums".to_string()));
        assert!(config.stems.contains(&"harmony".to_string()));
    }
}
