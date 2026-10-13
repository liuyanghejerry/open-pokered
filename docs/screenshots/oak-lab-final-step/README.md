# Oak enters the laboratory

Original `engine/overworld/auto_movement.asm`, `RLEList_ProfOakWalkToLab`
(`pret/pokered` fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c): D5, L1, D5, R3,
U1, NPC_CHANGE_FACING. The final U1 leads from (12,12) to (12,11).
Previously Oak disappeared at (12,12); only the player took the last northbound step.

`followNpcPath` preserves an explicit NPC route and leaves the player one tile
behind. After Oak completes U1, the scene hides him, then advances the player
onto the door. Ordinary coordinate-based `followNpc` callers keep their existing
finish-on-target behavior. Warp cleanup and laboratory entry are regression tested.

Before is master 451074bd17ba2759aeb42052cb3b6e3d788f694d (its tree equals the
recorded #210 head b7564dcc88efa67e5d1c1caa9bbf4aa873281997). After is this branch.
Both use `capture_natural_oak_615`, identical initial save/map/player/RNG and
inputs. Left-exit comparison: t=778; right-exit: t=846. No resampling or phase
alignment. Each after sequence was repeated byte-for-byte (all manifest rows and
PNG files); initial screenshots equal their base. Both enter OaksLab at the same
frame as base. Verification JSON contains exact positions, counters and differences.

Core: 2,801 passed. All 41 application test executables passed. The added engine
regression compares all 15 leader destination tiles from both exits and verifies
laboratory entry and Oak's hide flag. Existing tests verify lab entry cutscene,
saved hide bits and guide escorts. This proves the missing stride and route;
original hardware pacing, image publication, and every escort frame remain under
audit and are not claimed equivalent here.
