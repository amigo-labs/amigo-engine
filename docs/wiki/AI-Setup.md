# AI Setup

`amigo setup` installs the Python toolchain for AI pipelines (art generation, audio analysis, music generation) into an isolated `~/.amigo/` directory. No global Python installation required.

## Quick Start

```sh
# Install everything
amigo setup

# Only audio tools
amigo setup --only audio

# With NVIDIA GPU
amigo setup --gpu nvidia
```

## Commands

| Command                        | Description                                 |
| ------------------------------ | ------------------------------------------- |
| `amigo setup`                  | Full installation (uv + Python + all tools) |
| `amigo setup --only audio`     | Audio tools only (Demucs, Basic Pitch)      |
| `amigo setup --only artgen`    | Art tools only (ComfyUI)                    |
| `amigo setup --only music-gen` | Music tools only (ACE-Step)                 |
| `amigo setup --gpu nvidia`     | With CUDA support (NVIDIA GPU)              |
| `amigo setup --gpu mps`        | With Metal support (macOS)                  |
| `amigo setup --check`          | Show status of all installed tools          |
| `amigo setup --update`         | Update packages to latest versions          |
| `amigo setup --clean`          | Remove venv/cache/requirements              |
| `amigo setup --clean --all`    | Also remove uv binary                       |
| `amigo setup --python 3.12`    | Use a different Python version              |

## Tool Groups

| Group       | Tools                                    | Used for                         |
| ----------- | ---------------------------------------- | -------------------------------- |
| `audio`     | Demucs, Basic Pitch, midi_to_tidalcycles | [Audio Pipeline](Audio-Pipeline) |
| `artgen`    | ComfyUI                                  | Sprite/tileset generation        |
| `music-gen` | ACE-Step, AudioGen                       | AI music generation              |

## GPU Backends

| Backend     | Flag                  | Requirement                     |
| ----------- | --------------------- | ------------------------------- |
| CPU         | `--gpu cpu` (default) | No GPU needed                   |
| NVIDIA CUDA | `--gpu nvidia`        | NVIDIA GPU with current drivers |
| macOS Metal | `--gpu mps`           | Apple Silicon or AMD GPU        |

## Directory Structure

```
~/.amigo/
  bin/uv              # uv package manager
  venv/               # Isolated Python venv
  requirements/       # Generated requirement files
  config.toml         # Setup status and configuration
```

## Check Status

```sh
$ amigo setup --check

Amigo Python Toolchain Status
--------------------------------------------------
  uv:     installed (~/.amigo/bin/uv)
  venv:   created (~/.amigo/venv/)
  GPU:    NVIDIA CUDA

  Audio (Demucs, Basic Pitch):
    [+] demucs              4.0.1
    [+] basic-pitch         0.3.2
    [+] midi_to_tidalcycles ok
  ArtGen (ComfyUI):
    [-] comfyui             not installed
  MusicGen (ACE-Step):
    [-] audiocraft          not installed
--------------------------------------------------
```

## How It Works

`amigo setup` uses [uv](https://docs.astral.sh/uv/) as Python manager:

1. **Install uv** -- single-binary download (~15 MB)
2. **Install Python** -- `uv python install 3.11`
3. **Create venv** -- isolated in `~/.amigo/venv/`
4. **Install packages** -- separate requirements per tool group
5. **Verify** -- import check for each tool
