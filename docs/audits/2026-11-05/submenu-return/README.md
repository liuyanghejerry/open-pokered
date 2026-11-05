# Bag / Options return restoration

Cancelling Bag or in-game Options now waits for the original textbox-pattern transfer before returning to START. A short Down press during that work no longer selects the next START entry. The transfer runs through the existing source IRQ, with audio and the game timer active; the old submenu raster stays visible until the handoff. Options also plays the original exit confirmation sound. Title-screen Options still returns to the title menu.

This is a focused extraction from #144, built on #443. Input locking and completion pass; complete original visual parity remains **PARTIAL**. In particular, the original BG portion order and retained actor publication at t+6..8 still differ. This change does not claim to finish #144.

## Evidence

`verification.json` records exact binary/ROM/fixture hashes, local checks, and two identical recordings per case. Each compressed record keeps the full ordered frames and inputs, including pre-trigger state. `before` is #443 head `55b93a584b5621c6f32caf130b7aa9b11a30718c`; `after` is production commit `9ba0d3cc`, synchronized with that exact parent in `83739c5895b6e00494a09133373ef567075d3e23`. The recorded before/after binaries and default GBA ROM are from these current sources.

| Observation from B input, t+0 | Original Red | Before | After |
| --- | ---: | ---: | ---: |
| Logical START redisplay/handoff | 5 | 0 | 6 |
| START accepts input | 9 | 0 | 9 |
| Bag + short Down final START cursor | ITEM (2) | TRAINER (3) | ITEM (2) |
| Options + short Down final START cursor | OPTION (5) | EXIT (6) | OPTION (5) |

Four cases (Bag/Options, neutral/short Down at t+2..3) each have 80 consecutive original and native frames, repeated twice. Native boots an ordinary SRAM file, opens START and the submenu using actual buttons, and never changes state through the debugger. Every PNG hash and state sequence matches its repeat. Both native builds have identical pre-trigger images. Position/facing/inventory and SRAM remain unchanged; A reopens the intended submenu after the wait.

Original reference: normal Red ROM from `pret/pokered` commit `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, PyBoy with audio enabled, 160×144 at original VBlank cadence. It loads the recorded Party machine state and uses real inputs to reach the submenu. Native also renders 160×144, one driven update per captured frame, with the audio sequencer running without a host audio device. Fonts, menu layout, inventory and actor fixtures differ; no whole-image equality against the original is claimed.

Default GBA release (nightly-2025-12-07, fat LTO, no extra features): four cases × two repetitions × 800 consecutive **physical** frames in mGBA 0.10.5. Cold SRAM and physical button masks only. Full frame hashes repeat, the selected START cursor survives, A reopens the same submenu, and cartridge SRAM read through `savedataClone` is unchanged. These observations establish completion and storage/input behavior, not original GB animation speed.

## Local validation

- Targeted submenu tests cover outdoor/indoor maps, the actual resource copy, short input rejection, cached/full rendering, option persistence and title Options.
- Full app (1,110 test executions) and core (3,991), ARM check, and default GBA build passed; commands and existing ignored cases are in `verification.json`.
- Fresh NEW GAME through m06 passed in 41.07 seconds with the binary/source immutable. This is the scoped early-game regression for a menu-only change. The parent START/Party work has separate full first-clear evidence.

After synchronization, submenu/SAVE/shared Bag and core restoration tests plus ARM/default GBA builds pass. The unchanged seven-scenario GBA performance gate passes (idle 380, movement 392 ticks). All native and GBA recordings above were repeated on the synchronized build. Full-suite evidence is retained from `9ba0d3cc`; the parent additionally passed 1,104 app and 3,991 core tests with the empty-NPC optimization.

## Replay

From a checkout with the recorded binary available:

```sh
python3 docs/audits/2026-11-05/submenu-return/replay-native.py --binary /path/to/pokered-app --sha256 HASH --output /path/to/new-output --candidate
python3 docs/audits/2026-11-05/submenu-return/replay-original.py --rom /path/to/pokered.gbc --symbols /path/to/pokered.sym --output /path/to/new-output
python3 docs/audits/2026-11-05/submenu-return/replay-gba.py --rom /path/to/pokered-gba.gba --sha256 HASH --bridge /path/to/mgba-sram-bridge.so --output /path/to/new-output --case bag-short
```

The original needs PyBoy. Build the read-only bridge against mGBA 0.10.5 headers/library, including `mgba/flags.h`; the existing `save-return` bridge is the same source. Fixtures are reused from `start-menu-close/fixture.sav` and `start-menu-close/fast-cancel/reference.state`; no duplicate SRAM or full build archives are committed. Use separate output directories for each repetition. Omit `--candidate` to record the before binary.
