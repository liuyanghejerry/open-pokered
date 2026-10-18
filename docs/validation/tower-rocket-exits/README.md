# Pokémon Tower 7F Rocket departure paths

The original `scripts/PokemonTower7F.asm` selects one of twelve coordinate
rows after the Rocket approaches the player. Each row contains five to
seven strides before HideObject. The previous scene used fixed two-, three-
and four-waypoint shortcuts, omitting branches and hiding Rockets early.

The scene now follows all twelve original paths. A field-level regression
checks every reached coordinate and persistence after map re-entry; all
3,126 core library tests pass. `reference-cases.json` records the original
coordinate table and movement lists; its source hash is in `evidence.json`.

The real CLI comparison boots an offline constructed persistent fixture,
uses Continue and an actual Rocket battle, then samples every game frame.
Both screenshots show frame 3930, 140 frames after return to the overworld.
Before: Rocket 1 has vanished at (9,13). After: it is still walking the
original RIGHT, DOWN×5, LEFT route and vanishes at (9,16). The comparison
uses seed42, speed0, the same fixture and buttons, with no runtime setters.
The binary's actual build source and SHA are retained separately from later
documentation/rebase commits. Games were stopped by the diagnostic owner
after assertions; exit -15 is not natural game completion.

Build the base/candidate normal CLI with desktop,debug-server, then run:

```bash
python3 docs/validation/tower-rocket-exits/replay.py before /absolute/base/pokered-app /new/before-dir
python3 docs/validation/tower-rocket-exits/replay.py after /absolute/candidate/pokered-app /new/after-dir
```

This package verifies departure paths. It does not establish complete Tower
story, timing or whole-game parity. The first capture attempt used invalid
nested-array timeline entries; both diagnostic Games were stopped and the
corrected string/neutral timeline was used for the retained comparison.
