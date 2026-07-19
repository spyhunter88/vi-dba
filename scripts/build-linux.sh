#!/usr/bin/env bash
# Build Linux (.deb and .AppImage) installers for Vi DB Connect.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="$ROOT_DIR/frontend"
SRC_TAURI_DIR="$ROOT_DIR/src-tauri"
DIST_DIR="$ROOT_DIR/dist"

# Check if OS is Linux (unless running inside a container, which we assume is Linux)
if [[ "$(uname)" != "Linux" ]]; then
  echo "error: Linux bundles can only be built on Linux/Ubuntu or inside a Docker container" >&2
  exit 1
fi

# Detect tauri command (either 'tauri' in PATH or 'cargo tauri')
TAURI_CMD=""
if command -v tauri >/dev/null 2>&1; then
  TAURI_CMD="tauri"
elif cargo tauri --version >/dev/null 2>&1; then
  TAURI_CMD="cargo tauri"
elif npx @tauri-apps/cli@2 --version >/dev/null 2>&1; then
  TAURI_CMD="npx @tauri-apps/cli@2"
else
  echo "error: Tauri CLI not found. Please install it:" >&2
  echo "  - via Cargo: cargo install tauri-cli" >&2
  echo "  - via npm: npm install -g @tauri-apps/cli" >&2
  exit 1
fi

echo "==> Using Tauri CLI: $TAURI_CMD"

echo "==> Installing frontend dependencies"
npm install --prefix "$FRONTEND_DIR"

echo "==> Building Linux bundles (deb, appimage)"
# Forward all arguments (like --features oracle) to the build command
$TAURI_CMD build --config "$SRC_TAURI_DIR/tauri.conf.json" "$@"

# Copy build artifacts to the dist directory
mkdir -p "$DIST_DIR"

DEB_DIR="$ROOT_DIR/target/release/bundle/deb"
APPIMAGE_DIR="$ROOT_DIR/target/release/bundle/appimage"

echo "==> Staging build artifacts to $DIST_DIR"

# Copy DEB if exists
if [ -d "$DEB_DIR" ]; then
  find "$DEB_DIR" -maxdepth 1 -name '*.deb' -exec cp {} "$DIST_DIR/" \;
fi

# Copy AppImage if exists
if [ -d "$APPIMAGE_DIR" ]; then
  find "$APPIMAGE_DIR" -maxdepth 1 -name '*.AppImage' -exec cp {} "$DIST_DIR/" \;
fi

echo "==> Build complete! Output files in $DIST_DIR:"
ls -lh "$DIST_DIR"
