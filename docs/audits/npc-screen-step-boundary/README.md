# Ordinary NPC screen-edge step rejection

On Route 1, approach from (5,19) to (5,20) while the ordinary youngster is at (5,24). Before the fix, frame 1109 starts an outward Down stride (counter 0 → 16). The destination is then outside CheckSpriteAvailability, leaving the actor paused in that stride until the player approaches again.

Original `InitializeSpriteScreenPosition` and `CanWalkOntoTile` reject this attempt before animation: YPixels=124; adding the alignment offset 4 and direction delta 1 produces 129, outside the `<128` limit. X uses `<144`. Calculations use byte wrapping and nibble swapping. Scripted movement bypasses this boundary.

The game now rejects a newly scheduled ordinary stride at this boundary, keeping its selected facing and wait. No first pixel or map-coordinate update has occurred at this point. The generic engine builds its occupied tiles before scheduling, so rejecting the attempt does not change what later slots observe in the pass. Existing strides and scripted paths retain their behavior.

## Evidence

- `base-native.json.gz` / `fixed-native.json.gz`: normal SRAM boot and CONTINUE, identical physical input/readonly command tape, 900 observations each; every player state matches. First NPC difference: frame 1109. Before: one forbidden start; after: zero. After stepping closer to (5,21), the NPC can still move normally.
- Whole raw 160×144 before/after PNGs: same frame 1113. The 136 differing pixels are confined to the NPC at (65,125)–(78,139); no crops, masks, overlays, or replacements.
- Source, HEAD, and the sole native binary remained frozen during recordings. Intentional process shutdown occurs after assertions.
- `original-controlled-probes.json`: freshly repeated eight original-ROM routine probes, twice each. These deliberately construct RAM/register inputs and load a state to isolate the rule. They are a unit-level oracle, **not** normal-boot or original timing/raster evidence. ROM SHA-256 and pinned disassembly are in `manifest.json`.
- Four new core tests cover all four rejected edges, four inward controls, scripted bypass, and an already-running stride. Full core: 3610 passed; app: 386 passed, 38 ignored.
- The portable replay below was run separately and reproduced all 900 observations and every captured PNG byte exactly.

## Reproduce

Build the native debug-server binary using the repository instructions, then run:

```sh
python3 docs/audits/npc-screen-step-boundary/replay.py \
  --repo "$PWD" --binary /path/to/pokered-app \
  --out /path/to/new-evidence-directory \
  --extended --expect-freeze --expect-boundary
```

It reuses the committed fixture and boot commands from `../offscreen-npc-availability/`. To reproduce the old defect on the base revision, omit `--expect-boundary`.

This proves the screen-boundary correction. It does not establish full NPC movement parity: original asymmetric displacement counters and finer collision/clock behavior remain separate work. It is not a fresh playthrough or animation/audio parity proof.
