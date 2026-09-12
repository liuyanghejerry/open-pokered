# iOS development

iOS uses the same `pokered-mobile` ABI v1 as Android and HarmonyOS. The UIKit
and Metal host in `ios/Pokered/` owns input, display, lifecycle and audio
session integration. The Rust static library owns game state, the 160 × 144
RGBA frame, a committed JSON save, and the 44.1 kHz PCM queue.

Build an unsigned app with:

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
ios/build.sh
```

The script builds the device archive plus arm64 and x86_64 simulator archives,
joins the simulator archives with `lipo`, then invokes `xcodebuild`. The CI
workflow repeats those steps so the Xcode project cannot link a stale or
single-architecture library.

The host initializes the runner with `pokered:red:v1`, stores committed saves
as UTF-8 JSON in `UserDefaults` under `pokered-mobile-save-v1`, and exports a
save when the app resigns active or terminates. The former iOS-only
`pokered.sav` file is left in the app container and is not imported
automatically, because ABI v1 uses the shared committed-save envelope.

The audio callback calls `dotzuki_mobile_audio_fill` at 44.1 kHz. It must stop
before `dotzuki_mobile_destroy`, because the ABI requires the host to stop all
audio callbacks before releasing its runner.

For direct validation, build both host SDKs after running the script:

```bash
xcodebuild -project ios/Pokered.xcodeproj -target Pokered -sdk iphoneos -configuration Release CODE_SIGNING_ALLOWED=NO build
xcodebuild -project ios/Pokered.xcodeproj -target Pokered -sdk iphonesimulator -configuration Release CODE_SIGNING_ALLOWED=NO build
```
