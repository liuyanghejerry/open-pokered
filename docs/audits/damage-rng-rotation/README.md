# Damage random-byte rotation

Original RandomizeDamage rotates every sampled BattleRandom byte right once
before accepting values 217–255. The native damage hook used the raw byte,
changing both damage and the number of rejected draws. Apply the rotation to
the initial sample and every retry.

With pre-variance damage 200, raw byte 179 produces 170 damage in the original;
the parent rejected it and used the fallback 255, producing 200 damage. Raw
217 should produce 185 damage, while the parent produced 170.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm:5420–5454`.

## Verification

- Original-ROM RandomizeDamage execution in PyBoy: 268 prepared subroutine
  cases, covering all 256 first bytes, nine small-damage controls, and three
  sequences requiring retries. The producer observes the ROM without
  reimplementing its formula.
- Two public-FIGHT tests cover 518 prepared turns and drained presentations:
  all 256 bytes on both sides plus all three retry sequences on both sides.
  Expected damage and draw counts come from the observed ROM results. Both
  tests fail on the parent and pass with the repair.
- Full core: 3,741 passed, one existing ignored.
- CI exposed an existing trainer-Pokédex fixture whose two AI switches depended
  on the old damage draw sequence. Give its prepared opponents Growl so the
  test still drives real AI switches through normal game input without damage
  variance or a player KO. The original fixture also failed locally; after this
  test-only adjustment, all 393 app library tests pass (38 existing ignored).
- Fresh desktop NEW GAME through m06 and all 11 subsystem scenarios passed
  with unchanged source/binary identities.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged fixtures,
  limits and timeouts. Minimum heap: 7,816 B; stack margin: 8,704 B.

The capture prepares working Attack 203 and Defense 30 to isolate pre-variance
damage 200, then uses public FIGHT and actual BattleVisualEffects/draw_battle.
Identical input and frame 601 show target HP 250 before and 280 after, from
450 initially. Both have returned to the menu. Both PNGs were visually
inspected; verification.json records hashes and traces.

Reproduce the original oracle with PyBoy and matching local assets:

```sh
python original_oracle.py --rom original.gbc --symbols pokered.sym --state prepared.state --output observed.json
```

The ROM measurement is an isolated subroutine with prepared RAM/registers,
stack and link seeds. Native seeds still follow the current accuracy/damage
phase order. Skipping variance draws for damage 0/1 and overall RNG ordering
remain separate findings. These captures do not run the full game/audio clock,
and these regressions are not a new full m49 playthrough.
