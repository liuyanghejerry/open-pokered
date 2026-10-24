# Pay Day after Substitute breaks

`AttackSubstitute.nullifyEffect` clears the attacking move's effect. Both original
move execution tails read that cleared byte before checking
`AlwaysHappenSideEffects`, so a breaking Pay Day hit does not enter `PayDayEffect_`:
it neither adds money nor prints the scatter text. A surviving doll still permits
the usual award of twice the attacker's level.

The existing hit receipt awarded coins in both cases. Keep a zero-coin receipt
when the doll breaks, preserving the ordered break narration and its once-only
consumption without awarding money or printing a false scatter message.

The previous regression incorrectly expected a breaking hit to scatter. Replace
it with both-side production FIGHT cases. Each target first creates its doll by
using Substitute in a real turn; the next prepared move pair is Pay Day/Splash.
The four cases cover surviving/broken dolls on both sides, unchanged real HP,
connected doll damage, the money pool, scatter count, and exactly one break text
between Pay Day and the next actor's Splash. Before the fix the breaking case
awarded 200 instead of zero. Existing no-doll, immunity, fatal-hit, copy-move,
trainer-AI, once-only and settlement tests remain in the full core run.

The earlier source contract assumed returning from `AttackSubstitute` implied
that Pay Day still ran. That missed its `nullifyEffect` tail. The current audit
follows the caller's re-read of the effect byte; old expected behavior is not an
independent oracle.

Reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm`, `data/battle/always_happen_effects.asm`, and
`engine/battle/move_effects/pay_day.asm`. Source-backed prepared production
regression; not a fresh playthrough or an original-ROM recording. This is pure
battle-effect logic, with no renderer/layout changes; AGENTS.md's logic exemption
applies. Exact sources and local validation are recorded in `verification.json`.

Local validation: all 3,673 core tests pass (one existing ignored test); the
unmodified GBA release build and all seven original performance scenarios pass.
The baseline, thresholds, timeouts and CI workflows are unchanged.

Retested after PR381's immutable-hook borrowing change: full core 3,673 pass,
all seven original GBA performance scenarios pass, and all 13 original broad
memory scenarios pass (330 move animations, minimum contiguous heap 4,768 B,
minimum untouched stack 8,708 B). Gates and workload remain unchanged.
