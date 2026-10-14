# Prize dialogue fidelity

Original `engine/events/prize_menu.asm` exits directly for B or list index 3; `OhFineThenText` is used only for NO in the prize confirmation. `HereYouGoText` is explicitly unreferenced; successful delivery subtracts coins and redraws the balance. The scene incorrectly emitted both extra dialogues.

The regression fails on master at list index 3 and passes after the scene correction. All 63 `fidelity_` core tests pass, including confirmation and payment only after successful delivery for every prize in both game versions. This is scene interpreter evidence, not full storage/award-flow coverage.

Actual headless input starts at GameCornerPrizeRoom (2,3), seeds only COIN_CASE, interacts with counter 1, and selects NO THANKS. Identical inputs and 438 recorded frames produce the comparison at frame 437: master remains in ShowDialogue; fixed version has returned to player control. State traces retain both observations. Successful delivery dialogue removal is covered by scene tests; the captures show list cancellation only.

The native app debug-server build passed. CI must pass on the final PR head before merge. Other menu timing prototypes are not part of this branch.
