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


### Pokecenter audio work for the pending runtime integration

`AudioManager::update_frame_with_source_work` also accepts `MusicId::POKECENTER`.
This extends the existing live-sequencer work calculation; it does not create a
second sequencer or an independent PC clock. Other songs, unsupported effects,
fades and low-health alarm paths still reject work explicitly and tick the real
manager exactly once. The first unprimed constructor tick remains unsupported.

The pinned original Audio1 and Audio3 engines are identical after normalizing
engine/header/MAX-ID names and removing their first descriptive comment. This
supports sharing update helpers, **not** sharing their different PlaySound
header tables. Source hashes and the exact normalization are in
[pokecenter-audio-proof.json](pokecenter-audio-proof.json).

Two independent original normal boots, CONTINUE and physical PC-menu approaches
then idle for 10,000 updates each. Readonly entry/RET hooks observe channel inputs
and update T-cycles. The [complete readonly trace](original-pokecenter-updates.json.gz) retains both
input tape and guarded source identities. Both repeats match exactly; all 10,000 records also match
the native work calculation in an explicitly **controlled unit fixture**. That
test constructs channels from the original inputs and is not evidence of native
normal-boot timing parity. The 600,004-byte packed fixture stores three channels'
inputs and total cycles, not ROM data or recorded audio.

A separate native test runs the ordinary manager and the clocked manager for
20,000 frames without SFX and 20,000 with periodic PressAB, comparing every
channel after every tick. This proves the work observer leaves the real update
unchanged. The output adapter also compares exact PCM for 1,200 frames per
song (including Pokecenter and PressAB). The new original trace covers music without active SFX; it does not
establish new original-versus-native PressAB overlap parity.

Reproduce the original trace and check every published fixture record:

```sh
uv run --offline --python 3.12 --with pyboy==2.8.1 python \
  docs/audits/hof-pc-load/replay-pokecenter-audio.py \
  --rom /path/to/reference.gbc --sym /path/to/pokered.sym \
  --native-binary /path/to/the/sole/pokered-app \
  --out /path/to/scratch/pokecenter-original.json.gz
```

The supplied native binary is hash-guarded only, never run or copied. The script
guards source files, HEAD, binary, ROM and SRAM throughout recording; it performs
no runtime writes, warps or snapshot restores and captures no original fonts.
The existing SRAM fixture is loaded offline. **Actual PC picture loading,
shared display/IRQ scheduling and matched before/after gameplay remain open.**


### Resumable decompression, still awaiting runtime attachment

`front_pic_decode::continuation::DecompressWork` executes the unflipped
`_UncompressSpriteData` body through the shared instruction bus. It owns the
compressed asset, 784 private plane bytes and a queue capped at 256 instruction
lengths. It prepares at most one bounded block ahead, keeps partial bytes
private, and exposes output only after the final RET is accepted. The attached
IRQ owner must not read this private scratch. The body owns no clock, device,
interrupt service, bank-switch effects or entire-load timeout.

`front_pic_decode::caller::DecompressCallerWork` supplies the actual outer bank
and SRAM stores on that same bus. It reads the attached owner's loaded ROM bank
and restores that receipt. As in the original, SRAM remains enabled on bank0;
the wrapper does not invent a prior bank or restore controls the source leaves
changed. Output stays private until the wrapper's final RET. The 188 wrapper
T-cycles are executed instruction by instruction, not added as an idle delay.

Every one of the 151 images matches the previously recorded original bytes and
foreground work. Controlled bus tests cover refusal, serialized continuation,
malformed inputs, per-instruction versus exact-prefix execution, all wrapper
accesses and partial body grants. Prefix batching accepts an instruction count
only when the attached owner can prove no IRQ interleaves it; an owner without
that proof falls back to individual instructions. These are controlled seams,
not elapsed-time or native gameplay evidence.

The [sparse original entry trace](decompress-points-proof.json) covers 54,512
ordered sites across both natural modes and all three stock dimensions. Each
point's foreground offset matches the new continuation. Both existing entry
schedules were independently replayed twice, with identical points and the
entire published 151-image trial unchanged. This proves the observed entry
boundaries; it is not a trace of every source instruction or native device-phase
parity. Reproduce the points with the existing replay command plus
`--decompress-points`; the script checks the complete packed fixture too.

The first wrapper-refusal test driver initially waited for an internal boundary
that a successful `step` immediately passes. That test process was terminated,
the driver predicate was corrected to observe the pending instruction, and the
corrected suite was rerun. It is retained as a test-driver diagnostic, not a
player bug or a successful earlier run.

[Component validation and limits](decompress-validation.json) record the local
checks separately from gameplay evidence.

**The actual HoF transition remains unfixed.** Remaining work includes resumable
alignment/interlacing, canonical compressed-asset access, the shared display/IRQ
owner and runtime/renderer handoff, physical A/B controls during load, matched
before/after frames and appropriate gameplay/GBA regression.
