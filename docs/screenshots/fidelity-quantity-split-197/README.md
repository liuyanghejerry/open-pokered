# Quantity controls: standalone split 197

This PR contains quantity controls only, based on actual master `748c221`.
Original `home/list_menu.asm::DisplayChooseQuantityMenu` wraps DOWN from 1
back to the stack maximum and UP from maximum back to 1, checking A, B,
UP, DOWN in that order. PC withdraw/deposit/toss and bag toss now wrap.
PC, bag and mart quantity adapters prioritize A over cancellation/directions.
Mart already wrapped; its remaining flow is unchanged.

## Current standalone evidence

Actual master and final repair use the identical controlled Game/RenderSession
capture helper: four Potions in bag and PC, English, audio disabled, real PC
message/menu/list input followed by quantity input DOWN at frame1, UP at9,
DOWN at17, B at25. Each of three modes records forty frames. Before/after
screenshots share inputs and frame indices. Before quantities are 1,2,1;
after quantities are 4,1,4. B returns to the item list; both inventories stay4.
Every update checks all160x144 retained pixels against full rendering.
Each side is repeated twice: all120 PNG and all3 JSON files match byte-for-byte.
JSON reports quantity/phase/inputs and invariant inventories, not global RNG.

Five source_quantity core tests cover stacks1/4/99, allPC modes, bag toss,
mart buy/sell, simultaneous keys and stock/money behavior. Final core2712
and app191 tests pass (15 app tests intentionally ignored). The normal owner
pixel test passes; the ignored capture is invoked explicitly for screenshots.
App-only baseline/default desktop and core+app repair select the same features:
core has no default features. No source edits occurred during either build.
Frozen binary/source hashes and raw recordings are in verification/evidence.

The archive retains original ROM quantity/chord recordings and assembly from
historical185, including both repeats. Those recordings establish original
controls; older broad-branch Native/GBA results are not presented as results
for this independent PR. No ROM or executable is published. GBA release and
performance checks must pass on this PR's exact head without changing budgets.

Bag's subsequent toss confirmation and PC YES/NO defaults are separate audit
items; this PR does not claim to repair those flows. No fresh mainline claim.
