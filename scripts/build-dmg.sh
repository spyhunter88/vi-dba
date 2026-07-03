#!/usr/bin/env bash
# Build a signed/unsigned macOS .dmg installer for Vi DB Connect.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="$ROOT_DIR/frontend"
SRC_TAURI_DIR="$ROOT_DIR/src-tauri"
DIST_DIR="$ROOT_DIR/dist"

if [[ "$(uname)" != "Darwin" ]]; then
  echo "error: DMG bundles can only be built on macOS" >&2
  exit 1
fi

if ! command -v tauri >/dev/null 2>&1; then
  echo "error: tauri CLI not found — install it (e.g. 'brew install tauri-cli' or 'cargo install tauri-cli')" >&2
  exit 1
fi

echo "==> Installing frontend dependencies"
npm install --prefix "$FRONTEND_DIR"

echo "==> Building DMG bundle"
tauri build --config "$SRC_TAURI_DIR/tauri.conf.json" --bundles dmg "$@"

DMG_DIR="$ROOT_DIR/target/release/bundle/dmg"
DMG_PATH="$(find "$DMG_DIR" -maxdepth 1 -name '*.dmg' -print -quit 2>/dev/null || true)"

if [[ -z "$DMG_PATH" ]]; then
  echo "error: build finished but no .dmg was found in $DMG_DIR" >&2
  exit 1
fi

mkdir -p "$DIST_DIR"
cp "$DMG_PATH" "$DIST_DIR/"

echo "==> DMG ready: $DIST_DIR/$(basename "$DMG_PATH")"
