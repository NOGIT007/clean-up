#!/bin/bash
set -euo pipefail

# build.sh — Build Clean Up.app macOS application bundle (Tauri v2)
#
# Usage: ./scripts/build.sh
# Or:    bun run build:app
#
# Produces: dist/Clean Up.app/

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DIST="$PROJECT_DIR/dist"

echo "==> Building Clean Up.app (Tauri v2)"
echo ""

# ---------------------------------------------------------------------------
# 1. Check prerequisites
# ---------------------------------------------------------------------------
echo "[1/4] Checking prerequisites..."
command -v cargo >/dev/null 2>&1 || { echo "Error: Rust not installed. Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; exit 1; }
if ! cargo tauri --version >/dev/null 2>&1; then
    echo "Installing Tauri CLI..."
    cargo install tauri-cli
fi

# ---------------------------------------------------------------------------
# 2. Generate app icon
# ---------------------------------------------------------------------------
echo "[2/4] Generating app icon..."
mkdir -p "$DIST"
ICON_TMP="$DIST/icon_1024.png"
swift "$SCRIPT_DIR/generate-icon.swift" "$ICON_TMP"
cargo tauri icon "$ICON_TMP" 2>/dev/null || echo "  (cargo tauri icon not available, using existing icons)"
rm -f "$ICON_TMP"

# ---------------------------------------------------------------------------
# 3. Build with Tauri (release mode)
# ---------------------------------------------------------------------------
echo "[3/4] Building Tauri app (release)..."
cd "$PROJECT_DIR"
cargo tauri build 2>&1

# ---------------------------------------------------------------------------
# 4. Copy to dist/
# ---------------------------------------------------------------------------
echo "[4/4] Copying to dist..."
TAURI_OUT="$PROJECT_DIR/src-tauri/target/release/bundle/macos/Clean Up.app"
if [ ! -d "$TAURI_OUT" ]; then
    echo "Error: Tauri build output not found at: $TAURI_OUT"
    exit 1
fi

mkdir -p "$DIST"
rm -rf "$DIST/Clean Up.app"
cp -R "$TAURI_OUT" "$DIST/Clean Up.app"

# Remove target bundle copy to prevent duplicate Spotlight entries
rm -rf "$TAURI_OUT"

echo ""
echo "==> Build complete: dist/Clean Up.app/"
echo ""
BINARY="$DIST/Clean Up.app/Contents/MacOS/clean-up"
if [ -f "$BINARY" ]; then
    ls -lh "$BINARY" | awk '{print "    Binary:  " $5}'
fi
echo "    Bundle:  dist/Clean Up.app/"
echo ""
echo "Run 'bun run install:app' to install to ~/Applications."
