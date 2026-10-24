# Multi-hit count sampling and knockout termination

The original samples the low two bits for variable-length attacks. Values 0/1
select 2/3 hits immediately; values 2/3 consume another byte and select
`(second & 3) + 2`. A single threshold-based draw has the same aggregate
probabilities but different deterministic outcomes and subsequent random state.
Original `effects.asm:925-969` specifies this conditional draw. Both attack
paths return on target KO before further hits, count sampling or final riders
(`core.asm:3238-3255,5638-5654`). Reference revision:
`pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.

All game manifests and both lockfiles now pin tested engine revision
`688db209603860be52da3e6a77abe9de4182e495`, integrating dotzuki PRs #111 and
#112. The engine delta from the previous pin changes only the rules interpreter,
its public contract and its tests. The existing parity fixture now uses the
original conditional count bytes; legacy combat remains only its per-hit
damage oracle.

The new public FIGHT regression exercises 990 prepared turns (976 fully drained
presentations): seven variable-count moves, both sides, all low-bit pairs and
high-bit aliases, plus first/second/final-hit KO and survivor controls for ten
multi-hit moves. It checks damage, exact random consumption and actual move
presentation receipts. Both tests fail against the previous production libraries
and pass with the new pin. Full `pokered-core` tests: 3,693 passed, one existing
ignored test.

Native NEW GAME through m06 passed with no source/binary changes during the run.
All 11 native battle/items/menu/save scenarios passed too. The external recording
wrapper mislabeled the scenario suite's normal `SystemExit(0)` as a failure;
the original log records `SCENARIOS 11/11 PASS`. The raw result and an explicit
review are retained in the verification record. No product or scenario change
was needed.

All seven original GBA performance scenarios and 13 memory scenarios passed,
including 330 move animations. Minimum contiguous heap: 7,816 bytes; stack:
8,708 bytes. Workloads, gates and timeouts are unchanged. The first external
mGBA invocation lacked its shared-library path; the successful invocation uses
the same ROM with the existing tool directory added to `LD_LIBRARY_PATH`.

The before/after images show frame 256 of the same prepared FIGHT, driven by
`capture.rs` through actual `BattleVisualEffects` and `draw_battle`. Before, a
second Double Kick animation starts at frame 215 despite target HP zero. After,
only the first hit occurs and the presentation advances. Frame 223 was also
checked, but both images are identical at that animation phase; frame 256 shows
the difference. This harness does not drive the outer PokemonGame/audio clock.

The completed 49-milestone fresh playthrough and independent-process CONTINUE
on the parent master remain baseline evidence, not validation of this new pin.
The candidate's native integration coverage is m06 plus the 11 scenarios.
Twineedle's surviving-target Substitute/status/type guards and missing multi-hit
count captions remain separate findings. The early level/HP HUD update visible
in both captures is also outside this change. Remote merge requires all three
workflows and all six actual GBA runtime gates to succeed.
