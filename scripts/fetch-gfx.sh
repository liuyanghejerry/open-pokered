#!/usr/bin/env bash
#
# fetch-gfx.sh — populate the pokered example's `gfx/` asset directory
# (examples/pokered/gfx/) from upstream pret/pokered instead of
# vendoring the ~2.7 MB of PNG/blockset assets in this repository.
#
# The graphics this project consumes are a byte-for-byte copy of pret/pokered's
# `gfx/` tree (verified: identical git blob SHAs for every shared file). Rather
# than commit that copy here, we fetch it on demand, pinned to a specific
# upstream commit for reproducibility.
#
# Usage:
#   scripts/fetch-gfx.sh            # populate gfx/ if missing (idempotent)
#   scripts/fetch-gfx.sh --force    # re-sync even if gfx/ already exists
#
# The fetch is surgical: a blobless, sparse, single-commit checkout of only the
# `gfx/` subtree — it does NOT download the rest of pret/pokered (assembly,
# audio, text, history).
#
# Requires: git >= 2.27 (partial clone + cone-mode sparse checkout).

set -euo pipefail

# --- Pin -------------------------------------------------------------------
# The upstream commit whose gfx/ tree this project was built against. Bump this
# (and re-run the script) to pull newer upstream assets. Keep it a full 40-char
# SHA so the fetch is exact and reproducible.
PRET_REPO="https://github.com/pret/pokered"
PRET_PIN="1e96034092686d006e863cace09e87273051a3d8"  # master @ 2026-07-02

# --- Paths -----------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
GFX_DIR="$REPO_ROOT/examples/pokered/gfx"

FORCE=0
for arg in "$@"; do
  case "$arg" in
    --force|-f) FORCE=1 ;;
    -h|--help)
      sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "fetch-gfx.sh: unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

# --- Idempotence guard -----------------------------------------------------
# Treat gfx/ as "already present" when it holds real assets (a PNG), so a plain
# re-run after a fresh checkout is cheap and a no-op.
if [[ "$FORCE" -ne 1 ]] && [[ -e "$GFX_DIR/tilesets/overworld.png" ]]; then
  echo "gfx/ already populated (found gfx/tilesets/overworld.png). Use --force to re-sync." >&2
  exit 0
fi

echo "Fetching gfx/ from ${PRET_REPO} @ ${PRET_PIN} ..."

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/pokered-gfx.XXXXXX")"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

# Blobless, no-checkout clone of the default branch, then a sparse checkout of
# only gfx/ pinned to the exact commit. `git fetch <sha>` works against GitHub
# (it allows fetching reachable SHAs), so we never rely on the pin being the
# current branch tip.
git init -q "$TMP_DIR"
git -C "$TMP_DIR" remote add origin "$PRET_REPO"
git -C "$TMP_DIR" config extensions.partialClone origin
git -C "$TMP_DIR" sparse-checkout init --cone
git -C "$TMP_DIR" sparse-checkout set gfx
git -C "$TMP_DIR" -c protocol.version=2 fetch -q --filter=blob:none --depth 1 origin "$PRET_PIN"
git -C "$TMP_DIR" checkout -q FETCH_HEAD

if [[ ! -d "$TMP_DIR/gfx" ]]; then
  echo "fetch-gfx.sh: upstream checkout has no gfx/ directory — aborting." >&2
  exit 1
fi

# Overlay onto the repo-root gfx/ (no --delete: preserves any locally-authored
# assets, e.g. custom tilesets dropped in gfx/blocksets/ by the editors). For a
# pristine re-sync, `rm -rf gfx/` first, then run with --force.
mkdir -p "$GFX_DIR"
if command -v rsync >/dev/null 2>&1; then
  rsync -a "$TMP_DIR/gfx/" "$GFX_DIR/"
else
  cp -R "$TMP_DIR/gfx/." "$GFX_DIR/"
fi

# Prune upstream's RGBDS `INCBIN` loader stubs (gfx/*.asm). They drive the
# original assembly build only; this Rust project never reads them, and we
# never vendored them, so drop them to keep gfx/ matching our historical shape.
rm -f "$GFX_DIR"/*.asm

COUNT="$(find "$GFX_DIR" -type f | wc -l | tr -d ' ')"
echo "Done. gfx/ now holds ${COUNT} files (pinned to ${PRET_PIN:0:7})."
