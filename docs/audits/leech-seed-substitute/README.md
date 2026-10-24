# Leech Seed through Substitute

Original reference: `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.

[`LeechSeedEffect_`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/move_effects/leech_seed.asm)
checks the hit result, both Grass type slots, and whether the target is already
seeded. A Substitute does not prevent seeding. The shared `MoveHitTest` in
`engine/battle/core.asm` calls `CheckTargetSubstitute` for a broken drain-effect
check; this is not a general veto on moves targeting a Substitute.

The remake incorrectly rejected Substitute in the canonical RON rule, P5 native
setter and legacy effect helper. Remove those guards. Grass immunity, the
already-seeded guard, accuracy and residual damage mechanics stay unchanged.

## Regression evidence

Base: `523b82c0edaf74694d95debc8aed0bbfc225fcd1`.

- Before the fix, the production FIGHT-input test failed with an unseeded
  Rattata behind a Substitute. The legacy helper returned `StatusFailed` instead
  of `Seeded`. These are assertion failures, separate from an earlier test-import
  compile error.
- `leech_seed_fight_input_bypasses_substitute_on_either_side` uses real Pokémon
  constructors and `BattleScreen::update_frame` to select FIGHT and the move.
  Twelve cases cover both target sides, with/without Substitute, a non-Grass
  target, first-slot Grass and second-slot Grass. They verify the seed flag,
  real HP drain, seeder healing, unchanged Substitute HP, and player PP charge.
- `p5_leech_seed_substitute_bypass_and_reseed_guard` checks both compatibility
  paths and verifies that repeated application does not add another seed.
- `cargo test -p pokered-core --locked --offline`: 3,662 passed, 0 failed,
  1 existing ignored documentation test.

The FIGHT test starts from a prepared player-menu battle fixture with RNG seed
42. This is a production battle-input regression and original-source comparison,
not a new physical-input original-ROM capture or a full playthrough claim.
The separately running fresh mainline uses an older pinned binary and is not
evidence for this patch. No visual layout changes are included.

Build/performance measurements and source hashes are in `verification.json`.
