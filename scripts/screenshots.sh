#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────
# screenshots.sh — Start the engine with API and capture screenshots
#
# Usage:
#   ./scripts/screenshots.sh                     # defaults: 5 screenshots, 60 ticks apart
#   ./scripts/screenshots.sh --count 10          # 10 screenshots
#   ./scripts/screenshots.sh --ticks 120         # 120 ticks between each screenshot
#   ./scripts/screenshots.sh --out ./my-shots    # custom output directory
#   ./scripts/screenshots.sh --overlays collision,grid  # debug overlays
#   ./scripts/screenshots.sh --level dune_01     # specific level
# ─────────────────────────────────────────────────────────────
set -euo pipefail

PORT=9999
COUNT=5
TICKS_BETWEEN=60
OUT_DIR="./screenshots"
OVERLAYS=""
LEVEL_ARGS=""
ENGINE_PID=""

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --count N        Number of screenshots to take (default: $COUNT)"
    echo "  --ticks N        Game ticks between screenshots (default: $TICKS_BETWEEN)"
    echo "  --out DIR        Output directory (default: $OUT_DIR)"
    echo "  --port N         API port (default: $PORT)"
    echo "  --overlays LIST  Comma-separated overlays: collision,grid,paths,tower_ranges"
    echo "  --level NAME     Level to load (passed to amigo run)"
    echo "  --help           Show this help"
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --count)   COUNT="$2";         shift 2 ;;
        --ticks)   TICKS_BETWEEN="$2"; shift 2 ;;
        --out)     OUT_DIR="$2";       shift 2 ;;
        --port)    PORT="$2";          shift 2 ;;
        --overlays) OVERLAYS="$2";     shift 2 ;;
        --level)   LEVEL_ARGS="--level $2"; shift 2 ;;
        --help)    usage ;;
        *)         echo "Unknown option: $1"; usage ;;
    esac
done

mkdir -p "$OUT_DIR"

cleanup() {
    if [[ -n "$ENGINE_PID" ]] && kill -0 "$ENGINE_PID" 2>/dev/null; then
        echo "Stopping engine (PID $ENGINE_PID)..."
        kill "$ENGINE_PID" 2>/dev/null || true
        wait "$ENGINE_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# ── Send a JSON-RPC request and return the response ──
rpc() {
    local method="$1"
    local params="${2:-null}"
    local id="${3:-1}"
    local req="{\"jsonrpc\":\"2.0\",\"id\":${id},\"method\":\"${method}\",\"params\":${params}}"
    echo "$req" | nc -q 1 127.0.0.1 "$PORT" 2>/dev/null || \
    echo "$req" | nc -w 1 127.0.0.1 "$PORT" 2>/dev/null
}

# ── Wait for API server to be ready ──
wait_for_api() {
    echo "Waiting for API server on port $PORT..."
    for i in $(seq 1 30); do
        if rpc "engine.status" "null" 1 2>/dev/null | grep -q '"tick"'; then
            echo "API server ready."
            return 0
        fi
        sleep 1
    done
    echo "ERROR: API server did not start within 30 seconds."
    exit 1
}

# ── Start the engine ──
echo "Starting engine with API server..."
# shellcheck disable=SC2086
cargo run --release --features amigo_engine/api -p amigo_cli -- run --api $LEVEL_ARGS &
ENGINE_PID=$!
wait_for_api

# ── Build overlays JSON array ──
OVERLAYS_JSON="[]"
if [[ -n "$OVERLAYS" ]]; then
    OVERLAYS_JSON=$(echo "$OVERLAYS" | tr ',' '\n' | sed 's/.*/"&"/' | paste -sd ',' | sed 's/^/[/;s/$/]/')
fi

# ── Capture screenshots ──
echo "Taking $COUNT screenshots, $TICKS_BETWEEN ticks apart..."
echo "Output: $OUT_DIR"
echo ""

for i in $(seq 1 "$COUNT"); do
    # Advance simulation
    rpc "tick" "{\"count\":$TICKS_BETWEEN}" "$i" > /dev/null 2>&1
    # Small delay to let the render loop catch up
    sleep 0.3

    # Request screenshot
    FILENAME="$OUT_DIR/frame_$(printf '%04d' "$i").png"
    ABS_PATH="$(cd "$(dirname "$FILENAME")" && pwd)/$(basename "$FILENAME")"
    rpc "screenshot" "{\"path\":\"$ABS_PATH\",\"overlays\":$OVERLAYS_JSON}" "$i" > /dev/null 2>&1

    # Wait for screenshot to be written
    sleep 0.5

    # Check result
    RESULT=$(rpc "screenshot_results" "null" "$i" 2>/dev/null)
    if [[ -f "$ABS_PATH" ]]; then
        SIZE=$(stat -f%z "$ABS_PATH" 2>/dev/null || stat -c%s "$ABS_PATH" 2>/dev/null || echo "?")
        echo "  [$i/$COUNT] $FILENAME ($SIZE bytes)"
    else
        echo "  [$i/$COUNT] $FILENAME — waiting..."
        sleep 1
        if [[ -f "$ABS_PATH" ]]; then
            echo "           -> saved"
        else
            echo "           -> FAILED (check engine logs)"
        fi
    fi
done

echo ""
echo "Done. Screenshots saved to: $OUT_DIR"
echo "Stopping engine..."
