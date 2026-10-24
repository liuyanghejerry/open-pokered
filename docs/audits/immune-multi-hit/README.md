# Stop multi-hit dispatch after type immunity

An immune multi-hit attack previously entered the repeat effect despite dealing
zero damage. Variable-count moves consumed hit-count RNG and replayed failed
action presentations. A prepared Fury Attack against Ghost repeated the failed
action four times before returning to the menu.

The multi-hit bridge now returns when the action has missed, including type
immunity. Its existing first-hit Substitute-break guard remains in place.
Successful hits still perform their complete sequence.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm:3218–3227,5615–5624`: the missed-move path returns before
dispatching the multi-hit effect.

## Verification

- Four public-FIGHT tests resolve and drain 238 prepared turns. The eight moves
  with possible type immunity cover both sides, with/without Substitute and
  Rage. Controls cover ordinary misses, next-turn cleanup, and all ten moves'
  successful 2–5-hit sequences.
- The identical final test source gives two passes/two failures on the parent;
  all four pass on the candidate. Full core: 3,724 passed, one existing ignored.
- Fresh native NEW GAME through m06 and all 11 subsystem scenarios passed with
  source and executable identities unchanged.
- Seven GBA benchmark cases and 13 memory cases passed with unchanged fixtures,
  limits and timeouts. Minimum free heap: 7,816 B; stack margin: 8,704 B.

`capture.rs` uses prepared parties, deterministic RNG, public FIGHT and the
actual `BattleVisualEffects`/`draw_battle` renderer. At frame 80, the parent is
repeating Fury Attack's failed action while the candidate has returned to the
menu. Both retain all 450 target HP. The parent completes at frame 176 in the
same fixture. Both PNGs were visually inspected; `verification.json` records
the traces and identities.

The capture does not test the full game/audio clock or the original ROM, and
these fixtures are not a new full m49 playthrough. RNG assertions compare an
immune multi-hit move with an immune single-hit control; they do not certify
the whole RNG pipeline. Ordinary secondary effects on immune targets and other
original RNG byte/order discrepancies remain separate audit work.
