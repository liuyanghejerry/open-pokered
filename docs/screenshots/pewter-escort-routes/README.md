# Pewter escort routes

The museum guide previously stopped at (14,8) with the player on the same tile.
The gym guide and player both stopped at (12,18). Terrain pathfinding also
replaced the original routes and player positioning movements.

The independent routes now preserve `RLEList_PewterMuseumGuy/Player`,
`RLEList_PewterGymGuy/Player` and the reverse-consumed `PewterGuys` positioning
moves. `[0,0]` consumes an ordinary field update while the guide keeps walking.
Museum farewell: player (14,8), guide (13,8), guide faces up.
Gym farewell: player (11,18), guide (12,18), guide faces left.
Both restore city music before the farewell; eastern interception also starts
the original guide music. Hide, walk-off and respawn remain in the scene.

## Evidence

- `source-routes.json`: independently decoded original assembly routes for all
  four museum approaches and five gym starts, with source file hashes.
- `native-cases.json`: nine native scene continuations, repeated twice with exact
  state equality. Adjacent talk starts use the debug interaction command; eastern
  coordinate starts use the coordinate event on map positioning.
- `verification.json`: matched before/after frame 1200, with image hashes and poses.
  Both repeats yielded byte-identical comparison PNGs. Museum and gym captures
  hold the same first farewell page without further input.
- `original-*-runtime.json` and `original-*-farewell.png`: two exact original
  emulator repeats each. Controlled boot fixtures reload the original map header
  and rebuild the map-view pointer; the gym fixture additionally sets its initial
  player coordinates to (34,16) before loading the map. Setup hooks are removed
  before recording. Real input drives the original escort without movement-state
  writes. Museum completes 23 guide steps; gym completes 41.

Original synchronized escorts update sprite screen pixels while retaining stale
NPC map coordinates. The physical farewell pose is measured from the screen,
not those stale coordinates. These checks establish routes, completion and poses;
they do not claim full hardware, OAM, RNG or audio waveform parity.

## Comparison

Museum, frame 1200: `museum-before.png` / `museum-after.png`.
Gym, frame 1200: `gym-before.png` / `gym-after.png`.
