# Primary stat-drop prechecks

Original `StatModifierDownEffect` first draws an extra miss byte for the enemy
outside link battles: bytes below `25 percent + 1` (64/256) fail. It then checks
Substitute before entering `MoveHitTest`, which handles Mist and accuracy.
Production omitted the ordinary enemy roll and reached Substitute only after
accuracy, changing both stat results and subsequent random choices.

Carry the ordinary-enemy flag into the production rule adapter and perform
these primary-effect prechecks in their original order. Player attacks and
link peers have no extra miss roll. The primary Mist gate remains before
X Accuracy; damaging stat-down side effects remain a separate follow-up.

The production turn caller reproduced 27 wrong stat results and 162 wrong
random draw counts across 432 cases: nine moves, both sides, ordinary/link
modes, no protection/Substitute/Mist, and bytes 0/63/64/255. The deterministic
random source is supplied via the LinkRng slot independently of battle mode.
Both mode flags are asserted; two existing Mist fixtures now initialize both
flags just as the real link driver does. All 3,687 core tests pass, with one
existing ignored test. Exact failure, fixture and passing hashes are retained.

Pure battle logic is exempt from screenshots under AGENTS.md. This is prepared
production-turn evidence, not a fresh playthrough. Committed-source GBA tests
and all required remote workflows/runtime gates remain pending before merge.
