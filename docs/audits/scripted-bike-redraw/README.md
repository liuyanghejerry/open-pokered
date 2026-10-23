# First bicycle redraw during simulated movement

When the Route 23 badge guard sends a rider back, the native game published
four pixels of movement in the first frame. The original publishes two,
crosses the map redraw, then finishes that iteration's bicycle speedup.

The shared scripted path now uses the existing `bike_redraw_advance` state
to keep these two advances on their respective frames. Walking keeps its
existing cadence; later bicycle iterations still advance twice as quickly.
There is no new clock, allocation, or timing constant.

| Frame from the first nonzero scripted walk counter | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Original and fixed bicycle | 7 | 6 | 6 | 4 | 4 | 2 | 2 | 0 | 0 |
| Baseline bicycle | 6 | 6 | 6 | 4 | 4 | 2 | 2 | 0 | 0 |

The walking control retains the complete sequence
`7,7,7,6,6,5,5,4,4,3,3,2,2,1,1,0,0`.
The baseline fails the bicycle assertion and passes the walking control.
Both fixed cases pass, along with the guard, door, departure and physical
START controls. `validation.json` records the full local check results.

The original replay starts normally, selects CONTINUE, physically enters
the Route 22 gate with the Boulder Badge, and exits onto Route 23. It uses
START / ITEM / BICYCLE before physically triggering the next guard. Both
walking and bicycle cases were repeated identically; the portable replayer
also reproduces the complete records. The SRAM fixture is prepared offline.
There are no live memory writes, forced warps, savestate loads, or original
font renders. The native `PokemonGame` fixtures set walking/biking state
before the same physical guard interaction. They are controlled fixtures,
not a fresh mainline playthrough.

The ROM SHA-256 is
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`, built from
`pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.
The relevant source is [DoBikeSpeedup / AdvancePlayerSprite](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/overworld.asm)
and [Route23MovePlayerDownScript](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/Route23.asm).
The screenshots use actual `PokemonGame::draw` on that first scripted frame.

```sh
python docs/audits/scripted-bike-redraw/replay-original.py \
  --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --base-save docs/audits/route22-gate-guard/original-fixture.sav \
  --out /path/to/evidence --mode biking
# Walking control: --mode walking. Run from the repository root.
SCRIPT_INPUT_CAPTURE_DIR=/path/to/captures cargo test -p pokered-app --test scripted_bike_walk
```

Caller-specific input return, including the departure completion's short
START pulse, remains separate work. This change only fixes the first redraw.
