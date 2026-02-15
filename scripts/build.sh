#!/bin/bash
set -euo pipefail

# build.sh — Build Clean Up.app macOS application bundle
#
# Usage: ./scripts/build.sh
# Or:    bun run build:app
#
# Produces: dist/Clean Up.app/

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DIST="$PROJECT_DIR/dist"
APP="$DIST/Clean Up.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

echo "==> Building Clean Up.app"
echo ""

# Clean previous build
rm -rf "$APP"
mkdir -p "$MACOS" "$RESOURCES"

# ---------------------------------------------------------------------------
# 1. Compile standalone Bun binary (the actual server)
# ---------------------------------------------------------------------------
echo "[1/6] Compiling standalone binary..."
BUILD_TS=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
bun build "$PROJECT_DIR/src/index.ts" --compile --define "__BUILD_TIME__=\"$BUILD_TS\"" --outfile "$MACOS/clean-up-server"
chmod +x "$MACOS/clean-up-server"

# ---------------------------------------------------------------------------
# 2. Compile native Swift launcher (so macOS treats it as a GUI app)
# ---------------------------------------------------------------------------
echo "[2/6] Compiling native launcher..."
swiftc -O -o "$MACOS/clean-up" "$SCRIPT_DIR/launcher.swift" -framework AppKit

# ---------------------------------------------------------------------------
# 3. Generate app icon
# ---------------------------------------------------------------------------
echo "[3/6] Generating app icon..."
ICON_DIR="$DIST/AppIcon.iconset"
rm -rf "$ICON_DIR"
mkdir -p "$ICON_DIR"

# Generate 1024px master icon
swift "$SCRIPT_DIR/generate-icon.swift" "$DIST/icon_1024.png"

# Create all required iconset sizes
declare -A ICON_SIZES=(
    ["icon_16x16.png"]=16
    ["icon_16x16@2x.png"]=32
    ["icon_32x32.png"]=32
    ["icon_32x32@2x.png"]=64
    ["icon_128x128.png"]=128
    ["icon_128x128@2x.png"]=256
    ["icon_256x256.png"]=256
    ["icon_256x256@2x.png"]=512
    ["icon_512x512.png"]=512
    ["icon_512x512@2x.png"]=1024
)

for name in "${!ICON_SIZES[@]}"; do
    sz="${ICON_SIZES[$name]}"
    sips -z "$sz" "$sz" "$DIST/icon_1024.png" --out "$ICON_DIR/$name" >/dev/null 2>&1
done

# Convert iconset to icns
iconutil -c icns "$ICON_DIR" -o "$RESOURCES/AppIcon.icns"
rm -rf "$ICON_DIR" "$DIST/icon_1024.png"

# ---------------------------------------------------------------------------
# 4. Create Info.plist
# ---------------------------------------------------------------------------
echo "[4/6] Writing Info.plist..."
cat > "$CONTENTS/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Clean Up</string>
    <key>CFBundleDisplayName</key>
    <string>Clean Up</string>
    <key>CFBundleIdentifier</key>
    <string>com.kennetkusk.clean-up</string>
    <key>CFBundleVersion</key>
    <string>1.0.1</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.1</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>clean-up</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2026 Kennet Dahl Kusk. MIT License.</string>
</dict>
</plist>
PLIST

# ---------------------------------------------------------------------------
# 5. Copy web UI assets and PkgInfo
# ---------------------------------------------------------------------------
echo "[5/6] Copying web UI assets..."
cp "$PROJECT_DIR/src/web/ui.html" "$RESOURCES/ui.html"
printf 'APPL????' > "$CONTENTS/PkgInfo"

# ---------------------------------------------------------------------------
# 6. Ad-hoc code signing (inner binaries first, then bundle)
# ---------------------------------------------------------------------------
echo "[6/6] Signing app bundle..."
codesign --force --sign - "$MACOS/clean-up-server"
codesign --force --sign - "$MACOS/clean-up"
codesign --force --sign - "$APP"

echo ""
echo "==> Build complete: $APP"
echo ""
ls -lh "$MACOS/clean-up" | awk '{print "    Launcher:    " $5}'
ls -lh "$MACOS/clean-up-server" | awk '{print "    Server:      " $5}'
echo "    Bundle: dist/Clean Up.app/"
echo ""
echo "Run 'bun run install:app' to install to ~/Applications."
