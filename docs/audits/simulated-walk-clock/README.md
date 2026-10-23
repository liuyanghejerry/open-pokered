# Simulated player walks

The Route 22 guard's refusal and the two-step S.S. Anne departure walk advanced
once per displayed frame. The original advances in `OverworldLoop`, whose two
`DelayFrame` calls and initial map redraw also apply to simulated joypad input.
The old implementation additionally started the next stride on the previous
stride's completion frame.

`MovePlayerRelative` now uses that existing field clock, advances the first two
pixels before the redraw wait, and starts the next direction on the following
field iteration. The shared clock also lets stationary NPCs update during
these field iterations, as they do for the other movement commands.

Starting at the first visible `walk_counter = 7`, the original and fixed game
both produce `7,7,7,6,6,5,5,4,4,3,3,2,2,1,1,0,0`. Coordinates change at frame 15;
the next simulated stride starts at frame 17. The old game reached the next
coordinate at frame 7 and started the following stride on that same update.

Controls hold physical RIGHT starting on the following hardware frame and
verify that it cannot interrupt a scripted stride. The per-frame comparison
covers the complete simulated walk, including each inter-stride boundary.

## Remaining caller-specific input return

The original guard first advances physical RIGHT at frame 24; the clock-fixed
game does so at frame 19. For departure the corresponding frames are 38 and 37.
These are separately recorded continuation gaps, outside this shared-clock
change. The guard's `Delay3` and the return through `JoypadOverworld` need to be
modeled together. A generic forced `NoDirection` on every movement return is
incorrect: `Route16Gate1FPlayerMovingRightScript` explicitly clears
`BIT_SCRIPTED_MOVEMENT_STATE` before `JoypadOverworld` resumes. Caller-specific
return semantics must distinguish these paths before that follow-up is merged.

## Evidence and limits

- Baseline: `7b79a188a1fe696f7bb16c7c42d2baf2a7ed2aa9` (master after #368).
- Original: `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.
  ROM SHA-256: `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
- `replay-guard.py` prepares an offline Route 22 SRAM fixture, boots normally,
  selects CONTINUE, enters the gate with physical buttons, and talks to/crosses
  the guard. Both threshold lanes and direct talk are exercised.
- `replay-departure.py` prepares the dock fixture offline, boots normally,
  selects CONTINUE, and physically walks through the city warp. Both replays
  use PyBoy 2.8.1, read-only CPU hooks, and no original-font rendering, live
  memory writes, forced warps, or emulator savestates.
- Every original case was repeated with identical inputs and state traces.
  `original-runtime.json.gz` preserves the complete records; `comparison.json`
  lists the consecutive counter/coordinate comparisons through the last
  simulated stride with the actual shared `PokemonGame` input tests. The
  native fixtures start on controlled maps, so
  these are targeted reproductions, not a new mainline playthrough.
- `native-runtime.json.gz` preserves the fixed shared Game traces. The two app
  tests cover six cases, including held-input controls. The Lance core test
  checks all 37 strides and door closure against its reversed RLE path; its
  old 600-update settle window ended before the now-correct 629-frame walk.
  It does not establish complete Lance scene/caller timing parity.
- Before/after PNGs are native 160x144 frames captured eight updates after
  the first counter-7 frame using the same fixture and button stream. The
  player/camera displacement is the bug; both captures use Fusion Pixel.

`validation.json` records the tested code, commands, source and log hashes.
The PR requires its own complete latest-head CI.

Run the original reproductions from the repository root (the ROM and symbols
must come from the pinned original build):

```sh
python docs/audits/simulated-walk-clock/replay-guard.py \
  --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --base-save docs/audits/route22-gate-guard/original-fixture.sav --out /path/to/guard
python docs/audits/simulated-walk-clock/replay-departure.py \
  --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --base-save docs/audits/route22-gate-guard/original-fixture.sav --out /path/to/departure
# Add --hold-right to either command for its input-handoff control.
```

Native regressions and optional captures:

```sh
RELATIVE_CAPTURE_DIR=/path/to/captures cargo test -p pokered-app --test relative_script_walk
cargo test -p pokered-core fidelity_lance_intro_reaches_the_original_coordinate_and_closes_door
```

Relevant original code:
[OverworldLoop / JoypadOverworld](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/overworld.asm),
[Route22GatePlayerMovingScript](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/Route22Gate.asm),
[VermilionCityPlayerMovingUp2Script](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/VermilionCity.asm),
[WalkToLance_RLEList](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/LancesRoom.asm).
