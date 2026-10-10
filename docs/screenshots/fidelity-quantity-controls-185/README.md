# Quantity chooser controls (185)

Original `home/list_menu.asm::DisplayChooseQuantityMenu` wraps UP from the
maximum to 1 and DOWN from 1 to the maximum. It checks A, B, UP, DOWN in that
order. The PC and bag clamped at the boundaries; their A+direction input
changed the amount before confirming, and A+B cancelled. The mart already
wrapped, but its adapter also changed/cancelled before confirmation.

PC withdraw/deposit/toss and bag toss now wrap. The three production input
adapters give confirmation precedence over cancellation and direction changes
in the quantity phase. Other mart phases retain their existing controls.

## Evidence

The original ROM fixture and native fixture have four PC Potions and four bag
Potions, English Slow5 and incoming letter flags 1. After list selection, DOWN
at frame93, UP at117 and A at141 take out one Potion in the original/final
native, changing the bag from4 to5. Parent/master take out two (4 to6). Original
and final quantity selection become ready at frame48. Prefixes before list
selection are not claimed frame-aligned. Each original, parent, master and
final recording is repeated twice; every retained raw PNG/JSON is byte-exact
between copies, without masks.

The four screenshots use actual master31b1eda before and final frozen CLI
fbed4143 after, at the same post-list-selection hardware frames94 and118.
Both menus are ready before the tested direction input. Master's earlier
quantity opening was fixed separately in181; the displayed number differences
here demonstrate the boundary controls. No font/layout changes are included.

Two original ROM combined-key cases (A+UP and A+B+UP+DOWN), each repeated twice,
confirm the original keeps quantity1. Production core tests exercise PC's
three modes, bag toss and mart buy/sell with combined key input, retaining
stock/money/transaction assertions. The actual Game input bridges pass these
button flags to the tested adapters. These are not native CLI combined-key
recordings. PC/bag boundary tests cover stacks1,4,99. All five new regression
tests fail on the parent and pass on the repair.

The owning Game/RenderSession test drives DOWN to4, UP to1 and B back to the
list, compares every160x144 pixel with a full render after every update, checks
idle reuse and unchanged inventories. Final native suites pass: core2756,
data263, app230 (34ignored), agent54, audio99. Exact-source GBA production and
benchmark release builds pass; all31 original performance budgets pass.
Movement draw peak3078 is below3113.1. The unchanged IWRAM cap/stack reserve
holds: end0x030051f0 (20976bytes).

`verification.json` pins all63 synchronized GBA source hashes, frozen binaries,
archive and screenshots. `evidence.zip` contains raw recordings, scripts,
fixtures, original/final source, build/test/performance logs, including the
initial test-fixture type error. The ROM and executable binaries are not
bundled. These are seeded subsystem tests; no fresh mainline or independent
CONTINUE result is claimed for185. The separate frozen184 run validates PR144.
