#!/bin/bash
set -euo pipefail

# install.sh — Install Clean Up.app to ~/Applications and symlink CLI
#
# Usage: ./scripts/install.sh
# Or:    bun run install:app

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
APP_NAME="Clean Up.app"
APP_SOURCE="$PROJECT_DIR/dist/$APP_NAME"
APP_DEST="$HOME/Applications/$APP_NAME"
BIN_DIR="$HOME/.local/bin"
BIN_NAME="clean-up"

# Check that the app was built
if [ ! -d "$APP_SOURCE" ]; then
    echo "Error: $APP_SOURCE not found. Run 'bun run build:app' first."
    exit 1
fi

# Create ~/Applications if needed
mkdir -p "$HOME/Applications"

# Remove old install if present
if [ -d "$APP_DEST" ]; then
    echo "Removing previous install..."
    rm -rf "$APP_DEST"
fi

# Copy .app bundle
echo "Installing $APP_NAME to ~/Applications..."
cp -R "$APP_SOURCE" "$APP_DEST"

# Remove the dist copy so Spotlight only indexes the installed one
rm -rf "$APP_SOURCE"
echo "Removed dist copy to prevent duplicate Spotlight entries."

# Symlink the binary for CLI usage (launches the GUI app)
mkdir -p "$BIN_DIR"
if [ -L "$BIN_DIR/$BIN_NAME" ] || [ -f "$BIN_DIR/$BIN_NAME" ]; then
    rm "$BIN_DIR/$BIN_NAME"
fi
ln -s "$APP_DEST/Contents/MacOS/clean-up" "$BIN_DIR/$BIN_NAME"
echo "Symlinked CLI: $BIN_DIR/$BIN_NAME"

# Trigger Spotlight reindex for the installed app
mdimport "$APP_DEST" 2>/dev/null || true

echo ""
echo "Done! Clean Up is installed."
echo ""
echo "  Spotlight:  Search for \"Clean Up\" (may take a moment to index)"
echo "  CLI:        $BIN_NAME (launches the GUI)"
echo ""
echo "Note: On first launch, macOS may show a Gatekeeper warning."
echo "      Right-click the app > Open to bypass it."
echo ""
echo "  Full Disk Access must be granted in System Settings:"
echo "    Privacy & Security > Full Disk Access > Enable 'Clean Up'"
