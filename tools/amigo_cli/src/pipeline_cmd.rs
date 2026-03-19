use std::path::PathBuf;

use amigo_audio_pipeline::config::PipelineConfig;
use amigo_audio_pipeline::pipeline::{PipelineMetadata, PipelineOrchestrator, ToolchainPaths};

pub fn cmd_pipeline(args: &[String]) {
    if args.is_empty() {
        print_pipeline_usage();
        std::process::exit(1);
    }

    match args[0].as_str() {
        "convert" => cmd_convert(&args[1..]),
        "separate" => cmd_separate(&args[1..]),
        "transcribe" => cmd_transcribe(&args[1..]),
        "notate" => cmd_notate(&args[1..]),
        "batch" => cmd_batch(&args[1..]),
        "play" => cmd_play(&args[1..]),
        "help" | "--help" => print_pipeline_usage(),
        other => {
            eprintln!("Unknown pipeline command: {other}");
            print_pipeline_usage();
            std::process::exit(1);
        }
    }
}

fn print_pipeline_usage() {
    eprintln!(
        r#"amigo pipeline — Audio-to-TidalCycles Pipeline

USAGE:
    amigo pipeline <COMMAND> [OPTIONS]

COMMANDS:
    convert     Full pipeline: Audio -> .amigo.tidal
    separate    Only stem separation (Demucs)
    transcribe  Only audio-to-MIDI (Basic Pitch)
    notate      Only MIDI-to-TidalCycles
    batch       Process a directory of audio files
    play        Preview a .amigo.tidal file

COMMON OPTIONS:
    --input <PATH>     Input file or directory
    --output <PATH>    Output file or directory
    --config <PATH>    Pipeline config file (TOML)
    --bpm <BPM>        Override BPM (default: auto-detect)
    --name <NAME>      Composition name
    --license <TEXT>    License metadata
    --author <TEXT>     Author metadata
"#
    );
}

fn cmd_convert(args: &[String]) {
    let mut input = None;
    let mut output = None;
    let mut config_path = None;
    let mut bpm = 120.0_f64;
    let mut name = String::new();
    let mut meta = PipelineMetadata::default();

    parse_common_args(
        args,
        &mut input,
        &mut output,
        &mut config_path,
        &mut bpm,
        &mut name,
        &mut meta,
    );

    let input = input.unwrap_or_else(|| {
        eprintln!("--input is required");
        std::process::exit(1);
    });
    let output = output.unwrap_or_else(|| {
        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        PathBuf::from(format!("{stem}.amigo.tidal"))
    });
    if name.is_empty() {
        name = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
    }
    meta.source = Some(input.display().to_string());

    let config = load_config(config_path.as_deref());
    let orchestrator = create_orchestrator(config);

    println!("Converting {} -> {}", input.display(), output.display());
    match orchestrator.run_full(&input, &output, &name, bpm, meta) {
        Ok(()) => println!("Done: {}", output.display()),
        Err(e) => {
            eprintln!("Pipeline failed: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_separate(args: &[String]) {
    let mut input = None;
    let mut output = None;
    let mut config_path = None;
    let mut bpm = 120.0;
    let mut name = String::new();
    let mut meta = PipelineMetadata::default();

    parse_common_args(
        args,
        &mut input,
        &mut output,
        &mut config_path,
        &mut bpm,
        &mut name,
        &mut meta,
    );

    let input = require_input(input);
    let output = output.unwrap_or_else(|| PathBuf::from("./stems"));

    let config = load_config(config_path.as_deref());
    let orchestrator = create_orchestrator(config);

    println!("Separating {} -> {}", input.display(), output.display());
    match orchestrator.run_separate(&input, &output) {
        Ok(result) => {
            println!("Stems created:");
            for (name, path) in &result.stems {
                println!("  {name}: {}", path.display());
            }
        }
        Err(e) => {
            eprintln!("Separation failed: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_transcribe(args: &[String]) {
    let mut input = None;
    let mut output = None;
    let mut config_path = None;
    let mut bpm = 120.0;
    let mut name = String::new();
    let mut meta = PipelineMetadata::default();

    parse_common_args(
        args,
        &mut input,
        &mut output,
        &mut config_path,
        &mut bpm,
        &mut name,
        &mut meta,
    );

    let input_dir = require_input(input);
    let output = output.unwrap_or_else(|| PathBuf::from("./midi"));

    // Collect WAV files from input directory.
    let stems = collect_stems(&input_dir);
    if stems.is_empty() {
        eprintln!("No audio files found in {}", input_dir.display());
        std::process::exit(1);
    }

    let config = load_config(config_path.as_deref());
    let orchestrator = create_orchestrator(config);

    println!("Transcribing {} stems -> {}", stems.len(), output.display());
    match orchestrator.run_transcribe(&stems, &output) {
        Ok(result) => {
            println!("MIDI files created:");
            for (name, path) in &result.midi_files {
                println!("  {name}: {}", path.display());
            }
        }
        Err(e) => {
            eprintln!("Transcription failed: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_notate(args: &[String]) {
    let mut input = None;
    let mut output = None;
    let mut config_path = None;
    let mut bpm = 120.0;
    let mut name = String::new();
    let mut meta = PipelineMetadata::default();

    parse_common_args(
        args,
        &mut input,
        &mut output,
        &mut config_path,
        &mut bpm,
        &mut name,
        &mut meta,
    );

    let input_dir = require_input(input);
    let output = output.unwrap_or_else(|| PathBuf::from("./tidal"));

    let midi_files = collect_midi_files(&input_dir);
    if midi_files.is_empty() {
        eprintln!("No MIDI files found in {}", input_dir.display());
        std::process::exit(1);
    }

    let config = load_config(config_path.as_deref());
    let orchestrator = create_orchestrator(config);

    println!("Converting {} MIDI files -> TidalCycles", midi_files.len());
    match orchestrator.run_notate(&midi_files, &output) {
        Ok(result) => {
            println!("TidalCycles notation created:");
            for (name, _text) in &result.tidal_patterns {
                println!("  {name}");
            }
        }
        Err(e) => {
            eprintln!("Notation failed: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_batch(args: &[String]) {
    let mut input = None;
    let mut output = None;
    let mut config_path = None;
    let mut bpm = 120.0;
    let mut name = String::new();
    let mut meta = PipelineMetadata::default();

    parse_common_args(
        args,
        &mut input,
        &mut output,
        &mut config_path,
        &mut bpm,
        &mut name,
        &mut meta,
    );

    let input_dir = require_input(input);
    let output_dir = output.unwrap_or_else(|| PathBuf::from("./tidal"));

    std::fs::create_dir_all(&output_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create output directory: {e}");
        std::process::exit(1);
    });

    let audio_files = collect_audio_files(&input_dir);
    if audio_files.is_empty() {
        eprintln!("No audio files found in {}", input_dir.display());
        std::process::exit(1);
    }

    let config = load_config(config_path.as_deref());
    let orchestrator = create_orchestrator(config);

    println!("Batch processing {} files...", audio_files.len());
    let mut success = 0;
    let mut failed = 0;

    for (i, audio_path) in audio_files.iter().enumerate() {
        let file_name = audio_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let output_path = output_dir.join(format!("{file_name}.amigo.tidal"));

        print!("[{}/{}] {}... ", i + 1, audio_files.len(), file_name);
        let file_meta = PipelineMetadata {
            source: Some(audio_path.display().to_string()),
            ..meta.clone()
        };

        match orchestrator.run_full(audio_path, &output_path, file_name, bpm, file_meta) {
            Ok(()) => {
                println!("ok");
                success += 1;
            }
            Err(e) => {
                println!("FAILED: {e}");
                failed += 1;
            }
        }
    }

    println!("\nBatch complete: {success} succeeded, {failed} failed");
}

fn cmd_play(args: &[String]) {
    if args.is_empty() {
        eprintln!(
            "Usage: amigo pipeline play <file.amigo.tidal> [--bpm BPM] [--stems melody,bass]"
        );
        std::process::exit(1);
    }

    let file_path = PathBuf::from(&args[0]);
    if !file_path.exists() {
        eprintln!("File not found: {}", file_path.display());
        std::process::exit(1);
    }

    match amigo_tidal_parser::load(&file_path) {
        Ok(comp) => {
            println!(
                "Loaded: {} (BPM: {}, {} stems)",
                comp.name,
                comp.bpm,
                comp.stems.len()
            );
            for stem in &comp.stems {
                println!("  - {} ({} voices)", stem.name, stem.voices.len());
            }
            println!("\n(Audio playback requires the editor — use `amigo editor` to open the Tidal Playground)");
        }
        Err(e) => {
            eprintln!("Failed to load {}: {e}", file_path.display());
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_common_args(
    args: &[String],
    input: &mut Option<PathBuf>,
    output: &mut Option<PathBuf>,
    config_path: &mut Option<PathBuf>,
    bpm: &mut f64,
    name: &mut String,
    meta: &mut PipelineMetadata,
) {
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--input" | "-i" => {
                i += 1;
                if i < args.len() {
                    *input = Some(PathBuf::from(&args[i]));
                }
            }
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    *output = Some(PathBuf::from(&args[i]));
                }
            }
            "--config" | "-c" => {
                i += 1;
                if i < args.len() {
                    *config_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--bpm" => {
                i += 1;
                if i < args.len() {
                    *bpm = args[i].parse().unwrap_or(120.0);
                }
            }
            "--name" => {
                i += 1;
                if i < args.len() {
                    *name = args[i].clone();
                }
            }
            "--license" => {
                i += 1;
                if i < args.len() {
                    meta.license = Some(args[i].clone());
                }
            }
            "--author" => {
                i += 1;
                if i < args.len() {
                    meta.author = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }
}

fn require_input(input: Option<PathBuf>) -> PathBuf {
    input.unwrap_or_else(|| {
        eprintln!("--input is required");
        std::process::exit(1);
    })
}

fn load_config(path: Option<&std::path::Path>) -> PipelineConfig {
    match path {
        Some(p) => PipelineConfig::load(p).unwrap_or_else(|e| {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }),
        None => PipelineConfig::default(),
    }
}

fn create_orchestrator(config: PipelineConfig) -> PipelineOrchestrator {
    let toolchain = ToolchainPaths::detect().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    PipelineOrchestrator::new(config, toolchain)
}

fn collect_stems(dir: &std::path::Path) -> Vec<(String, PathBuf)> {
    collect_files_with_ext(dir, &["wav", "ogg", "mp3", "flac"])
}

fn collect_audio_files(dir: &std::path::Path) -> Vec<PathBuf> {
    collect_stems(dir)
        .into_iter()
        .map(|(_, path)| path)
        .collect()
}

fn collect_midi_files(dir: &std::path::Path) -> Vec<(String, PathBuf)> {
    collect_files_with_ext(dir, &["mid", "midi"])
}

fn collect_files_with_ext(dir: &std::path::Path, extensions: &[&str]) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    if dir.is_file() {
        let name = dir
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        files.push((name, dir.to_path_buf()));
        return files;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    files.push((name, path));
                }
            }
        }
    }
    files
}
