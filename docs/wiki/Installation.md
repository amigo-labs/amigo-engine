# Installation

## CLI Binary (recommended)

The `amigo` CLI is distributed as a pre-built binary. No Rust required.

**Linux / macOS:**

```sh
curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.ps1 | iex
```

The binary is installed to `~/.amigo/bin/amigo` and automatically added to your PATH.

### Environment Variables

| Variable            | Default        | Description                                |
| ------------------- | -------------- | ------------------------------------------ |
| `AMIGO_INSTALL_DIR` | `~/.amigo/bin` | Custom install directory                   |
| `AMIGO_VERSION`     | `latest`       | Install a specific version (e.g. `v0.1.0`) |

## Build from Source

Requires the [Rust toolchain](https://rustup.rs/):

```sh
# Install Rust (if not already present)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build and install the CLI
cargo install --path tools/amigo_cli
```

## Requirements for Game Development

The `amigo` CLI alone is sufficient for `amigo setup` and `amigo pipeline`. To **create and build games** you also need:

- **Rust toolchain** -- [rustup.rs](https://rustup.rs/)
- **GPU drivers** -- Vulkan (Linux/Windows), Metal (macOS), or DX12 (Windows)

### System Dependencies (Linux)

```sh
sudo apt-get install -y \
  libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev \
  libx11-dev libxi-dev libxrandr-dev libxcursor-dev libxinerama-dev pkg-config
```

## First Project

```sh
amigo new my_game
cd my_game
cargo run
```

Available templates: `platformer`, `topdown-rpg`, `turn-based-rpg`, `roguelike`, `tower-defense`, `bullet-hell`, `puzzle`, `farming-sim`, `fighting`, `visual-novel`

```sh
amigo new my_platformer --template platformer
```
