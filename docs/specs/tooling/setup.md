---
status: draft
crate: amigo_cli
depends_on: ["tooling/cli"]
last_updated: 2026-03-18
author: Daniel
---

# Amigo Setup (Python-Toolchain via uv)

## Purpose

`amigo setup` installiert und verwaltet die gesamte Python-Toolchain der Amigo Engine — ohne dass der User Python, pip, conda oder Docker manuell installieren muss. Ein einziger Befehl richtet alles ein: `uv` als Python-Manager, ein isoliertes venv, und alle Python-Dependencies (Demucs, Basic Pitch, ComfyUI, ACE-Step, etc.).

**Designprinzip:** Zero-Friction. Wer `cargo install amigo-cli` ausführen kann, kann auch `amigo setup` ausführen. Keine Vorkenntnisse in Python nötig.

## Überblick

```
amigo setup
    │
    ├── 1. uv installieren (falls nicht vorhanden)
    │      └── curl/wget → ~/.amigo/bin/uv (single binary, ~15MB)
    │
    ├── 2. Python installieren (via uv)
    │      └── uv python install 3.11 → ~/.amigo/python/
    │
    ├── 3. venv erstellen
    │      └── uv venv ~/.amigo/venv --python 3.11
    │
    ├── 4. Core-Dependencies installieren
    │      └── uv pip install -r ~/.amigo/requirements/core.txt
    │
    └── 5. Verifikation
           └── Jedes Tool testen (import check)
```

## CLI-Interface

```bash
# Alles installieren (empfohlen für Ersteinrichtung)
amigo setup

# Nur bestimmte Tool-Gruppen installieren
amigo setup --only audio       # Demucs, Basic Pitch, midi_to_tidalcycles
amigo setup --only artgen      # ComfyUI
amigo setup --only music-gen   # ACE-Step, AudioGen

# GPU-Support (Standard: CPU-only)
amigo setup --gpu nvidia       # PyTorch mit CUDA 12.4
amigo setup --gpu mps          # macOS Metal Performance Shaders

# Status prüfen
amigo setup --check            # Welche Tools sind installiert?

# Update: alle Python-Tools auf neueste Versionen
amigo setup --update

# Aufräumen: venv und Tools komplett entfernen
amigo setup --clean

# Bestimmte Python-Version erzwingen
amigo setup --python 3.12
```

## Public API

### SetupConfig

```rust
/// Konfiguration für den Setup-Prozess.
#[derive(Debug, Clone)]
pub struct SetupConfig {
    /// Basis-Verzeichnis für alle Amigo-Tools.
    /// Default: ~/.amigo/
    pub amigo_home: PathBuf,
    /// Welche Tool-Gruppen installiert werden sollen.
    pub groups: Vec<ToolGroup>,
    /// GPU-Backend.
    pub gpu: GpuBackend,
    /// Python-Version.
    pub python_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGroup {
    /// Demucs, Basic Pitch, midi_to_tidalcycles
    Audio,
    /// ComfyUI + Custom Nodes
    ArtGen,
    /// ACE-Step, AudioGen
    MusicGen,
    /// Alles
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    /// CPU-only PyTorch (Standard, funktioniert überall)
    Cpu,
    /// NVIDIA CUDA 12.4
    Nvidia,
    /// macOS Metal Performance Shaders
    Mps,
}
```

### SetupManager

```rust
pub struct SetupManager {
    config: SetupConfig,
}

impl SetupManager {
    pub fn new(config: SetupConfig) -> Self;

    /// Prüft ob uv installiert ist.
    pub fn has_uv(&self) -> bool;

    /// Installiert uv (single binary download).
    pub fn install_uv(&self) -> Result<(), SetupError>;

    /// Installiert Python via uv.
    pub fn install_python(&self) -> Result<(), SetupError>;

    /// Erstellt isoliertes venv.
    pub fn create_venv(&self) -> Result<(), SetupError>;

    /// Installiert Python-Packages in das venv.
    pub fn install_packages(&self, group: ToolGroup) -> Result<(), SetupError>;

    /// Prüft ob alle Tools funktionieren.
    pub fn verify(&self) -> Vec<ToolStatus>;

    /// Führt den gesamten Setup-Prozess aus.
    pub fn run_full_setup(&self) -> Result<SetupResult, SetupError>;

    /// Führt einen Befehl im venv aus (uv run).
    pub fn run_in_venv(&self, cmd: &str, args: &[&str]) -> Result<Output, SetupError>;
}
```

### ToolStatus

```rust
#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub name: String,
    pub group: ToolGroup,
    pub installed: bool,
    pub version: Option<String>,
    pub gpu_available: bool,
}

#[derive(Debug, Clone)]
pub struct SetupResult {
    pub tools: Vec<ToolStatus>,
    pub venv_path: PathBuf,
    pub python_version: String,
    pub disk_usage_mb: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("uv installation failed: {0}")]
    UvInstallFailed(String),
    #[error("Python installation failed: {0}")]
    PythonInstallFailed(String),
    #[error("Package installation failed: {0}")]
    PackageInstallFailed(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Disk space insufficient: need {need_mb}MB, have {have_mb}MB")]
    DiskSpace { need_mb: u64, have_mb: u64 },
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

## Verzeichnisstruktur

```
~/.amigo/
├── bin/
│   └── uv                         # uv binary (~15MB)
├── python/
│   └── cpython-3.11.x-linux-x86_64/  # Python (via uv, ~30MB)
├── venv/                           # Isoliertes venv
│   ├── bin/
│   │   ├── python -> ../python/...
│   │   ├── demucs
│   │   ├── basic-pitch
│   │   └── ...
│   └── lib/
│       └── python3.11/site-packages/
├── requirements/                   # Requirement-Files (vom CLI mitgeliefert)
│   ├── core.txt                   # torch, numpy, etc.
│   ├── audio.txt                  # demucs, basic-pitch, midi_to_tidalcycles
│   ├── artgen.txt                 # comfyui + custom nodes
│   └── music-gen.txt              # ace-step, audiocraft
├── cache/                          # uv cache für schnelle Re-Installs
└── config.toml                    # Setup-Status und Konfiguration
```

### config.toml (Setup-Status)

```toml
[setup]
version = "0.1.0"
installed_at = "2026-03-18T14:30:00Z"
python_version = "3.11.9"
gpu_backend = "cpu"

[groups]
audio = true
artgen = false
music_gen = false

[tools]
demucs = { installed = true, version = "4.0.1" }
basic_pitch = { installed = true, version = "0.3.0" }
midi_to_tidalcycles = { installed = true, version = "0.2.0" }
comfyui = { installed = false }
ace_step = { installed = false }
```

## Requirement-Files

### core.txt (Basis für alle Gruppen)

```
# GPU-spezifisch: wird von amigo setup dynamisch ersetzt
--index-url https://download.pytorch.org/whl/cpu
torch>=2.2.0
torchaudio>=2.2.0
numpy>=1.24.0
```

### audio.txt

```
-r core.txt
demucs>=4.0.0
basic-pitch>=0.3.0
midi_to_tidalcycles>=0.2.0
pretty-midi>=0.2.10
librosa>=0.10.0
soundfile>=0.12.0
```

### artgen.txt

```
-r core.txt
comfyui>=0.2.0
```

### music-gen.txt

```
-r core.txt
# ACE-Step: Git-Install da kein PyPI-Paket
# ace-step @ git+https://github.com/AceStepper/ACE-Step.git
audiocraft>=1.3.0
```

## Behavior

### Erstinstallation (`amigo setup`)

1. **Disk-Space prüfen**: CPU-only ~2GB, mit CUDA ~5GB. Warnung wenn nicht genug Platz.
2. **uv installieren**: Download von `https://astral.sh/uv/install.sh` → `~/.amigo/bin/uv`. Kein root/sudo nötig.
3. **Python installieren**: `uv python install 3.11` → `~/.amigo/python/`. uv bringt eigene Python-Builds mit, kein System-Python nötig.
4. **venv erstellen**: `uv venv ~/.amigo/venv --python 3.11`. Komplett isoliert vom System.
5. **PyTorch installieren**: Anhand von `--gpu` Flag die richtige `--index-url` setzen:
   - `cpu`: `https://download.pytorch.org/whl/cpu` (~800MB)
   - `nvidia`: `https://download.pytorch.org/whl/cu124` (~2.5GB)
   - `mps`: Standard PyPI (Metal-Support automatisch)
6. **Tool-Packages installieren**: `uv pip install -r <group>.txt` für jede gewählte Gruppe.
7. **Verifizierung**: Für jedes Tool einen Import-Check ausführen (`uv run python -c "import demucs; print(demucs.__version__)"`).
8. **config.toml schreiben**: Setup-Status für spätere Checks.

### Inkrementelles Setup (`amigo setup --only artgen`)

- Prüft ob uv/Python/venv bereits existieren → überspringt wenn ja
- Installiert nur die fehlende Gruppe
- Aktualisiert config.toml

### Tool-Aufruf aus der Engine

Alle Python-Aufrufe laufen über `SetupManager::run_in_venv()`:

```rust
// Demucs aufrufen
let manager = SetupManager::from_config_toml()?;
let output = manager.run_in_venv(
    "demucs",
    &["--two-stems", "vocals", "-o", "./stems/", "track.wav"],
)?;

// Basic Pitch aufrufen
let output = manager.run_in_venv(
    "basic-pitch",
    &["./midi/", "./stems/melody.wav"],
)?;

// ComfyUI starten
let output = manager.run_in_venv(
    "comfyui",
    &["--listen", "127.0.0.1", "--port", "8188"],
)?;
```

Intern ruft `run_in_venv` auf:
```bash
~/.amigo/bin/uv run --python ~/.amigo/venv/bin/python <cmd> <args...>
```

### Status-Check (`amigo setup --check`)

```
$ amigo setup --check

Amigo Python Toolchain Status
──────────────────────────────────────────────
  uv:                 ✓ 0.6.x (~/.amigo/bin/uv)
  Python:             ✓ 3.11.9 (~/.amigo/python/)
  venv:               ✓ ~/.amigo/venv/
  GPU:                CPU-only (use --gpu nvidia to enable CUDA)
  Disk usage:         1.8 GB

  Audio Tools:
    demucs            ✓ 4.0.1
    basic-pitch       ✓ 0.3.0
    midi_to_tidal     ✓ 0.2.0

  Art Generation:
    comfyui           ✗ not installed (amigo setup --only artgen)

  Music Generation:
    ace-step          ✗ not installed (amigo setup --only music-gen)
    audiocraft        ✗ not installed
──────────────────────────────────────────────
```

### Cleanup (`amigo setup --clean`)

- Entfernt `~/.amigo/venv/`, `~/.amigo/python/`, `~/.amigo/cache/`
- Behält `~/.amigo/bin/uv` und `~/.amigo/config.toml`
- Fragt vorher nach Bestätigung
- `amigo setup --clean --all` entfernt auch uv selbst

## Internal Design

- **uv als einzige Dependency**: uv ist ein single binary (~15MB), braucht kein Python zum Installieren, und kann Python selbst installieren. Kein Bootstrapping-Problem.
- **Requirement-Files eingebettet**: Die `.txt`-Dateien werden beim `amigo setup` aus dem CLI-Binary nach `~/.amigo/requirements/` geschrieben (embedded via `include_str!` oder `include_bytes!`).
- **GPU-Detection**: `amigo setup` kann optional `nvidia-smi` aufrufen um CUDA-Verfügbarkeit zu prüfen. Wenn vorhanden, schlägt `--gpu nvidia` vor. Ansonsten CPU-Default.
- **Offline-Resilienz**: `uv` cached alle Downloads in `~/.amigo/cache/`. Nach einmaligem Setup funktioniert `amigo setup` auch offline (aus Cache).
- **Keine root-Rechte**: Alles in `~/.amigo/`, kein `/usr/local/` oder System-Python betroffen.
- **Cross-Platform**: uv unterstützt Linux, macOS, Windows. Python-Builds von uv sind plattformspezifisch.

## Non-Goals

- **Docker.** Die gesamte Toolchain läuft nativ via uv. Kein Docker, kein Container, keine Images.
- **Conda/Mamba.** uv ersetzt conda komplett für diesen Anwendungsfall.
- **System-Python.** Wir nutzen nie das System-Python. uv installiert sein eigenes.
- **Virtuelle Maschinen.** Kein Vagrant, kein Nix, kein Devcontainer.
- **GPU-Treiber-Installation.** CUDA-Treiber müssen vom User installiert sein. `amigo setup` installiert nur PyTorch mit CUDA-Support.
- **Python-Scripting in der Engine.** Python ist ein Build-/Pipeline-Tool, keine Runtime-Dependency. Die Engine selbst ist pure Rust.

## Open Questions

- Soll `amigo setup` automatisch GPU erkennen und vorschlagen, oder immer explizit `--gpu` verlangen?
- Soll ComfyUI als managed Service laufen (`amigo comfyui start/stop`) oder manuell gestartet werden?
- Braucht es ein `amigo doctor` Command für Troubleshooting (ähnlich `flutter doctor`)?
- Soll `amigo setup --update` auch uv selbst updaten?
- Wie umgehen mit ACE-Step, das kein PyPI-Paket hat? Git-Clone in venv, oder eigenes Wheel bauen?

## Referenzen

- [tooling/cli](cli.md) → Bestehende CLI-Commands
- [ai-pipelines/artgen](../ai-pipelines/artgen.md) → ComfyUI-Integration
- [ai-pipelines/audiogen](../ai-pipelines/audiogen.md) → ACE-Step/AudioGen-Integration
- [ai-pipelines/tidal-pipeline](../ai-pipelines/tidal-pipeline.md) → Demucs/Basic Pitch Pipeline
- [uv Documentation](https://docs.astral.sh/uv/) → Python-Paketmanager
