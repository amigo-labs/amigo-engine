use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::SeparationConfig;
use crate::pipeline::PipelineError;

/// Demucs source separation stage.
pub struct SeparationStage {
    config: SeparationConfig,
    uv_path: PathBuf,
    venv_python: PathBuf,
}

/// Result of stem separation.
#[derive(Debug, Clone)]
pub struct SeparationResult {
    /// Paths to the separated stem files, keyed by stem name.
    pub stems: Vec<(String, PathBuf)>,
}

impl SeparationStage {
    pub fn new(config: SeparationConfig, uv_path: PathBuf, venv_python: PathBuf) -> Self {
        Self {
            config,
            uv_path,
            venv_python,
        }
    }

    /// Check if input is mono (single voice) by examining channel count.
    fn is_mono(input: &Path) -> bool {
        // Simple heuristic: check file size or use basic audio analysis.
        // For now, we rely on the user flag or config.
        let _ = input;
        false
    }

    /// Run Demucs source separation on the input audio file.
    pub fn run(&self, input: &Path, output_dir: &Path) -> Result<SeparationResult, PipelineError> {
        if !self.config.enabled {
            // Skip separation — treat input as single stem.
            return Ok(SeparationResult {
                stems: vec![("full".into(), input.to_path_buf())],
            });
        }

        if self.config.skip_if_mono && Self::is_mono(input) {
            return Ok(SeparationResult {
                stems: vec![("full".into(), input.to_path_buf())],
            });
        }

        std::fs::create_dir_all(output_dir).map_err(PipelineError::Io)?;

        let output = Command::new(&self.uv_path)
            .args([
                "run",
                "--python",
                &self.venv_python.display().to_string(),
                "demucs",
                "--two-stems",
                "vocals",
                "-n",
                &self.config.model,
                "--out",
                &output_dir.display().to_string(),
                &input.display().to_string(),
            ])
            .output()
            .map_err(|e| PipelineError::ToolExecFailed {
                tool: "demucs".into(),
                message: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PipelineError::ToolExecFailed {
                tool: "demucs".into(),
                message: stderr.to_string(),
            });
        }

        // Demucs outputs stems into <output_dir>/<model>/<track_name>/*.wav
        let track_name = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let stems_dir = output_dir.join(&self.config.model).join(track_name);

        let mut stems = Vec::new();
        let demucs_stems = ["vocals", "bass", "drums", "other"];
        for demucs_name in &demucs_stems {
            let stem_path = stems_dir.join(format!("{demucs_name}.wav"));
            if stem_path.exists() {
                let mapped_name = self
                    .config
                    .stem_mapping
                    .get(*demucs_name)
                    .cloned()
                    .unwrap_or_else(|| demucs_name.to_string());
                stems.push((mapped_name, stem_path));
            }
        }

        Ok(SeparationResult { stems })
    }
}
