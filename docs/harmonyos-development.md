# HarmonyOS development

The game implements `dotzuki_mobile::MobileGame` in `pokered-mobile`. The engine owns the C ABI, PCM queue, frame pacing, Native/ArkTS host, and export template. Pokemon owns the 160×144 indexed framebuffer, game audio commands, initialization payload and committed save format.

All dotzuki dependencies are pinned to v0.7.1. The build script still writes
an explicit local Cargo patch so the game adapter and the engine-owned host use
the same checkout while developing platform changes. Do not mix engine crates
from different revisions.

From the repository root, with Python 3.11+, Rust's `aarch64-unknown-linux-ohos` target and DevEco's SDK installed:

```sh
python3 scripts/build-harmony.py \
  --engine-checkout /path/to/dotzuki \
  --native-sdk /path/to/sdk/openharmony/native \
  --out dist/harmony
```

Fetch `gfx/` with `scripts/fetch-gfx.sh` first. The export directory must be empty to protect edits to a generated host. The script leaves engine source untouched, builds `libpokered_mobile.a`, and calls the engine's host exporter. `--version blue` selects Blue. Use DevEco to build/sign the generated `entry` module; unsigned debug HAPs are only suitable for an emulator configured to allow them.

The generated `target/harmony/engine.toml` also supports local validation:

```sh
cargo test --config target/harmony/engine.toml -p pokered-mobile
cargo run --release --config target/harmony/engine.toml --bin pokered-app -- screenshot-all -o target/harmony/screenshots
```

The host's startup payload is `pokered:red:v1` or `pokered:blue:v1`, not a `.dzpk` project, even though the shared template retains that resource filename. Saves are UTF-8 JSON envelopes containing version 1, Pokemon SaveData and companion script flags. Menu SAVE commits a new envelope; backgrounding does not snapshot unsaved progress. The host polls committed saves every 500 ms and flushes them to Preferences, and also attempts a flush on page hide. Abrupt termination before the asynchronous flush finishes can lose the most recent commit. Soft reset restores the last committed envelope.

The desktop frontend retains its default GPU/device-audio behavior. Mobile
builds disable desktop features and use embedded graphics. Android now uses
the same `pokered-mobile` adapter and ABI version 1; iOS still uses its
game-specific ABI.

## Validation on 2026-09-12

The original HarmonyOS validation used engine baseline `341ca857` and game
baseline `746f7dd`. Android shared-host validation used the v0.7.0 candidate
at `6cbe0c2` and this game branch.

- Shared runtime, SPSC queue, existing runner real-pack tests and Harmony exporter tests passed.
- Pokemon mobile tests passed: embedded title/sprite loading with an empty filesystem asset root, 160×144 RGBA, non-silent PCM, explicit SAVE, script flags on Continue, invalid-save rejection and soft reset.
- Desktop starter-selection and language-selection regression tests passed. Web target compilation passed.
- All 12 desktop screenshot-all outputs are byte-identical to the baseline. Hashes are in `screenshots/shared-mobile-host/desktop-comparison.json`; title, battle and overworld before/after PNGs are retained alongside it.
- ARM64 OHOS static library and unsigned HAP built successfully with the installed DevEco SDK. Installed and launched on the local Pura 90 API 24 emulator; title graphics and touch navigation to the main menu were verified. Captures: [title](screenshots/shared-mobile-host/harmony-title.jpeg), [menu](screenshots/shared-mobile-host/harmony-menu.jpeg).

The emulator captures demonstrate the new platform; there is no baseline Harmony build of this game. Full playthrough, physical-device audio/latency, suspend/resume stress and on-device save/relaunch testing remain release validation work. Android now builds through the shared host and has been verified through the language-selection screen on an Android 15 arm64 emulator. iOS migration remains a separate follow-up.
