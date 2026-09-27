# GBA frame pacing audit — 2026-09-28

Baseline: master `198a0b957220f1fb7b4fb126e357aa614be162e3` (merged PR #104).
Candidate: `fix/gba-frame-pacing`. Toolchain: nightly-2025-12-07; mGBA 0.10.5.
Measurements use emulated ARM7 hardware cycles (Timer 2/3, 262.144 ticks/ms),
not host wall time. These are emulator results, not a physical GBA measurement.

## Cause and changes

The frontend previously ran up to eight updates before one render when it fell
behind. Intermediate connection-scroll and move-animation states never reached
the display. It now performs at most one update per draw, retains the fractional
clock remainder and discards accumulated whole ticks. A slow effect therefore
runs longer rather than losing its visible frames; this is not a 60 fps claim.

Map registration also spent substantial ARM7 time hashing strings with software
64-bit multiplication, copying function names, scanning all map functions and
parsing JSON bindings. GBA now uses a 32-bit word hasher, borrowed ROM names,
indexed function ranges and build-time generated typed bindings. Hosted builds
retain their original hasher/config loader. Moving destination NPC previews now
preserve their background underlay and permit retained overworld rendering.

Battle fonts/HUD tiles are expanded into 16 KiB of ROM; no decoded heap cache is
added. Tile rows and aligned transparent mon rows use word copies in IWRAM.
OAM scanline selection uses a ten-entry stack array instead of allocating a Vec
for each scanline. Normal play disables Info log formatting.

## Before/after results

The diagnostic fixture walks both connections, feeds the same pending wild
encounter consumed by grass rolls through the production update path, then
plays ten moves from both sides. The wild case excludes the random grass roll;
battle construction and the entire encounter transition are measured.

| Scenario | Maximum gap between draws, before → after | Other measurement |
|---|---:|---|
| Pallet → Route 1 | 66.97 → 50.22 ms | Maximum update: 36.03 → 18.06 ms |
| Route 1 → Viridian | 117.20 → 66.96 ms | Maximum update: 80.19 → 32.66 ms |
| Wild entry | 100.46 → 83.71 ms | Maximum draw: 82.81 → 69.48 ms |
| Tackle, player | 70.74 → 35.42 ms | Mean draw: 58.23 → 24.92 ms |
| Ember, player | 70.76 → 50.23 ms | Mean draw: 56.32 → 26.16 ms |
| Thunderbolt, player | 100.46 → 100.46 ms | Mean draw: 67.35 → 38.75 ms |
| Explosion, player | 117.20 → 83.71 ms | Mean draw: 69.58 → 39.06 ms |

All 23 candidate scenarios have at most one update between draws. Old-loop
Thunderbolt displayed 27 draws covering 129 logic ticks; the candidate displays
all 127 ticks (the old loop can overshoot completion before the fixture sees it).
Heavy effects still have slow frames, particularly Thunderbolt, and map/encounter
loading still causes a visible pause. This change reduces those pauses and
prevents animation skipping; further renderer work is needed to eliminate them.

`before.log` / `after.log` contain all samples and completion markers; their JSON
summaries are produced by `scripts/gba_frame_timing.py`. `baseline-harness.patch`
applies only the diagnostic feature/fixture/timer hooks to master in a separate
worktree. The baseline retains its original catch-up loop and Info logger.

Captured LCD screenshots were tagged with the committed page's scene and
relative logical tick via `FRAME_TIMING_VIEW`, updated at the VBlank flip.
Across connection ticks 1–24, wild-entry ticks 1–200, Tackle and Thunderbolt,
198 common captured frames are byte-for-byte equal in RGB. The screenshots
under `docs/screenshots/2026-09-28-gba-frame-pacing/` use matching logical ticks:
connection 10, wild entry 100, Tackle 5 and Thunderbolt 12. Equal stills establish
pixel preservation; frame counts/timing logs establish the skipped-frame fix.

## Regression checks

- GBA broad memory suite: all 13 cases pass (302 dex pictures, 248 maps, Oak
  scripts, fossil, slots, menus, 330 move/side cases, evolution, trade, ending,
  300 boxed Pokémon and full SRAM round-trip). Minimum contiguous heap: 11,560 B;
  untouched stack: 16,520 B, both above the 4,096 B gate.
- GBA Route 22: all 40 encounters pass, before/after parcel and both coordinate
  triggers, including SRAM round-trip. Minimum heap: 23,712 B; stack: 17,184 B.
- Hosted core: 2,903 tests; app: 284; renderer with GBA resources: 392;
  data: 257; platform: 3. App GBA-resource library coverage: 99 tests.
- Python performance/memory/frame-pacing tools: 24 tests.
- Normal playable ROM: 3,000 hardware-frame boot smoke completed. IWRAM 5,720 B;
  initialized EWRAM 30,300 B; BSS 65,856 B (includes the 64 KiB main stack).
- First-clear playthrough: result recorded in the PR validation section.

The existing seven-window performance baseline is re-recorded because removing
catch-up changes the rendered workload. For example the trainer-entry window
now draws 224/500 samples versus 114/389: average presentation work rises when
previously invisible frames are shown. Draw cost per rendered trainer frame
falls from about 5,989 to 5,455 timer ticks. The original baseline is preserved
in `performance-before.json`, candidate in `performance.json`; the existing
15%/25-tick allowances are unchanged. CI also runs the new 23-scenario gate
which rejects multiple simulation updates between draws.

## Reproduction

```sh
cd crates/pokered-gba
cargo +nightly-2025-12-07 build --release --features frame-timing
agb-gbafix target/thumbv4t-none-eabi/release/pokered-gba -o /tmp/frame-timing.gba
cd ../..
python3 scripts/gba_frame_timing.py --rom /tmp/frame-timing.gba \
  --log /tmp/frame-timing.log --output /tmp/frame-timing.json
# To reparse the committed evidence:
python3 scripts/gba_frame_timing.py \
  --log docs/audits/2026-09-28-gba-frame-pacing/after.log --output /tmp/after.json
```

The recorder creates an isolated temporary ROM/save path. Use `--allow-skips`
only for baseline evidence. The diagnostic feature and autopilot must be absent
from the playable ROM.

Playable ROM SHA-256:
`2601e080118be57ea383ac76c19e70e27cd516b3ede8656e33c6d974a8edb5b8`.
