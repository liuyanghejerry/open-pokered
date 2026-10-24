# Hyper Beam against an immune target

A Hyper Beam aimed at a Ghost-type target could incorrectly install recharge,
forcing its user to forfeit the following turn. The recharge hook now respects
the move's failed-hit flag. Ordinary misses, knockouts, and Substitute breaks
continue to skip recharge; surviving hits and intact Substitute hits still
require it.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm:3218–3243,5615–5643`,
`engine/battle/effects.asm:1171–1180`, and
`data/battle/always_happen_effects.asm`. The missed path returns before
HyperBeamEffect can set NEEDS_TO_RECHARGE.

## Verification

- Three public-FIGHT tests cover 34 prepared turns on both sides: Gastly,
  Haunter, Gengar, Conversion to Ghost, ordinary hit/miss, Substitute intact or
  broken, and knockout. The 32 non-KO turns drain their presentations. The
  immunity test fails on the parent; both control tests already pass there.
- Full core: 3,739 passed, one existing ignored.
- Fresh native NEW GAME through m06 and all 11 subsystem scenarios passed,
  with unchanged source/binary identities.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged fixtures,
  limits and timeouts. Minimum heap: 7,816 B; stack margin: 8,704 B.

The prepared capture uses public FIGHT, actual `BattleVisualEffects` and
`draw_battle`, with identical input and frame 240 for both builds. After the
same first immune Hyper Beam, the old build says `MEWTWO must recharge!`;
the repair says `MEWTWO used HYPER BEAM!` on the second turn. Enemy HP remains
450 in both. Both PNGs were visually inspected; `verification.json` records
source identities, hashes and traces.

These are prepared-state checks plus the stated native regressions, not a new
full m49 playthrough. The captures do not execute the full game/audio clock or
original ROM. Damage RNG rotation/order remain separate findings.
