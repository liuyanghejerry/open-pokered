# Rare Candy can restore a fainted Pokemon through HP growth

The original Rare Candy routine checks level 100, not whether current HP is
zero. After recalculating stats it adds the increase in maximum HP to current
HP, including zero. Native `use_rare_candy` rejected fainted Pokemon before
that routine could run. This correction removes that extra guard and keeps
the level-100 rejection, stat calculation, move learning and evolution flow.

## Original actual menu and save

Reference source is `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, retail Red
ROM SHA-1 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`, PyBoy 2.8.1.
`engine/items/item_effects.asm` `.useRareCandy` / `.addHP` checks the level,
recalculates stats, and adds the maximum-HP difference without an HP-zero
guard. `source-verification.json` records the source case and capture hashes.

The initial constructed state has a fainted level-12 Bulbasaur, DV bytes
`9a78`, Tackle/Growl, and a healthy second Bulbasaur. It contains two Rare
Candies and 3000 cash. All initial writes finish before the input window;
actual START / ITEM / USE / party inputs and ordinary SAVE/YES then produce:

- Level 12 → 13; HP 0/35 → 2/37.
- Stats `[35,18,19,17,22]` → `[37,20,20,18,23]`.
- Moves Tackle/Growl → Tackle/Growl/Vine Whip, PP `[35,40,10,0]`.
- One item consumed; the healthy partner and cash remain unchanged.

An independent SRAM read matches the original post-use record. Its stored
and calculated main checksum are both 221. The inspected original status
image is archived as `rare-candy-fainted-source-after.png`.

## Matched native before and after

Before uses the frozen binary from master `117a326a`; its production sources
also match master `959b3e38`, which only adds navigation tooling. After is
built from the Rare Candy correction. Both use the same fixture bytes and
fixed real button inputs; there are no state writes after boot.

Both builds repeat twice with identical observations, action traces and PNG
bytes. Result notices are frame 640, status pages frame 1216 and move pages
frame 1280. Master leaves level 12 / HP 0 / two items unchanged. The branch
produces the original level, HP, all five stats, StatExp, move list and PP,
and consumes exactly one item. Cash and every healthy-partner field remain
unchanged. The snapshot comparison accounts for the serializer's default
empty nickname and OT-name fields.

Native persistence is checked through the debug SAVE command followed by
SRAM import. That command uses ordinary save serialization, but these native
runs are not SAVE-menu input tests. The original reference above does perform
SAVE/YES. Full native input/state/readback files are retained under
`.task-tmp/fidelity-842-native-candy-v2`; the compact hashes and results are
in `replay-comparison.json`. The first capture assertion compared omitted
JSON default-name fields and used the wrong money key; it was rejected and
replaced by these four complete verified runs.

## Regression coverage

Sixteen selected vitamin/Rare Candy tests and all 2848 core library tests
pass. The corrected fainted case verifies HP growth, learning Vine Whip and
preserving spent PP on existing moves; a fainted level-100 case remains
unchanged. The debug application builds and a fresh power-on m01–m06 run
passes (frame 12213, OaksLab `(5,6)`, overworld without an active effect).

These checks establish this item rule and persistence result. They do not
claim original rendered timing, complete save-format parity, or complete
Rare Candy learning-message/choice coverage. The pre-existing fainted status
label on the native stats page is audited separately.
