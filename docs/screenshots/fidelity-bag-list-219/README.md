# Bag list selection and marked-row rules, standalone219

Original DisplayListMenuID gives A priority over B/SELECT after cursor movement.
UP wins simultaneous UP+DOWN even at the top. A selects an item while a swap mark
exists; B exits the list. HandleItemListSwapping ignores SELECT on the marked item
or CANCEL without clearing the mark. Actual master differs in all these cases.
The repair touches one production file (core/bag_screen.rs); Game only adds an
actual-input regression/capture helper. Existing merge/quantity/field-use/menu
rendering remains governed by its existing code.

Original7 cases (mark-b, mark-a, self-select, cancel-select, A+B, A+SELECT,
UP+DOWN) each480 frames twice produce6720 PNG and full state/hooks byte exact.
Both current native/master record7 cases each121 frames twice (3388 PNG), using
Potion3/Antidote4, seed42, English/Medium, injured Bulbasaur5, collision-checked
Viridian Center4,4 and direct Bag entry. Inventory is unchanged in these cases.
Baseline owner fails at marked B: stays Bag instead of StartMenu. Final actual
Game owner and all suites pass. Screenshots are matched100/4-frame states.

This PR does not complete the entire swap lifecycle: the original20-frame mark/
swap protected waits remain a confirmed gap (separate original4-case evidence),
and persistence of a completed swap into saved inventory needs an actual Game
reopen probe. These pending audits are not claimed fixed by this selection change.
No font/translation/dialogue-box layout changes are included. Historical59-based
build/captures remain retained in the workspace; current evidence below is61.

Current integration evidence: actual master61a1e66 vs frozen source b9f9cc7c809a8e91d822be85fc68a6ec9e9c5973.
Full core 2722 / app199 pass (23 opt-in capture helpers ignored).
All 10108 PNG paths plus complete raw JSON/logs/source are read back from the
lossless SHA256 PNG archive and compared byte-for-byte. Both sides repeat exactly;
every frame compares all retained/full pixels. Baseline Game production prefix
is unchanged, with identical test-only helpers. Same inputs/state/frame screenshots
are published above. No ROM/SRAM/emulator state/executable is distributed. These
are controlled collision-checked owner fixtures, not natural mainline traversal.
Latest own CI and unchanged GBA gates must independently pass before merge.


## Current master integration 591480e

The paired images above have been replaced with actual master591480e and
this independently frozen candidate using the identical helper, fixture,
inputs and relative frame. Production master is unchanged: only cfg(test)
helpers are appended. Full source/binary hash manifests, complete fresh
core/app regression logs, negative master owner test, and 3388
PNG with complete raw JSON are in integration-591480e.zip; every archive
file is read back and byte-compared. Both repetitions are byte-exact,
every recorded cached frame matches every full-redraw pixel. Earlier
evidence.zip remains historical and is not claimed as this integration.
Latest doc commit CI must be green before merge; stacked swap still
requires its list dependency and own master integration checks.
