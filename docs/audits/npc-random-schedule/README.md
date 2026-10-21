# NPC random-call scheduling

A ready ordinary walker draws one direction byte. If that attempt succeeds,
`TryWalking` starts its 16-pixel stride without drawing or starting the next
movement delay. `UpdateSpriteInWalkingAnimation.initNextMovementCounter` draws
that delay when the walking counter reaches zero. An unsuccessful
`CanWalkOntoTile` attempt draws a separate delay immediately. Both delays mask
with `$7f`; a zero stored byte wraps to 256 updates. Scripted movement skips
these ordinary direction/completion draws.

Previously the live native pass used one frame-derived LCG byte for every
wanderer and selected its next delay at stride start, reusing the direction
byte. It now consumes the game's RNG as each sprite is visited. Stationary
turns and stride completions consume that same stream in slot order. Screen
and displacement rejection happen inside that actor's attempt, before the next
slot consumes randomness. Forced movement selectors still consume the ready
direction draw.

The RNG algorithm, original DIV phase, foreground instruction durations and
raster timing are separate work. This change aligns when movement consumes
random bytes; it does not claim an identical original/native random trajectory.
The raw inactive ready-delay byte is not a canonical native status owner: the
native ready actor represents inactivity with `delay_counter == 0`.

## Original evidence

Source: `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/overworld/movement.asm` (`UpdateNPCSprite.randomMovement`, `TryWalking`,
`UpdateSpriteInWalkingAnimation`, `CanWalkOntoTile.impassable`).

- `original-controlled.json`: three controlled component probes, each repeated
  exactly. These explicitly construct registers/RAM and load a component state;
  they are **not** normal-startup recordings. Original Random and ROM bytes are
  untouched. Success calls ready + completion Random; failure calls ready +
  failure Random; scripted movement calls neither. The inactive sentinel delay
  stays 13 throughout the successful stride and becomes 18 only at completion;
  the independent failure delay becomes 102.
- `original-normal.json.gz`: normal original power-on/CONTINUE, physical Start/A
  and 4096 idle frames, readonly memory/call-site hooks. The offline-constructed
  `original-fixture.sav` is disclosed in `original-fixture.json`; this is not a
  fresh playthrough. Two runs reproduce exactly: 159 events and 16 completed
  successful stride cycles. Every cycle draws/stores its next delay at counter
  zero, retaining the previous inactive byte until then.
- `replay-original.py` reproduces the normal trace using PyBoy 2.8.1. It does not
  set runtime registers/RAM, restore snapshots, warp, replace Random or save.
  ROM SHA-256:
  `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
  The ROM and matching `pokered.sym` are external inputs, not bundled copies.

```sh
uv run --python 3.12 --with pyboy==2.8.1 python \
  docs/audits/npc-random-schedule/replay-original.py \
  --rom /path/to/reference.gbc --sym /path/to/pokered.sym \
  --fixture docs/audits/npc-random-schedule/original-fixture.sav \
  --out /path/to/new-original-result.json.gz
```

## Native evidence

`replay-native.py` normally boots and chooses CONTINUE from the existing
`../npc-state-persistence/counter-eight.sav` offline fixture, then sends 1024
physical idle frames. Only physical input, readonly observations and framebuffer
captures are allowed. It uses seed 42 at process startup; it does not change RNG
at runtime. Source files, HEAD and the sole binary are guarded throughout.
Player position stays Route1 `(5,21)`; save bytes are unchanged.

```sh
python3 docs/audits/npc-random-schedule/replay-native.py \
  --repo "$PWD" --binary /path/to/sole/pokered-app \
  --fixture docs/audits/npc-state-persistence/counter-eight.sav \
  --out /path/to/new-native-recording
```

Component tests use constructed states and controlled byte queues, separately
from actual startup recordings. They verify successful completion delays,
independent failed-attempt delays and zero wrap, displacement/screen rejection
before later slots draw, completion/stationary/walker interleaving, scripted
movement without ordinary draws, hidden/offscreen/delayed eligibility, and
forced selectors. A live `OverworldScreen` test additionally checks the seeded
RNG state before, throughout, and after an actual foreground stride.

Both native recordings passed their guards and 513 observations each, with
identical physical tape and observation schedule. The first logical difference
is absolute frame 750 (idle offset 142); the whole before/after screenshots use
absolute frame 864 (offset 256). There are 212 differing NPC observations. Full
traces are `native-before.json.gz` / `native-after.json.gz`; the structured
comparison is `native-comparison.json`. Both source and candidate recordings
leave player position and SRAM bytes unchanged.

Local validation at code commit `c0dfeba7d5f2e54207e4a369a8fd7bf314377cba`:
3632 core tests passed (1 existing ignored doctest), 400 app tests with
`debug-server` passed (40 existing ignored tests), 131 audio tests passed, and
the native debug binary built successfully. Nine new movement tests cover the
controlled stream and live foreground integration. The standalone original
replay reproduced the previous original normal trace exactly. Full fresh
m01–m49 regression and all original CI jobs are separate merge gates; their
status is reported in the PR.
