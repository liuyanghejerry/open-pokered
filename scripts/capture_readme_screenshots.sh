#!/usr/bin/env bash
#
# capture_readme_screenshots.sh — regenerate the README screenshots in
# docs/screenshots/readme/ in both English and Chinese (zh/ subdirectory).
#
# The frame counts below are generous on purpose: several screens animate in
# or typewriter their text over hundreds of frames, and the old README shots
# were captured with the default of 5 frames, i.e. before the picture had
# finished loading.
#
# Requires a release build first:
#   scripts/fetch-gfx.sh   # once, if gfx/ is missing
#   cargo build --release --bin pokered-app
#
# Override the binary path with BIN=/path/to/pokered-app if needed.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

BIN="${BIN:-target/release/pokered-app}"
OUT="docs/screenshots/readme"
mkdir -p "$OUT/zh"

FRAMES_TITLE=400     # title screen scroll/mon animation settled
FRAMES_OVERWORLD=120 # past the fade-in
FRAMES_BATTLE=400    # intro slide/cry done, opening message fully typed
                     # (captures feed no input, so the battle menu never opens)
FRAMES_TOWNMAP=60
FRAMES_OAK=1500      # Oak's greeting fully typewritten at medium text speed

# --- English (in place, same filenames as before) ---------------------------
"$BIN" screenshot --screen title     --output "$OUT/title.png"     --frames "$FRAMES_TITLE"
"$BIN" screenshot --screen overworld --output "$OUT/overworld.png" --frames "$FRAMES_OVERWORLD"
"$BIN" battle --config sample_battle.json --screenshot "$OUT/battle.png" --frames "$FRAMES_BATTLE"
"$BIN" screenshot --screen town-map  --output "$OUT/town-map.png"  --frames "$FRAMES_TOWNMAP"
"$BIN" screenshot --screen oak       --output "$OUT/oak.png"       --frames "$FRAMES_OAK"

# --- 中文 (zh/ subdirectory) -------------------------------------------------
"$BIN" screenshot --screen title     --output "$OUT/zh/title.png"     --frames "$FRAMES_TITLE"     --lang zh
"$BIN" screenshot --screen overworld --output "$OUT/zh/overworld.png" --frames "$FRAMES_OVERWORLD" --lang zh
"$BIN" battle --config sample_battle.json --screenshot "$OUT/zh/battle.png" --frames "$FRAMES_BATTLE" --lang zh
"$BIN" screenshot --screen town-map  --output "$OUT/zh/town-map.png"  --frames "$FRAMES_TOWNMAP"  --lang zh
"$BIN" screenshot --screen oak       --output "$OUT/zh/oak.png"       --frames "$FRAMES_OAK"       --lang zh

echo "All README screenshots regenerated under $OUT (English + zh/)."
