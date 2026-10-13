# NPC motion after conversation at an aligned step start

Base: master `0919db8cd1a1d6a7f4200df78773ed2d37451185`.
Production/test fix: `79557e073851ab8fd81b33e9f73dfe5002e3e053`.
Engine pin: `976f86b42813a657047db89ca4dca2d6b9172dff`, merged by dotzuki#92
at `9f3407448bb420f9cf8f773ecdd940d928f64456` (identical workspace tree).

Both screenshots are absolute native hardware frame **650**
(`frame-000649.png`). Baseline was freshly built from the named master.
A SaveBuilder snapshot starts the player at MtMoonPokecenter (11,6),
with a Charizard and money1000; EVENT_BOUGHT_MAGIKARP is false. No NPC
runtime, movement counters or RNG bytes are injected. Seed0. PressLeft1,
then neutral until the salesman starts LEFT from (10,6), counter16; holdA2.
He turns RIGHT toward the player. Skip dialogue, wait for choice, pressDOWN
andA to decline the purchase, finish his response, then observe36 neutral
frames. Both runs open the same dialogue at frame285 and use the same
inputs/timelines. Frame285 PNG and opened state are byte-identical.

Before: the saved walk direction is lost when facing turns RIGHT. The NPC
finishes on the player's (11,6) tile and overlaps the player.
After: display facing stays RIGHT while motion continues LEFT to (9,6).
The published pose, renderer fallback, follow destination, player collision
reservation and fallback render-cache key all use the independent motion
direction; sprite image selection keeps the display-facing direction.
Two before and two after runs each repeat all651 PNGs plus the frame
manifest exactly. The earlier controller-input baseline record also matches the
fresh master run. Counts and snapshots are in verification.json.

Original source `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/overworld/movement.asm: MakeNPCFacePlayer`, changes only the facing
byte; the walking update continues using saved X/Y step vectors.
`original-probe.json` executes this policy for four directions, repeated
twice, over17 logical updates. It seeds walking state and the face-player
flag at the original routine entry. This is a controlled policy probe,
not a naturally triggered original conversation movie; full original/native
pixel, hardware-frame, and RNG parity are not claimed.

Validation:499 engine tests,3 rustdoc tests (7 ignored), examples and
all-crate packaging pass. Game40 core suites / 3,245 tests and41 app test
executables pass. GBA memory-scenarios release builds with --locked. Full
GBA runtime/CI on the final game head remains the merge gate.
