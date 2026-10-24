# Secondary status and Substitute

Original Red distinguishes damaging confusion from Confuse Ray/Supersonic.
`ConfusionSideEffect` jumps past the primary Substitute check and uses
`cp 10 percent` without `+1`: random bytes 0–24 succeed, byte 25 fails.
An intact substitute can therefore absorb the damage while its owner becomes
confused. When the attack breaks the substitute, `AttackSubstitute` clears the
move effect before secondary dispatch, including its random chance draw.

The native data record blocked damaging confusion behind an intact substitute,
used 26/256, and allowed secondary status after the doll broke and its volatile
was removed. Preserve a per-attacker break receipt through the action and reject
chance-gated secondary status/stat hooks before drawing. Reset that receipt for
new moves/turns; keep the primary confusion guard intact. Align the compatibility
confusion helper with the same primary/secondary distinction.

Validation exercises prepared production battle fixtures:

- FIGHT input on both sides, with positive unshielded controls: Confusion and
  Psybeam through intact dolls; Confusion, Psybeam, Poison Sting and Acid after
  breaking a doll; Confuse Ray still blocked by an intact doll (14 cases).
- The actual canonical stack consumes explicit bytes for both confusion moves:
  no doll, intact doll and broken doll; chance bytes 24/25; five-turn duration;
  no chance/duration draw after breaking the doll (12 cases).
- Two original production assertions failed at Confusion seed 6 before the fix.

Sources: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/effects.asm` (`ConfusionSideEffect`, `ConfusionEffect`,
`StatModifierDownEffect`), `engine/battle/core.asm` (`AttackSubstitute`), and
`macros/data.asm` (`percent`). This is source-backed production regression,
not an original-ROM recording or fresh playthrough. The scope is chance-gated
secondary status/stat effects; other native effects and multi-hit continuation
remain separate audit questions. This pure battle-logic change needs no visual
screenshots under the repository policy.

Local validation: all 3,670 core tests passed (one existing ignored test); the
unmodified GBA release build and all seven original performance scenarios passed.
The production fixtures and explicit-byte stack tests are included in that run.
See `verification.json` for source hashes, logs and measured values.
