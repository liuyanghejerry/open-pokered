# Empty-party START menu: standalone split 200

Based on actual master748c221. Only StartMenuState item visibility and empty
party selection change. No field-clock, three-frame initialization/redraw,
party restore, input sampling or renderer rewrite is imported.

Original DrawStartMenu prints POKEMON unconditionally, six entries without
Pokedex/seven with it. StartMenu_Pokemon checks wPartyCount and jumps back
to RedisplayStartMenu if zero. This split keeps the entry and stays on START
when A selects it with no party; after receiving a party member it opens Party.
Core tests cover both Pokedex states, item order, A and subsequent party entry.

The current standalone owner test uses a controlled English empty-party Game,
Pallet(5,5), directly opened StartMenuState without Pokedex. A occurs at frame1;
all8 rendered frames compare retained/full pixels. Master's first item is ITEM
and A opens Bag; repair's first item is POKEMON and A keeps START. Both sides
use the identical helper/inputs/frame index and repeat twice; all8PNG+JSON are
byte-exact per side. No global RNG claim and no field-opening timing claim.
Frozen binary/source hashes, all raw frames and logs are in evidence.zip.
Core2708/app191 tests pass (15app intentionally ignored). GBA is gated by this
PR's own latest-head build/performance CI without relaxing budgets.

Historical106 original ROM recordings/probe and original assembly are retained
as source evidence. Its older Native/GBA claims are excluded. Three-frame
redisplay and field input boundaries remain with the larger movement/input
foundation split; this PR addresses menu availability/action only. No new full
mainline or independent Continue result is claimed. No ROM/binaries published.

## Preview CI follow-up

CI on d1d98b1 failed only two UI-preview layout-mutation tests. Six menu rows
make their old width/height operands visually equivalent after clipping/natural
size. Fixed width8 and min-height16 exercise visible changes; both existing
pixel inequality assertions remain. Full preview suite58PASS/1ignored. Tests
and mock comment only changed; production renderer, screenshot inputs and images
are unchanged. The negative CI excerpt and successful local log are archived;
verification pins the added test source/binary. Latest-head CI must pass again.
