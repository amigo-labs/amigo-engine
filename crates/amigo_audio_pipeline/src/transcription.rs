use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::TranscriptionConfig;
use crate::pipeline::PipelineError;

/// Basic Pitch audio-to-MIDI transcription stage.
pub struct TranscriptionStage {
    config: TranscriptionConfig,
    uv_path: PathBuf,
    venv_python: PathBuf,
}

/// Result of MIDI transcription.
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    /// Paths to generated MIDI files, keyed by stem name.
    pub midi_files: Vec<(String, PathBuf)>,
}

impl TranscriptionStage {
    pub fn new(config: TranscriptionConfig, uv_path: PathBuf, venv_python: PathBuf) -> Self {
        Self {
            config,
            uv_path,
            venv_python,
        }
    }

    /// Transcribe audio stems to MIDI files using Basic Pitch.
    pub fn run(
        &self,
        stems: &[(String, PathBuf)],
        output_dir: &Path,
    ) -> Result<TranscriptionResult, PipelineError> {
        std::fs::create_dir_all(output_dir).map_err(PipelineError::Io)?;

        let mut midi_files = Vec::new();

        for (stem_name, stem_path) in stems {
            let midi_output = output_dir.join(format!("{stem_name}.mid"));

            let output = Command::new(&self.uv_path)
                .args([
                    "run",
                    "--python",
                    &self.venv_python.display().to_string(),
                    "basic-pitch",
                    "--onset-threshold",
                    &self.config.onset_threshold.to_string(),
                    "--frame-threshold",
                    &self.config.frame_threshold.to_string(),
                    "--minimum-note-length",
                    &self.config.min_note_length_ms.to_string(),
                    "--minimum-frequency",
                    &self.config.min_frequency_hz.to_string(),
                    "--maximum-frequency",
                    &self.config.max_frequency_hz.to_string(),
                    &output_dir.display().to_string(),
                    &stem_path.display().to_string(),
                ])
                .output()
                .map_err(|e| PipelineError::ToolExecFailed {
                    tool: "basic-pitch".into(),
                    message: e.to_string(),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(PipelineError::ToolExecFailed {
                    tool: "basic-pitch".into(),
                    message: stderr.to_string(),
                });
            }

            // Basic Pitch outputs to <output_dir>/<input_stem>_basic_pitch.mid
            let expected_name = stem_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let actual_path = output_dir.join(format!("{expected_name}_basic_pitch.mid"));

            // Rename to our convention if it exists.
            if actual_path.exists() && actual_path != midi_output {
                std::fs::rename(&actual_path, &midi_output).map_err(PipelineError::Io)?;
            }

            if midi_output.exists() {
                midi_files.push((stem_name.clone(), midi_output));
            } else if actual_path.exists() {
                midi_files.push((stem_name.clone(), actual_path));
            }
        }

        Ok(TranscriptionResult { midi_files })
    }
}
