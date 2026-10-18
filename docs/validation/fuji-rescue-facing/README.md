# Mr. Fuji rescue arrival facing

`PokemonTower7FWarpToMrFujiHouseScript` explicitly sets SPRITE_FACING_UP
before warping to Mr. Fuji's house. The native scene omitted that write and
preserved the approach direction. A legal approach from (11,3), facing left,
therefore arrived at MrFujisHouse (3,7) still facing left.

The scene now faces up immediately before the warp. Core regression drives
real talk and rescue from all three walkable adjacent tiles: (11,3), (10,2),
and (10,4). Each arrives at (3,7) facing up with the rescue flag set. The
regression failed on the original scene; all 3,127 core library tests pass.

The ordinary CLI comparison starts from the same offline constructed legal
right-side fixture, uses Continue and actual rescue dialogue/warp, and captures
both images at frame1500 with seed42 and speed0. Base product code/data are
byte-equal to master e8518c59; the actual binary build commit is recorded
separately. Both arrive at the same map/tile; only the repaired player faces up.
A first diagnostic used unwalkable (9,3), and a first script used an unsupported
wait condition. Those are excluded from the legal-player evidence retained here.
Games were stopped by the diagnostic owner after assertions; exit -15 is not
natural game completion. This is not a fresh-route proof.

Build normal base/candidate CLIs with desktop,debug-server, then run:

```bash
python3 docs/validation/fuji-rescue-facing/replay.py before /absolute/base/pokered-app /new/before-dir
python3 docs/validation/fuji-rescue-facing/replay.py after /absolute/candidate/pokered-app /new/after-dir
```

The replay permits observations, button input, dialogue advancement and PNG
capture. It does not use runtime setters, warp injection or debug SAVE.

The packaged replay passed and reproduced the exact candidate PNG. The
comparison establishes arrival-facing behavior; wandering NPC phase also
changes after the extra scene operation, so it is not whole-frame/cycle parity.
