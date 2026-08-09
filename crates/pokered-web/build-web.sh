#!/usr/bin/env bash
set -euo pipefail

CRATE_PKG="pokered-web"
CRATE_BIN="pokered_web"
WASM_TARGET="wasm32-unknown-unknown"
WORKSPACE_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WASM_OUT_DIR="$(dirname "$0")/frontend/src/wasm"
FRONTEND_DIR="$(dirname "$0")/frontend"
BUILD_TYPE="${1:-release}"

echo "========================================="
echo "Pokémon Red Web Build (Vite + Vue 3)"
echo "========================================="
echo ""

if ! command -v wasm-bindgen &>/dev/null; then
  echo "Error: wasm-bindgen not found."
  echo "  cargo install wasm-bindgen-cli"
  exit 1
fi

if ! rustup target list --installed | grep -q "$WASM_TARGET"; then
  echo "Error: $WASM_TARGET target not installed."
  echo "  rustup target add $WASM_TARGET"
  exit 1
fi

if [ ! -d "$FRONTEND_DIR/node_modules" ]; then
  echo "Installing npm dependencies..."
  (cd "$FRONTEND_DIR" && npm install)
  echo ""
fi

echo "Step 1/3 — Compiling Rust → WASM ($BUILD_TYPE)..."
if [ "$BUILD_TYPE" = "release" ]; then
  cargo build -p "$CRATE_PKG" --target "$WASM_TARGET" --release \
    --manifest-path "$WORKSPACE_ROOT/Cargo.toml"
  WASM_BIN="$WORKSPACE_ROOT/target/$WASM_TARGET/release/${CRATE_BIN}.wasm"
else
  cargo build -p "$CRATE_PKG" --target "$WASM_TARGET" \
    --manifest-path "$WORKSPACE_ROOT/Cargo.toml"
  WASM_BIN="$WORKSPACE_ROOT/target/$WASM_TARGET/debug/${CRATE_BIN}.wasm"
fi
echo "✓ Rust compiled"
echo ""

echo "Step 2/3 — Generating JS bindings with wasm-bindgen..."
rm -f "$WASM_OUT_DIR"/*.wasm "$WASM_OUT_DIR"/*.js
wasm-bindgen \
  --target web \
  --no-typescript \
  --out-dir "$WASM_OUT_DIR" \
  "$WASM_BIN"
echo "✓ wasm-bindgen done → frontend/src/wasm/"
echo ""

echo "Step 3/3 — Building Vite bundle..."
(cd "$FRONTEND_DIR" && npm run build)
echo "✓ Vite build done → frontend/dist/"
echo ""

echo "========================================="
echo "Build complete!"
echo "========================================="
echo ""
echo "Output: $FRONTEND_DIR/dist/"
echo ""
echo "To preview:"
echo "  cd frontend && npm run preview"
echo ""
