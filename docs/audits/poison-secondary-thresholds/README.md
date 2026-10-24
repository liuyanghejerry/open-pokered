# Poison secondary-effect thresholds

`PoisonEffect` compares against `20 percent + 1` and `40 percent + 1`.
The original `percent` macro expands to `* 255 / 100`, so the thresholds are
52 and 103. The production rules still used 51 and 102 even though the legacy
test oracle already had the correct values. Poison Sting therefore missed
poison at byte 51, and Smog/Sludge missed it at byte 102.

Update the two production records. Prepared `BattleScreen` turn-caller tests
use the real link random stream on both sides, with bytes 0, threshold−1,
threshold and 255. The new boundary assertion failed before the change;
all 3,680 core tests pass afterwards (one existing ignored test). The fixture
checks that damage connects and exactly four random bytes are consumed.

This is pure game logic, exempt from screenshots under AGENTS.md. It is a
prepared-state regression, not an original-ROM recording or a fresh playthrough.
The separately reproduced early-return RNG order issue remains a follow-up.
Exact source and log hashes are recorded in `verification.json`.

After the merged GBA memory and hosted-loop fixes, this isolated branch at
`2337ee8b0d965b55343a93747c8cba823d1387b3` passed all 3,680 core tests
(zero failures, one existing ignored test), the unmodified GBA release build
and all seven original performance scenarios. Subsequent dependency rebases
change documentation only; product files are checked byte-for-byte. Historical
results remain separately identified in `verification.json`. All three remote
workflows and all six original GBA runtime gates must pass on the final PR
source before merge. No thresholds, timeouts, scenarios or workflows changed.
