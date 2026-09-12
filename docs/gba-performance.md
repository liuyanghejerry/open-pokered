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

## Architecture follow-up (2026-09-12)

The sections below preserve the early investigation's measurements. They are
historical, not a claim that the current moving Overworld still runs at 20 FPS.
PR #77 contains the subsequent incremental-rendering A/B measurements.

This follow-up retains those optimizations while restoring shared contracts:

| Concern | Shared source / owner | Retained optimization |
| --- | --- | --- |
| Battle hook topology | All targets install hooks from the same compiled RON definitions, including ordering and event routing | Equal complete hook slices share one allocation; no GBA-only subscription list |
| Hot menu layouts | `battle_main.gui`, `battle_safari.gui`, `options.gui`, `save.gui` feed both editor JSON and build-generated static layouts | No JSON parsing or layout-tree allocation in the static draw path; bindings may still format dynamic strings |
| Frame reuse and damage | `pokered-app::render::RenderSession` owns visual keys, caches and redraw decisions | GBA consumes `Reuse` / `Full` / `Damage` and retains MMIO, DMA and page presentation |
| Framebuffer representation | Dotzuki's explicit packed/linear types have the same contracts on every target | The GBA adapter selects word-aligned linear indices; packed remains the default |
| Map metadata lifetime | `MapJsonHandle` retains borrowed hosted data or shared embedded data | Four-entry recent-map cache; evicted maps are released after the last live handle; block bytes borrow ROM |
| Platform synchronization | `pokered-platform` owns the hosted/bare-metal synchronization contract | Four game crates share one implementation; recursive bare-metal locks/initializers fail instead of creating mutable aliases |
| Native script command parsing | Dotzuki's native-AST `core_host` owns the generic async `game.*` catalog, argument validation and `ScriptCommand` construction | Pokered's host keeps only stateful queries/RNG and Pokémon-specific extensions; no second engine protocol table in the game |
| Script capability contract | Dotzuki walks structured scene ASTs; `pokered-data` owns the Pokémon-specific capability catalog | Every scene is validated during the data build, and tests require both Boa registration and the native GBA host to cover that catalog |

The static GUI compiler is generic dotzuki functionality, not a Pokemon-specific
layout table. It lowers the normal GUI compiler output (after component expansion),
and rejects unsupported properties or expressions at build time. Static and
dynamic rendering share text/cursor primitives and menu bindings. Battle, Safari
and save cursor positions/glyphs are read from the generated layout as well.

The follow-up commit also moved every English/Chinese Options cursor coordinate
into `options.gui`. Runtime bindings now expose only semantic state (active row,
language and selected value), and both full rendering and incremental cursor
updates resolve position and glyph from the generated layout. The UI crate owns
the cursor damage footprint for all four compiled menus; `RenderSession` only
converts that frontend-neutral rectangle to its presentation type.

Validation:

- Core: 2,559 unit tests passed. UI: all 8 tests passed, including static/dynamic
  parity for the four menus, both languages and all enumerated states.
- App: 94 unit tests passed, including incremental options rendering versus a full
  draw, changed-pixel damage coverage, reuse and black-screen invalidation.
- Dotzuki renderer: 474 default-feature unit tests passed. Packed/linear storage
  is checked pixel-for-pixel across odd dimensions, fills, overlapping copies,
  scrolling, clipped/flipped/transparent blits; direct binding semantics are
  checked against the dynamic context. Static compiler rejection/source tests pass.
- Embedded metadata test visits every generated map, checks the four-entry bound,
  and proves an evicted live handle remains valid and is freed after release.
- Native app, TUI and web-crate checks and the production GBA release build pass
  with the pinned remote dependency and no local patch configuration (the web
  check is not a wasm-target test).
- Dotzuki's native dispatcher, capability validator and `return` control-flow
  semantics pass all 295 DSL crate unit tests. Pokered data passes 250 native
  and 259 Boa-feature tests; both script backends are checked against the same
  Pokémon-specific capability catalog.
- Fresh-start playthrough m01–m10 passes through defeating Brock. Seeded scenarios
  pass 10/11: `s07-save-roundtrip` fails waiting for CONTINUE to reach Overworld.
  It fails twice on this change and also on unmodified PR head `383c82c`, so it is
  recorded as a pre-existing failure, not waived or reported as passing.
- Release GBA autopilot/profiling ROM runs through 159,396 simulated frames without
  panic, allocation failure or invalid-address crash. This exercises boot, Oak,
  scripted bedroom movement and then a long idle period, not 159k frames of broad
  gameplay. Idle still reports zero draw/present work at hardware cadence.
- The shared-dispatcher integration was additionally soaked through 99,456
  simulated frames without a capability error, panic or invalid-address jump.
- Representative movement draw windows remain around 1.8k–2.1k ticks in both the
  `383c82c` baseline and this follow-up. Profiling mark 3 now includes damage-list
  assembly, and window phases/render counts differ; these are smoke measurements,
  not a controlled percentage improvement or proof of no regression in every scene.

Visual comparisons under `docs/screenshots/gba-architecture-*` use master
`bb6df8b` as before and the refactor as after, at screenshot frame 10. Options EN
and save ZH PNGs are byte-identical. The battle target is a transition frame;
its difference from master already exists at `383c82c`, whose PNG is byte-identical
to the refactor's battle image. It is not a battle action-menu screenshot.

The resource catalog is now target-independent: asset categories, canonical path
parsing, Pokémon sprite dimensions and the typed named-loader API have one source
of truth. A small compile-time macro emits that API for both resource managers;
only the filesystem/PNG provider and the preconverted ROM registry/cache remain
platform-specific. Script representation APIs are also feature-gated now: native
builds expose scene-AST loading, while Boa builds expose raw-JavaScript loading;
unsupported engine/custom commands produce an observable error effect instead of
silently succeeding as `Void`. Incremental-menu damage footprints now live beside
their UI/renderer implementations (including Town Map's layered regions), while
`RenderSession` only chooses between reuse, full redraw and those declared regions.
Pokémon-specific script extensions now cross the engine boundary through one typed
`PokemonScriptCommand` catalog: Boa and native producers construct typed values,
the bridge decodes and validates the same schema, and malformed payloads become
observable capability errors. Generic async commands are now parsed by dotzuki's
shared no_std native-host dispatcher. Its exported static command catalog can also
be consumed by build-time capability validation; pokered no longer copies movement,
audio, object, shop or scene command schemas into its native host. The build now
walks every structured scene AST and rejects undeclared `game.*` calls with source
locations. Introducing that check exposed and fixed two dormant semantic gaps:
bare `return` had been compiled as a host call, and a structured Viridian City
command incorrectly used the raw-JS-style `game.` prefix. The same contract audit
also exposed the native backend's missing `getGameVersion` query, which is now
implemented and guarded by catalog-coverage tests for both native and Boa engines.
The cursor erasure fast paths still assume the current arrow's 8×9 ink footprint
and plain background; this is now an explicit UI-owned damage contract, but it
is not inferred from glyph metrics. Changing those authored shapes requires
updating that contract and its parity coverage. `RenderSession` is desktop-testable but desktop presentation
does not yet opt into it, and the deferred-transition protocol remains separate.

## Historical findings

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
`feat/gba-renderer-performance` branch (through commit `ff66260e9c9382fd3aa53afc05dc2e1ce57c61e0`). Every
open-pokered consumer is pinned to that remote revision, so CI and independent
checkouts do not require the sibling repository or new vendor changes.

## Historical remaining bottleneck (before incremental rendering)

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
