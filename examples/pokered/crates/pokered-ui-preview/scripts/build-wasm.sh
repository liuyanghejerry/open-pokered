#!/usr/bin/env bash
set -euo pipefail
# Build the pokered-ui-preview wasm bundle for the Vue editor
# Usage: ./scripts/build-wasm.sh [--release]

cd "$(dirname "$0")/.."

PROFILE="dev"
TARGET_DIR="debug"
if [[ "${1:-}" == "--release" ]]; then
  PROFILE="release"
  TARGET_DIR="release"
fi

# Use wasm-pack if available, fall back to cargo build
if command -v wasm-pack >/dev/null 2>&1; then
  if [[ "$PROFILE" == "release" ]]; then
    wasm-pack build --target web --out-dir pkg --release
  else
    wasm-pack build --target web --out-dir pkg --dev
  fi
else
  echo "WARN: wasm-pack not found, falling back to plain cargo build"
  if [[ "$PROFILE" == "release" ]]; then
    cargo build --target wasm32-unknown-unknown --release
  else
    cargo build --target wasm32-unknown-unknown
  fi
  echo "Built: ../../target/wasm32-unknown-unknown/$TARGET_DIR/pokered_ui_preview.wasm"
fi
