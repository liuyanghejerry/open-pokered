# Substitute checks before secondary RNG

The original Poison, burn/freeze/paralysis, flinch and stat-drop handlers check
an intact Substitute before drawing their secondary-chance byte. Production
instead drew the byte and then vetoed the effect, moving the shared battle
random stream forward even though the effect could never run.

Check authored Substitute guards before the single-hit secondary chance gate,
and do the same in the stat-down bridge. Damaging confusion deliberately has
no Substitute guard, so it still rolls its chance and duration through the doll.
The existing broken-doll cancellation is preserved.

The prepared production turn caller with real `LinkRng` reproduced 24 extra
chance draws across both sides and six representative effect families. The
regression also covers 24 broken/absent-doll controls and eight damaging
confusion cases. All 3,683 core tests pass, with one existing ignored test.

This is pure battle logic, exempt from screenshots under AGENTS.md. It is not
a fresh playthrough or an original-ROM recording. Twineedle's final-hit rider
and other early returns remain separate work. Exact source and log hashes are
in `verification.json`.

After the merged GBA memory and hosted-loop fixes, this isolated branch at
`eddd1774841cbf01b1c75c026596d310ae1fe50a` passed all 3,683 core tests
(zero failures, one existing ignored test), the unmodified GBA release build
and all seven original performance scenarios. Subsequent dependency rebases
change documentation only; product files are checked byte-for-byte. Historical
results remain separately identified in `verification.json`. All three remote
workflows and all six original GBA runtime gates must pass on the final PR
source before merge. No thresholds, timeouts, scenarios or workflows changed.
