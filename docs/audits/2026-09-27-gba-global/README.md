# GBA global memory regression evidence — 2026-09-27

All constrained-target runs use nightly-2025-12-07 and libmgba 0.10.5.
The before cases are incremental reproductions while developing this audit,
not copies of the same baseline ROM. The branch baseline is `5162065`.

## Confirmed failures

- `pokedex-before.log`: the uncapped graphics cache exhausts memory while
  opening entry 38. The minimal driver opens all 151 owned entries. Matching
  screenshots at emulator frame 30,000 are in
  `../../screenshots/gba-pokedex-memory-{before,after}.png`; the after image
  uses the initial bounded-cache fix. The final suite also repeats in Chinese.
- `oak-before.log`: Oak's post-Pokédex dialogue deserializes all ratings into
  one AST and exhausts memory. This excerpt only claims the dialogue
  reproduction; the prototype's earlier map sweep was subsequently corrected
  to wait for and assert each committed destination.
- `seismic-toss-before.log`: the first Seismic Toss animation (move 69)
  exhausts memory while a late Route 22 battle script remains suspended.
  The effect cloned the 23,040-byte framebuffer.
- `full-save-route22-before.log`: after visiting all maps and filling storage,
  expanding both Route 22 encounters (and cloning the selected branch) still
  exhausts memory before the first dialogue. The native selector now loads
  only the eligible body; all 256 flag/result/row/language combinations match
  the full original script.
- `maps-before.log`: actual sequential map loads accumulate previous maps'
  trigger bindings; allocation fails on map 88, Bill's House. Both current
  map ID and loaded map-data ID are asserted between warps.

## Workload definitions

`memory-scenarios` uses the production GBA update/render loop, with seeded
fixtures. It is a memory stress suite, not an unassisted story playthrough.

- Open all 151 Pokédex entries twice, in English then Chinese, without
  clearing the resource cache between entries.
- Retain six party members, 240 stored Pokémon and 50 six-member Hall of
  Fame records throughout the remaining cases.
- Load all 248 map IDs (including unused/copy IDs), waiting through each
  transition and asserting the loaded map. Map scripts and rendering run;
  this does not exercise every NPC or story branch.
- Deliver Oak's Parcel through the NPC dialogue; run all 16 rating bands
  and the fossil scientist's no-item dialogue. Separately, hosted tests
  compare the optimized Oak branches against the full original AST for
  every owned count 0–151, both languages and story/item combinations.
- Complete three slot-machine spins and exits before later battles;
  this case also guards against restoring their previous permanent 4,608-byte
  decoded allocation. The final renderer reads the tile bytes directly in ROM.
- Open overworld/start/party/six stats/bag/town map/trainer card screens four
  times. Play all 165 move animations from both sides during a real late
  Route 22 battle, retaining its paused script. These are animation command
  streams, not 330 complete battles or every move's battle-logic branch.
- Play three evolutions and three seeded trade movies after leaving the
  battle fixture; run the Hall of Fame ceremony and credits; browse all
  300 recorded Hall of Fame Pokémon in the PC viewer; write and reload a
  full SRAM save, asserting party, storage and Hall of Fame counts.

`scenarios-after.log` completes all **13 workloads**, including the full SRAM
roundtrip. Minimum sampled contiguous free block: **13,256 B**; untouched
stack: **17,656 B**.

`route22-after.log` independently completes 40 battles, rival exits and a
real SRAM write. Minimum sampled contiguous free block: **21,560 B**;
untouched stack: **18,336 B**.

`performance.json` and `performance-compare.log` contain all seven existing
hardware-timer windows. The original 15%/25-tick regression budget is unchanged.

Hosted library results: **391 renderer**, **99 app**, **2,580 core**, and
**256 data** tests pass, plus **19 Python gate tests**. The renderer/app runs
include `--features gba-resource-tests` so actual ROM decoding paths are tested
on the host. Full command/flag traces are checked for **688 Oak** and
**256 Route 22** cases; pixels are compared for transitions, shifts, raster
rows, cache reloads and slot-machine flashes.

The final normal ROM is rebuilt without diagnostic features and boots for
3,000 emulator frames without panic. Its SHA-256, byte size and ELF sections
are recorded in `result.json`. Raw EWRAM remaining for the allocator is
165,988 B before alignment/metadata. The 64 KiB stack is already accounted
for in `.bss`. Runtime headroom is measured separately above.

## Limits

Heap probes sample the largest allocatable contiguous block, not total free
memory, and can themselves affect fragmentation. Stack measurements paint
the 64 KiB EWRAM stack before game construction. Neither metric proves a
bound for every possible game state; transient allocations inside a frame
are additionally checked by real completion and allocation-panic detection.

The full hosted m01–m49 playthrough was attempted. m01–m08 pass; m09 fails
with `nav_to_map(Route2,3,44) did not converge`. The same failure occurs on
an isolated unchanged `5162065` worktree (`playthrough-baseline.log`), as
well as the final candidate (`playthrough-candidate.log`). Thus m09–m49
are not certified by this audit. The seeded GBA ending fixture is separate
evidence and must not be described as a completed story playthrough.

During PR preparation, `viridian_mart_shop.rs` and `pc_trigger.rs` were
updated to borrow owned dialogue lines instead of moving them out of borrowed
pages. The complete `cargo test -p pokered-core` suite now passes: 2903
tests (2,580 library + 323 integration), with one ignored doctest.
See `core-full-tests.log`. This closes the earlier audit's integration-build
limitation; the m09 navigation limitation above remains.

## PR baseline screenshots

`gba-pr104-{overworld,battle,save}-{before,after}.png` under
`docs/screenshots/` compare `master@7e7e983` with the PR's shared hosted
renderer. All three pairs have **zero changed pixels**. Captures use EN,
`pokered-app screenshot --screen overworld|save -f 10`, and
`pokered-app battle --config sample_battle.json --frames 200 --screenshot …`.
See `master-screenshot-comparison.json`. GBA-specific paths are separately
covered by the ROM tests and incremental freeze/recovery captures above.
