# Rage gains from multi-hit attacks

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

A raging defender previously gained only one Attack stage from an entire
multi-hit attack. Each survived strike now grants one stage, including hits
absorbed by Substitute. The knockout hit grants none. Gains stop at +6; when
the actual Attack stat is already 999, the stage stays unchanged and that
attempt prints `Nothing happened!`.

Each successful gain reapplies badge boosts and the original attacker's status
penalties. Failed attempts do neither. Action receipts preserve the number of
successful and capped attempts for narration even if a later action changes
the final battle state. Each attempt gets `RAGE is building!` and its result;
the aggregate stat event no longer produces a duplicate or `greatly rose` line.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm:3243,4913–4957,5643` and
`engine/battle/effects.asm:351–514`.

## Verification

- Seven public-FIGHT tests resolve 426 prepared turns: all ten multi-hit moves,
  both sides, counts 2–5, stage and 999 caps, knockout on every hit, Substitute,
  misses/immunity, repeated burn penalties and next-turn receipt cleanup.
- The first six tests all failed against the parent libraries. The added 999
  case failed against the intermediate repair; all seven pass with the final
  source. Full core: 3,720 passed, one existing ignored.
- Fresh native NEW GAME through m06 and all 11 subsystem scenarios passed;
  source and binary identities were unchanged throughout both runs.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged fixtures,
  limits and timeouts. Minimum free heap: 7,816 B; stack margin: 8,704 B.

The two capture programs use prepared parties, deterministic RNG, public FIGHT,
actual `BattleVisualEffects` and `draw_battle`. Each parent/candidate pair uses
the same input and frame. At frame 427, the old Snorlax caption reports a single
Attack rise; the repair begins the five Rage/result pairs. At frame 443, a
999-Attack Dragonite now shows the failed attempt instead of the completed hit
count. Both pairs retain the same five hit animations and HP changes. All four
PNGs were visually inspected; `verification.json` records hashes and traces.

These fixtures are not a new full m49 playthrough. The captures do not run the
full game/audio clock or the original ROM. Individual hit animations still run
before the queued result lines: their interleaving, repeated critical and
effectiveness captions, and Substitute-break text order need separate work.
Single-hit Rage and the overall original RNG pipeline are also outside this
repair.
