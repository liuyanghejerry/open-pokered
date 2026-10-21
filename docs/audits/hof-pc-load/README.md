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

The current component has passed the full core suite: 3,639 tests, zero
failures and one existing ignored doctest. The nested GBA `no_std` core check
also passes. The three component tests include the all151 byte/work comparison,
truncated/invalid assets, and an explicitly constructed mode0 stream.
`portable-replay.json` records the independent source replay guards without
relabeling those earlier original-only recordings as a later native build.
