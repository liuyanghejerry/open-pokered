# iOS development

The iOS app is a native UIKit/Metal shell over `pokered-mobile`'s shared
mobile ABI. It uses dotzuki's `DotzukiTouch` Swift package for its safe-area
aware touch controls and settings editor.

## Engine and controller sources

Cargo dependencies are pinned to the dotzuki `v0.10.0` release commit. The
`ios/DotzukiTouch` local Swift package is the matching
`workspace/crates/dotzuki-mobile/controls` directory from that same release;
the Xcode project links its `DotzukiTouch` product. Keep both revisions in
sync when upgrading dotzuki.

The overlay fills the host view, applies the safe area internally, and leaves
blank regions available to the game view. `GameViewController` reads
`consumeInput()` on each game tick so quick taps between display frames reach
the game. Opening the native controls editor pauses the frame loop; closing it
resumes only while the app is active. Touch state is released on app
deactivation and before the runtime is destroyed. The Metal viewport preserves
the 160×144 framebuffer aspect ratio, and the host supports both landscape
orientations.

## Build

Build on macOS with Xcode and the iOS Rust targets installed:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
scripts/fetch-gfx.sh
ios/build.sh
```

The script builds device and simulator static libraries, then builds the Xcode
app without signing. The touch overlay package has its upstream XCTest suite
under `ios/DotzukiTouch/tests/ios`; verify it in an Xcode iOS simulator when
changing or upgrading the native controller.
