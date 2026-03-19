use std::path::{Path, PathBuf};
use std::process;

use amigo_core::game_preset::{
    project_templates, GameProject, ProjectTemplate, SceneDef, ScenePreset,
};
use amigo_editor::{save_level, AmigoLevel, EntityPlacement, LayerData, PathData};

mod setup;
mod pipeline_cmd;

// ---------------------------------------------------------------------------
// CLI argument parsing (minimal, no external dependency)
// ---------------------------------------------------------------------------

fn print_usage() {
    eprintln!(
        r#"amigo — Amigo Engine CLI

USAGE:
    amigo <COMMAND> [OPTIONS]

COMMANDS:
    new <name> [--template <TEMPLATE>]   Create a new game project
    scene <name> [--preset <PRESET>]     Add a scene to the current project
    build                                Check that the project compiles
    run [--headless] [--api]             Run the game (cargo run)
    pack                                 Pack assets into atlas (release build)
    release [--target <TARGET>]          Build optimized release binary
    publish steam                        Prepare and upload to Steam (via steamcmd)
    publish itch [--channel CHANNEL]     Upload to itch.io (via butler)
    editor                               Launch the Amigo editor
    setup [--only GROUP] [--gpu BACKEND] Install Python toolchain (Demucs, etc.)
    pipeline <COMMAND>                   Audio-to-TidalCycles pipeline
    list-templates                       Show available project templates
    list-presets                         Show available scene presets
    export-level <path> [--format json]  Convert a .amigo level to JSON
    info                                 Show current project info

TEMPLATES:
    platformer, topdown-rpg, turn-based-rpg, roguelike, tower-defense,
    bullet-hell, puzzle, farming-sim, fighting, visual-novel

PRESETS:
    top-down, platformer, turn-based, arpg, roguelike, tower-defense,
    bullet-hell, puzzle, farming-sim, fighting, visual-novel, menu,
    world-map, custom
"#
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "new" => cmd_new(&args[2..]),
        "scene" => cmd_scene(&args[2..]),
        "build" => cmd_build(&args[2..]),
        "run" => cmd_run(&args[2..]),
        "pack" => cmd_pack(&args[2..]),
        "release" => cmd_release(&args[2..]),
        "publish" => cmd_publish(&args[2..]),
        "editor" => cmd_editor(&args[2..]),
        "setup" => setup::cmd_setup(&args[2..]),
        "pipeline" => pipeline_cmd::cmd_pipeline(&args[2..]),
        "list-templates" => cmd_list_templates(),
        "list-presets" => cmd_list_presets(),
        "export-level" => cmd_export_level(&args[2..]),
        "info" => cmd_info(),
        "help" | "--help" | "-h" => {
            print_usage();
        }
        other => {
            eprintln!("Unknown command: {other}");
            print_usage();
            process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Project manifest (amigo.toml)
// ---------------------------------------------------------------------------

/// Minimal project manifest stored as `amigo.toml`.
///
/// Schema aligns with engine spec §24:
/// - `[project]` — name, version, engine_version, start_scene, scenes
/// - `[window]` — title, width, height, fullscreen, vsync
/// - `[render]` — virtual resolution, scale_mode
/// - `[audio]` — volume channels
/// - `[dev]` — hot_reload, debug_overlay, api_server
/// - `[distribution]` — Steam / itch.io
#[derive(serde::Serialize, serde::Deserialize)]
struct ProjectManifest {
    name: String,
    version: String,
    engine_version: String,
    start_scene: String,
    scenes: Vec<SceneEntry>,
    #[serde(default)]
    window: WindowConfig,
    #[serde(default)]
    render: RenderConfig,
    #[serde(default)]
    audio: AudioConfig,
    #[serde(default)]
    dev: DevConfig,
    #[serde(default)]
    distribution: Option<DistributionConfig>,
}

/// `[window]` section of `amigo.toml`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct WindowConfig {
    #[serde(default = "default_window_title")]
    title: String,
    #[serde(default = "default_window_width")]
    width: u32,
    #[serde(default = "default_window_height")]
    height: u32,
    #[serde(default)]
    fullscreen: bool,
    #[serde(default = "default_true")]
    vsync: bool,
}

fn default_window_title() -> String {
    "Amigo Game".into()
}
fn default_window_width() -> u32 {
    1280
}
fn default_window_height() -> u32 {
    720
}
fn default_true() -> bool {
    true
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: default_window_title(),
            width: default_window_width(),
            height: default_window_height(),
            fullscreen: false,
            vsync: true,
        }
    }
}

/// `[render]` section of `amigo.toml`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RenderConfig {
    #[serde(default = "default_virtual_width")]
    virtual_width: u32,
    #[serde(default = "default_virtual_height")]
    virtual_height: u32,
    #[serde(default = "default_scale_mode")]
    scale_mode: String,
}

fn default_virtual_width() -> u32 {
    480
}
fn default_virtual_height() -> u32 {
    270
}
fn default_scale_mode() -> String {
    "pixel_perfect".into()
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            virtual_width: default_virtual_width(),
            virtual_height: default_virtual_height(),
            scale_mode: default_scale_mode(),
        }
    }
}

/// `[audio]` section of `amigo.toml`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct AudioConfig {
    #[serde(default = "default_volume")]
    master_volume: f32,
    #[serde(default = "default_volume_full")]
    sfx_volume: f32,
    #[serde(default = "default_music_volume")]
    music_volume: f32,
}

fn default_volume() -> f32 {
    0.8
}
fn default_volume_full() -> f32 {
    1.0
}
fn default_music_volume() -> f32 {
    0.6
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            master_volume: default_volume(),
            sfx_volume: default_volume_full(),
            music_volume: default_music_volume(),
        }
    }
}

/// `[dev]` section of `amigo.toml`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct DevConfig {
    #[serde(default = "default_true")]
    hot_reload: bool,
    #[serde(default = "default_true")]
    debug_overlay: bool,
    #[serde(default)]
    api_server: bool,
    #[serde(default = "default_api_port")]
    api_port: u16,
}

fn default_api_port() -> u16 {
    9999
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            hot_reload: true,
            debug_overlay: true,
            api_server: false,
            api_port: default_api_port(),
        }
    }
}

/// Distribution platform configuration stored in `amigo.toml` under `[distribution]`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct DistributionConfig {
    #[serde(default)]
    steam: Option<SteamConfig>,
    #[serde(default)]
    itch: Option<ItchConfig>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct SteamConfig {
    /// Steam application ID (e.g. 480 for Spacewar test app).
    app_id: u32,
    /// Steam depot ID for the build.
    depot_id: u32,
    /// Path to steamcmd binary (default: searches PATH).
    #[serde(default)]
    steamcmd_path: Option<String>,
    /// Steam build description template.
    #[serde(default)]
    build_description: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ItchConfig {
    /// itch.io project path (e.g. "studio-name/game-name").
    project: String,
    /// Default upload channel (e.g. "linux", "windows", "mac").
    #[serde(default)]
    channel: Option<String>,
    /// Path to butler binary (default: searches PATH).
    #[serde(default)]
    butler_path: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SceneEntry {
    id: String,
    name: String,
    preset: String,
}

fn manifest_path() -> PathBuf {
    PathBuf::from("amigo.toml")
}

fn load_manifest() -> Option<ProjectManifest> {
    let path = manifest_path();
    let contents = std::fs::read_to_string(&path).ok()?;
    toml::from_str(&contents).ok()
}

fn save_manifest(manifest: &ProjectManifest) {
    let path = manifest_path();
    let contents = toml::to_string_pretty(manifest).expect("Failed to serialize manifest");
    std::fs::write(&path, contents).expect("Failed to write amigo.toml");
}

fn project_from_manifest(manifest: &ProjectManifest) -> GameProject {
    let mut project = GameProject::new(&manifest.name);
    project.version = manifest.version.clone();
    project.virtual_width = manifest.render.virtual_width;
    project.virtual_height = manifest.render.virtual_height;
    project.start_scene = manifest.start_scene.clone();
    project
}

fn manifest_from_project(project: &GameProject) -> ProjectManifest {
    let scenes = project
        .scenes
        .iter()
        .map(|s| SceneEntry {
            id: s.id.clone(),
            name: s.name.clone(),
            preset: format!("{:?}", s.preset),
        })
        .collect();

    ProjectManifest {
        name: project.name.clone(),
        version: project.version.clone(),
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        start_scene: project.start_scene.clone(),
        scenes,
        window: WindowConfig {
            title: project.name.clone(),
            ..WindowConfig::default()
        },
        render: RenderConfig {
            virtual_width: project.virtual_width,
            virtual_height: project.virtual_height,
            ..RenderConfig::default()
        },
        audio: AudioConfig::default(),
        dev: DevConfig::default(),
        distribution: None,
    }
}

// ---------------------------------------------------------------------------
// `amigo new`
// ---------------------------------------------------------------------------

fn cmd_new(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: amigo new <name> [--template <TEMPLATE>]");
        process::exit(1);
    }

    let name = &args[0];
    let template_name = find_flag(args, "--template").unwrap_or("platformer".to_string());

    let templates = project_templates();
    let template = templates
        .iter()
        .find(|t| {
            t.name.to_lowercase().replace(' ', "-") == template_name.to_lowercase()
                || t.name.to_lowercase().replace(' ', "_") == template_name.to_lowercase()
                || t.name.to_lowercase() == template_name.to_lowercase()
        })
        .unwrap_or_else(|| {
            eprintln!("Unknown template: {template_name}");
            eprintln!("Use `amigo list-templates` to see available templates.");
            process::exit(1);
        });

    let project = template.create_project(name);

    // Create project directory structure
    let base = Path::new(name);
    create_dirs(base);

    // Write manifest
    let manifest = manifest_from_project(&project);
    let manifest_path = base.join("amigo.toml");
    let contents = toml::to_string_pretty(&manifest).expect("Failed to serialize manifest");
    std::fs::write(&manifest_path, contents).expect("Failed to write amigo.toml");

    // Write a starter level for the gameplay scene
    let level = AmigoLevel {
        name: format!("{name} - Level 1"),
        width: 40,
        height: 23,
        tile_size: project.virtual_width / 20, // reasonable default
        layers: vec![LayerData {
            name: "ground".to_string(),
            tiles: vec![0; 40 * 23],
            visible: true,
        }],
        entities: vec![EntityPlacement {
            entity_type: "player_spawn".to_string(),
            x: 160.0,
            y: 90.0,
            properties: std::collections::HashMap::new(),
        }],
        paths: Vec::new(),
        metadata: std::collections::HashMap::new(),
    };
    let level_path = base.join("assets").join("levels").join("level_01.amigo");
    save_level(&level_path, &level).expect("Failed to write starter level");

    // Write Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
amigo_engine = {{ git = "https://github.com/amigo-labs/amigo-engine", features = ["audio"] }}

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 2

[profile.release]
lto = true
strip = true
panic = "abort"
"#
    );
    let cargo_toml_path = base.join("Cargo.toml");
    std::fs::write(&cargo_toml_path, cargo_toml).expect("Failed to write Cargo.toml");

    // Write src/main.rs
    let main_rs = format!(
        r#"use amigo_engine::prelude::*;

struct MyGame;

impl Game for MyGame {{
    fn init(&mut self, ctx: &mut GameContext) {{
        // Initialize your game here
    }}

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {{
        // Update game logic here
        SceneAction::Continue
    }}

    fn draw(&self, ctx: &mut DrawContext) {{
        // Draw your game here
        ctx.draw_rect(
            Rect::new(100.0, 100.0, 32.0, 32.0),
            Color::new(0.2, 0.6, 1.0, 1.0),
        );
    }}
}}

fn main() {{
    Engine::build()
        .title("{name}")
        .virtual_resolution(480, 270)
        .window_size(1280, 720)
        .build()
        .run(MyGame);
}}
"#
    );
    let main_rs_path = base.join("src").join("main.rs");
    std::fs::write(&main_rs_path, main_rs).expect("Failed to write src/main.rs");

    println!("Created project '{name}' with template '{}'", template.name);
    println!("  Directory: {}", base.display());
    println!("  Template:  {}", template.name);
    println!(
        "  Resolution: {}x{}",
        project.virtual_width, project.virtual_height
    );
    println!("  Scenes:    {}", project.scenes.len());
    println!();
    println!("Next steps:");
    println!("  cd {name}");
    println!("  cargo run");
}

fn create_dirs(base: &Path) {
    let dirs = [
        "",
        "assets",
        "assets/sprites",
        "assets/levels",
        "assets/audio",
        "assets/tilesets",
        "assets/fonts",
        "src",
        "src/scenes",
    ];
    for dir in &dirs {
        let path = base.join(dir);
        std::fs::create_dir_all(&path).unwrap_or_else(|e| {
            eprintln!("Failed to create {}: {e}", path.display());
            process::exit(1);
        });
    }
}

// ---------------------------------------------------------------------------
// `amigo scene`
// ---------------------------------------------------------------------------

fn cmd_scene(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: amigo scene <name> [--preset <PRESET>]");
        process::exit(1);
    }

    let name = &args[0];
    let preset_name = find_flag(args, "--preset").unwrap_or("custom".to_string());
    let preset = parse_preset(&preset_name);

    let mut manifest = load_manifest().unwrap_or_else(|| {
        eprintln!("No amigo.toml found. Run `amigo new <name>` first.");
        process::exit(1);
    });

    let id = name.to_lowercase().replace(' ', "_");

    if manifest.scenes.iter().any(|s| s.id == id) {
        eprintln!("Scene '{id}' already exists.");
        process::exit(1);
    }

    manifest.scenes.push(SceneEntry {
        id: id.clone(),
        name: name.clone(),
        preset: format!("{preset:?}"),
    });

    save_manifest(&manifest);
    println!("Added scene '{name}' (id: {id}, preset: {preset:?})");
}

// ---------------------------------------------------------------------------
// `amigo build`
// ---------------------------------------------------------------------------

fn cmd_build(_args: &[String]) {
    let manifest = load_manifest().unwrap_or_else(|| {
        eprintln!("No amigo.toml found. Run `amigo new <name>` first.");
        process::exit(1);
    });

    println!("Checking project '{}'...", manifest.name);

    // Validate scenes
    let mut errors = 0u32;

    if manifest.scenes.is_empty() {
        eprintln!("  WARNING: No scenes defined");
        errors += 1;
    }

    if !manifest.scenes.iter().any(|s| s.id == manifest.start_scene) {
        eprintln!(
            "  ERROR: Start scene '{}' not found in scene list",
            manifest.start_scene
        );
        errors += 1;
    }

    // Check asset directories
    let asset_dirs = ["assets/sprites", "assets/levels", "assets/audio"];
    for dir in &asset_dirs {
        if !Path::new(dir).exists() {
            eprintln!("  WARNING: Missing directory: {dir}");
        }
    }

    // Check for level files
    let levels_dir = Path::new("assets/levels");
    if levels_dir.exists() {
        let level_count = std::fs::read_dir(levels_dir)
            .map(|rd| {
                rd.filter(|e| {
                    e.as_ref()
                        .map(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "amigo")
                                .unwrap_or(false)
                        })
                        .unwrap_or(false)
                })
                .count()
            })
            .unwrap_or(0);
        println!("  Levels: {level_count}");
    }

    println!("  Scenes: {}", manifest.scenes.len());
    println!(
        "  Resolution: {}x{}",
        manifest.virtual_width, manifest.virtual_height
    );

    if errors == 0 {
        println!("  OK — project looks good!");
    } else {
        eprintln!("  Found {errors} issue(s).");
        process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// `amigo pack`
// ---------------------------------------------------------------------------

fn cmd_pack(_args: &[String]) {
    use amigo_assets::pak::{AssetKind, PakWriter};

    let manifest = load_manifest().unwrap_or_else(|| {
        eprintln!("No amigo.toml found. Run `amigo new <name>` first.");
        process::exit(1);
    });

    println!("Packing assets for '{}'...", manifest.name);

    let mut pak = PakWriter::new();

    // ── 1. Sprites → texture atlas ──────────────────────────────────
    let sprites_dir = Path::new("assets/sprites");
    if sprites_dir.exists() {
        let mut sprite_files: Vec<(String, PathBuf)> = Vec::new();
        collect_pngs(sprites_dir, "", &mut sprite_files);

        if !sprite_files.is_empty() {
            println!("  Sprites: {} files", sprite_files.len());

            let mut atlas_builder = amigo_render::atlas::AtlasBuilder::new(4096, 2);
            let mut images: Vec<(String, image::RgbaImage)> = Vec::new();

            for (name, path) in &sprite_files {
                match image::open(path) {
                    Ok(img) => {
                        let rgba = img.to_rgba8();
                        atlas_builder.add(name.clone(), rgba.width(), rgba.height());
                        images.push((name.clone(), rgba));
                    }
                    Err(e) => {
                        eprintln!("  WARNING: Failed to load {}: {e}", path.display());
                    }
                }
            }

            match atlas_builder.pack() {
                Ok(pack) => {
                    // Blit sprites into atlas image
                    let mut atlas_image = image::RgbaImage::new(pack.width, pack.height);
                    for (name, img) in &images {
                        if let Some(entry) = pack.get(name) {
                            for y in 0..img.height() {
                                for x in 0..img.width() {
                                    let pixel = img.get_pixel(x, y);
                                    atlas_image.put_pixel(
                                        entry.rect.x + x,
                                        entry.rect.y + y,
                                        *pixel,
                                    );
                                }
                            }
                        }
                    }

                    // Encode atlas PNG to memory and add to pak
                    let mut atlas_png = Vec::new();
                    atlas_image
                        .write_to(
                            &mut std::io::Cursor::new(&mut atlas_png),
                            image::ImageFormat::Png,
                        )
                        .expect("Failed to encode atlas PNG");
                    pak.add("atlas.png", AssetKind::AtlasImage, atlas_png);

                    // Atlas manifest (RON with UV coords)
                    let entries: Vec<(String, [f32; 4])> = pack
                        .entries
                        .iter()
                        .map(|(name, entry)| {
                            (
                                name.clone(),
                                [entry.uv.x, entry.uv.y, entry.uv.w, entry.uv.h],
                            )
                        })
                        .collect();
                    let manifest_ron =
                        ron::ser::to_string_pretty(&entries, ron::ser::PrettyConfig::default())
                            .expect("Failed to serialize atlas manifest");
                    pak.add(
                        "atlas.ron",
                        AssetKind::AtlasManifest,
                        manifest_ron.into_bytes(),
                    );

                    println!(
                        "  Atlas: {}x{} ({} sprites)",
                        pack.width,
                        pack.height,
                        images.len()
                    );
                }
                Err(e) => {
                    eprintln!("  ERROR: Atlas packing failed: {e}");
                    process::exit(1);
                }
            }
        }
    }

    // ── 2. Audio files ──────────────────────────────────────────────
    let audio_dir = Path::new("assets/audio");
    if audio_dir.exists() {
        let mut count = 0u32;
        collect_files_recursive(audio_dir, "", &["wav", "ogg", "mp3"], &mut |name, path| {
            if let Err(e) = pak.add_file(&name, AssetKind::Audio, path) {
                eprintln!("  WARNING: Failed to read {}: {e}", path.display());
            } else {
                count += 1;
            }
        });
        if count > 0 {
            println!("  Audio: {} files", count);
        }
    }

    // ── 3. Data files (RON, TOML, JSON) ─────────────────────────────
    let data_dir = Path::new("assets/data");
    if data_dir.exists() {
        let mut count = 0u32;
        collect_files_recursive(data_dir, "", &["ron", "toml", "json"], &mut |name, path| {
            if let Err(e) = pak.add_file(&name, AssetKind::Data, path) {
                eprintln!("  WARNING: Failed to read {}: {e}", path.display());
            } else {
                count += 1;
            }
        });
        if count > 0 {
            println!("  Data: {} files", count);
        }
    }

    // ── 4. Level files (.amigo) ─────────────────────────────────────
    let levels_dir = Path::new("assets/levels");
    if levels_dir.exists() {
        let mut count = 0u32;
        collect_files_recursive(levels_dir, "", &["amigo"], &mut |name, path| {
            if let Err(e) = pak.add_file(&name, AssetKind::Level, path) {
                eprintln!("  WARNING: Failed to read {}: {e}", path.display());
            } else {
                count += 1;
            }
        });
        if count > 0 {
            println!("  Levels: {} files", count);
        }
    }

    // ── 5. Font files (.ttf, .otf) ─────────────────────────────────
    let fonts_dir = Path::new("assets/fonts");
    if fonts_dir.exists() {
        let mut count = 0u32;
        collect_files_recursive(fonts_dir, "", &["ttf", "otf"], &mut |name, path| {
            if let Err(e) = pak.add_file(&name, AssetKind::Font, path) {
                eprintln!("  WARNING: Failed to read {}: {e}", path.display());
            } else {
                count += 1;
            }
        });
        if count > 0 {
            println!("  Fonts: {} files", count);
        }
    }

    // ── Write game.pak ──────────────────────────────────────────────
    if pak.len() == 0 {
        println!("  No assets found. Nothing to pack.");
        return;
    }

    let out_dir = Path::new("assets/packed");
    std::fs::create_dir_all(out_dir).unwrap();
    let pak_path = out_dir.join("game.pak");

    match pak.write_to(&pak_path) {
        Ok(size) => {
            let size_kb = size as f64 / 1024.0;
            let size_display = if size_kb > 1024.0 {
                format!("{:.1} MB", size_kb / 1024.0)
            } else {
                format!("{:.0} KB", size_kb)
            };
            println!(
                "  Packed {} assets → {} ({})",
                pak.len(),
                pak_path.display(),
                size_display,
            );
        }
        Err(e) => {
            eprintln!("  ERROR: Failed to write game.pak: {e}");
            process::exit(1);
        }
    }
}

fn collect_pngs(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path.file_name().unwrap().to_string_lossy();
            let new_prefix = if prefix.is_empty() {
                dir_name.to_string()
            } else {
                format!("{prefix}/{dir_name}")
            };
            collect_pngs(&path, &new_prefix, out);
        } else if path.extension().is_some_and(|ext| ext == "png") {
            let stem = path.file_stem().unwrap().to_string_lossy();
            let name = if prefix.is_empty() {
                stem.to_string()
            } else {
                format!("{prefix}/{stem}")
            };
            out.push((name, path));
        }
    }
}

/// Recursively collect files with specific extensions and invoke a callback.
fn collect_files_recursive(
    dir: &Path,
    prefix: &str,
    extensions: &[&str],
    callback: &mut dyn FnMut(String, &Path),
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path.file_name().unwrap().to_string_lossy();
            let new_prefix = if prefix.is_empty() {
                dir_name.to_string()
            } else {
                format!("{prefix}/{dir_name}")
            };
            collect_files_recursive(&path, &new_prefix, extensions, callback);
        } else if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if extensions.iter().any(|&e| e == ext_str) {
                let file_name = path.file_name().unwrap().to_string_lossy();
                let name = if prefix.is_empty() {
                    file_name.to_string()
                } else {
                    format!("{prefix}/{file_name}")
                };
                callback(name, &path);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// `amigo release`
// ---------------------------------------------------------------------------

fn cmd_release(args: &[String]) {
    let manifest = load_manifest().unwrap_or_else(|| {
        eprintln!("No amigo.toml found. Run `amigo new <name>` first.");
        process::exit(1);
    });

    let target = find_flag(args, "--target");

    println!("Building release for '{}'...", manifest.name);

    // Step 1: Pack assets first
    println!("\n[1/3] Packing assets...");
    cmd_pack(&[]);

    // Step 2: Cargo build --release
    println!("\n[2/3] Compiling release binary...");
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build").arg("--release");

    if let Some(ref target_triple) = target {
        cmd.arg("--target").arg(target_triple);
    }

    let status = cmd.status().unwrap_or_else(|e| {
        eprintln!("Failed to run `cargo build --release`: {e}");
        process::exit(1);
    });

    if !status.success() {
        eprintln!("Release build failed.");
        process::exit(status.code().unwrap_or(1));
    }

    // Step 3: Summary
    println!("\n[3/3] Release summary:");
    println!("  Project:    {}", manifest.name);
    println!("  Version:    {}", manifest.version);
    println!(
        "  Resolution: {}x{}",
        manifest.virtual_width, manifest.virtual_height
    );
    if let Some(ref t) = target {
        println!("  Target:     {t}");
    }
    println!("  Binary:     target/release/{}", manifest.name);
    println!();
    println!("Release build complete!");
}

// ---------------------------------------------------------------------------
// `amigo list-templates`
// ---------------------------------------------------------------------------

fn cmd_list_templates() {
    let templates = project_templates();
    println!("Available project templates:");
    println!();
    for t in &templates {
        let slug = t.name.to_lowercase().replace(' ', "-");
        println!(
            "  {:<20} {}x{} — {:?}",
            slug, t.resolution.0, t.resolution.1, t.primary_preset
        );
    }
}

// ---------------------------------------------------------------------------
// `amigo list-presets`
// ---------------------------------------------------------------------------

fn cmd_list_presets() {
    let presets = [
        ("top-down", ScenePreset::TopDown),
        ("platformer", ScenePreset::Platformer),
        ("turn-based", ScenePreset::TurnBased),
        ("arpg", ScenePreset::Arpg),
        ("roguelike", ScenePreset::Roguelike),
        ("tower-defense", ScenePreset::TowerDefense),
        ("bullet-hell", ScenePreset::BulletHell),
        ("puzzle", ScenePreset::Puzzle),
        ("farming-sim", ScenePreset::FarmingSim),
        ("fighting", ScenePreset::Fighting),
        ("visual-novel", ScenePreset::VisualNovel),
        ("menu", ScenePreset::Menu),
        ("world-map", ScenePreset::WorldMap),
        ("custom", ScenePreset::Custom),
    ];

    println!("Available scene presets:");
    println!();
    for (name, preset) in &presets {
        let systems = preset.default_systems();
        println!("  {name:<16} systems: {}", systems.join(", "));
    }
}

// ---------------------------------------------------------------------------
// `amigo export-level`
// ---------------------------------------------------------------------------

fn cmd_export_level(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: amigo export-level <path.amigo> [--format json]");
        process::exit(1);
    }

    let path = Path::new(&args[0]);
    let format = find_flag(args, "--format").unwrap_or("json".to_string());

    let level = amigo_editor::load_level(path).unwrap_or_else(|e| {
        eprintln!("Failed to load level: {e}");
        process::exit(1);
    });

    match format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&level).expect("Failed to serialize to JSON");
            println!("{json}");
        }
        "ron" => {
            let ron_str = ron::ser::to_string_pretty(&level, ron::ser::PrettyConfig::default())
                .expect("Failed to serialize to RON");
            println!("{ron_str}");
        }
        _ => {
            eprintln!("Unknown format: {format}. Use 'json' or 'ron'.");
            process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// `amigo info`
// ---------------------------------------------------------------------------

fn cmd_info() {
    let manifest = load_manifest().unwrap_or_else(|| {
        eprintln!("No amigo.toml found. Run `amigo new <name>` first.");
        process::exit(1);
    });

    println!("Project: {}", manifest.name);
    println!("Version: {}", manifest.version);
    println!("Engine:  {}", manifest.engine_version);
    println!(
        "Resolution: {}x{}",
        manifest.virtual_width, manifest.virtual_height
    );
    println!("Start scene: {}", manifest.start_scene);
    println!();
    println!("Scenes:");
    for s in &manifest.scenes {
        println!("  {} — {} ({})", s.id, s.name, s.preset);
    }
}

// ---------------------------------------------------------------------------
// `amigo run`
// ---------------------------------------------------------------------------

fn cmd_run(args: &[String]) {
    if !manifest_path().exists() {
        eprintln!("No amigo.toml found in the current directory.");
        eprintln!("Run `amigo new <name>` to create a project, then cd into it.");
        process::exit(1);
    }

    let headless = args.iter().any(|a| a == "--headless");
    let api = args.iter().any(|a| a == "--api") || headless;

    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("run");

    // Enable the api feature when --api or --headless is requested
    if api {
        cmd.arg("--features").arg("amigo_engine/api");
    }

    cmd.arg("--");

    // Pass flags as environment variables for the game binary to read
    if headless {
        cmd.env("AMIGO_HEADLESS", "1");
    }
    if api {
        cmd.env("AMIGO_API", "1");
    }

    let status = cmd.status().unwrap_or_else(|e| {
        eprintln!("Failed to run `cargo run`: {e}");
        process::exit(1);
    });

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}

// ---------------------------------------------------------------------------
// `amigo editor`
// ---------------------------------------------------------------------------

fn cmd_editor(_args: &[String]) {
    println!("The Amigo Editor is a visual level and scene editor for Amigo Engine projects.");
    println!();
    println!("The editor feature is coming soon. Stay tuned!");
    println!("Follow progress at: https://github.com/amigo-labs/amigo-engine");
}

// ---------------------------------------------------------------------------
// `amigo publish`
// ---------------------------------------------------------------------------

fn cmd_publish(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: amigo publish <steam|itch> [OPTIONS]");
        process::exit(1);
    }

    match args[0].as_str() {
        "steam" => cmd_publish_steam(&args[1..]),
        "itch" => cmd_publish_itch(&args[1..]),
        other => {
            eprintln!("Unknown platform: {other}. Use 'steam' or 'itch'.");
            process::exit(1);
        }
    }
}

fn cmd_publish_steam(_args: &[String]) {
    let manifest = load_manifest().unwrap_or_else(|| {
        eprintln!("No amigo.toml found. Run `amigo new <name>` first.");
        process::exit(1);
    });

    let dist = manifest
        .distribution
        .as_ref()
        .and_then(|d| d.steam.as_ref())
        .unwrap_or_else(|| {
            eprintln!("No [distribution.steam] section in amigo.toml.");
            eprintln!();
            eprintln!("Add the following to your amigo.toml:");
            eprintln!();
            eprintln!("  [distribution.steam]");
            eprintln!("  app_id = 480");
            eprintln!("  depot_id = 481");
            eprintln!();
            process::exit(1);
        });

    let steamcmd = dist.steamcmd_path.as_deref().unwrap_or("steamcmd");

    // Check steamcmd is available
    let check = std::process::Command::new(steamcmd)
        .arg("+quit")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if check.is_err() {
        eprintln!("steamcmd not found. Install it or set `steamcmd_path` in amigo.toml.");
        eprintln!("  https://developer.valvesoftware.com/wiki/SteamCMD");
        process::exit(1);
    }

    // Step 1: Build release
    println!("[1/3] Building release...");
    cmd_release(&[]);

    // Step 2: Generate VDF build script
    println!("\n[2/3] Generating Steam build script...");
    let build_dir = Path::new("target/steam_build");
    std::fs::create_dir_all(build_dir).unwrap();

    let default_desc = format!("{} v{}", manifest.name, manifest.version);
    let description = dist.build_description.as_deref().unwrap_or(&default_desc);

    let app_vdf = format!(
        r#""AppBuild"
{{
    "AppID" "{app_id}"
    "Desc" "{desc}"
    "ContentRoot" "../release/"
    "BuildOutput" "./output/"
    "Depots"
    {{
        "{depot_id}"
        {{
            "FileMapping"
            {{
                "LocalPath" "*"
                "DepotPath" "."
                "recursive" "1"
            }}
        }}
    }}
}}"#,
        app_id = dist.app_id,
        depot_id = dist.depot_id,
        desc = description,
    );

    let vdf_path = build_dir.join("app_build.vdf");
    std::fs::write(&vdf_path, &app_vdf).unwrap();
    println!("  Generated: {}", vdf_path.display());

    // Step 3: Show upload command
    println!("\n[3/3] To upload, run:");
    println!(
        "  {steamcmd} +login <username> +run_app_build {} +quit",
        vdf_path.display()
    );
    println!();
    println!(
        "Steam build prepared for app {} (depot {}).",
        dist.app_id, dist.depot_id
    );
}

fn cmd_publish_itch(args: &[String]) {
    let manifest = load_manifest().unwrap_or_else(|| {
        eprintln!("No amigo.toml found. Run `amigo new <name>` first.");
        process::exit(1);
    });

    let dist = manifest
        .distribution
        .as_ref()
        .and_then(|d| d.itch.as_ref())
        .unwrap_or_else(|| {
            eprintln!("No [distribution.itch] section in amigo.toml.");
            eprintln!();
            eprintln!("Add the following to your amigo.toml:");
            eprintln!();
            eprintln!("  [distribution.itch]");
            eprintln!("  project = \"your-studio/your-game\"");
            eprintln!();
            process::exit(1);
        });

    let butler = dist.butler_path.as_deref().unwrap_or("butler");
    let channel = find_flag(args, "--channel")
        .or_else(|| dist.channel.clone())
        .unwrap_or_else(|| detect_platform_channel());

    // Check butler is available
    let check = std::process::Command::new(butler)
        .arg("version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if check.is_err() {
        eprintln!("butler not found. Install it or set `butler_path` in amigo.toml.");
        eprintln!("  https://itch.io/docs/butler/");
        process::exit(1);
    }

    // Step 1: Build release
    println!("[1/2] Building release...");
    cmd_release(&[]);

    // Step 2: Push via butler
    println!("\n[2/2] Uploading to itch.io...");
    let target_dir = format!("target/release/");
    let push_target = format!("{}:{}", dist.project, channel);

    println!("  {} push {} {}", butler, target_dir, push_target);
    let status = std::process::Command::new(butler)
        .args([
            "push",
            &target_dir,
            &push_target,
            "--userversion",
            &manifest.version,
        ])
        .status()
        .unwrap_or_else(|e| {
            eprintln!("Failed to run butler: {e}");
            process::exit(1);
        });

    if !status.success() {
        eprintln!("butler push failed.");
        process::exit(status.code().unwrap_or(1));
    }

    println!();
    println!(
        "Published {} v{} to itch.io ({})!",
        manifest.name, manifest.version, push_target
    );
}

fn detect_platform_channel() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "mac".to_string()
    } else {
        "linux".to_string()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn parse_preset(name: &str) -> ScenePreset {
    match name.to_lowercase().replace('-', "_").as_str() {
        "top_down" | "topdown" => ScenePreset::TopDown,
        "platformer" => ScenePreset::Platformer,
        "turn_based" | "turnbased" => ScenePreset::TurnBased,
        "arpg" => ScenePreset::Arpg,
        "roguelike" => ScenePreset::Roguelike,
        "tower_defense" | "towerdefense" | "td" => ScenePreset::TowerDefense,
        "bullet_hell" | "bullethell" => ScenePreset::BulletHell,
        "puzzle" => ScenePreset::Puzzle,
        "farming_sim" | "farmingsim" | "farming" => ScenePreset::FarmingSim,
        "fighting" => ScenePreset::Fighting,
        "visual_novel" | "visualnovel" | "vn" => ScenePreset::VisualNovel,
        "menu" => ScenePreset::Menu,
        "world_map" | "worldmap" => ScenePreset::WorldMap,
        "custom" | _ => ScenePreset::Custom,
    }
}
