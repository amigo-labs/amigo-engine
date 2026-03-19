use std::path::{Path, PathBuf};

use crate::config::PipelineConfig;
use crate::conversion::ConversionStage;
use crate::separation::{SeparationResult, SeparationStage};
use crate::transcription::TranscriptionStage;

/// Pipeline orchestration error.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Amigo Python toolchain not set up. {hint}")]
    SetupRequired { hint: String },
    #[error("tool '{tool}' execution failed: {message}")]
    ToolExecFailed { tool: String, message: String },
    #[error("IO error: {0}")]
    Io(std::io::Error),
    #[error("config error: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("tidal parse error: {0}")]
    TidalParse(String),
}

/// Paths to the Amigo Python toolchain.
#[derive(Debug, Clone)]
pub struct ToolchainPaths {
    pub amigo_home: PathBuf,
    pub uv_path: PathBuf,
    pub venv_python: PathBuf,
}

impl ToolchainPaths {
    /// Detect toolchain from default location (~/.amigo/).
    pub fn detect() -> Result<Self, PipelineError> {
        let home = home_dir().ok_or_else(|| PipelineError::SetupRequired {
            hint: "Could not determine home directory".into(),
        })?;
        let amigo_home = home.join(".amigo");

        let uv_path = if cfg!(windows) {
            amigo_home.join("bin").join("uv.exe")
        } else {
            amigo_home.join("bin").join("uv")
        };

        if !uv_path.exists() {
            return Err(PipelineError::SetupRequired {
                hint: "Run `amigo setup` to install the Python toolchain".into(),
            });
        }

        let venv_python = if cfg!(windows) {
            amigo_home.join("venv").join("Scripts").join("python.exe")
        } else {
            amigo_home.join("venv").join("bin").join("python")
        };

        if !venv_python.exists() {
            return Err(PipelineError::SetupRequired {
                hint: "Run `amigo setup` to create the Python virtual environment".into(),
            });
        }

        Ok(Self {
            amigo_home,
            uv_path,
            venv_python,
        })
    }
}

/// Full pipeline orchestrator.
pub struct PipelineOrchestrator {
    config: PipelineConfig,
    toolchain: ToolchainPaths,
}

impl PipelineOrchestrator {
    pub fn new(config: PipelineConfig, toolchain: ToolchainPaths) -> Self {
        Self { config, toolchain }
    }

    /// Create orchestrator with default config, auto-detecting toolchain.
    pub fn with_defaults() -> Result<Self, PipelineError> {
        let toolchain = ToolchainPaths::detect()?;
        Ok(Self {
            config: PipelineConfig::default(),
            toolchain,
        })
    }

    /// Run the full pipeline: Audio -> Stems -> MIDI -> TidalCycles -> .amigo.tidal
    pub fn run_full(
        &self,
        input: &Path,
        output: &Path,
        name: &str,
        bpm: f64,
        metadata: PipelineMetadata,
    ) -> Result<(), PipelineError> {
        let work_dir = output
            .parent()
            .unwrap_or(Path::new("."))
            .join(".amigo_pipeline_tmp");
        std::fs::create_dir_all(&work_dir).map_err(PipelineError::Io)?;

        // Stage 1: Separation.
        let stems_dir = work_dir.join("stems");
        let sep_result = self.run_separate(input, &stems_dir)?;

        // Stage 2: Transcription.
        let midi_dir = work_dir.join("midi");
        let trans_result = self.run_transcribe(&sep_result.stems, &midi_dir)?;

        // Stage 3: Conversion.
        let tidal_dir = work_dir.join("tidal");
        let conv_result = self.run_notate(&trans_result.midi_files, &tidal_dir)?;

        // Stage 4: Assemble .amigo.tidal file.
        let mut file_content = String::new();
        file_content.push_str("-- amigo:meta\n");
        file_content.push_str(&format!("-- name: \"{name}\"\n"));
        file_content.push_str(&format!("-- bpm: {bpm}\n"));
        if let Some(ref source) = metadata.source {
            file_content.push_str(&format!("-- source: \"{source}\"\n"));
        }
        if let Some(ref license) = metadata.license {
            file_content.push_str(&format!("-- license: \"{license}\"\n"));
        }
        if let Some(ref author) = metadata.author {
            file_content.push_str(&format!("-- author: \"{author}\"\n"));
        }
        file_content.push('\n');

        for (stem_name, tidal_text) in &conv_result.tidal_patterns {
            file_content.push_str(&format!("-- amigo:stem {stem_name}\n"));
            file_content.push_str(tidal_text);
            file_content.push_str("\n\n");
        }

        std::fs::write(output, &file_content).map_err(PipelineError::Io)?;

        // Cleanup temp dir.
        let _ = std::fs::remove_dir_all(&work_dir);

        Ok(())
    }

    /// Run only the separation stage.
    pub fn run_separate(
        &self,
        input: &Path,
        output_dir: &Path,
    ) -> Result<SeparationResult, PipelineError> {
        let stage = SeparationStage::new(
            self.config.separation.clone(),
            self.toolchain.uv_path.clone(),
            self.toolchain.venv_python.clone(),
        );
        stage.run(input, output_dir)
    }

    /// Run only the transcription stage.
    pub fn run_transcribe(
        &self,
        stems: &[(String, PathBuf)],
        output_dir: &Path,
    ) -> Result<crate::transcription::TranscriptionResult, PipelineError> {
        let stage = TranscriptionStage::new(
            self.config.transcription.clone(),
            self.toolchain.uv_path.clone(),
            self.toolchain.venv_python.clone(),
        );
        stage.run(stems, output_dir)
    }

    /// Run only the MIDI-to-Tidal conversion stage.
    pub fn run_notate(
        &self,
        midi_files: &[(String, PathBuf)],
        output_dir: &Path,
    ) -> Result<crate::conversion::ConversionResult, PipelineError> {
        let stage = ConversionStage::new(
            self.config.conversion.clone(),
            self.toolchain.uv_path.clone(),
            self.toolchain.venv_python.clone(),
        );
        stage.run(midi_files, output_dir)
    }
}

/// Optional metadata for the output file.
#[derive(Debug, Clone, Default)]
pub struct PipelineMetadata {
    pub source: Option<String>,
    pub license: Option<String>,
    pub author: Option<String>,
}

/// Cross-platform home directory.
fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
