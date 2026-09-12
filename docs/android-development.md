# Android development

Pokered uses the shared dotzuki mobile ABI on Android. `pokered-mobile`
contains the game adapter and embedded assets. The engine repository owns the
Kotlin Activity, frame clock, display, audio, input, persistence, JNI bridge,
and Gradle project template.

## Prerequisites

- the `aarch64-linux-android` Rust target;
- Android SDK 35, NDK 27, CMake 3.22.1, and JDK 17;
- the `gfx/` assets from `scripts/fetch-gfx.sh`; and
- a dotzuki engine checkout at v0.7.0 or later.

## Export the Android Studio project

From the repository root:

```bash
python3 scripts/build-android.py \
  --engine-checkout ../dotzuki \
  --ndk "$ANDROID_NDK_HOME" \
  --out dist/android
```

Pass `--version blue` for the Blue data set. The output directory must be
empty because it is a generated platform project that may be customized after
export.

The script builds `libpokered_mobile.a` from the tag-pinned Cargo dependencies
and asks the selected engine checkout's
`export-mobile-host.py --platform android` command to assemble the host. The
initialization payload is `pokered:red:v1` or `pokered:blue:v1`; the game
factory validates it.

Open `dist/android` in Android Studio, or assemble a debug APK with the
checked-in Gradle wrapper:

```bash
android/gradlew --project-dir dist/android assembleDebug --no-daemon
```

`android/build.sh` performs export and assembly when
`DOTZUKI_ENGINE_CHECKOUT` points at the engine checkout:

```bash
DOTZUKI_ENGINE_CHECKOUT=../dotzuki android/build.sh
```

The APK is written to
`dist/android/app/build/outputs/apk/debug/app-debug.apk`.

## Runtime ownership

The Activity owns `Choreographer`, the `SurfaceView`, `AudioTrack`, touch and
physical-controller input, lifecycle callbacks, and `SharedPreferences`.
`pokered-mobile` owns the 160×144 frame, game update, PCM production, and the
versioned save envelope. The audio thread is joined before the native runtime
is destroyed.

## Validation on 2026-09-12

- `cargo test -p pokered-mobile` passed against the v0.7.0 candidate. The tests
  cover embedded graphics, RGBA frame output, nonzero intro PCM, committed-save
  restoration, invalid-save rejection, and soft reset.
- `scripts/build-android.py --assemble` cross-compiled the arm64 static library
  with NDK 27 and assembled the generated project with JDK 17 and Gradle 8.11.
- The clean APK installed on an Android 15 arm64 emulator. The GAME FREAK intro
  rendered, and a touch on A advanced to the bilingual language-selection
  screen without AndroidRuntime errors.

Physical-device audio and latency, suspend/resume stress, and on-device
save/relaunch remain release validation work.
