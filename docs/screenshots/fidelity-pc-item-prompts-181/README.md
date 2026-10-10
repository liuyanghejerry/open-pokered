# PC item questions and input ownership (181)

The English player PC omitted the authored question before its item menu and
item list, and opened quantity selection immediately after choosing an item.
Original `engine/menus/players_pc.asm` prints each question with DONE before
handing input to the next menu. The quantity question is also printed for a
single item. Receipts, quantity cancellation, and both NO/B toss cancellation
return through the list question.

The repair uses the existing field-text clock, leaves the selecting menu behind
the question, and waits for DONE before accepting quantity input. Returning to
the list preserves its cursor. Chinese content/flow and existing font/layout
remain unchanged. The retained frame key includes the item-menu cursor.

## Reproduction and validation

Five controlled source-ROM scenarios cover withdraw at speeds 1/3/5 and
deposit/toss at speed 5. Native before selects quantity at frame 0; final native
opens at frames 12/30/48, matching the original routine-entry cue with the same
post-selection A,A,release controls. All original, before and final recordings
repeat twice with byte-exact raw screenshots and JSON; no pixel masks. The
inventories remain unchanged before any quantity confirmation. Opening prefixes
are not claimed frame-aligned, and these are seeded subsystem fixtures.

Before images come from actual master 31b1eda. After images come from the final
frozen native CLI (including NO/B toss cancellation). Both use one PC Potion,
empty bag, English Slow5 and incoming letter flags 1. Menu/list images use the
same semantic ready state; quantity images use hardware frame +10 after list
selection. The two copies of each native capture are byte-exact.

Final frozen debug suites: core 2751, data 263, app 230 (34 ignored), agent 54,
audio 99 passed. The owning Game/RenderSession test compares all 160x144 pixels
with a full draw on every update through menu/list/quantity/B return and checks
idle reuse. Core coverage retains key-item, capacity, receipt, quantity, cursor,
flags and inventory assertions and adds NO/B toss return coverage.

`verification.json` pins source/binary hashes and archive contents. `evidence.zip`
contains recordings, scripts, source and all trial logs, including discarded
fixture mistakes. Earlier Medium3/flags3 probes are diagnostics only; final
matched fixtures use source options and incoming flags 1. The ROM is not bundled.

## Remaining gates

The final exact-source GBA benchmark builds and passes 30 of 31 unchanged
budgets. Movement draw peak 3122 still exceeds 3113.1; no green performance or
merge-ready claim. The frozen parent179 NEW GAME reached m21, then terminated
at m22 with `face(up) failed: Left`; its state/NPC/flag evidence is retained for
triage. This is not a final-source complete mainline or independent CONTINUE.
The draft PR still needs the remaining text/movement fidelity audits, final
mainline and latest-head green CI before merging.
