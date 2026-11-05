# Earliest START cancellation: preserve the opening noise

The preceding #441 candidate could lock when B was pressed as soon as START
accepted input. Its CHAN8 opening sound was still playing alongside CHAN5
PressAB; the restoration audio preflight rejected that valid noise program
with `ActiveSfx(7)`. This is a regression found before merging #441, not a
claim that the immediate-close implementation on the earlier master locked.

The shared audio owner now accepts the canonical START noise stream using
its original sound ID 143. It executes the existing noise instruction path
and preserves both sounds, rather than stopping the noise or waiting for it
to finish. Modified programs and unrelated native sound IDs remain rejected.

Normal Red ROM SHA-256 is
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
The original instruction oracle covers both notes and the ending in banks
2 and 31, with/without overlapping PressAB: 12 cases and 64 actual APU
accesses. Tests also compare complete native sequencer state across three
music tracks, 13 opening-sound ages, both confirmation states, interruption
and resumption. An app regression uses actual START and the earliest B.

## Actual input recordings

Original recordings start from the included prepared original state, return
to the field through ordinary B presses, open START, then press B at its
first observed input poll. CHAN8 is still sound 143 at that point. Desktop
recordings cold-boot the parent `fixture.sav` through CONTINUE and press B
on START's first observable ready frame. There are no recording-time state
setters. Both repeat twice; every measured frame is consecutive.

| Observation | Original | Previous #441 | Fixed |
| --- | ---: | ---: | ---: |
| First field loop/screen after B, relative frame | 29 | never in 80 frames | 29 |
| Opening-sound handoff | completes | `ActiveSfx(7)` | completes |
| Final position | (20,30) | (20,30), locked | (20,30) |

The desktop pre-trigger images are byte-identical. The fixed build can then
walk and reopen START. SRAM remains unchanged. `native-*.json.gz` retain
the complete ordered input/frame/state series and hashes of the full local
records; original records retain all frame hashes and source-hook events.

The default GBA controls wait for the full stable menu raster, then press B,
record 800 physical frames, walk and reopen START. All frame hashes repeat
twice per ROM; direct cartridge SRAM readback is unchanged. **Both the old
and fixed GBA builds pass this control**: the desktop lock was not reproduced
on the default GBA build. This is platform regression evidence, not a claim
of matching original physical elapsed time or an observed GBA fix.

## Validation and replay

Application: 1,096 passing executions (92 existing ignored); core: 3,988
(one existing ignored); audio: 225. ARM check, default release GBA build and
fresh NEW GAME through m06 (41.15 seconds) pass. The separate full START
route and its precise source scope are reported in the parent PR; this
small audio follow-up does not relabel an earlier binary as a new build.
The parent report's remaining animation qualifications still apply.

`verification.json` pins binaries, resources and source files. Replayers
accept explicit binary/ROM hashes and write only to a new output directory:

```sh
python3 replay-native.py --binary /path/to/pokered-app --sha256 HASH \
  --output /path/to/new-output --expect-fixed
python3 replay-original.py --rom /path/to/normal-red.gbc \
  --symbols /path/to/pokered.sym --output /path/to/new-output
python3 replay-gba.py --rom /path/to/default-release.gba --sha256 HASH \
  --bridge /path/to/mgba-sram-bridge.so --output /path/to/new-output
```

The original replay requires PyBoy. `mgba-sram-bridge.c` uses mGBA 0.10.5's
ordinary core and read-only SRAM clone API; build it against the matching
headers/library. The GBA replay's warm-up boot only observes the full menu
raster, then starts a new cold boot for the measured trial.
