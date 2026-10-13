# Native Touch Controls

> - **Audience**: rust developers, engine contributors
> - **Type**: how-to
> - **Status**: active
> - **Last verified**: v0.9.0

Add the shared mobile touch overlay to an iOS game, or change the controller
used by the Android and HarmonyOS exporters.

Before you start, read [Mobile Architecture](../../../docs/explanation/mobile-architecture.md).
The controller emits the [engine](../../../docs/reference/glossary.md#engine)'s eight input bits
without depending on a game, renderer, or Rust runtime instance.

## iOS integration

Add this directory as a local Swift package in Xcode and link the **DotzukiTouch** product.
The package requires iOS 15 or later and builds its Objective-C++ source with C++17.
Import `DotzukiTouch` in Swift, or `DotzukiTouchOverlay.h` in Objective-C.

1. Create a `DTZTouchOverlay` and add it above the game's rendering view.
   Constrain it to the host view's full bounds. The overlay applies `safeAreaInsets` internally;
   constraining it to the safe area again would apply the inset twice.
2. Assign `onInputChanged`. The callback provides the current touch bitmask:
   A, B, Select, Start, Right, Left, Up, Down occupy bits 0–7.
   For game ticks, read `consumeInput()` once per tick and merge its result with hardware input.
   This retains quick taps between frames; the callback alone reports held state.
3. Assign `onSettingsVisibilityChanged`. Pause game updates while the settings editor is visible.
   Resume only when both the editor is closed and the application's scene is active.
4. Call `releaseAll()` when host menus take input, when the scene resigns active,
   or before detaching the game. The overlay also releases on app inactivity,
   window focus loss, layout changes, canceled touches, and removal from its window.
5. Set `controlsEnabled = false` while a host modal owns input or a physical controller is active.
   Re-enable it when touch control becomes available again. This releases held buttons and
   hides the overlay; it does not change hardware input.
6. Enable both landscape orientations in the host's supported orientations. Keep the game's
   aspect ratio; draw backgrounds under the safe area while keeping controls inside it.

The overlay's gear opens a native editor. Settings persist in the app's `UserDefaults`
under `dotzuki-touch-v1`. The editor offers size, inward/vertical offsets, opacity, side swapping,
a floating direction pad, optional haptics, and reset. Haptics default to off.
The host retains ownership of status bars, Home indicator preferences, and system gestures.
The overlay does not disable system navigation.

The package supplies controls, not a game loop, audio, save storage, or an iOS exporter.
For SwiftUI hosts, embed the view through `UIViewRepresentable` and forward its two callbacks
into the host's input and pause state. Ensure a host view controller can present the editor.

## Android and HarmonyOS

The exporters install `include/dotzuki_touch.hpp` alongside the mobile C ABI header.
Android calls the controller through JNI; HarmonyOS calls it through Node-API.
The controls render above the full-size game surface, with four-direction movement at the
lower left, A/B at the lower right, Start/Select at the upper right, and settings at the upper left.
Side swapping mirrors the entire arrangement. The game framebuffer keeps its aspect ratio.

The shared controller operates in logical pt/dp/vp coordinates. Native hosts convert device
insets and pointer coordinates before calling it. Each finger owns its original control until
release. Movement uses an 18% central dead zone, a 1.22 axis-switch ratio near diagonals, and a
bounded tracking radius. Sliding from A to Start cannot activate Start. Two fingers holding A
keep A pressed until both release. Resize and cancellation discard ownership rather than
allowing old move events to press a button again. A frame sampler retains brief presses
between ticks without adding automatic repeat or queuing opposite movement directions.

The visible A/B circles sit inside their larger hit rectangles. Independent targets do not
overlap; menu targets are 64×48 logical units. Controls adapt to compact windows down to a
320×220 safe area. Below that size, controls remain disabled until the window becomes usable.
The floating pad only starts within the movement-side thumb region.

With the default vertical offset, fixed thumb controls leave 22% of the viewport
height below them for the runner's bottom dialogue, including on unfolded phones
and tablets. This reserve includes the bottom system inset once. Compact windows
reduce the reserve when needed to keep the thumb controls below the top buttons.
Moving controls downward or using a floating pad can place them in this band;
custom games with larger dialogue panels should check their own layout.

## Verification

Verification includes the shared sanitizer suite, Android emulator instrumentation,
HarmonyOS emulator interaction, and UIKit simulator tests. Physical HarmonyOS and
iOS device behavior still needs device testing.

The dialogue-clearance regression was checked on a HarmonyOS 6.1.1 API 24 arm64
emulator at 2584×1828, with the default layout and mirrored controls at scale 1.2:
[before](../../../docs/assets/harmony-touch/harmony-dialogue-before.png),
[after](../../../docs/assets/harmony-touch/harmony-dialogue-after.png), and
[mirrored](../../../docs/assets/harmony-touch/harmony-dialogue-mirrored.png).
These captures use the same NPC dialogue. Choice menus above the bottom dialogue
band can still overlap A/B; the generic mobile ABI does not expose their bounds.

Run the shared controller tests from the repository root:

```bash
workspace/crates/dotzuki-mobile/controls/tests/run.sh
```

The path above is relative to the repository root. The test runner uses C++17 and address/undefined
behavior sanitizers. Tests cover multi-touch ownership, continuous turns, dead zones, diagonal
hysteresis, cancellation, layout changes, settings activation, safe areas, and mirrored/floating
layouts. The repository's Linux CI runs this suite.

Native acceptance checks remain separate from the geometry tests: build the exported hosts,
exercise the editor, hold movement plus A/B, rotate in both landscape directions, interrupt the
app with system UI, and verify that return to play starts with every button released. Verify on
16:9 and ultrawide phones, a cutout device, and a resized tablet/foldable window. iOS integration
must also verify the host's scene-pause handling and controller handoff.

The Android native-view harness is `tests/android/TouchInstrumentation.java`.
`scripts/test-android-touch.py --help` describes how to run it against an exported project
and an adb device. The script builds an isolated copy with application ID
`com.dotzuki.touchregression`; the input project remains unchanged. It checks full-window
layout, pointer IDs after one finger releases, focus cancellation, and resize cancellation.
The installed regression app uses its own save and settings storage.

The iOS package contains XCTest integration tests under `tests/ios`; the
**iOS touch controls** workflow builds and tests the package on a macOS runner's iPhone simulator.
The UIKit tests validate Swift API import, layout hit testing, mirrored preferences, and
release callbacks, and attach a rendered overlay to the test result.
