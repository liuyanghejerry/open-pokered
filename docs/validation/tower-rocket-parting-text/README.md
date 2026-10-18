# Tower Rocket post-victory dialogue

Original `PokemonTower7FEndBattleScript` calls EndTrainerBattle, then
DisplayTextID (the defeated trainer's AfterBattleText), then the exit movement.
The scene already contained each line for later interaction, but its on-load
victory continuation only moved and hid the Rocket. Ordinary real-battle
observation confirmed that Rocket 1 said "I give up!" inside the battle and
left without "I'm not going to forget this!" afterward.

Each of the three departures now prints its existing bilingual AfterBattleText
before walking. Core controls check all three NPCs stay visible at their battle
square without an ACK, resume and hide after ACK, and do not replay on map
re-entry. The twelve original departure paths remain covered. All 3,127 core
library tests pass. The added regression failed on the previous scene.

Before/after captures use the same constructed persistent fixture, seed42,
speed0, ordinary Continue and a real Rocket 1 battle. Both images show frame
3892, 102 frames after overworld return. The old Rocket is already moving;
the repaired Rocket waits while the complete parting line is visible.
`evidence.json` retains post-victory dialogue observations and tile traces.
Games were stopped by the diagnostic owner after assertions; exit -15 is not
natural game completion. This is bounded story validation, not a fresh route.

The fixture is reused from the already committed Tower exit proof. Build normal
base/candidate CLIs with desktop,debug-server, then run from this checkout:

```bash
python3 docs/validation/tower-rocket-parting-text/replay.py before /absolute/base/pokered-app /new/before-dir
python3 docs/validation/tower-rocket-parting-text/replay.py after /absolute/candidate/pokered-app /new/after-dir
```

Only observation, button/timeline input, dialogue advancement and PNG capture
commands are allowed; no runtime state setters or debug SAVE are used.
