# Retry PP Up after selecting a capped move

On master `117a326a5a442d47951203c59760c780ea8c82c7`, selecting a move
that already has three PP Ups prints the generic no-effect message and
returns to the overworld. The original names the capped move, then opens
the same Pokemon's move selection again, starting at the first move.

## Original evidence

Audited pret/pokered revision:
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.

- `engine/items/item_effects.asm:2001–2006`: the three-use check prints
  `PPMaxedOutText`, then jumps to `.chooseMove` without consuming an item.
- `.chooseMove`, lines 1968–1984: reset `wPlayerMoveListIndex` to zero and
  enter `MoveSelectionMenu` again for the same selected Pokemon.
- `data/text/text_6.asm:140`: the message names the move via the string
  buffer and says its PP is maxed out.

This source evidence establishes message and control flow. It does not
establish original-ROM rendered timing or typing-speed parity.

## Matched native input

The fixture has a level-12 Bulbasaur with Tackle at 1 PP / three PP Ups,
uncapped Growl at 40 PP, and two PP Ups in the bag. Both sides use actual
START → ITEM → USE → party → move input; the initial constructed state is
not changed after boot.

Before/after input is identical through refusal and confirmation:

| Frame | Master | Fixed branch |
| --- | --- | --- |
| 400 | Choose Tackle | Choose Tackle |
| 616 | Generic no-effect text over the field | Named maxed-PP notice over the party |
| 752 | Idle overworld | Same Pokemon's move selection, cursor 0 |

After refusal, saved snapshots are byte-for-byte equivalent as parsed
JSON: Tackle PP/PP-Up count and both items remain unchanged. The fixed
run then selects Growl: PP becomes 47, its PP-Up count becomes one, and
exactly one item is consumed. Tackle remains untouched.

Both sides repeat twice with identical states, actions, and screenshot
bytes. Before uses the frozen PP-Up numeric-fix binary whose production
sources match master above. After is rebuilt from the branch after
integrating that master. Binary hashes, screenshot hashes, and the
compact observations are in `replay-comparison.json`; full local records
are in `.task-tmp/fidelity-scan-pp-up-maxed/{before-117,after-839-master117}`.

## Regression coverage

All 2848 core library tests pass on the updated branch. Three application
tests drive the actual bag/party/move entry, including A/B confirmation,
a capped later slot resetting to the first move, successful reselection,
and cancellation through party back to the bag without consuming an item.
The debug application builds, and a fresh power-on m01–m06 run passes.

The numeric PP-Up behavior was fixed independently in PR #227. This PR
only changes the capped refusal message and retry flow. Other no-effect
paths and the existing HM refusal flow keep their current behavior.
The shared move-choice notice's typing timing remains unaudited; these
checks are not a complete visual-fidelity verdict.
