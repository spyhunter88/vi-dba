#!/usr/bin/env bash
# Build Linux binaries (.deb and .AppImage) using Docker.
# This works on macOS, Linux, and Windows (under Git Bash, WSL, or MSYS).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_NAME="vidba-linux-builder"

echo "==> Building Docker image: $IMAGE_NAME"
docker build -t "$IMAGE_NAME" -f "$ROOT_DIR/Dockerfile.linux" "$ROOT_DIR"

echo "==> Running build inside Docker container"
# We mount the root directory to /workspace, but use anonymous volumes to isolate:
# - node_modules: prevents overwriting host's Windows/macOS node_modules with Linux ones
# - target: prevents Rust compilation target conflicts
# The compiled outputs will still be written to /workspace/dist, which maps back to the host.
docker run --rm \
  -v "$ROOT_DIR:/workspace" \
  -v "/workspace/frontend/node_modules" \
  -v "/workspace/src-tauri/target" \
  -v "/workspace/target" \
  "$IMAGE_NAME" "$@"

echo "==> Build process completed! Check the 'dist' directory for outputs."
ls -lh "$ROOT_DIR/dist" 2>/dev/null || true
