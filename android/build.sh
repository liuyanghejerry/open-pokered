#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

if [ -z "${DOTZUKI_ENGINE_CHECKOUT:-}" ]; then
    echo "Set DOTZUKI_ENGINE_CHECKOUT to the dotzuki engine checkout." >&2
    exit 2
fi

exec python3 "$PROJECT_DIR/scripts/build-android.py" \
    --engine-checkout "$DOTZUKI_ENGINE_CHECKOUT" \
    --out "$PROJECT_DIR/dist/android" \
    --assemble \
    "$@"
