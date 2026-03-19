# Installation

## CLI Binary (empfohlen)

Das `amigo` CLI wird als vorkompiliertes Binary installiert. Kein Rust noetig.

**Linux / macOS:**
```sh
curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.ps1 | iex
```

Das Binary wird nach `~/.amigo/bin/amigo` installiert und automatisch zum PATH hinzugefuegt.

### Umgebungsvariablen

| Variable | Default | Beschreibung |
|----------|---------|-------------|
| `AMIGO_INSTALL_DIR` | `~/.amigo/bin` | Anderes Installationsverzeichnis |
| `AMIGO_VERSION` | `latest` | Bestimmte Version installieren (z.B. `v0.1.0`) |

## Aus dem Quellcode bauen

Braucht die [Rust Toolchain](https://rustup.rs/):

```sh
# Rust installieren (falls noch nicht vorhanden)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# CLI bauen und installieren
cargo install --path tools/amigo_cli
```

## Voraussetzungen fuer Game-Development

Die `amigo` CLI allein reicht fuer `amigo setup` und `amigo pipeline`. Zum **Erstellen und Bauen von Spielen** brauchst du zusaetzlich:

- **Rust Toolchain** -- [rustup.rs](https://rustup.rs/)
- **GPU-Treiber** -- Vulkan (Linux/Windows), Metal (macOS), oder DX12 (Windows)

### Systemabhaengigkeiten (Linux)

```sh
sudo apt-get install -y \
  libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev \
  libx11-dev libxi-dev libxrandr-dev libxcursor-dev libxinerama-dev pkg-config
```

## Erstes Projekt

```sh
amigo new my_game
cd my_game
cargo run
```

Verfuegbare Templates: `platformer`, `topdown-rpg`, `turn-based-rpg`, `roguelike`, `tower-defense`, `bullet-hell`, `puzzle`, `farming-sim`, `fighting`, `visual-novel`

```sh
amigo new my_platformer --template platformer
```
