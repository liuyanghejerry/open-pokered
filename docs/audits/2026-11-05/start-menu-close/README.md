# START close: restore the map before returning control

Closing START with B, START or EXIT previously switched straight to the field. The shared runtime now executes `CloseStartMenu` and its `CloseTextDisplay` resource work, retaining the menu/map rasters and the foreground input owner until completion. Textbox patterns, sprite reloads, map upload, OAM and IRQ work determine the duration; there is no scene-total countdown. This is one independently reviewable part of #144, built on #439.

Implementation: `c482de53f4464fe45329fb3a31cbd3a06274c990`. Before: master `a718349657a5a75ec559dd29870b93d4397775d0`. All 2,457 crate/Cargo input files of that master matched the frozen before build. The before/after initial menu screenshot hashes are identical for every case.

## Observed behavior

The reference is normal Red built from pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, ROM SHA-256 `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`. PyBoy runs real button inputs from a prepared state; recording performs no RAM or ROM writes. Native runs cold-boot the included SRAM through CONTINUE, with PCM synthesis active and synchronous `press_timeline` frames. The reference state and native SRAM are different formats, not interchangeable files.

At Viridian City (20,30), close with B on frames 0–1. The neutral control releases all buttons; the short control holds Right on frames 3–4; the held control holds Right on frames 3–44. Each records 80 consecutive logical frames and repeats twice. The retained records include complete coordinate/input series, selected native screenshots and all original raw RGBA hashes.

| Observation | Original | Before master | Candidate |
| --- | ---: | ---: | ---: |
| Return to field loop/screen | 29 | 0 | 29 |
| Final X, neutral or short | 20 | 20 | 20 |
| Final X, held Right | 21 | 23 | 21 |
| First logical X change, held Right | 46 | 21 | 48 |

The candidate preserves facing as well as position through short direction pulses. Held START does not reopen the menu, and held A on EXIT does not wait for release. RedsHouse2F, dark RockTunnel1F and Safari each passed B/held START/held EXIT twice, followed by 18 separate-process CONTINUE checks. Darkness and Safari counters are preserved.

The default release GBA ROM (`9b8fa1e5db1d46cbf3cc0585032a5bb198d93807f8a4deb225a0acd4e0bfe91d`) passed three physical-input cases twice, recording 600 frames each. Every frame hash repeats; neutral and short-direction controls have identical image series. The game can then walk and reopen START. These use ordinary SRAM, no autopilot, state setters or frameskip.

## Validation and scope

Local application tests: 1,090 successful test executions, 92 existing ignored. Core: 3,982 successful executions, one existing ignored. Targeted menu tests, native debug build and ARM check passed. Frozen-source guards confirmed that builds did not modify the protected working tree or earlier frozen binaries. A fresh NEW GAME → m49 run is pending and is not claimed complete here.

The resource/input repair is verified. Overall animation equivalence remains **PARTIAL**: subsequent coordinate commitment is two logical frames later than the reference, and complete actor/camera trajectories have not been declared equivalent. GBA physical elapsed time is not original Game Boy timing. Fonts and dialogue layout remain the project's intentional differences. Native grass input is sampled from the current stationary map; general NPC grass masking is a separate pending fix.

Replay the native controls with a freshly built debug-server binary:

```bash
python3 docs/audits/2026-11-05/start-menu-close/replay-native.py \
  --binary /absolute/path/to/pokered-app --sha256 <binary-sha256> \
  --output /absolute/path/to/new-output --candidate
```

`verification.json` records hashes and validation scope. Screenshots under `docs/screenshots/start-menu-close-1390/` compare the same input offsets (4 and 79) without phase resampling.
