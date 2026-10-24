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

Initial PR381 head `327a1abb` failed the original broad GBA memory gate at
enemy Hyper Beam (game frame 86882): contiguous free heap was 3,839 bytes,
below the existing 4,096-byte minimum. The exact local source repeated that
failure. Checking the break flag before classifying the record did not resolve
it on its own.

The bridge was cloning each compiled hook, including nested operation lists,
for read-only classification and execution. Installed rules hosts are immutable
and already have static lifetimes across hot reloads, so return their borrowed
hooks. The final candidate passes the full 3,673 core tests, the original seven
performance scenarios, and all 13 original GBA memory scenarios (all 330 move
animations included): lowest contiguous heap 4,768 bytes, untouched stack
8,708 bytes. These are measurements of the final stress build, not a claim
that every allocator layout has the same margin. No gate or workload changed.
