---
status: done
crate: amigo_cli
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# CLI Tool (amigo)

## Purpose

Provides a command-line interface for creating, building, running, packing, releasing, and publishing Amigo Engine projects. Handles project scaffolding from templates, scene management with presets, asset packing into `.pak` files, release builds, and platform-specific publishing to Steam and itch.io.

Existing implementation in `tools/amigo_cli/src/main.rs` (1305 lines).

## Public API

### Commands

```
amigo <COMMAND> [OPTIONS]

COMMANDS:
    new <name> [--template <TEMPLATE>]   Create a new game project
    scene <name> [--preset <PRESET>]     Add a scene to the current project
    build                                Check that the project compiles
    run [--headless] [--api]             Run the game (cargo run)
    dev [--port <PORT>]                  Run with watch mode + snapshot restore
    pack                                 Pack assets into atlas (release build)
    release [--target <TARGET>]          Build optimized release binary
    publish steam                        Prepare and upload to Steam (via steamcmd)
    publish itch [--channel CHANNEL]     Upload to itch.io (via butler)
    editor                               Launch the Amigo editor
    connect [--global] [--port PORT]     Write MCP config for Claude Code
    setup [--only G] [--gpu B] [--check] Install Python toolchain (Demucs, etc.)
    pipeline <COMMAND>                   Audio-to-TidalCycles pipeline
    list-templates                       Show available project templates
    list-presets                         Show available scene presets
    export-level <path> [--format json]  Convert a .amigo level to JSON
    info                                 Show current project info
```

### Templates

Available via `amigo new <name> --template <TEMPLATE>`:

| Template | Default |
|----------|---------|
| platformer | (default if `--template` omitted) |
| topdown-rpg | -- |
| turn-based-rpg | -- |
| roguelike | -- |
| tower-defense | -- |
| bullet-hell | -- |
| puzzle | -- |
| farming-sim | -- |
| fighting | -- |
| visual-novel | -- |

Each template sets a resolution, primary scene preset, and scene list appropriate for that game type. Templates are defined in `amigo_core::game_preset::project_templates()`.

### Scene Presets

Available via `amigo scene <name> --preset <PRESET>`:

| Preset | Aliases | Default Systems |
|--------|---------|-----------------|
| top-down | topdown | Per `ScenePreset::default_systems()` |
| platformer | -- | -- |
| turn-based | turnbased | -- |
| arpg | -- | -- |
| roguelike | -- | -- |
| tower-defense | towerdefense, td | -- |
| bullet-hell | bullethell | -- |
| puzzle | -- | -- |
| farming-sim | farmingsim, farming | -- |
| fighting | -- | -- |
| visual-novel | visualnovel, vn | -- |
| menu | -- | -- |
| world-map | worldmap | -- |
| custom | (default) | -- |

### Project Manifest (amigo.toml)

```rust
struct ProjectManifest {
    name: String,
    version: String,
    engine_version: String,
    start_scene: String,
    scenes: Vec<SceneEntry>,
    window: WindowConfig,
    render: RenderConfig,
    audio: AudioConfig,
    dev: DevConfig,
    distribution: Option<DistributionConfig>,
}

struct SceneEntry {
    id: String,
    name: String,
    preset: String,
}
```

#### WindowConfig

```rust
struct WindowConfig {
    title: String,         // default: "Amigo Game"
    width: u32,            // default: 1280
    height: u32,           // default: 720
    fullscreen: bool,      // default: false
    vsync: bool,           // default: true
}
```

#### RenderConfig

```rust
struct RenderConfig {
    virtual_width: u32,    // default: 480
    virtual_height: u32,   // default: 270
    scale_mode: String,    // default: "pixel_perfect"
}
```

#### AudioConfig

```rust
struct AudioConfig {
    master_volume: f32,    // default: 0.8
    sfx_volume: f32,       // default: 1.0
    music_volume: f32,     // default: 0.6
}
```

#### DevConfig

```rust
struct DevConfig {
    hot_reload: bool,      // default: true
    debug_overlay: bool,   // default: true
    api_server: bool,      // default: false
    api_port: u16,         // default: 9999
}
```

#### DistributionConfig

```rust
struct DistributionConfig {
    steam: Option<SteamConfig>,
    itch: Option<ItchConfig>,
}

struct SteamConfig {
    app_id: u32,
    depot_id: u32,
    steamcmd_path: Option<String>,
    build_description: Option<String>,
}

struct ItchConfig {
    project: String,           // "studio-name/game-name"
    channel: Option<String>,   // "linux", "windows", "mac"
    butler_path: Option<String>,
}
```

## Behavior

### `amigo new`

1. Looks up the template by name (case-insensitive, supports `-` and `_` separators).
2. Calls `template.create_project(name)` to produce a `GameProject` with scenes and resolution.
3. Creates the project directory structure:
   - `<name>/` root
   - `assets/sprites/`, `assets/levels/`, `assets/audio/`, `assets/tilesets/`, `assets/fonts/`
   - `src/`, `src/scenes/`
4. Writes `amigo.toml` from the project manifest.
5. Writes a starter level file at `assets/levels/level_01.amigo` with a 40x23 tile grid and a `player_spawn` entity at (160, 90).
6. Writes `Cargo.toml` with:
   - `amigo_engine` git dependency with `audio` feature
   - Dev profile: `opt-level = 1`, dependencies at `opt-level = 2`
   - Release profile: `lto = true`, `strip = true`, `panic = "abort"`
7. Writes `src/main.rs` with a minimal `Game` trait implementation using `Engine::build()`.

### `amigo scene`

1. Loads `amigo.toml` (fails if not found).
2. Parses the preset name into a `ScenePreset` enum via `parse_preset()`, supporting aliases (e.g., "td" maps to `TowerDefense`, "vn" maps to `VisualNovel`).
3. Generates a scene ID from the name (lowercase, spaces to underscores).
4. Checks for duplicate scene IDs and rejects them.
5. Appends the new scene entry and saves the manifest.

### `amigo build`

Validates the project without compiling:

1. Checks that at least one scene is defined.
2. Verifies the `start_scene` exists in the scene list.
3. Checks for expected asset directories (`assets/sprites`, `assets/levels`, `assets/audio`).
4. Counts `.amigo` level files in `assets/levels/`.
5. Reports scene count and virtual resolution.

### `amigo run`

1. Verifies `amigo.toml` exists in the current directory.
2. `--headless` sets `AMIGO_HEADLESS=1` environment variable.
3. `--api` (or implied by `--headless`) enables the `amigo_engine/api` Cargo feature and sets `AMIGO_API=1`.
4. Runs `cargo run` with the appropriate features and environment variables, passing `--` to separate cargo args from game args.

### `amigo pack`

Packs all project assets into `assets/packed/game.pak`:

1. **Sprites**: Collects all `.png` files from `assets/sprites/` recursively, builds a texture atlas using `amigo_render::atlas::AtlasBuilder` (max 4096px, 2px padding), blits source images into the atlas, encodes as PNG, and writes `atlas.png` + `atlas.ron` (UV coordinates as `(name, [u, v, w, h])` tuples) into the pak.
2. **Audio**: Collects `.wav`, `.ogg`, `.mp3` from `assets/audio/` recursively.
3. **Data**: Collects `.ron`, `.toml`, `.json` from `assets/data/` recursively.
4. **Levels**: Collects `.amigo` from `assets/levels/` recursively.
5. **Fonts**: Collects `.ttf`, `.otf` from `assets/fonts/` recursively.
6. Writes the final `game.pak` using `amigo_assets::pak::PakWriter` and reports the asset count and file size.

Asset names preserve directory structure relative to the asset root (e.g., `enemies/goblin` for `assets/sprites/enemies/goblin.png`).

### `amigo release`

Three-step release pipeline:

1. **[1/3] Pack assets**: Calls `cmd_pack()` to create `game.pak`.
2. **[2/3] Compile**: Runs `cargo build --release`, optionally with `--target <triple>` for cross-compilation.
3. **[3/3] Summary**: Reports project name, version, resolution, target, and binary path.

### `amigo publish steam`

1. Loads `[distribution.steam]` from `amigo.toml` (requires `app_id` and `depot_id`).
2. Verifies `steamcmd` is available (checks PATH or `steamcmd_path` config).
3. Builds a release via `cmd_release()`.
4. Generates a Steam VDF build script at `target/steam_build/app_build.vdf` with the configured app/depot IDs and a build description.
5. Prints the `steamcmd` command the user must run to upload (authentication is manual).

### `amigo publish itch`

1. Loads `[distribution.itch]` from `amigo.toml` (requires `project` path).
2. Verifies `butler` is available.
3. Determines the upload channel from `--channel` flag, config, or auto-detected platform (`linux`, `windows`, `mac` based on `cfg!(target_os)`).
4. Builds a release.
5. Runs `butler push target/release/ <project>:<channel> --userversion <version>`.

### `amigo export-level`

Loads a `.amigo` level file via `amigo_editor::load_level()` and serializes it to stdout. Supports `--format json` (default) and `--format ron`.

### `amigo info`

Displays project name, version, engine version, virtual resolution, start scene, and a list of all scenes with their IDs, names, and presets.

### `amigo connect`

Writes an MCP server configuration file so Claude Code auto-discovers the Amigo MCP servers.

1. Without `--global`: writes `.mcp.json` in the current directory. Prompts before overwriting an existing file.
2. With `--global`: writes to `~/.claude/claude_code_config.json`. Merges with existing config (preserving other keys).
3. `--port PORT` overrides the engine API port (default: reads from `amigo.toml` `[dev].api_port`, falling back to 9999).
4. Configures three MCP servers: `amigo` (engine control), `amigo-artgen` (pixel art), `amigo-audiogen` (audio generation).

### `amigo setup`

Installs the Python toolchain required for AI asset pipelines (Demucs, Basic Pitch, ACE-Step, etc.).

1. `--only <GROUP>`: Install a specific tool group only (`audio`, `artgen`, `music-gen`, or `all`).
2. `--gpu <BACKEND>`: Select GPU backend (`cpu`, `nvidia`/`cuda`, `mps`/`metal`).
3. `--python <VERSION>`: Python version (default: 3.11).
4. `--check`: Show installation status without installing.
5. `--clean [--all]`: Remove installed environments.
6. `--update`: Update existing packages.

Creates a Python virtual environment, installs requirements from bundled `requirements_*.txt` files, and validates tool availability. See [AI Setup](../../wiki/AI-Setup.md).

### `amigo pipeline`

Audio-to-TidalCycles conversion pipeline. Subcommands:

- `convert --input F --output F`: Full pipeline (separate → transcribe → notate).
- `separate --input F --output D`: Stem separation only (Demucs).
- `transcribe --input D --output D`: Audio-to-MIDI only (Basic Pitch).
- `notate --input D --output F`: MIDI-to-TidalCycles only.
- `batch --input D --output D`: Process directory of audio files.
- `play <file>`: Preview `.amigo.tidal` file.

Common flags: `--config`, `--bpm`, `--name`, `--license`, `--author`. See [Audio Pipeline](../../wiki/Audio-Pipeline.md).

### `amigo list-templates` / `amigo list-presets`

- `list-templates`: Enumerates all templates from `project_templates()` showing slug, resolution, and primary preset.
- `list-presets`: Enumerates all 14 scene presets with their default systems (from `ScenePreset::default_systems()`).

## Internal Design

### Argument Parsing

The CLI uses manual argument parsing with no external dependency (no `clap` or `structopt`). The `find_flag(args, flag)` helper searches for a flag string and returns the following argument as its value. Commands are dispatched via a `match` on `args[1]`.

### Manifest Persistence

`amigo.toml` is read with `toml::from_str` and written with `toml::to_string_pretty`. All config sections (`WindowConfig`, `RenderConfig`, `AudioConfig`, `DevConfig`) implement `Default` with sensible values, so missing TOML sections are handled gracefully via `#[serde(default)]`.

### Asset Collection

Two recursive directory walkers:
- `collect_pngs(dir, prefix, out)`: Collects `.png` files into a `Vec<(name, path)>`.
- `collect_files_recursive(dir, prefix, extensions, callback)`: Generic recursive collector for any set of extensions, invoking a callback per matched file.

Both preserve directory hierarchy in the name prefix (e.g., `subdir/filename`).

### Atlas Packing

Uses `amigo_render::atlas::AtlasBuilder` with a maximum atlas size of 4096x4096 and 2px padding. Images are loaded via the `image` crate, packed by the atlas builder, then blitted into a combined `RgbaImage`. The atlas manifest is serialized as RON containing `Vec<(String, [f32; 4])>` with normalized UV coordinates.

### Preset Parsing

`parse_preset()` normalizes the input (lowercase, hyphens to underscores) and maps it to a `ScenePreset` enum variant. Supports multiple aliases per preset. Unknown names fall through to `ScenePreset::Custom`.

## Non-Goals

- Interactive terminal UI or TUI-based project management.
- Hot-reloading from the CLI (hot-reload is a runtime engine feature).
- Dependency management beyond the Cargo ecosystem.
- Automated Steam/itch authentication -- publishing requires manual login.
- Continuous integration configuration generation.
- macOS/iOS/Android/console builds in the initial implementation.

## Open Questions

- ~~Whether to add a `watch` command for automatic rebuild on file changes.~~ **Resolved**: `amigo dev` provides this. See [tooling/dev-workflow](dev-workflow.md).
- Whether to support WASM/web export as a publish target.
- Whether to add an `update` command for upgrading the engine version in existing projects.
- Whether `amigo pack` should support incremental packing (only changed assets).
- Whether `amigo build` should run `cargo check` in addition to manifest validation.

## Referenzen

- [engine/core](../engine/core.md) -- Game trait, builder pattern, and project templates
- [assets/format](../assets/format.md) -- `.pak` file format used by `amigo pack`
- [assets/atlas](../assets/atlas.md) -- Texture atlas packing algorithm
- [config/amigo-toml](../config/amigo-toml.md) -- Full `amigo.toml` specification
- [tooling/editor](editor.md) -- Visual editor (launched via `amigo editor`)
- [tooling/dev-workflow](dev-workflow.md) -- `amigo dev` watch mode with snapshot restore
