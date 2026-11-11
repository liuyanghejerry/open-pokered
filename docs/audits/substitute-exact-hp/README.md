# Substitute at exactly zero HP

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

The original `AttackSubstitute` subtracts damage, writes the remaining HP, and
returns when there was no borrow. An exact hit therefore leaves the substitute
up at zero HP. The next positive hit breaks it without spilling into real HP.

The native absorb helper used `damage >= hp`, removing the substitute one hit
early at this boundary. Use strict `>` in the production helper and the legacy
and stack-parity compatibility implementations. Existing parity tests had pinned
the shared incorrect behavior; their expected boundary now follows the original
assembly rather than agreement between two native implementations.

Two production `BattleScreen` regressions first failed on `b7d5e99b`:

- Actual FIGHT selections on either side use a level-20 Seismic Toss against
  substitutes with 19, 20 and 21 HP. Damage stays off real HP; the exact case
  remains up at zero, the below case breaks and the above case remains at one.
- After finishing the normal turn presentation, the zero-HP substitute absorbs
  the next FIGHT hit. A third hit finally damages real HP. The case runs with
  either player or enemy as the protected target.

Reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm`, `AttackSubstitute.applyDamageToSubstitute`.
These are prepared battle fixtures exercising the production menu and turn
pipeline, not fresh-playthrough or original-ROM recordings. The change is pure
battle logic; no renderer or UI assets change.

Validation: all 3,666 core tests passed (one existing ignored doc-test), the
formatted production regressions passed, and all seven original GBA performance
scenarios passed under the unmodified release profile and thresholds. See
`verification.json` for exact hashes and metrics.
