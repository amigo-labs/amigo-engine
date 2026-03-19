# AI Setup

`amigo setup` installiert die Python-Toolchain fuer AI-Pipelines (Art Generation, Audio Analysis, Music Generation) in ein isoliertes `~/.amigo/` Verzeichnis. Kein globales Python noetig.

## Schnellstart

```sh
# Alles installieren
amigo setup

# Nur Audio-Tools
amigo setup --only audio

# Mit NVIDIA GPU
amigo setup --gpu nvidia
```

## Befehle

| Befehl | Beschreibung |
|--------|-------------|
| `amigo setup` | Volle Installation (uv + Python + alle Tools) |
| `amigo setup --only audio` | Nur Audio-Tools (Demucs, Basic Pitch) |
| `amigo setup --only artgen` | Nur Art-Tools (ComfyUI) |
| `amigo setup --only music-gen` | Nur Musik-Tools (ACE-Step) |
| `amigo setup --gpu nvidia` | Mit CUDA-Support (NVIDIA GPU) |
| `amigo setup --gpu mps` | Mit Metal-Support (macOS) |
| `amigo setup --check` | Status aller installierten Tools anzeigen |
| `amigo setup --update` | Packages auf neueste Version aktualisieren |
| `amigo setup --clean` | venv/cache/requirements loeschen |
| `amigo setup --clean --all` | Auch uv-Binary loeschen |
| `amigo setup --python 3.12` | Andere Python-Version verwenden |

## Tool-Gruppen

| Gruppe | Tools | Verwendung |
|--------|-------|-----------|
| `audio` | Demucs, Basic Pitch, midi_to_tidalcycles | [Audio Pipeline](Audio-Pipeline) |
| `artgen` | ComfyUI | Sprite/Tileset-Generierung |
| `music-gen` | ACE-Step, AudioGen | KI-Musikgenerierung |

## GPU-Backends

| Backend | Flag | Voraussetzung |
|---------|------|--------------|
| CPU | `--gpu cpu` (default) | Kein GPU noetig |
| NVIDIA CUDA | `--gpu nvidia` | NVIDIA GPU + aktuelle Treiber |
| macOS Metal | `--gpu mps` | Apple Silicon oder AMD GPU |

## Verzeichnisstruktur

```
~/.amigo/
  bin/uv              # uv Package Manager
  venv/               # Isoliertes Python-venv
  requirements/       # Generierte Requirement-Files
  config.toml         # Setup-Status und Konfiguration
```

## Status pruefen

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

## Wie es funktioniert

`amigo setup` nutzt [uv](https://docs.astral.sh/uv/) als Python-Manager:

1. **uv installieren** -- Single-Binary Download (~15 MB)
2. **Python installieren** -- `uv python install 3.11`
3. **venv erstellen** -- Isoliert in `~/.amigo/venv/`
4. **Packages installieren** -- Pro Tool-Gruppe separate Requirements
5. **Verifizieren** -- Import-Check fuer jedes Tool
