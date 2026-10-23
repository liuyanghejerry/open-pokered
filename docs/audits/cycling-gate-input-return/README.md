# Cycling gate input return

Both Cycling Road guards lost a one-frame START press at the end of their
rightward push and resumed held directions two frames late. The original
`Route16Gate1FPlayerMovingRightScript` and its Route 18 counterpart clear
the simulated-movement flag before the same `JoypadOverworld` call polls
physical input. They therefore skip `doneSimulating` and preserve held
directions. They do not use Route 22's Delay3 or the departure stop path.

Both scenes now explicitly call `cyclingGateReturn()` after that final
relative step. Its zero-VBlank continuation unlocks and resumes the current
field iteration without synthesizing a no-direction input. It adds no
persistent field, allocation or duration constant. Full movement input
ownership remains marked unmodeled where the caller has not been proved.

For each gate, nine physical input cases were replayed twice with identical
complete records. Frame zero is the first post-update counter-7 sample of
the final rightward step. CPU hooks run within a tick, so original CPU
offset 16 corresponds to native post-update frame 17.

| Physical input | Original result | Fixed native |
| --- | --- | --- |
| START [15, 16) | No menu | No menu |
| START [16, 17) | Menu CPU 16 | Menu frame 17 |
| START [17, 19) | Menu CPU 18 | Menu frame 19 |
| START [0, 30) | Menu CPU 16 | Menu frame 17 |
| START [0, 10) | No menu | No menu |
| DOWN/LEFT/RIGHT [0, 30) | Next stride frame 17 | Frame 17 |
| UP [0, 30) | Blocked by the counter | Blocked |

The unchanged baseline passes three return controls and fails three return
regressions. The candidate passes all 32 selected actual Game tests,
including both guards, Route 22, departure, bicycle and doorway controls.
The four screenshots show actual `PokemonGame::draw` at frame 59 with the
same one-frame completion START. Before the fix the game remains in the
field; afterward the menu is open. Text uses Fusion Pixel.

The UP approach remains unfinished. Route 16 still uses absolute movement,
whose first counter is 8 rather than 7. Sixteen additional original runs
cover all four approach tiles of both gates. They also reject a universal
first-stride wait: Route 16's two-UP entry and Route 18's two/three-UP entries
show only two initial counter-7 frames, while other entries show three.
An attempted approach change and its constant-17-frame oracle were withheld;
the original counterexamples are retained in `validation.json` and the
complete runtime archive. Exact caller entry/VBlank timing needs further
work. All eight approach controls still reach the final RIGHT return at
CPU offset 16; this PR only fixes that separately verified return.

```sh
python docs/audits/cycling-gate-input-return/replay-original.py \
  --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --base-save docs/audits/route22-gate-guard/original-fixture.sav \
  --out /path/to/evidence --gate 16 --approach 3 \
  --button start --press-from 16 --release-at 17
CYCLING_GATE_CAPTURE_DIR=/path/to/captures \
  cargo test -p pokered-app --test cycling_gate_input_return
```

Use gate 18 for the other checkpoint, `--approach 0..3` for the approach
tiles, or direction buttons held over [0, 30) for movement. Every original
run uses normal boot/CONTINUE from offline-prepared SRAM and physical gate
entry. Hooks only read state. There are no live RAM writes, forced warps,
savestate loads or original-font renders. Native fixtures exercise actual
Game update/draw; these controlled cases do not replace a fresh mainline run.

The source is pinned to `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.
Sources: [Route 16 callback](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/Route16Gate1F.asm),
[Route 18 callback](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/Route18Gate1F.asm),
[JoypadOverworld](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/overworld.asm).

`validation.json` records source hashes, baseline failures and checks.
Full package checks and all original GBA gates are still required to merge.
