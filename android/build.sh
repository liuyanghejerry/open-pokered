#!/bin/bash
set -euo pipefail

# ── Pokémon Red Android Build Script ──────────────────────────────────
# Prerequisites:
#   - Rust Android target: rustup target add aarch64-linux-android
#   - Android NDK (brew install android-ndk)
#   - Java 17+

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ANDROID_DIR="$SCRIPT_DIR"
TARGET="aarch64-linux-android"
JNILIBS_DIR="$ANDROID_DIR/app/src/main/jniLibs/arm64-v8a"

# Ensure Android SDK and NDK are findable
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-/opt/homebrew/share/android-ndk}"

# Gradle/Kotlin in this project requires running on a supported JDK.
# Java 25 is currently unsupported by the pinned Android/Kotlin plugin set,
# so prefer JDK 21 (or 17) when available.
if [ -z "${JAVA_HOME:-}" ] && command -v /usr/libexec/java_home >/dev/null 2>&1; then
    for version in 21 17; do
        if JAVA_CANDIDATE="$(/usr/libexec/java_home -v "$version" 2>/dev/null)"; then
            export JAVA_HOME="$JAVA_CANDIDATE"
            export PATH="$JAVA_HOME/bin:$PATH"
            break
        fi
    done
fi

if [ -z "${JAVA_HOME:-}" ]; then
    for brew_java in \
        /opt/homebrew/opt/openjdk@21 \
        /opt/homebrew/opt/openjdk@17 \
        /usr/local/opt/openjdk@21 \
        /usr/local/opt/openjdk@17
    do
        if [ -x "$brew_java/bin/java" ]; then
            export JAVA_HOME="$brew_java"
            export PATH="$JAVA_HOME/bin:$PATH"
            break
        fi
    done
fi

if ! java -version 2>&1 | grep -Eq 'version "(17|21)\.'; then
    # Try to recover automatically when JAVA_HOME points to an unsupported JDK
    # (e.g. Java 25) by switching to a compatible local JDK if available.
    for version in 21 17; do
        if JAVA_CANDIDATE="$(/usr/libexec/java_home -v "$version" 2>/dev/null)"; then
            export JAVA_HOME="$JAVA_CANDIDATE"
            export PATH="$JAVA_HOME/bin:$PATH"
            break
        fi
    done

    if ! java -version 2>&1 | grep -Eq 'version "(17|21)\.'; then
        for brew_java in \
            /opt/homebrew/opt/openjdk@21 \
            /opt/homebrew/opt/openjdk@17 \
            /usr/local/opt/openjdk@21 \
            /usr/local/opt/openjdk@17
        do
            if [ -x "$brew_java/bin/java" ]; then
                export JAVA_HOME="$brew_java"
                export PATH="$JAVA_HOME/bin:$PATH"
                break
            fi
        done
    fi
fi

if ! java -version 2>&1 | grep -Eq 'version "(17|21)\.'; then
    echo "❌ Unsupported Java runtime for Android build."
    echo "   Current: $(java -version 2>&1 | head -n 1)"
    echo "   JAVA_HOME: ${JAVA_HOME:-<unset>}"
    echo "   Required: JDK 17 or JDK 21 (Java 25 is not supported here)."
    echo "   Install one of:"
    echo "     brew install --cask temurin@21"
    echo "     brew install --cask temurin@17"
    echo "   Then re-run this script (or set JAVA_HOME explicitly)."
    exit 1
fi

# Ensure Gradle can always resolve Android SDK path in non-interactive shells.
LOCAL_PROPERTIES="$ANDROID_DIR/local.properties"
if [ ! -f "$LOCAL_PROPERTIES" ]; then
    escaped_android_home="${ANDROID_HOME//\\/\\\\}"
    escaped_android_home="${escaped_android_home//:/\\:}"
    printf 'sdk.dir=%s\n' "$escaped_android_home" > "$LOCAL_PROPERTIES"
fi

NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin"
if [ -d "$NDK_BIN" ]; then
    export PATH="$NDK_BIN:$PATH"
fi

echo "=== Step 1: Build Rust library ==="
cd "$PROJECT_DIR"
cargo build -p pokered-android --target "$TARGET" --release

echo ""
echo "=== Step 2: Copy .so + libc++ to jniLibs ==="
mkdir -p "$JNILIBS_DIR"
cp "target/$TARGET/release/libpokered_android.so" "$JNILIBS_DIR/"
cp "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/sysroot/usr/lib/$TARGET/libc++_shared.so" "$JNILIBS_DIR/"
echo "  → $JNILIBS_DIR/libpokered_android.so"
echo "  → $JNILIBS_DIR/libc++_shared.so"

echo ""
echo "=== Step 3: Build APK ==="
cd "$ANDROID_DIR"
./gradlew assembleRelease

APK_PATH="$ANDROID_DIR/app/build/outputs/apk/release/app-release-unsigned.apk"
if [ -f "$APK_PATH" ]; then
    echo ""
    echo "✅ APK built: $APK_PATH"
    ls -lh "$APK_PATH"
else
    echo ""
    echo "❌ APK not found. Check Gradle output above."
    exit 1
fi
