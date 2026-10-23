# Route 22 Boulder Badge guard

Crossing either checkpoint lane with the Boulder Badge previously skipped
the guard's permission text and item fanfare. Talking directly to the guard
without the badge printed the refusal but left the player in place.

Both entry lanes now check the actual badge bit and announce the first
permission. Direct conversation also pushes a refused player one tile down.
The existing item-text effect plays GET_ITEM_1 after the permission text;
the refusal waits for DENIED before continuing. The admitted state maps to
the original `wRoute22GateCurScript = 2`, at SRAM offset `0x28ba`, so it
survives map changes and CONTINUE. Manual conversation remains available.
An old companion flag cannot override this original saved byte.

The source is
[`Route22Gate.asm`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/Route22Gate.asm):
both `(4,2)` and `(5,2)` call the guard while the script is DEFAULT;
permission changes it to NOOP; refusal simulates one DOWN step and then
returns to DEFAULT. The NOOP byte lives in saved `wGameProgressFlags` and
is not reset by an ordinary map entry.

## Reproduction

`original-fixture.sav` is an offline prepared SRAM fixture outside the Route
22 gate, not a fresh playthrough. The replay uses normal boot/CONTINUE and
physical inputs to enter the gate through its real doorway, loading the
original NPC/warp data. It uses read-only observation hooks at executable
instructions and `PrintText`, and never renders the original font.

```sh
python replay-original.py --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --base-save original-fixture.sav --out /path/to/evidence
```

The ROM is not included. Its required hash and pinned source revision are
recorded in `original-runtime.json.gz`. Two replays produce identical inputs,
guard events and final positions for five cases: both lanes with and without
the badge, plus a direct refusal. The admitted runs also leave and re-enter
the gate without another automatic interruption, then talk to the guard
successfully again.

The native regression starts controlled map fixtures and drives the shared
`PokemonGame` with real button input. It covers the same entry/refusal paths,
repeated refusals, badge bits independent of the gym-result flag, the live
fanfare, map re-entry, and an exported SRAM loaded by a cold app through
physical title/menu CONTINUE input. The native tests do not claim fresh
travel or a cycle-for-cycle match of the complete dialogue sequence.

```sh
cargo test -p pokered-core --lib route22 --locked
cargo test -p pokered-core --lib bedroom --locked
GATE_CAPTURE_DIR=/path/to/captures cargo test -p pokered-app --test route22_gate_guard --locked
```

The before/after permission images are the same frame: two setup updates,
eight UP updates, one release update and 180 neutral updates, with no
dialogue confirmation. The before was captured on master `ea385d8a` and
fails both new regressions. Captures use the game's Fusion Pixel renderer.
The validation JSON records the local checks; full applicable CI is required
before merging.
