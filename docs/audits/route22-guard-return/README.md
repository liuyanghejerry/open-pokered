# Route 22 guard input return

After the no-badge guard pushes the player down, the original
`Route22GatePlayerMovingScript` clears `wJoyIgnore`, calls `Delay3`, then
returns to `JoypadOverworld`. The native scene previously ended immediately.
That lost a one-frame START press on the original return frame, accepted a
press during the source wait, and resumed held movement too early.

Both refusal callers now explicitly run `route22GuardReturn()` after their
relative walk. This typed callback owns the source three-VBlank wait;
neither Joypad nor NPC movement runs inside it. Completion resumes the
already-started field iteration, preserving fresh START/A edges while
clearing held movement through the existing simulated-return path. The
ordinary no-direction handling records the stop before the next turn.
It adds no persistent screen field or allocation. Other movement callers
retain their own completion behavior and remain explicitly unmodeled where
their full input ownership has not been proved.

Frame zero below is the first post-update walk-counter-7 observation.
Original CPU hooks fire within a tick; their offset 19 corresponds to the
native post-update frame 20.

| Physical input | Original result | Fixed native |
| --- | --- | --- |
| START [18, 19) | No menu | No menu |
| START [19, 20) | Menu CPU offset 19 | Menu frame 20 |
| START [20, 22) | Menu CPU offset 21 | Menu frame 22 |
| START [0, 30) | Menu CPU offset 19 | Menu frame 20 |
| START [0, 10) | No menu | No menu |
| DOWN [0, 30) | Next stride frame 22 | Frame 22 |
| UP/LEFT/RIGHT [0, 30) | Next stride frame 24 | Frame 24 |

All nine original cases were repeated twice with identical complete records.
A further read-only `UpdateSprites` probe confirms no sprite update during
the callback wait: updates resume at CPU offsets 19 and 23. The baseline
native suite passes nine controls and fails all five new regressions;
the candidate passes all 26 selected tests, including guard permission,
direct talk, both gate lanes, departure, bicycle and door controls.

The two screenshots under `docs/screenshots/route22-guard-return-*.png` show
actual `PokemonGame::draw` at frame 59 with the same [19, 20) START input.
The baseline remains in the field; the candidate has opened the menu.

```sh
python docs/audits/route22-guard-return/replay-original.py \
  --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --base-save docs/audits/route22-gate-guard/original-fixture.sav \
  --out /path/to/evidence --press-from 19 --release-at 20
SCRIPT_INPUT_CAPTURE_DIR=/path/to/captures \
  cargo test -p pokered-app --test scripted_input_poll
```

Use `--control-button up|down|left|right --press-from 0 --release-at 30`
for movement controls. `replay-sprites.py` adds only the read-only sprite
hook. Both replayers boot normally, select CONTINUE from offline-prepared
SRAM and physically enter the gate. They perform no live memory writes,
forced warps, savestate loads or original-font renders. Native cases use
controlled actual Game fixtures; these are not a fresh mainline playthrough.

`validation.json` records hashes and results; the compressed files retain
complete original and native traces. Full package checks and the unchanged
GBA gates must pass before merging.

Pinned source: `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`;
ROM SHA-256 `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
Sources: [guard callback](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/Route22Gate.asm),
[field input](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/overworld.asm),
[Delay3](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/palettes.asm).
