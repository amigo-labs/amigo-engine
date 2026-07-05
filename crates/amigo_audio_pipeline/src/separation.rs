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

    /// Check if input is a mono WAV by reading the channel count from the
    /// `fmt ` chunk. Non-WAV or unreadable files are treated as non-mono so
    /// they still go through full separation.
    fn is_mono(input: &Path) -> bool {
        let Ok(data) = std::fs::read(input) else {
            return false;
        };
        // RIFF header: "RIFF" .... "WAVE", then chunks of [id, size, data].
        if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return false;
        }
        let mut pos = 12;
        while pos + 8 <= data.len() {
            let chunk_id = &data[pos..pos + 4];
            let chunk_size =
                u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                    as usize;
            if chunk_id == b"fmt " {
                // Channel count is a u16 at offset 2 within the fmt chunk.
                if pos + 8 + 4 <= data.len() {
                    let channels = u16::from_le_bytes([data[pos + 10], data[pos + 11]]);
                    return channels == 1;
                }
                return false;
            }
            // Chunks are word-aligned.
            pos += 8 + chunk_size + (chunk_size & 1);
        }
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
        // `--two-stems vocals` produces vocals.wav + no_vocals.wav (the full
        // instrumental); the four-stem names are checked as well in case the
        // command is ever switched back to full separation.
        let demucs_stems = ["vocals", "no_vocals", "bass", "drums", "other"];
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
