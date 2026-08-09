#!/usr/bin/env bash
set -euo pipefail

CRATE_PKG="pokered-web"
CRATE_BIN="pokered_web"
WASM_TARGET="wasm32-unknown-unknown"
WORKSPACE_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WASM_OUT_DIR="$(dirname "$0")/frontend/src/wasm"
FRONTEND_DIR="$(dirname "$0")/frontend"
WASM_BIN="$WORKSPACE_ROOT/target/$WASM_TARGET/debug/${CRATE_BIN}.wasm"

echo "========================================="
echo "Pokémon Red — Dev Server (Vite + Vue 3)"
echo "========================================="
echo ""

if ! command -v wasm-bindgen &>/dev/null; then
  echo "Error: wasm-bindgen not found."
  echo "  cargo install wasm-bindgen-cli"
  exit 1
fi

if ! command -v cargo-watch &>/dev/null; then
  echo "Note: cargo-watch not installed — Rust auto-rebuild disabled."
  echo "  cargo install cargo-watch   (optional)"
  echo ""
  WATCH_AVAILABLE=false
else
  WATCH_AVAILABLE=true
fi

if [ ! -d "$FRONTEND_DIR/node_modules" ]; then
  echo "Installing npm dependencies..."
  (cd "$FRONTEND_DIR" && npm install)
  echo ""
fi

rebuild_wasm() {
  echo "[wasm] Rebuilding..."
  cargo build -p "$CRATE_PKG" --target "$WASM_TARGET" \
    --manifest-path "$WORKSPACE_ROOT/Cargo.toml" 2>&1 || return 1

  rm -f "$WASM_OUT_DIR"/*.wasm "$WASM_OUT_DIR"/*.js
  wasm-bindgen \
    --target web \
    --no-typescript \
    --out-dir "$WASM_OUT_DIR" \
    "$WASM_BIN" 2>&1 || return 1

  echo "[wasm] ✓ Done"
}

echo "Step 1 — Initial WASM build (debug)..."
rebuild_wasm
echo ""

echo "Step 2 — Starting Vite dev server on http://localhost:8080 ..."
echo ""

if [ "$WATCH_AVAILABLE" = "true" ]; then
  (
    cargo watch \
      -w "$(dirname "$0")/src" \
      -w "$WORKSPACE_ROOT/crates/pokered-app/src" \
      -w "$WORKSPACE_ROOT/crates/pokered-renderer/src" \
      -w "$WORKSPACE_ROOT/crates/pokered-core/src" \
      -x "build -p $CRATE_PKG --target $WASM_TARGET" \
      --shell "wasm-bindgen --target web --no-typescript --out-dir $WASM_OUT_DIR $WASM_BIN && echo '[wasm] ✓ hot-updated'" \
      2>&1 | sed 's/^/[rust-watch] /'
  ) &
  WATCH_PID=$!
  trap "kill $WATCH_PID 2>/dev/null; exit" INT TERM
fi

(cd "$FRONTEND_DIR" && npm run dev)
