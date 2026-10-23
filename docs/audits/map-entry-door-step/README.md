# Automatic steps out of doors

Leaving the Route 22 gate stopped the player in the doorway at `(8,5)`.
After obtaining a starter, leaving the player's house or Oak's lab likewise
stopped on the outdoor doorway. Cerulean's post-champion and Saffron's
post-rescue/post-liberation entries exhibit the same bug. The destination's
`@load` hides or shows an NPC, and
the warp code incorrectly treated any pending script effect as player
movement, suppressing the normal DOWN step.

Door arrivals now retain that step through NPC initialization. An explicit
player movement command takes ownership and cancels the pending step, even
if earlier entry commands only changed NPC visibility. A later warp resets
the old door state before inspecting its own destination tile. The existing
door animation clock and movement implementation remain in use.

The original
[RunNPCMovementScript](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/npc_movement.asm)
calls
[PlayerStepOutFromDoor](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/overworld/auto_movement.asm),
which identifies a door tile and queues one DOWN input. The subsequent map
script can replace simulated movement; an NPC visibility update does not.
The validation record verifies these source files against their pinned
remote Git blobs, rather than identifying the extracted tree as a Git checkout.

## Reproduce the original

`original-fixture.sav` is an offline prepared original SRAM outside the
Route 22 gate. The replay uses normal boot/CONTINUE, enters the actual gate,
walks to each exit lane, exits, and releases input for 100 frames. Read-only
CPU hooks observe the automatic step at `(8,5)`; both lanes finish at
`(8,6)`. There are no live memory writes or original-font captures.

```sh
python replay-original.py --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --save original-fixture.sav --out /path/to/results.json
```

Use PyBoy 2.8.1. The ROM is not included; its required SHA-256 is checked by
the script. Two independent replays give byte-identical input/observation
records, retained in `original-runtime.json.gz`.

## Native validation

`crates/pokered-app/tests/door_exit.rs` drives `PokemonGame` with physical
buttons from controlled map fixtures. It holds a direction until the map
changes, then releases for 100 updates. The eight door cases cover both gate
lanes, the player's house, Oak's lab and Viridian's Pokémon Center, including
Cerulean's center after becoming champion and Saffron's center after rescue
and liberation. Each turns back inside without an extra DOWN. Separate cases cover the bedroom
stairs and the two-UP S.S. Anne departure script.

The matched gate before/after captures use master `eb3ce841` and the fix with the
same two setup updates, 51 DOWN updates and 100 neutral updates. They are
captured before any re-entry inputs, using the shared Fusion Pixel renderer.
On master all seven cases with NPC entry effects fail the final-position
regression; the Viridian center, stairs and departure controls already pass.

Core regressions cover NPC initialization, all three explicit player-path
commands after a visibility command, clearing a stale pending door on a
subsequent stair warp, and the existing foreground walk-clock assertions.
The stale-state and custom movement cases are internal fixtures, not claims
of additional player-reproduced story bugs.

```sh
cargo test -p pokered-core --lib tests_map_on_load --locked
cargo test -p pokered-core --lib tests_field_moves --locked
cargo test -p pokered-core --lib tests_oak_event --locked
cargo test -p pokered-core --locked
DOOR_CAPTURE_DIR=/path/to/captures cargo test -p pokered-app --test door_exit --locked
```

These prepared fixtures establish the missing automatic step and protect
adjacent behavior; they do not establish fresh-playthrough coverage or
cycle-for-cycle map-entry timing. Full applicable CI, including the original
GBA memory, pacing, slow-cartridge and unskipped-opening gates, must pass
before merge.
