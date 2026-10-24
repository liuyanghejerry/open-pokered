# Rage after a zero-damage trapping continuation

An immune first Bind/Wrap hit does not build the defender's Rage. In the
original, the next forced trapping action skips MoveHitTest and reuses its
stored damage, even when that damage is zero. A surviving raging defender
then attempts its Attack increase. The native survival receipt discarded
all zero-damage trapping actions, so this continuation never built Rage.

Allow that zero-damage continuation to record the surviving hit. Keep the
miss guard, faint guard, stage cap and Attack-999 behavior.

Original reference: pret/pokered
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, `engine/battle/core.asm`:
player continuation 3554–3568, enemy continuation 5928–5943, and post-hit
dispatch 3218–3243 / 5615–5643.

## Verification

- Eight prepared native FIGHT turns confirmed the missing zero-damage
  continuation on both sides, with ordinary damaging continuations as controls.
- Four original-ROM post-hit dispatcher cases: both sides with zero stored
  damage and target HP 450 or 0. Living raging targets reached both
  HandleBuildingRage and StatModifierUpEffect; fainted targets returned early.
- Three public-FIGHT tests cover 88 prepared turns and drained presentations:
  Bind/Wrap, both sides, first hit versus continuation, ordinary damage versus
  immunity, full versus one HP, Attack stages 0/5/6 and working Attack 999.
  The two zero-damage tests fail on the parent; the ordinary control passes.
  Continuations consume no additional RNG.
- Full core suite: 3,744 passed, one existing ignored.
- Fresh desktop NEW GAME through m06 and all 11 subsystem scenarios passed
  with unchanged source and binary identities.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged
  fixtures, thresholds and timeouts; minimum heap 7,816 B and stack 8,704 B.

Actual BattleVisualEffects/draw_battle captures use the same input and frame
219 of a prepared immune Wrap battle. Both show enemy HP 450 and four RNG
draws. Before, the second action skips directly to “can't move!” at Attack
stage 0. After, it shows “RAGE is building!” at stage 1. Both PNGs were
visually inspected. The parent capture's production source is unchanged by
the later test-only PR #407 CI follow-up; verification.json records that link.

Reproduce the original dispatcher observation with PyBoy and matching assets:

```sh
python original_oracle.py --rom original.gbc --symbols pokered.sym --state prepared.state --output observed.json
```

This ROM check enters an isolated dispatcher with prepared RAM, registers and
stack. An existing RETI opcode enables VBlank for its HP animation; read-only
hooks observe the two effect entries. It does not execute two complete
original turns or validate the later stat-animation timing. The native
captures do not validate the full game/audio clock, and these regressions
are not a new m49 playthrough. Trapping continuation announcement text is a
separate recorded gap.
