# NPC saved state and original displacement counters

The old SRAM writer emitted 512 zero bytes for both sprite tables; the importer skipped them. Thus an NPC that naturally walked from Route 1 (5,24) to (5,25) returned to its home (5,24) after physical SAVE and an independent-process CONTINUE. The player correctly remained at (5,21).

SaveData now retains the two 16×16-byte original tables. The live save writer projects actor positions, facing, stride direction/counter and delays into them; CONTINUE restores these logical fields. Source map coordinates name the stride destination, while the generic engine retains its origin until the last pixel: the bridge translates in both directions. Displacement bytes have a game-owned lifetime and survive saves and frame snapshots. True map replacement resets them, including a same-map warp. Unknown imported table bytes survive codec round trips.

Ordinary stride acceptance now checks original Y/X displacement arithmetic, initialized at 8. Up/Left reject underflow; non-Up adds dy and rejects a result below 5. The non-Left branch has no corresponding lower-bound rejection. Byte overflow and these original bugs are retained. Scripted paths bypass the checks and stores.

Older all-zero native sprite tables use current-map defaults; stale object metadata cannot override those defaults. Uninitialized source slots initialize counters to 8, while ready/delayed actors with genuine zero counters keep zero. An explicit startup map override does not import another map's actors. The ending retarget to Pallet Town discards Hall of Fame actor data before saving, matching the original subsequent map reload.

## Actual gameplay evidence

`base-native.json.gz` and `fixed-native.json.gz` use the same constructed legacy-format SRAM, normal boot/CONTINUE, physical walking and the physical SAVE menu, then a second process CONTINUE. No debug SAVE, setters, warp, state loading/restoring or dialogue skip. Source, HEAD and the sole executable remain frozen during each recording. Player states, input/readonly requests and pre-quit NPC observations match; after CONTINUE master resets NPC0 to (5,24), candidate retains (5,25). Whole unmodified 160×144 screenshots show the same frame 608. Shutdown is intentional after assertions.

`fixture-preparation.json` explains the preboot construction: the old edited route fixture retained foreign-map sprite bytes. Both new recordings use the identical repaired fixture with a cleared actor table and recalculated checksum. This preparation is not gameplay evidence; the recordings start normally from the resulting SRAM. Historical audit fixtures are left unchanged.

`counter-native.json.gz`: two ordinary CONTINUE runs from offline variants of the naturally UI-saved SRAM. They differ only in Y displacement 0 versus 8 and the checksum. With the same 1024-frame idle tape, the zero case never starts walking; the 8 case moves normally. All player observations match. These are constructed save fixtures, not fresh playthroughs.

`original-format-import-native.json.gz` additionally verifies an original-format component fixture via normal CONTINUE and a physical Down step: a previously displaced, moving actor resumes near (19,30), instead of returning to home (13,20). It does not establish original rendering/clock parity.

Both portable scripts were rerun without the private transport adapter; observations and PNG bytes match the internal runs. `replay-save.py` also reproduces the UI-written SRAM exactly.

## Original oracle and local tests

`original-controlled-probes.json.gz` contains 132 actual original-ROM CanWalkOntoTile probes, each repeated twice. RAM/register inputs and a restored component state deliberately isolate the arithmetic; **these are unit-level logic probes, not normal gameplay or animation/audio timing evidence**. The production Rust golden is `crates/pokered-core/src/save/npc_displacement_golden.json`.

Reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`; ROM SHA-256 `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`; movement.asm SHA-256 `87939b64f3c4edacc5651d1f22b5a677967184bf19fef5879f2c274cf7b7596a`.

Core: 3623 passed (1 ignored doc test); app: 389 passed, 38 pre-existing ignored. Coverage includes all sprite bytes through SRAM/JSON/checksum/clear, 12 in-flight coordinate/direction boundaries, wrapping delays, legacy defaults, live counter stores, scripted bypass, snapshot/map reset, real app save writer and ending map retarget.

Full NPC instruction timing, random-draw scheduling, OAM/raster publication and animation/audio parity remain separate work. The saved table bridge is a functional projection, not a canonical instruction-clock producer.

## Reproduce

Build the repository's native binary with `--features debug-server`, then:

```sh
python3 docs/audits/npc-state-persistence/replay-save.py \
  --repo "$PWD" --binary /path/to/pokered-app \
  --out /path/to/new-save-evidence --expect-retained
python3 docs/audits/npc-state-persistence/replay-counters.py \
  --repo "$PWD" --binary /path/to/pokered-app \
  --out /path/to/new-counter-evidence
```

The scripts use the committed fixtures and the existing sibling audit's boot command tape. On the base revision, omit `--expect-retained` for the SAVE defect replay.
