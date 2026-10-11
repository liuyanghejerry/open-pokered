# Bag protected swap and physical inventory commit, standalone221

Actual master61a1e66 swaps only the screen list, leaving save_data inventory
unchanged; exiting and reopening restores the old order. It also omits both
original20-frame waits. Same-kind merges incorrectly keep the first slot rather
than the second. Primary HandleItemListSwapping waits20 after marking, then20
before a valid second swap. It combines into the second slot, erases the first,
and resets current row/scroll0; overflow keeps first remainder and second99.
AddItemToInventory_ explicitly creates overflow slots, so duplicates are supported
in the original, although these recordings seed them rather than acquire them.

Repair: two private wait fields freeze all input, then emit one ItemsReordered
commit; validated Inventory::replace_item_slots atomically preserves physical
order and duplicates. Game commits at that event and rolls the screen back to
authoritative inventory on validation failure. Existing same/CANCEL SELECT and
A/B priority rules from PR161 are retained. Three production files (Bag, Inventory,
Game); render/menu changes only its cached/full test to let the protected mark
wait finish before testing a real cursor move. No font/Chinese/dialogue-box
layout, PC or other menu changes are included.

This branch is stacked on PR161af67939. Before is actual master61a1e66 production
with only the identical owner helper appended, independently compiled/frozen.
After frozen source hashes match source7893247. Own latest master integration CI
must be rerun after PR161 merges; stacked green is not that proof. Do not merge
before dependency and all own final CI/GBA gates are green, with no threshold edits.

Original controlled field fixture records brief B/A/DOWN/SELECT at mark+10..11:
all ignored by the20-frame delay, mark1/cursor0 retained, no list exit. Four cases
350 frames twice yield2800 PNG and full hook/state data byte exact. Valid two-item
swap and duplicate merge/overflow each350 frames twice add2100 PNG. Second SELECT
is299; physical inventory first changes319 exactly20 later. Merge becomes
Antidote4/Potion7 with row/scroll0; overflow Potion41/Antidote4/Potion99 with row2.
The mark byte changes before the delay, while the already drawn marker is held;
we do not equate that transient raw byte with frontend phase or PPU timing.

Actual Game starts on collision-checked Viridian Center4,4, seed42, English/Medium,
injured Bulbasaur5 and direct Bag entry. First SELECT0, movement40/64, second90:
inventory unchanged through109 and committed110; B120 exits, DOWN130/A138 reopen
and preserve physical slot order. Four brief interference cases start10 and stay
marked with row0. Before normal regression fails at premature phase change90;
raw data show displayed swap90 but saved order old, and reopened138 old again.
Expanded7 cases each161 frames twice on both sides produce4508 PNG, full JSON
byte exact within-side. Every frame compares every retained/full pixel. Core2724
and app200 pass (24 opt-in captures ignored), including PR161/field Bag regressions,
exact20-frame once-only commit and atomic validation/duplicate tests. Images use
matched states/input/frame: wait100, reopen145, merge115, early direction16.

Evidence archive retains all9408 original/current PNG paths losslessly via SHA256
blobs; every PNG and every other file is read back and compared byte-for-byte.
Complete raw JSON, hooks, primary assembly, source/hash manifests, scripts and
full logs are included. Initial5-case baseline evidence is retained as diagnostic
in the workspace and its verification/log is archived; final symmetrical evidence
uses expanded7-case helper on both sides. No ROM, SRAM, emulator state or binary
is published. These are controlled runtime probes, not natural mainline traversal,
a complete SAVE/CONTINUE proof, or absolute originalCPU/PPU pixel/frame alignment.
Broader held-repeat/scroll/cursor-return and other fidelity audits remain open.


## Current master integration 591480e

The paired images above have been replaced with actual master591480e and
this independently frozen candidate using the identical helper, fixture,
inputs and relative frame. Production master is unchanged: only cfg(test)
helpers are appended. Full source/binary hash manifests, complete fresh
core/app regression logs, negative master owner test, and 4508
PNG with complete raw JSON are in integration-591480e.zip; every archive
file is read back and byte-compared. Both repetitions are byte-exact,
every recorded cached frame matches every full-redraw pixel. Earlier
evidence.zip remains historical and is not claimed as this integration.
Latest doc commit CI must be green before merge; stacked swap still
requires its list dependency and own master integration checks.
