# PP Up preserves spent PP

On master `862bb216d4733e1f361f615447fe3aab74e49982`, using a PP Up on
Tackle at 0/35 PP fills it to 42/42. The original increases both capacity
and current PP by seven, producing 7/42: the 35 already-spent PP stays spent.

## Original rule

The audited pret/pokered source is
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, `engine/items/item_effects.asm`:

- `ItemUsePPRestore`, lines 2000–2018: increment the packed PP-Up count,
  set `wUsingPPUp = 1`, then call `RestoreBonusPP`.
- `RestoreBonusPP` / `AddBonusPP`, lines 2380–2455: restrict the operation
  to the selected move and add `min(base_pp / 5, 7)` **once** to its current
  PP. Using a second or third PP Up does not add earlier bonuses again.

Two identical sets of 669 isolated original-ROM `AddBonusPP` calls cover
all legal base PP values (5 through 40 in steps of five), 0–2 existing
PP Ups, and every legal current PP from zero through the old maximum.
Every packed result preserves the PP-Up bits and adds the single bonus.
These are isolated function calls with IRQs disabled and an exact stack
return hook; they do not constitute an original-ROM bag playthrough.
Synthetic base PP 0/1 cases were recorded separately and excluded from the
669 legal cases.

## Native input and persistence evidence

Constructed saves contain one level-12 Bulbasaur and two PP Ups. Each run
uses real input through START → ITEM → USE → party → move, dismisses the
result, then opens POKEMON → STATS → the moves page. START is held for four
frames so it reaches the field input sampling window. There are no state
writes after boot. After applying exactly one item, the ordinary save is
read back to verify PP, the PP-Up count, and the remaining item quantity.

| Move | Initial PP | Existing PP Ups | Master result | Fixed/original result |
| --- | ---: | ---: | ---: | ---: |
| Tackle | 0 | 0 | 42/42 | 7/42 |
| Tackle | 10 | 1 | 49/49 | 17/49 |
| Tackle | 10 | 2 | 56/56 | 17/56 |
| Growl | 0 | 0 | 47/47 | 7/47 |
| Growl | 10 | 1 | 54/54 | 17/54 |
| Growl | 10 | 2 | 61/61 | 17/61 |

All six cases repeat twice on each side with identical observations and
PNG hashes. Before/after actions match, and all captures are frame 1040.
The saved snapshots differ only in the chosen move's current PP. The
illustrated Tackle comparison changes 25 pixels, all within the current
PP digits at x=112–120, y=83–89. Capacity, PP-Up count, and consumption of
exactly one item match. See `replay-comparison.json` for binary/script
hashes and the complete case summary.

The baseline frozen binary is from PR #226's head; its game sources are
identical to the merged master commit above. The fixed binary differs by
the PP-Up calculation. The local replay evidence is retained under
`.task-tmp/fidelity-scan-ppup/{before,after}-stats-v3`, with second repeats
in the corresponding `-repeat2` directories.

## Regression coverage

22 PP restoration tests and all 2847 core tests pass; the debug application
build passes. The two new regressions cover spent PP on the first through
third use, unchanged other move slots, and low/high base PP bonuses.
Existing coverage still checks full PP, the three-use limit, and empty
slots. This numeric item fix does not establish complete game fidelity or
a fresh first-clear result; broader main-flow validation is tracked
separately.
