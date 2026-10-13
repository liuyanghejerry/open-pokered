# Door exit auto-step cadence

Baseline: master `26a68b51be0dcf3b48c78270a9bd900b7c71b235` (after PR #215).
Production fix: `5cf30b59c7caa0fd422b248d553cea5280152a0d`.

Both images are native hardware frame **276** (`frame-000275.png`).
Identical input starts in ViridianCity (23,26), walks UP through the actual
Center entrance to (3,3), then DOWN until the actual exit changes maps.
Release DOWN immediately, advance 120 neutral hardware frames, then UP12
and neutral8. Seed0; no party; no save state mutation. The original uses a
naturally walked Center emulator save, with no RAM edits.

Both native runs repeat all 382 PNGs and the frame manifest exactly. Frame266
immediately before auto-walking is byte-identical across branches. The fresh
master capture is also byte-identical to the earlier PR215-head capture.
At frame276 baseline has already completed its eight-frame step; the fix is
still walking with counter3. The fixed 15 visible walking counters equal
original Red: `7,7,7,6,6,5,5,4,4,3,3,2,2,1,1`.

This verifies the exit stride and its subsequent stopped turn, not complete
warp/fade timing, RNG, or original-renderer pixel parity. Original fade
latency differs; strides are aligned at the first walking frame for the
original/native counter comparison. The before/after PNGs use the same
absolute native hardware frame, without timeline shifting.

Validation: 40 core suites / 3,240 tests and all 41 app test executables pass.
The targeted test also checks that physical UP cannot interrupt simulated
DOWN and that stopping remembers DOWN after the automatic step.
