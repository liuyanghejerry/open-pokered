# Recharge and self-KO after Substitute breaks

Original `AttackSubstitute.nullifyEffect` clears the attacker's move-effect byte.
The subsequent `AlwaysHappenSideEffects` lookup therefore does not run
Explosion/Selfdestruct's self-KO, and the later effect dispatch does not install
Hyper Beam recharge. An intact doll still permits those effects.

The production stack retained the selected move's immutable effect and applied
both effects after a break. Use the action's existing Substitute-break receipt
to suppress Hyper Beam recharge and the explosion record's post-hit self-KO.
No-doll and intact-doll controls retain their ordinary behavior.

Prepared production battle fixtures use PlayerMenu → FIGHT → MoveSelect → the
real stack, on both sides:

- Hyper Beam, Explosion and Selfdestruct against no doll, an intact 60 HP doll
  and a broken 1 HP doll: 18 cases, with connecting no-doll controls.
- On both sides, finish the first turn's normal text, then use Hyper Beam again:
  the second attack damages real HP and requires recharge. The player spends PP.
  This also checks that a previous break does not leak into the next action.

Three production assertions failed before the fix. Full core validation after
the fix passes 3,673 tests, with one existing ignored test. See
`verification.json` for final build/performance evidence and source hashes.

Reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm` (`AttackSubstitute`, both move execution tails), and
`data/battle/always_happen_effects.asm`. This is source-backed prepared-state
regression, not an original-ROM recording or a fresh playthrough. It is pure
battle logic and is exempt from screenshots under AGENTS.md. Recoil, Pay Day,
and multi-hit continuation are outside this change and remain audit questions.

The unmodified GBA release build and all seven original performance scenarios
also pass. No baseline, threshold, timeout or CI workflow was changed.
