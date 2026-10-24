# Secondary status preconditions

Original PoisonEffect and FreezeBurnParalyzeEffect reject an existing status or
an immune target type before drawing the secondary chance byte. The latter
compares the move type, including Body Slam against Normal types. See original
revision `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/effects.asm:78-118,194-227,256-280`.

The production bridge now checks the current battle types, including Conversion,
before the roll. CheckDefrost also respects the preceding fainted-target and
intact/just-broken Substitute guards. An eligible, surviving frozen target still
thaws after a Fire hit, without consuming a secondary chance byte. Twineedle's
nested final-hit rider is separate and is not changed by this patch.

The public FIGHT tests finish 520 prepared turns: 16 canonical moves, both sides,
eight conditions and two chance values (512), plus eight Conversion controls.
They assert status, exact random consumption and defrost caption count. The
parent fails 240 matrix cases (192 extra draws and 48 improper defrosts) and four
Conversion cases. Two older immune-target fixture streams were corrected to
omit the nonexistent chance byte, with explicit six-draw assertions added.

The full core suite passes 3,698 tests with one existing ignored test. A fresh
native NEW GAME through m06 and all 11 independent battle/items/menu/save
scenarios pass with source and executable identity held fixed. All seven GBA
performance scenarios and 13 memory scenarios pass, including 330 move
animations; minimum contiguous heap is 7,832 bytes and stack is 8,708 bytes.
No gate, workload or timeout was relaxed. The completed memory log was losslessly
compressed and round-trip verified. Details and hashes are in verification.json.
These are scoped regressions, not a new full first-clear run.

Both screenshots use the same prepared public FIGHT and frame 171. Ember knocks
out the frozen target. Before still prints an impossible defrost; after skips
that caption and retains Freeze on the fainted target. The capture uses actual
BattleVisualEffects and draw_battle. It does not drive the outer PokemonGame or
audio clock and is not an original-ROM timing comparison. The committed capture
source differs from the executed source only by removed trailing whitespace.
All three remote workflows and six actual GBA runtime steps must pass before
merge.
