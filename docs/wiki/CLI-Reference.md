# CLI Reference

## Project Management

| Command | Description |
|---------|-------------|
| `amigo new <name> [--template T]` | Create a new game project |
| `amigo scene <name> [--preset P]` | Add a scene to the project |
| `amigo info` | Show project information |
| `amigo list-templates` | Available project templates |
| `amigo list-presets` | Available scene presets |

## Build & Run

| Command | Description |
|---------|-------------|
| `amigo build` | Check compilation |
| `amigo run [--headless] [--api]` | Run the game |
| `amigo dev [--port PORT]` | Watch mode: rebuild + restart on changes |
| `amigo editor` | Open the level editor |
| `amigo pack` | Pack assets into atlas |
| `amigo release [--target T]` | Build optimized release binary |

## Publishing

| Command | Description |
|---------|-------------|
| `amigo publish steam` | Upload to Steam (via steamcmd) |
| `amigo publish itch [--channel C]` | Upload to itch.io (via butler) |

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

## Utilities

| Command | Description |
|---------|-------------|
| `amigo export-level <path> [--format json]` | Export level as JSON |
