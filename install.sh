#!/usr/bin/env bash
# Amigo Engine CLI installer
# Usage: curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.sh | sh
set -euo pipefail

REPO="amigo-labs/amigo-engine"
INSTALL_DIR="${AMIGO_INSTALL_DIR:-$HOME/.amigo/bin}"
BINARY_NAME="amigo"

# ---------------------------------------------------------------------------
# Detect platform
# ---------------------------------------------------------------------------

detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "macos" ;;
        *)       echo "unsupported" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)  echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *)             echo "unsupported" ;;
    esac
}

OS="$(detect_os)"
ARCH="$(detect_arch)"

if [ "$OS" = "unsupported" ] || [ "$ARCH" = "unsupported" ]; then
    echo "Error: Unsupported platform: $(uname -s) $(uname -m)"
    echo "Supported: Linux (x86_64, aarch64), macOS (x86_64, aarch64)"
    exit 1
fi

# Map to Rust target triples.
case "${OS}-${ARCH}" in
    linux-x86_64)   TARGET="x86_64-unknown-linux-gnu" ;;
    linux-aarch64)  TARGET="aarch64-unknown-linux-gnu" ;;
    macos-x86_64)   TARGET="x86_64-apple-darwin" ;;
    macos-aarch64)  TARGET="aarch64-apple-darwin" ;;
    *)
        echo "Error: No pre-built binary for ${OS}-${ARCH}"
        exit 1
        ;;
esac

# ---------------------------------------------------------------------------
# Resolve version
# ---------------------------------------------------------------------------

VERSION="${AMIGO_VERSION:-latest}"

if [ "$VERSION" = "latest" ]; then
    echo "Fetching latest release..."
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

    if [ -z "$VERSION" ]; then
        echo "Error: Could not determine latest version."
        echo "Check https://github.com/${REPO}/releases"
        exit 1
    fi
fi

echo "Installing amigo ${VERSION} for ${OS}/${ARCH}..."

# ---------------------------------------------------------------------------
# Download and install
# ---------------------------------------------------------------------------

ASSET_NAME="amigo-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET_NAME}"

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading ${DOWNLOAD_URL}..."
if ! curl -fSL --progress-bar "$DOWNLOAD_URL" -o "${TMPDIR}/${ASSET_NAME}"; then
    echo ""
    echo "Error: Download failed."
    echo "  URL: ${DOWNLOAD_URL}"
    echo ""
    echo "If this is a new installation, make sure a release exists at:"
    echo "  https://github.com/${REPO}/releases"
    echo ""
    echo "Alternatively, build from source:"
    echo "  cargo install --path tools/amigo_cli"
    exit 1
fi

echo "Extracting..."
tar -xzf "${TMPDIR}/${ASSET_NAME}" -C "${TMPDIR}"

# ---------------------------------------------------------------------------
# Install binary
# ---------------------------------------------------------------------------

mkdir -p "$INSTALL_DIR"
mv "${TMPDIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

echo ""
echo "Installed amigo to ${INSTALL_DIR}/${BINARY_NAME}"

# ---------------------------------------------------------------------------
# PATH check
# ---------------------------------------------------------------------------

if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    echo ""
    echo "Add amigo to your PATH by adding this to your shell profile:"
    echo ""

    SHELL_NAME="$(basename "${SHELL:-/bin/bash}")"
    case "$SHELL_NAME" in
        zsh)
            echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc"
            echo "  source ~/.zshrc"
            ;;
        fish)
            echo "  fish_add_path ${INSTALL_DIR}"
            ;;
        *)
            echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc"
            echo "  source ~/.bashrc"
            ;;
    esac
fi

echo ""
echo "Run 'amigo --help' to get started."
echo "Run 'amigo setup' to install the Python AI toolchain."
