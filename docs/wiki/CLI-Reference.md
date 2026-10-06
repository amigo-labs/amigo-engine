# CLI Reference

## Project Management

| Command | Description |
|---------|-------------|
| `amigo new <name> [--template T]` | Create a new game project |
| `amigo new <name> --path <ENGINE_DIR>` | ... against a local engine checkout |
| `amigo new <name> --rev <REV> \| --tag <TAG>` | ... pinned to an engine git rev or tag |
| `amigo scene <name> [--preset P]` | Add a scene to the project |
| `amigo info` | Show project information |
| `amigo list-templates` | Available project templates (the names `--template` takes) |
| `amigo list-presets` | Available scene presets |

## Build & Run

| Command | Description |
|---------|-------------|
| `amigo build` | Validate the manifest and run `cargo check` |
| `amigo run [--headless] [--api]` | Run the game |
| `amigo run --port <PORT>` | ... with the API server on a specific port |
| `amigo run --restore-snapshot <PATH>` | ... resuming a dev snapshot |
| `amigo run --record <FILE>` | ... recording the input to a replay, written on exit |
| `amigo run --replay <FILE>` | ... playing a replay instead of live input |
| `amigo run --seed <N>` | ... with a fixed seed for `ctx.rng` |
| `amigo dev [--port PORT]` | Watch mode: rebuild + restart on changes |
| `amigo editor` | Run the game with the level editor built in; F9 opens it over the game |
| `amigo pack` | Pack assets into atlas |
| `amigo release [--target T]` | Build optimized release binary |

## Publishing

| Command | Description |
|---------|-------------|
| `amigo publish steam [--target T]` | Prepare a Steam upload (via steamcmd) of `target/dist/steam` |
| `amigo publish itch [--channel C] [--target T]` | Upload `target/dist/<channel>` to itch.io (via butler) |

Both publish commands build the release, then stage what players need in `target/dist/<channel>/`: the binary, `assets/` (without loose sprites when `game.pak` holds them) and `amigo.toml`/`input.ron`.

## Setup (Python Toolchain)

See [AI Setup](AI-Setup) for details.

| Command | Description |
|---------|-------------|
| `amigo setup` | Full installation |
| `amigo setup --only <group>` | Install specific tool group only |
| `amigo setup --gpu <backend>` | Select GPU backend (cpu/nvidia/mps) |
| `amigo setup --check` | Show status |
| `amigo setup --update` | Update packages |
| `amigo setup --clean [--all]` | Clean up |

## Pipeline (Audio-to-TidalCycles)

See [Audio Pipeline](Audio-Pipeline) for details.

| Command | Description |
|---------|-------------|
| `amigo pipeline convert --input F --output F` | Full pipeline |
| `amigo pipeline separate --input F --output D` | Stem separation only |
| `amigo pipeline transcribe --input D --output D` | Audio-to-MIDI only |
| `amigo pipeline notate --input D --output F` | MIDI-to-TidalCycles only |
| `amigo pipeline batch --input D --output D` | Batch processing |
| `amigo pipeline play <file>` | Play .amigo.tidal file |

### Common Pipeline Flags

| Flag | Description |
|------|-------------|
| `--input <path>` | Input file or directory |
| `--output <path>` | Output file or directory |
| `--config <path>` | Pipeline configuration (TOML) |
| `--bpm <number>` | Override BPM |
| `--name <text>` | Composition name |
| `--license <text>` | License metadata |
| `--author <text>` | Author metadata |

## MCP / Claude Code

| Command | Description |
|---------|-------------|
| `amigo connect` | Write `.mcp.json` in current directory |
| `amigo connect --global` | Write to `~/.claude/claude_code_config.json` |
| `amigo connect --port PORT` | Use custom engine API port |
| `amigo mcp-server [--host H] [--port P]` | Run the MCP stdio bridge to the engine API |

`amigo mcp-server` is what the `.mcp.json` written by `amigo connect` launches; it
was previously undocumented here despite being referenced by the agent-api spec.

## Utilities

| Command | Description |
|---------|-------------|
| `amigo export-level <path> [--format json]` | Export level as JSON |
| `amigo version` | CLI version and the engine revision new projects pin to |
