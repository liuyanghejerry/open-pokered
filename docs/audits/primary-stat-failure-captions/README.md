# Primary stat-down outcome captions

Original `StatModifierDownEffect` routes failed primary moves through
`MoveMissed` / `ConditionalPrintButItFailed` and prints **But, it failed!**.
An already-minimum stat instead reaches `CantLowerAnymore` and prints
**Nothing happened!**. Failed secondary drops remain silent. See
`engine/battle/effects.asm:539-581,705-716,1403-1414` and
`data/text/text_3.asm:196-206` at original revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.

The production translator now selects the primary failure caption from the
executed move effect. A fixed-size per-side receipt records actual stat-floor
refusal when the action resolves, before a later Haze can reset those stages.
It resets at action/turn boundaries and does not infer failure from the final
state. Two static localization entries keep the new English captions supported
by the existing language switch. No accuracy, stat or RNG rules change.

The new public FIGHT tests complete 174 prepared turns: 144 cases across nine
canonical primary moves, both sides and eight conditions; four turns checking
later Haze and next-turn receipt clearing; 24 silent secondary controls; and
two ordinary damaging-miss controls. On the parent, 117 of the 144 primary cases
have missing/wrong captions (99 failure, 18 floor), and the Haze case fails.
The corrected tests pass with identical expected stat changes and random draws.

Full core suite: 3,696 passed, one existing ignored test. After adding the two
localization entries, the data text tests and the new production-path tests
passed again. Native NEW GAME through m06 and all 11 independent battle/items/
menu/save scenarios passed with unchanged source and executable during each run.
These are scoped integration checks, not another complete first-clear run.

All seven original GBA performance scenarios and 13 memory scenarios passed,
including 330 move animations. Minimum contiguous heap is 7,832 bytes and stack
is 8,708 bytes. Gates, workloads and timeouts are unchanged. The completed memory
log was losslessly compressed and round-trip verified; identities are recorded
in `verification.json`.

Both screenshot pairs use the same prepared FIGHT against Mist and the same
frame 34. `capture.rs` drives actual BattleScreen input and `draw_battle`;
captions come from the simulation. Before shows an ordinary attack miss; after
shows the original primary failure. This prepared-state harness does not drive
the outer PokemonGame/audio clock or reproduce original-ROM timing. All three
remote workflows and six actual GBA runtime steps must pass before merge.
