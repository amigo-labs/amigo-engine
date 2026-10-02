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
            check_stem_name(stem_name)?;
            let tidal_output = output_dir.join(format!("{stem_name}.tidal"));

            // Run midi_to_tidalcycles via Python. The MIDI path travels as an
            // argument (sys.argv[1]), never inside the source: it comes from
            // file names and the --config TOML, and a name like
            // `x'); __import__('os').system('…'); ('.wav` spliced into a
            // string literal ran arbitrary Python.
            let script = format!(
                "import sys, midi_to_tidalcycles as m2t; \
                 print(m2t.convert(sys.argv[1], resolution={}, consolidate={}))",
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
                .arg(midi_path)
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

/// Stem names become file names under the output directory, so they must be
/// a single plain path component: `../x` would write outside it.
fn check_stem_name(name: &str) -> Result<(), PipelineError> {
    let plain = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', '\0'])
        && !name.contains(':');
    if plain {
        Ok(())
    } else {
        Err(PipelineError::ToolExecFailed {
            tool: "midi_to_tidalcycles".into(),
            message: format!("invalid stem name {name:?}: must be a plain file name"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_names_cannot_leave_the_output_directory() {
        for bad in ["", ".", "..", "../x", "a/b", "a\\b", "c:evil"] {
            assert!(check_stem_name(bad).is_err(), "{bad:?}");
        }
        for good in ["bass", "lead-2", "drums.kick", "Größe"] {
            assert!(check_stem_name(good).is_ok(), "{good:?}");
        }
    }
}
