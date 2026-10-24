# Recoil after Substitute breaks

Original `AttackSubstitute` clears the attacker's move effect when the doll
breaks, before the `AlwaysHappenSideEffects` dispatch. It therefore skips
`RecoilEffect_`. An intact doll still allows recoil from its absorbed damage:
one quarter for Take Down, Double-Edge and Submission, one half for Struggle,
with a minimum of one HP while the effect remains active.

Extend the existing post-hit cancellation guard to both recoil data records.
The production FIGHT regressions cover the three ordinary recoil moves on both
sides against no doll, an intact 60 HP doll and a broken 1 HP doll (18 cases).
Three more cases exhaust all player PP, select FIGHT, and advance the actual
60-frame forced-Struggle phase before checking the same HP/effect boundaries.
Intact-doll controls verify the original recoil fraction from absorbed damage.

Both ordinary Take Down and forced Struggle wrongly lost one HP after a break
before the fix. The first forced-Struggle fixture only dealt one damage, so its
one-HP doll correctly survived at exactly zero; the retained red regression
selects a positive control dealing more than one damage. That fixture correction
preserves the separate original exact-zero behavior.

Reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm`, `data/battle/always_happen_effects.asm`, and
`engine/battle/move_effects/recoil.asm`. Source-backed prepared production
regressions, not a fresh playthrough or original-ROM recording. This pure logic
change is exempt from screenshots under AGENTS.md. Final local evidence and
source hashes are in `verification.json`.

After the merged GBA memory and hosted-loop fixes, this isolated branch at
`f74550238d0aee212d3ab0d7d9c76ad3b6840799` passed all 3,675 core tests
(zero failures, one existing ignored test), the unmodified GBA release build
and all seven original performance scenarios. Subsequent dependency rebases
change documentation only; product files are checked byte-for-byte. Historical
results remain separately identified in `verification.json`. All three remote
workflows and all six original GBA runtime gates must pass on the final PR
source before merge. No thresholds, timeouts, scenarios or workflows changed.
