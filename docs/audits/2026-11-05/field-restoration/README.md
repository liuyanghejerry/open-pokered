# FLASH screen restoration on current master

Code commit `009e6942`, based on master `303aaceb`. The new captures use matching
prepared SRAM, seed 42, identical physical input, matching initial images and
native frame 960. Both runs for each side repeat exactly. Prepared saves are
regression controls, not a fresh playthrough.

The master build has the same SHA-256 for all 2,378 compiled crate/Cargo files
as master `303aaceb`. Its build head was `9e7d11f2`; it is not relabeled as a
new build. Candidate binary SHA-256 is
`6cf0dda7cbe6618f8af94b3ae2b605412ef0388d4623a972ec0ca19ce0590181`.

The shared runtime retains the party/text caller until the map's display,
pattern transfers, background palette and sprite restoration return. It runs
the common VBlank video/audio/input/playtime work before returning control.
The raster records actual palette writes; there is no fixed white-screen count
or hardcoded white strip. Visible text continues to use Fusion Pixel.

The latest optimizations avoid preparing a new audio frame twice, preserve an
exclusive audio cursor, batch unobservable plain instructions and shadow OAM
stores, and pack the retained map image in groups of eight pixels. Differential
tests preserve every accepted instruction, refusal/snapshot continuation, APU
access, display state and once-only audio commit.

The default GBA release ROM, without autopilot or skipped frames, was cold
booted twice from ordinary SRAM. All 600 frame hashes repeat exactly; the
initial, fully restored and subsequent walking images equal the prior build.
Complete player restoration improved from 359 physical frames (6.011 s) to
270 (4.521 s). **This remains a performance gap, not original-speed parity.**
The source LCD entry phase is also not an assertion of exact original hardware
time. The earlier original/native Safari palette interval discrepancy was
7,904 source T-cycles (1.884 ms), and remains separately recorded.

Current full NEW GAME through m49, Hall of Fame, ending autosave and independent
CONTINUE is running on the frozen candidate. Earlier successful runs are not
counted as this binary's result. See verification.json for completed checks.

Screenshots are under `docs/screenshots/field-restoration-1390/`. Before/after
pairs use the same scene and recorded input index. At index 174 the baseline
has already returned to the map while the candidate is still restoring. At
Pallet index 197 / Safari index 195 the background has appeared while sprite
restoration is still pending. At the final capture the map is controllable.
