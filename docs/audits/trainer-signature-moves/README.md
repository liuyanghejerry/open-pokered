# Trainer signature moves

Reference: `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.
The checked-in oracle records hashes of the four source files used.

`ReadTrainer.AddLoneMove` decrements the table selector, then passes the raw
party offset unchanged to `AddNTimes`, starting at `wEnemyMon1Moves + 2`.
The offsets 1,1,2,2,3,3,3,4 therefore select Onix, Starmie, Raichu,
Vileplume, Weezing, Alakazam, Arcanine and Rhydon. They are not one-based
party numbers. The earlier port subtracted one from all eight offsets.
Giovanni's shorter Hideout and Silph parties must remain unchanged.

`ReadTrainer.ChampionRival` branches on **the rival's** starter:
STARTER3 (Bulbasaur) => Mega Drain, STARTER1 (Charmander) => Fire Blast,
otherwise Squirtle => Blizzard. The earlier port rotated all three moves.
Pidgeot's Sky Attack and the Elite Four's fifth-member overrides remain intact.

## Verification

- Core integration tests use all eight real leader parties, all three real
  champion parties, and both non-gym Giovanni parties. They compare every
  member's four move slots against an independent original-source oracle.
- Native integration `trainer_signature_boot` constructs SRAM offline, then
  boots normally and uses A/directional input to CONTINUE, face the boss and
  advance the actual map dialogue. After construction it never sets game
  state, warps, restores a snapshot or injects a battle. It reads the roster
  at battle entry before any move selection and asserts trainer identity.
- `base-native.json`: 13 actual entries on master a0324ea; all eight leaders,
  Silph Giovanni and all three champion starters reproduce the wrong moves.
  Hideout Giovanni is the unchanged negative control.
- This is a pure battle-logic change; screenshots are exempt under AGENTS.md.
  Roster observations directly establish the affected behavior.
- This targeted test does not prove full-game completion, animation parity,
  or the still-open boulder/FLASH issues.

Validation completed: `cargo test --offline -p pokered-core`: 3598 passed,
0 failed, 1 ignored. Native boot test: all 13 cases passed.
`fixed-native.json` records the corrected rosters and exact slot changes;
all battle-entry frame numbers and species/order match the baseline.
Only the 12 expected configurations change; Hideout Giovanni is identical.
