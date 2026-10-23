# Physical START during simulated walking

During S.S. Anne's two-step departure walk, holding START from the first
stride through the second incorrectly opened the native START menu afterward.
The original stays on the map: `JoypadOverworld` calls `Joypad` at each stride
boundary, updating `hJoyLast` before substituting the scripted direction.
The physical START has already been sampled, even though `wJoyIgnore` hides it.

The shared movement path now records that actual foreground poll and updates
the field sampler's compatibility state. It does not poll on intervening
animation frames. This preserves a fresh press after the last stride's poll,
and a fresh press after a single-step guard refusal.

Four actual `PokemonGame` input regressions cover:

| Case | Original and fixed behavior |
| --- | --- |
| START held across departure's second stride | Stay on the map; release and press START again to open the menu |
| START first pressed after the second stride starts | Open the menu after movement |
| Short START pulse between stride polls | Stay on the map; a later fresh START still works |
| START pressed during the guard's only stride | Open the menu after movement |

The baseline passes the three controls and fails the held-across-strides case.
Native traces cover the first 60 hardware frames from the first counter-7
sample. Before/after screenshots use the real `PokemonGame::draw` at frame 59,
so they show the actual START menu or map, with Fusion Pixel text.

The original replays boot normally, select CONTINUE, and use physical inputs
to enter the gate or exit the dock. Fixtures are prepared offline from the
existing guard SRAM. They use PyBoy 2.8.1, read-only hooks, no live RAM writes,
no forced warps/savestates, and no original-font rendering. All four cases
were repeated with identical complete inputs and records. `original-runtime.json.gz`
preserves these records; `validation.json` records source and check hashes.

The original ROM SHA-256 is
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`, built from
`pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.
The decisive code is [JoypadOverworld](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/overworld.asm)
and [_Joypad](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/joypad.asm).

```sh
python docs/audits/scripted-input-polls/replay-departure.py \
  --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --base-save docs/audits/route22-gate-guard/original-fixture.sav \
  --out /path/to/evidence --press-from 0 --release-at 46 --fresh-start-at 60
# Late START: --press-from 17 --release-at 46 (omit --fresh-start-at).
# Pulse: --press-from 0 --release-at 10 --fresh-start-at 60.
# Single-step control: replay-guard.py, --press-from 0 --release-at 30.
SCRIPT_INPUT_CAPTURE_DIR=/path/to/captures cargo test -p pokered-app --test scripted_input_poll
```

These are controlled fixtures, not a fresh mainline playthrough. Caller-specific
return timing, direction/held-state cleanup, and complete source ownership
remain separate work; this fix does not promote the legacy movement caller to
a fully modeled foreground owner. In particular, the previously recorded
RIGHT-return timing differences are not claimed fixed here.
