# S.S. Anne departure input return

A short START press as the two-step departure walk finished was lost. The
native continuation spent another update on `clearJoyIgnore`, then waited
for another field iteration before reading input. Held RIGHT also resumed
one frame earlier than the original because the stop/turn state was missing.

`VermilionCityPlayerMovingUp2Script` actually clears the mask inside
`RunMapScript`, before `Joypad` in that same field iteration. Then
`JoypadOverworld.doneSimulating` clears held movement but preserves fresh
A/START edges. The ordinary no-direction path records the previous moving
direction and arms the next turn.

The implementation consumes the explicit unlock continuation without an
extra VBlank and resumes that already-started field iteration. It uses the
existing field sampler and stop/turn logic; it adds no persistent state,
allocation or guessed duration. Among current map scenes, this exact
`movePlayerRelative` / `clearJoyIgnore` / return sequence belongs to the
Vermilion City departure load handler. Guard and cycling-gate callbacks
remain separate: their source completion behavior differs.

| Physical START interval from the first counter-7 sample | Original | Fixed native |
| --- | --- | --- |
| [30, 32), source-only control | No menu | Not newly compared |
| [32, 33) | No menu | No menu |
| [33, 34) | CPU menu entry at offset 33 | START screen at post-update 34 |
| [34, 36) | CPU menu entry at offset 35 | START screen at post-update 36 |
| [0, 46), already sampled at the second stride | No menu | No menu |

CPU hooks execute during a tick, while native observations are made after
an update. These two offset conventions must not be equated. The native
one-frame and next-poll cases assert the actual post-update frames.
The held-RIGHT control starts its next stride at frame 38, matching the
[existing original movement record](../simulated-walk-clock/original-runtime.json.gz).
The baseline starts at 37. Other walking, bicycle, door and guard controls
still pass.

All five new source input cases were repeated twice with identical complete
records. Each boots normally, selects CONTINUE from an offline-prepared SRAM
fixture, and physically exits the dock. Read-only hooks observe the callback,
Joypad, simulated completion and no-direction instructions. There are no
live memory writes, forced warps, savestate loads or original-font renders.
The native fixtures run actual `PokemonGame::update` and `Game::draw`;
these controlled cases do not constitute a fresh mainline playthrough.

The fresh-press control following frame 60 now holds START for two hardware
frames, matching the original replayer's existing two-frame control. The
older native helper used one frame that happened to coincide with its late
poll. The corrected baseline still passes five controls and fails the three
new regressions. The short completion probe remains exactly one frame.

`validation.json` records source hashes, baseline failures and checks.
The two screenshots capture the actual native screen at frame 59 using the
same [33, 34) physical input: the baseline remains on the map, while the fix
opens the START menu. Text uses Fusion Pixel.

```sh
python docs/audits/departure-input-return/replay-original.py \
  --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --base-save docs/audits/route22-gate-guard/original-fixture.sav \
  --out /path/to/evidence --press-from 33 --release-at 34
SCRIPT_INPUT_CAPTURE_DIR=/path/to/captures \
  cargo test -p pokered-app --test scripted_input_poll
```

Pinned reference: `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`;
ROM SHA-256 `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
Source: [departure callback](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/VermilionCity.asm),
[JoypadOverworld and no-direction handling](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/overworld.asm).
