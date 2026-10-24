# Battle presentation with nicknames and Transform

Battle narration already uses a nickname or the original identity retained
after Transform. Its animation/HP queue still matched messages using the current
species name. The different strings lose the move or residual publication
boundary: a named move can skip its animation, and damage can drain only after
the action or after the battle menu returns. Transform has the same mismatch
even without a nickname because the current species changes during simulation.

Use the same BattleNameContext for move, blocked-action and residual HP queue
entries as narration already uses. Existing unnamed callers retain their
species-name fallback. No damage, status, RNG or move mechanics change.

The public-FIGHT tests finish 72 prepared turns. Sixty-four cover both speed
orders, both status recipients, normal/poison/burn/freeze conditions, and no/
player/enemy/both nicknames. They compare move and residual boundaries, HP at each
boundary, final displayed/simulated HP, eight additional menu frames and RNG.
On the parent all 48 named traces differ from their 16 unnamed controls. Eight
further turns exercise actual Transform and the following copied move, on both
sides, with and without names. The parent also loses the unnamed Transform
animation. All cases pass after the shared name context is used.

The full core suite passes 3,702 tests with one existing ignored test. Fresh
native NEW GAME through m06 and all 11 independent subsystem scenarios pass with
source and binary identities fixed. Seven original GBA performance scenarios and
13 memory scenarios pass with unchanged gates, workloads and timeouts; minimum
contiguous heap is 7,832 bytes and stack is 8,708 bytes. Completed memory logs
were losslessly compressed and verified. These are scoped regressions, not a
new full first-clear run.

The same prepared FIGHT inputs and renderer frame 34 are captured before/after.
Only the player has a nickname. Before has already skipped to the enemy's Growth;
after retains the player's Tackle animation and its completion barrier. The
exact capture.rs uses BattleVisualEffects and draw_battle, not the outer
PokemonGame/audio clock or an original-ROM timing comparison. Verification and
binary/source hashes are in verification.json. All three remote workflows and
six actual GBA runtime steps must pass before merge.
