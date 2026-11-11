# Completed multi-hit attack captions

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

A surviving target previously returned to the next action/menu without the hit
count. The player now sees `Hit the enemy\nN times!`; an enemy attack prints
`Hit N times!`. The count follows the last hit and existing Rage stat narration,
then precedes Twineedle poison and later actions. A target knocked out during the
sequence gets no count. First-hit Substitute breaks cancel the sequence; an
intact doll or a later break can still complete it.

The last hit records survival before HP is written. This keeps the result valid
when the defender later uses Selfdestruct or residual damage changes the final
turn state. Receipts reset each turn and carry the actual move identity.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm:3238–3267,5638–5667` and
`data/text/text_2.asm:977–982,1201–1205`.

## Verification

- Six public-FIGHT tests resolve 194 prepared turns. They cover all ten moves,
  both sides, counts 2–5, knockout on every hit, Substitute, a later Selfdestruct,
  Rage/count/poison ordering, and next-turn miss/block cleanup. They drain 164
  presentations and require the count only after the final animation.
- The same final test source against the parent libraries: one passed, five
  failed. Candidate: all six passed. Full core: 3,713 passed, one existing ignored.
- Fresh native NEW GAME through m06 and all 11 seeded subsystem scenarios passed,
  with the source and executable checked before and after each run.
- Seven original GBA benchmark cases and 13 memory cases passed without changing
  fixtures, limits or timeouts. Minimum free heap: 7,832 B; stack margin: 8,708 B.

`capture.rs` uses prepared parties and deterministic RNG, then public FIGHT input,
actual `BattleVisualEffects` and `draw_battle`. Both versions capture frame 347:
Double Kick starts at frames 155 and 231, with target HP 450 → 442 → 434. The parent
has returned to the menu; the candidate shows the completed two-hit caption.
The PNGs were visually inspected. The exact library, executable and image hashes
are in `verification.json`.

The renderer capture does not exercise the full game/audio clock or run the
original ROM. The fixtures are not a new m49 playthrough. Per-hit Rage mechanics
and narration, including repeated effectiveness text, remain separate findings.
