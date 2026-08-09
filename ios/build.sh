#!/bin/bash
set -euo pipefail

# ── Pokémon Red iOS Build Script ──────────────────────────────────────────
# Prerequisites:
#   - Rust iOS target: rustup target add aarch64-apple-ios aarch64-apple-ios-sim
#   - Xcode 15+ (for iOS 17+ SDK)
#
# Phases:
#   1. Build Rust static lib for aarch64-apple-ios (device)
#   2. Build Rust static lib for aarch64-apple-ios-sim (simulator)
#   3. Copy .a artifacts to Xcode linkable location
#   4. xcodebuild (unsigned — for CI validation)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
IOS_DIR="$SCRIPT_DIR"
LIBS_DIR="$IOS_DIR/Pokered/lib"

echo "=== phase 1: Build Rust library (device) ==="
cd "$PROJECT_DIR"
cargo build -p pokered-ios --target aarch64-apple-ios --release

echo ""
echo "=== phase 2: Build Rust library (simulator) ==="
cargo build -p pokered-ios --target aarch64-apple-ios-sim --release

echo ""
echo "=== phase 3: Copy .a to Xcode lib directory ==="
mkdir -p "$LIBS_DIR"
cp "target/aarch64-apple-ios/release/libpokered_ios.a" "$LIBS_DIR/libpokered_ios_device.a"
cp "target/aarch64-apple-ios-sim/release/libpokered_ios.a" "$LIBS_DIR/libpokered_ios_sim.a"
echo "  → $LIBS_DIR/libpokered_ios_device.a"
echo "  → $LIBS_DIR/libpokered_ios_sim.a"

echo ""
echo "=== phase 4: Build Xcode project ==="
xcodebuild -project "$IOS_DIR/Pokered.xcodeproj" \
  -target Pokered \
  -sdk iphoneos \
  -configuration Release \
  CODE_SIGNING_ALLOWED=NO \
  build 2>&1 | tail -5
echo "  → iOS .app built successfully (unsigned)"

echo ""
echo "✅ iOS Rust libraries built successfully"
ls -lh "$LIBS_DIR/" 2>/dev/null || echo "  (lib directory empty — check cargo build output above)"
