# First-hit substitute break and multi-hit moves

`AttackSubstitute` clears the attacker's move effect before the first hit
returns to `AlwaysHappenSideEffects`. Therefore `TwoToFiveAttacksEffect`
does not install its hit loop when that first hit breaks the doll. A later
break does not clear the already-set multiple-attack status, so an existing
sequence continues. See original `engine/battle/core.asm:3218-3268,4849-4902`
and `engine/battle/effects.asm:925-970` at `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.

Before the fix, production FIGHT with Doubleslap against a one-HP doll
incorrectly lowered the target's real HP from 450 to 446. The data bridge now
skips the `RepeatHits` hook when it is first entered after a doll break.
The existing loop handles any later break normally.

Four production-input regression tests cover both sides and all ten multi-hit
moves: 20 first-hit breaks, 20 intact dolls, 14 later breaks on the seven
2-5-hit moves, and two next-action checks. The complete core suite passes
3,679 tests with one existing ignored test. This is prepared-state evidence,
not another ordinary NEW GAME run. Pure battle logic uses the repository's
screenshot exemption.

Multi-hit random-byte sampling and Twineedle's final poison gate remain
separate audit candidates; this change does not claim to resolve them.

After the merged GBA memory and hosted-loop fixes, this isolated branch at
`900b8702d77e2d894c9abcfb82046e60272591e5` passed all 3,679 core tests
(zero failures, one existing ignored test), the unmodified GBA release build
and all seven original performance scenarios. Subsequent dependency rebases
change documentation only; product files are checked byte-for-byte. Historical
results remain separately identified in `verification.json`. All three remote
workflows and all six original GBA runtime gates must pass on the final PR
source before merge. No thresholds, timeouts, scenarios or workflows changed.
