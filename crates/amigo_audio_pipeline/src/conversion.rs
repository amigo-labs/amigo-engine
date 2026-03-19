use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::ConversionConfig;
use crate::pipeline::PipelineError;

/// MIDI-to-TidalCycles conversion stage using midi_to_tidalcycles.
pub struct ConversionStage {
    config: ConversionConfig,
    uv_path: PathBuf,
    venv_python: PathBuf,
}

/// Result of MIDI-to-Tidal conversion.
#[derive(Debug, Clone)]
pub struct ConversionResult {
    /// Generated TidalCycles notation per stem.
    pub tidal_patterns: Vec<(String, String)>,
}

impl ConversionStage {
    pub fn new(config: ConversionConfig, uv_path: PathBuf, venv_python: PathBuf) -> Self {
        Self {
            config,
            uv_path,
            venv_python,
        }
    }

    /// Convert MIDI files to TidalCycles mini-notation.
    pub fn run(
        &self,
        midi_files: &[(String, PathBuf)],
        output_dir: &Path,
    ) -> Result<ConversionResult, PipelineError> {
        std::fs::create_dir_all(output_dir).map_err(PipelineError::Io)?;

        let mut tidal_patterns = Vec::new();

        for (stem_name, midi_path) in midi_files {
            let tidal_output = output_dir.join(format!("{stem_name}.tidal"));

            // Run midi_to_tidalcycles via Python.
            let script = format!(
                "import midi_to_tidalcycles as m2t; \
                 result = m2t.convert('{}', resolution={}, consolidate={}); \
                 print(result)",
                midi_path.display().to_string().replace('\\', "\\\\"),
                self.config.resolution,
                if self.config.consolidate {
                    "True"
                } else {
                    "False"
                },
            );

            let output = Command::new(&self.uv_path)
                .args([
                    "run",
                    "--python",
                    &self.venv_python.display().to_string(),
                    "python",
                    "-c",
                    &script,
                ])
                .output()
                .map_err(|e| PipelineError::ToolExecFailed {
                    tool: "midi_to_tidalcycles".into(),
                    message: e.to_string(),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(PipelineError::ToolExecFailed {
                    tool: "midi_to_tidalcycles".into(),
                    message: stderr.to_string(),
                });
            }

            let tidal_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            std::fs::write(&tidal_output, &tidal_text).map_err(PipelineError::Io)?;
            tidal_patterns.push((stem_name.clone(), tidal_text));
        }

        Ok(ConversionResult { tidal_patterns })
    }
}
