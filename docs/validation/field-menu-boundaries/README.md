# Field choice endpoint and cancel rules

Original `home/window.asm::HandleMenuInput` clamps UP/DOWN when
`wMenuWrappingEnabled` is zero, tests UP before DOWN, and clears wrapping on
return. Fossil (`engine/events/cinnabar_lab.asm::GiveFossilToCinnabarLab`)
and drink (`scripts/CeladonMartRoof.asm::CeladonMartRoofScript_GiveDrinkToGirl`)
callers do not enable wrapping and test B before accepting the returned index.
Ordinary field YES/NO choices use the same bounds and cancellation rules.
Reference: pret/pokered revision fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c.

Previously UP at the first fossil/drink selected the last item; A+B accepted
instead of cancelling. Scene choices also wrapped, including the drink girl's
YES/NO question. Filtered menus and native scene choices now clamp at both
ends, preserve UP priority, and give B priority over A. Floor menus keep their
existing separate policy. Confirmation audio still plays exactly once.

The two persistent JSON fixtures are constructed offline, not fresh-route
saves. They place the player on a walkable tile near a walking NPC, contain all
three relevant items, and leave the six reward/revival flags clear. Replays
boot through ordinary Continue, locate the actual current NPC position, and
use normal buttons to interact. No runtime state setters or debug SAVE are
allowed. After testing downward overflow and cancelling, the replay reopens
the menu, presses UP at the first item, confirms, and checks the actual reward.
The doctor confirmation is also accepted through normal input.

Before/after screenshots use identical inputs, seed42 and speed0. Fossil
TOP captures are frame2047; drink TOP captures are frame1663. Baseline chose
Old Amber/Aerodactyl and Lemonade/TM49; fixed chose Dome Fossil/Kabuto and
Fresh Water/TM13. Both versions retain the bag on B cancel. The girl's initial
YES/NO question also proves that UP wraps to NO before, and stays YES after.
Two UP taps restore YES in both versions, preserving the matched timeline.

Native DSL tests check actual 1/2/3-option continuations and A+B cancellation
branch receipts; the two generic tests fail before the fix. Filtered tests
cover counts1/2/3/11 and simultaneous key priority. Complete core library
regression: 3,133 passed, including the updated B-priority audio control.

Build ordinary base and candidate CLI binaries with desktop,debug-server,
then run from a checkout containing this proof package:

```bash
python3 docs/validation/field-menu-boundaries/replay.py before /absolute/base/pokered-app /new/before-dir
python3 docs/validation/field-menu-boundaries/replay.py after /absolute/fixed/pokered-app /new/after-dir
```

The driver stops its owned game after assertions (exit -15); this is bounded
menu/story validation, not natural game completion or a fresh playthrough.
Evidence concerns settled-menu bounds and key priority. Initial/cursor poll
cadence and menu rendering/backing parity remain separate audit items; the
full-screen white menu shown in the captures is not evidence of layout parity.


After synchronizing onto master25709ad8, the ordinary CLI base build531039ff
has byte-identical production sources (crates/scripts/Cargo manifests/gfx).
The rebased candidate build and hashes are recorded in evidence.json. Both
actual fossil/drink replays pass again, with matching before/after frames;
3133 full core and10 field-move continuation tests pass on rebased source.

A separate full NEW GAME run at pre-rebase sourcec4164f22 completed all49
milestones: HOF1, THE END, title and a separate process Continue to PalletTown
with all8 badges. It began without a persistent save and used no runtime
setters/debug SAVE. This validates that candidate's route and saved return;
it is not relabeled as a run of the rebased source or final merged master.
