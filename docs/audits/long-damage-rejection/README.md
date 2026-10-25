# Damage variance rejection

Original RandomizeDamage retries until a rotated random byte is at least 217.
The port stopped after 65 draws and clamped a rejected byte to 217. Actual
link streams can need more draws, so that fallback changed the damage factor
and the later random stream.

Remove the arbitrary cutoff. The zero/one-damage early return and byte
rotation stay intact. Update 43 existing unit fixtures to provide accepted
variance samples; their former all-zero ScriptedRng tails never terminate
under the original loop. Keep all existing behavioral assertions. Disable's
one-turn fixture specifies its duration byte explicitly.

Original reference: pret/pokered
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm`, RandomizeDamage.

## Verification

- 256 original-ROM RandomizeDamage cases: uniform 10-byte link seed arrays,
  typed damage 2, initial list index 2. 60 cases need more than 65 draws; the
  maximum is 206. This is a targeted input matrix, not a frequency estimate.
- 510 prepared public-FIGHT turns on both sides, raw seeds 0–254. The pre-fix build
  differs in random consumption in 120 cases; damage at this rounding boundary
  agrees. The long-sequence test fails on the pre-fix build and the short control
  passes. Both pass after the fix, with presentations drained.
- Raw 255 is excluded from the native matrix because the current native
  accuracy check misses before variance. Overall phase ordering is a separate
  confirmed gap; this change does not claim to repair it.
- Full core: 3,753 passed, one existing ignored.
- Full app library: 393 passed, 38 existing ignored.
- Fresh desktop NEW GAME through m06 and all 11 subsystem scenarios passed
  with unchanged source and binary identities.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged
  fixtures, limits and timeouts; minimum heap 7,816 B and stack 8,704 B.

The actual renderer uses identical inputs and frame 139. A prepared enemy
Tackle critical hit leaves the player's Snorlax at 668/999 HP before and
665/999 HP after, with 67 versus 100 total RNG draws. Both PNGs were
inspected. This visible native difference is separate from the damage-2
original subroutine matrix and is not a full original-turn comparison.

Reproduce the original subroutine observation with PyBoy and matching assets:

```sh
python original_oracle.py --rom original.gbc --symbols pokered.sym --state prepared.state --output observed.json
```

The original check prepares RAM, registers, stack and link seeds, then calls
RandomizeDamage with the actual BattleRandom implementation. Read-only hooks
count calls across list refills. It does not run an entire original battle.
The native tests use level 1/Attack 1 versus Defense 250 to keep typed damage 2
even on critical hits. Captures use the actual battle renderer with prepared
combatants; they do not validate the full game/audio clock or a new m49 run.
