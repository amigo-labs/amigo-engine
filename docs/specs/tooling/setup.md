---
status: spec
crate: amigo_cli
depends_on: ["tooling/cli"]
last_updated: 2026-03-18
author: Daniel
---

# Amigo Setup (Python-Toolchain via uv)

## Purpose

`amigo setup` installiert und verwaltet die gesamte Python-Toolchain der Amigo Engine — ohne dass der User Python, pip, conda oder Docker manuell installieren muss. Ein einziger Befehl richtet alles ein: `uv` als Python-Manager, ein isoliertes venv, und alle Python-Dependencies (Demucs, Basic Pitch, ComfyUI, ACE-Step, etc.).

**Designprinzip:** Zero-Friction. Ein Befehl installiert alles. Weder Rust noch Python müssen vorher installiert sein.

## 0. Amigo CLI installieren (One-Liner)

Die CLI selbst wird als vorkompiliertes Binary von GitHub Releases heruntergeladen. Kein Rust/Cargo nötig.

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.ps1 | iex
```

### install.sh

```bash
#!/bin/sh
set -e

REPO="amigo-labs/amigo-engine"
INSTALL_DIR="$HOME/.amigo/bin"

# Plattform erkennen
OS=$(uname -s)    # Linux, Darwin
ARCH=$(uname -m)  # x86_64, aarch64, arm64

# arm64 → aarch64 normalisieren (macOS meldet arm64)
[ "$ARCH" = "arm64" ] && ARCH="aarch64"

PLATFORM="${OS}-${ARCH}"

# Neueste Version von GitHub API
VERSION=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)

if [ -z "$VERSION" ]; then
    echo "Error: Could not fetch latest version from GitHub"
    exit 1
fi

echo "Installing amigo $VERSION for $PLATFORM..."

# Binary herunterladen
URL="https://github.com/$REPO/releases/download/$VERSION/amigo-$PLATFORM"
mkdir -p "$INSTALL_DIR"
curl -fSL --progress-bar -o "$INSTALL_DIR/amigo" "$URL"
chmod +x "$INSTALL_DIR/amigo"

# PATH konfigurieren
SHELL_NAME=$(basename "$SHELL")
case "$SHELL_NAME" in
    zsh)  RC="$HOME/.zshrc" ;;
    bash) RC="$HOME/.bashrc" ;;
    fish) RC="$HOME/.config/fish/config.fish" ;;
    *)    RC="$HOME/.profile" ;;
esac

if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    if [ "$SHELL_NAME" = "fish" ]; then
        echo "set -gx PATH $INSTALL_DIR \$PATH" >> "$RC"
    else
        echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$RC"
    fi
    echo "Added $INSTALL_DIR to PATH in $RC"
fi

echo ""
echo "✓ amigo $VERSION installed to $INSTALL_DIR/amigo"
echo ""
echo "Next steps:"
echo "  source $RC        # Reload PATH (or open new terminal)"
echo "  amigo setup       # Install Python tools (Demucs, ComfyUI, etc.)"
echo "  amigo new my-game # Create your first game"
```

### install.ps1 (Windows)

```powershell
$repo = "amigo-labs/amigo-engine"
$installDir = "$env:USERPROFILE\.amigo\bin"

$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$version = $release.tag_name
$url = "https://github.com/$repo/releases/download/$version/amigo-Windows-x86_64.exe"

Write-Host "Installing amigo $version..."
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
Invoke-WebRequest -Uri $url -OutFile "$installDir\amigo.exe"

# PATH setzen (User-Level, persistent)
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($currentPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$installDir;$currentPath", "User")
    Write-Host "Added $installDir to PATH"
}

Write-Host ""
Write-Host "amigo $version installed to $installDir\amigo.exe"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  amigo setup       # Install Python tools"
Write-Host "  amigo new my-game # Create your first game"
```

### GitHub Actions: Release-Binary bauen

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: amigo-Linux-x86_64
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            artifact: amigo-Linux-aarch64
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: amigo-Darwin-x86_64
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: amigo-Darwin-aarch64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: amigo-Windows-x86_64.exe
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }} -p amigo_cli
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: target/${{ matrix.target }}/release/amigo${{ contains(matrix.os, 'windows') && '.exe' || '' }}

  release:
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v4
      - uses: softprops/action-gh-release@v2
        with:
          files: |
            amigo-Linux-x86_64/amigo
            amigo-Linux-aarch64/amigo
            amigo-Darwin-x86_64/amigo
            amigo-Darwin-aarch64/amigo
            amigo-Windows-x86_64.exe/amigo.exe
```

### Gesamter Onboarding-Flow

```bash
# 1. CLI installieren (one-liner, kein Rust nötig)
curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.sh | sh
source ~/.bashrc

# 2. Python-Tools installieren (kein Python nötig)
amigo setup

# 3. Spiel erstellen (kein Cargo nötig — Template wird heruntergeladen)
amigo new my-game
cd my-game
amigo run
```

## Überblick (amigo setup)

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
