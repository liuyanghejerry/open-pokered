#!/usr/bin/env bash
# pokered-gba build: compile the .gba ROM (and optionally launch mGBA).
#
# Usage:
#   ./build.sh            # release build -> target/.../pokered-gba.gba
#   ./build.sh debug      # dev build
#   ./build.sh run        # release build + launch in mGBA (SDL)
#   ./build.sh logs       # release build + run mGBA ~10s capturing the
#                         # agb::println log channel to stdout (Ctrl-C safe)
set -euo pipefail
cd "$(dirname "$0")"

MODE="${1:-release}"
PROFILE=release
[[ "$MODE" == "debug" ]] && PROFILE=dev

cargo +nightly build "--$PROFILE"
ELF="target/thumbv4t-none-eabi/$PROFILE/pokered-gba"
agb-gbafix "$ELF"
ROM="$ELF.gba"
echo "ROM: $ROM ($(du -h "$ROM" | cut -f1))"

case "$MODE" in
  run)
    exec /opt/homebrew/bin/mgba -3 -C logToStdout=1 -C logLevel.gba.debug=127 "$ROM"
    ;;
  logs)
    /opt/homebrew/bin/mgba -1 -C logToStdout=1 -C logLevel.gba.debug=127 "$ROM" &
    MGBA_PID=$!
    sleep 10
    kill "$MGBA_PID" 2>/dev/null || true
    ;;
esac
