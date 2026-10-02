# Mt Moon driver route recovery

This repairs the playthrough driver, not the game map. Fresh attempt 2 reached
m11, then after a normal m12 blackout/retry navigation landed at MtMoonB1F
(25,9). `nav_warp(21,17, "MtMoonB1F", "MtMoonB2F")` searched only B1F and
failed: these ladder rooms are disconnected in the original and current maps.

Canonical collision-aware `bfs_cross` finds a 57-step path from (25,9) to the
inward neighbor (21,16). It returns through the ladder at B1F (25,9) to 1F
(17,11), crosses 1F, enters the ladder at (5,5), arrives at B1F (5,5), and walks
to (21,16). The final downward step reaches B2F (21,17).

`nav_warp` now uses the existing cross-map navigator for its fallback inward
approach. It still rejects an unexpected map transition/blackout. The
cross-map planner uses canonical collision pairs, warp destinations and live
NPC blocking; no collision bypass or state-changing debug command was added.
The same recovery applies to an explicitly directional warp approach.

Validation used the supplied final native binary, seed 42 and speed 0. A
separate initial SaveData fixture placed one level-20 Ivysaur at B1F (25,9),
with last_map Route4 and default event flags. After loading that fixture via
CONTINUE, all movement used real directional input through `press_timeline`.
The runtime probe rejects commands outside the read-only/input allowlist.
Observed route: **B1F → 1F → B1F → B2F**, frame 589 → 1856. Final position
**B2F (21,17)**. The attached JSON records all navigation commands and observed
positions. This is a seeded route regression, not a fresh playthrough claim.

To reproduce, create a JSON snapshot with `SaveData::new()`, assign
`game_data.position.{map_id,x,y} = {60,25,9}`,
`game_data.last_map = 15`, and add
`create_pokemon(Species::Ivysaur,20,[0x88,0x88])` to its party. Run:

```sh
python3 docs/audits/2026-10-02-full-fidelity/validation/mtmoon-driver-route.py \
  --fixture /path/to/fixture.json --binary /path/to/pokered-app
```

`python3 -m py_compile scripts/playthrough.py` and `git diff --check` passed.
No Cargo invocation or product-source change was needed.
