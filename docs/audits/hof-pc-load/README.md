# Hall of Fame picture-load work (in progress)

The merged cry fix waits for sound, but the native viewer still replaces the
next portrait immediately. The original whites out the background, loads the
picture, restores its palette and then starts the cry. This player-facing gap
is **not fixed yet**.

The new pure `front_pic_decode` component decodes the original compressed,
unflipped front-picture asset and computes its foreground instruction work.
It has no clock or device scheduling. It must still be connected to the
whiteout, header/load, alignment, VRAM transfer and palette/cry caller. Its
receipt is not permission to replace that entire caller with a fixed delay.

`front-codec-cases.bin` provides all151 canonical nonfont `.pic` assets plus
original readonly front-picture VRAM output and foreground-work receipts.
Normal original boot/CONTINUE and physical PC input were repeated twice for
each of two entry schedules. All151 per-asset foreground-work values remain
identical between schedules, while110 elapsed values change with interrupts.
The Rust component reproduces all151 final pictures byte for byte and all151
foreground-work totals exactly, without consulting a species timing table.

The source is pret/pokered at
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`; see `home/uncompress.asm`,
`home/pics.asm` and `home/tilemap.asm`. `validation.json` discloses fixture
construction, limits and the work calculation. The all151 checks are asset
and component evidence, not complete native gameplay/timing parity.

The portable original replay accepts a matching original ROM and symbol file:

```sh
uv run --python 3.12 --with pyboy==2.8.1 python \
  docs/audits/hof-pc-load/replay-original.py \
  --rom /path/to/reference.gbc --sym /path/to/pokered.sym \
  --native-binary /path/to/sole/pokered-app \
  --phase-offset 0 --out /path/to/proof-phase-0.json.gz
```

Repeat with `--phase-offset 1`. Each command normally boots twice and compares
all events, frame/cycle observations, interrupt spans, input actions and all151
records exactly against the respective `original-phase-*.json.gz`. The source
ROM hash is checked. HEAD, the input fixture, selected source files and the
sole native binary are frozen guards; the binary is not executed by this
original-only replay. Do not rebuild, edit those sources or change branches
while recording. No whole-ROM copy or original-font capture is produced.

The current component has passed the full core suite: 3,645 tests, zero
failures and one existing ignored doctest. The nested GBA `no_std` core check
also passes. The nine component tests include the all151 byte/work comparison,
truncated/invalid assets, a constructed mode0 stream, both-schedule alignment,
source copy-loop receipts, refused instructions and interrupt-return waits.
`portable-replay.json` records the independent source replay guards without
relabeling those earlier original-only recordings as a later native build.

The picture component also computes the alignment/interlace foreground work
from `home/pics.asm`. `alignment-work.json` contains all151 original receipts
from both entry schedules. These receipts exclude interrupts, and end at the
entry to `CopyVideoData`; they are not the total white-screen duration.

`source_video` supplies the resumable `CopyVideoData[Double]` and
`DelayFrame[s]` foreground programs on an attached instruction bus. Logical
source ports are not native platform addresses. The programs own no clock or
ISR and cannot replace a missing owner with a frame counter, cleared copy
size, or guessed wake. The 49-tile caller's seven loops match the original
remaining-count and source/destination/control receipts in
`copy-caller-49.json`. Sizes0/8/16 still require their terminal zero-size wait;
refusal and state persistence are tested using explicit controlled seams.
Those seams are not native gameplay or ISR-timing evidence.

The original replay now observes that first copy at one additional executable
loop site, using [PyBoy hooks and register reads](https://docs.pyboy.dk/api/register_file.html).
Both schedules still reproduce the entire published151-species trial exactly.
Two earlier dense-hook attempts produced no valid recording; they were
terminated and retained as observation diagnostics, not treated as game bugs.

Runtime integration is still pending. In particular, the pure decoder's work
totals do not provide a complete resumable instruction stream or authorize an
aggregate picture-load timeout. The real caller must execute whiteout and
screen-clear waits, header/loading work, shared-device/IRQ scheduling, VRAM
request service, palette restoration and cry/input sequencing. The native PC
still changes its picture immediately; this foundation does not close that gap.
