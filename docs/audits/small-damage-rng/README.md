# Skip variance draws for typed damage 0 or 1

Original RandomizeDamage returns before calling BattleRandom when damage is
0 or 1. The native hook drew its variance byte before calculating typed
damage, advancing the battle stream for immunity, resistance rounded to zero,
and one-damage hits. That changed later critical, accuracy and effect rolls.

Calculate typed damage with the maximum multiplier first, then sample and
apply variance only when that damage exceeds 1. Damage 2 still draws even
when the multiplier reduces the final result to 1.

Reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm:5420–5454` (RandomizeDamage).

## Verification

- Original-ROM execution: 768 prepared RandomizeDamage calls, covering every
  first byte for initial damage 0, 1 and 2 with fallback byte 255. All 512
  damage-0/1 cases consume no random byte. The producer observes actual ROM
  execution without reimplementing the formula.
- Three public-FIGHT tests cover 2,048 prepared turns with presentations
  drained: both sides, every first byte, immunity, resistance rounded to zero,
  one damage and two damage. The zero/one tests fail on the parent; the
  two-damage control already passes. Expected two-damage outcomes and counts
  come from the original observations.
- Full core: 3,747 passed, one existing ignored. Full app library: 393 passed,
  38 existing ignored. The Hyper Beam immunity test now expects only critical
  and accuracy draws. The secondary-status fixture uses a level-40 attacker
  so its draw assertions continue to isolate secondary guards above the
  one-damage boundary; the new exhaustive tests cover that boundary directly.
- Fresh desktop NEW GAME through m06 and all 11 subsystem scenarios passed
  with unchanged source and binary identities.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged
  fixtures, limits and timeouts. Minimum heap: 7,816 B; stack: 8,704 B.

Actual BattleVisualEffects/draw_battle captures prepare a sleeping Snorlax
with Ground conversion, then use Thunderbolt followed by Ice Beam. Identical
inputs and frame 795 show enemy HP 430 with eight draws before, versus HP 416
with five draws and “Critical hit!” after, from HP 450 initially. The first
immune move no longer consumes three rejected/accepted variance samples.
The input traces are compared exactly and hashed; both PNGs were inspected.

Reproduce the original observation with PyBoy and matching local assets:

```sh
python original_oracle.py --rom original.gbc --symbols pokered.sym --state prepared.state --output observed.json
```

The ROM check is an isolated subroutine with prepared RAM, registers, stack
and link seeds. Native tests use prepared working stats and conversion types.
The captures do not run the full game/audio clock, and these regressions are
not a new m49 playthrough. Overall critical/accuracy/damage phase ordering and
the existing 65-draw rejection fallback remain separate recorded gaps.
