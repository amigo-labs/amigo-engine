#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────
# install-system-deps.sh — Install the Linux system libraries the engine
# links against: audio (alsa), input (udev), and windowing (wayland/X11).
#
# Keep the package list in sync with:
#   .github/actions/install-system-deps/action.yml
#   docs/wiki/Installation.md
#   CONTRIBUTING.md
#
# Idempotent: exits early when the libraries are already discoverable, so it
# is safe to run from a SessionStart hook on every session.
# ─────────────────────────────────────────────────────────────
set -euo pipefail

PACKAGES=(
    libasound2-dev
    libudev-dev
    libwayland-dev
    libxkbcommon-dev
    libx11-dev
    libxi-dev
    libxrandr-dev
    libxcursor-dev
    libxinerama-dev
    pkg-config
)

# The two libraries whose absence actually breaks `cargo build` (alsa-sys and
# libudev-sys both resolve through pkg-config). If they are present, assume the
# rest of the set is too.
if command -v pkg-config >/dev/null 2>&1 &&
    pkg-config --exists libudev alsa 2>/dev/null; then
    echo "system dependencies already present"
    exit 0
fi

if ! command -v apt-get >/dev/null 2>&1; then
    echo "install-system-deps: apt-get not found — install these manually:" >&2
    printf '  %s\n' "${PACKAGES[@]}" >&2
    exit 1
fi

# Only use sudo when we are not already root; container images usually are.
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    if command -v sudo >/dev/null 2>&1; then
        SUDO="sudo"
    else
        echo "install-system-deps: need root or sudo to install packages" >&2
        exit 1
    fi
fi

echo "Installing engine system dependencies..."
$SUDO apt-get update -qq
$SUDO apt-get install -y -qq "${PACKAGES[@]}"
echo "System dependencies installed."
