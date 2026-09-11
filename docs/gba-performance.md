# GBA performance investigation

Measured on the release ROM in mGBA with GBA timer 2 at CPU/64. One timer
tick is about 3.815 µs and one video frame is exactly 4,389 ticks. Enable the
instrumentation with:

```bash
cd crates/pokered-gba
cargo +nightly build --release --features profiling
agb-gbafix target/thumbv4t-none-eabi/release/pokered-gba
mgba -1 -C logToStdout=1 -C logLevel.gba.debug=127 \
  target/thumbv4t-none-eabi/release/pokered-gba.gba
```

## Findings

The original startup frame averaged 29,500 ticks (about 112.5 ms, or 8.9
FPS). Game update took only 13 ticks; almost all time was spent redrawing the
software framebuffer and converting/copying it to Mode 3 VRAM:

| Stage | Original | Optimized representative frame |
| --- | ---: | ---: |
| Game update | 0.05 ms | 0.05 ms at boot; about 5.4 ms per Overworld step |
| Software draw | 76.7 ms | 11–15k ticks for a changed Title/Oak frame; 26–28 ms moving Overworld |
| Present | 35.4 ms | 3.1–3.2 ms |
| Changed Overworld outer loop | not reachable (allocation failure) | 50.1 ms / about 20 FPS |
| Unchanged ordinary Overworld | not reachable (allocation failure) | 4,351 ticks / about 59.7 Hz |

The old presenter also decoded the planar 2bpp framebuffer as four adjacent
chunky pixels. Besides being slow, that produced repeated/garbled glyphs in
the actual GBA output. The replacement uses Mode 4 palette indices and two
pages, with the completed page flipped at VBlank.

## Changes made

- Added opt-in hardware-timer profiling. The normal ROM no longer writes a
  framebuffer snapshot to SRAM every 60 frames; `framebuffer-dump` retains
  that diagnostic when explicitly requested.
- Added an exact O(1) path for the standard four-shade grayscale quantizer.
- Added a generated 128-entry ASCII glyph-offset table, avoiding a binary
  search through roughly 25,000 glyphs for every Latin character.
- Optimized packed planar clears and rectangle fills.
- On bare-metal ARM only, traded the 5,760-byte planar framebuffer for a
  23,040-byte, 32-bit-aligned index buffer. This costs 17,280 bytes of EWRAM
  but allows direct Mode 4 DMA.
- Replaced per-pixel VRAM conversion with one DMA3 transfer per viewport row.
  Presentation fell from 35.4 ms to about 3.05 ms (roughly 91%).
- Removed duplicate full-screen clears from the Game Freak and language
  selection render paths.
- Added a tile blit API that quantizes a four-entry palette once per tile.
  Identity-palette GB backgrounds use a GBA-specific row-copy path that writes
  decoded 2-bit indices directly to the Mode 4 staging buffer. Title and Oak
  tiles now use the same path automatically; transparent sprites only test and
  skip source index zero.
- Word-aligned decoded tiles keep their original 64-byte size. On GBA, the
  opaque row-copy path now emits two 32-bit writes for word-aligned targets or
  four 16-bit writes for halfword-aligned targets, with a byte-copy fallback
  for incompatible row strides or addresses. The copy shape is selected once
  per tile, and framebuffer widths other than 160 remain safe.
- The Overworld renderer now culls off-screen margin tiles, reuses the current
  map metadata instead of formatting and looking it up for every out-of-bounds
  tile, and indexes the selected blockset directly. Its background pass fell
  from about 52,000 to 5,700 ticks (roughly 90%).
- The GBA resource manager checks its decoded cache before scanning the asset
  registry and remembers immutable misses. Boot assets are released before
  entering the Overworld.
- Fully opaque Normal layer stacks composite directly into the destination,
  avoiding the 92 KiB RGBA scratch allocation that cannot fit in GBA EWRAM.
- Kept game simulation tied to the 59.7 Hz hardware clock. When a dynamic
  redraw exceeds one video frame, cheap update steps catch up independently
  instead of slowing the whole game. Fully static splash and language-select
  frames reuse the displayed page and therefore remain at VBlank rate. Title,
  Main Menu and Oak also retain the last page until their visual state changes,
  so pauses and typewriter delay frames no longer rerasterize an identical
  screen.
- Added a conservative visual key for the ordinary Overworld view. When the
  map blocks, player/camera state, and visible NPC state are unchanged, the GBA
  skips both software drawing and presentation. Dialogues, menus, scripted
  movement, fades, and other complex overlays deliberately bypass this cache
  and continue to redraw every frame.
- Removed the remaining duplicate clears from Intro, Title, Oak and Overworld.

In the measured boot sequence, static phases now advance at about 59.7 Hz.
Dynamic splash frames now fit close to one video frame. A changed Title/Oak
frame fell from roughly 19,000–22,000 draw ticks to roughly 11,000–15,000,
while unchanged frames perform no draw or present work. A steady software-
rendered Overworld frame now costs about 6,904–7,270 draw ticks (26.3–27.7 ms),
with an outer loop rate around 20 FPS once simulation catch-up and VBlank
synchronization are included. Before the aligned row-write change, the same
movement windows cost 7,722 and 8,158 ticks respectively, so the final tile
write optimization reduces changed-frame draw time by 10.6%–10.9%. An unchanged
ordinary Overworld view performs no draw or present work and completes in about
4,351 ticks (16.6 ms), sustaining the hardware's 59.7 Hz cadence. The same
changed-frame path previously spent about 55,000 draw ticks and could not enter
the Overworld before the layer allocation was removed.

## Invalid-address crash

The `Jumped to invalid address: F901F900` failure was a corrupted return
address, not a deliberate jump. Selecting NEW GAME constructed a complete
`SaveData` temporary inside the already-large update state machine; the next
transition then constructed a second `OverworldScreen`, and the renderer tried
to allocate a full 160×144 RGBA layer buffer. Together these exceeded the GBA
stack/EWRAM budget.

The GBA path now clears the large save arrays in place, reuses an Overworld
screen prebuilt for the new-game map, performs screen transitions after the
update stack has unwound, and renders opaque layers without an RGBA scratch
buffer.

A second occurrence at the bottom edge of Pallet Town had a separate allocation
trigger: the first connection lookup lazily built all 248 map-connection entries
and a string-keyed map-name index. On GBA, connection data is now built only for
the requested map and map names are resolved by scanning the generated ROM
table, avoiding both heap structures. The movement regression crosses that map
edge in two directions, and the emulator soak continued through more than 8,000
simulated frames without another crash.

## Dotzuki dependency

The reusable no_std and renderer work lives in dotzuki PR #63 on the
`feat/gba-renderer-performance` branch (through commit `dcacf65`). Every
open-pokered consumer is pinned to that remote revision, so CI and independent
checkouts do not require the sibling repository or new vendor changes.

## Remaining bottleneck

Full-scene software rasterization remains dominant when pixels actually
change. Overworld motion is still about 20 FPS and cannot produce one fresh
frame per VBlank. The next large improvement should reduce the work within a
changed frame:

1. retain a static Overworld background and redraw only scrolling edges,
   animated tiles, objects, and damaged regions;
2. batch remaining monochrome glyph writes and transparent sprite spans;
3. ultimately map backgrounds and sprites to native GBA tile/OAM hardware
   instead of treating the device as a software framebuffer.

The before/after captures used for visual verification are
`docs/screenshots/gba-performance-before.png` and
`docs/screenshots/gba-performance-after.png`.
