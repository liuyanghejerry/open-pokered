## Latest integration on master61a1e66

Current integration evidence: actual master61a1e66 vs frozen source cd8a4fc8ad76781f1fddee3feeae637348feda59.
Full core 2722 / app199 pass (23 opt-in capture helpers ignored).
All 2904 PNG paths plus complete raw JSON/logs/source are read back from the
lossless SHA256 PNG archive and compared byte-for-byte. Both sides repeat exactly;
every frame compares all retained/full pixels. Baseline Game production prefix
is unchanged, with identical test-only helpers. Same inputs/state/frame screenshots
are published above. No ROM/SRAM/emulator state/executable is distributed. These
are controlled collision-checked owner fixtures, not natural mainline traversal.
Latest own CI and unchanged GBA gates must independently pass before merge.

## Historical evidence retained

# Bag USE/TOSS selection, independent repair 218

Actual master 59d519f has an extra CANCEL row and processes simultaneous UP+DOWN
as DOWN at the top. The original USE_TOSS template has exactly USE and TOSS,
maximum cursor 1, no wrap, and HandleMenuInput gives UP priority even at the top.
The repair changes only action selection and its full/cached rendering. B still
cancels; action A+B still gives B priority. Quantity B already returns to the
list and is unchanged. English rectangle (13,10)..(19,14), labels X15 and cursor
X14 at rows 11/13 follow the primary template. Existing Chinese rectangle and
font spacing are preserved. Fonts, translations and wider menu flows are outside
this change. Production changes touch core/bag_screen.rs and app/render/menu.rs;
app/game.rs only adds an actual Game regression/capture helper.

The original controlled pre-PC emulator fixture uses Potion3 and real START,
item selection, then six input cases: down-down, up, down-up, up-down, A+B and
down+A. Each records 370 frames twice: 4440 PNG and full raw state/hooks repeat
byte exact. The maximum remains 1; down-down/down+A open quantity1; UP+DOWN stays
at USE; A+B returns to the list. This is not natural traversal or a claim of
absolute original CPU/PPU to native frame/pixel equivalence.

Baseline production is actual master59d519f, with the byte-identical helper only
appended to Game; its original production prefix hash is recorded. Candidate
source eb72e7d is frozen and its three file hashes checked before packaging.
Independent targets/builds avoid stale shared artifacts. Both sides use the
same actual Game, seed42, English/Medium, injured Bulbasaur5/Potion3, collision-
checked Viridian Center floor4,4 warmed120 frames, and direct Bag entry. Six
cases each capture121 frames twice on each side (2904 PNG). All PNG and full
JSON repeat byte exact; every frame asserts retained/full drawing pixel equality.
Baseline owner test fails with cursor2 rather than1. Final full core2722 and
app198 tests pass (22 opt-in helpers ignored). Before/after screenshots use the
same state/input/frame: initial menu4, cursor16, toss result30, UP+DOWN result30.

Initial a1 full tests exposed a stale cached cursor coordinate; both full and
partial coordinates were corrected, without weakening pixel equality. The a2
integration build exposed an incomplete test-module delimiter, corrected by
keeping both complete owner modules. All failed logs remain in the evidence;
a3 build and full tests pass. The archive includes primary assembly, scripts,
full raw records, source/hash manifests and logs. PNGs are SHA256 deduplicated
losslessly; all7344 original PNG paths and all other files are read back and
compared byte-for-byte. No ROM, SRAM, emulator state or executable is published.
Current-head CI and unchanged GBA performance gates must pass before merge.
