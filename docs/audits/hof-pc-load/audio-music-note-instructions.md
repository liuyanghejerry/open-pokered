# Retained Audio1/3 music note body

The Hall of Fame PC picture-loading issue remains **confirmed unfixed / unwired**.
This is a reusable CPU-work component, not an end-to-end game fix, physical APU
write scheduler, complete audio frame, IRQ epilogue, or foreground wakeup.

`MusicNoteInstructions` now composes supported PlayNextNote entry, command
fetch/call/return/loop processing, note-length arithmetic, note-pitch handling,
channel setup and the actual enclosing RET. Every child retains its own next
instruction; the composition inserts no synthetic CALL/RET or fixed-duration
placeholder. Tempo fractional resets, paired-SFX suppression, rests, wave RAM
copy loops, perfect pitch and cry frequency-modifier carry branches are included.

The existing native sequencer remains the only canonical channel owner. Cursors
retain scalar CPU temporaries and read the owner's converted stream; they do not
clone a native channel, tick a second sequencer, or execute original ROM opcodes
at runtime. Pitches come from the existing source pitch data. Inputs must come
from the owning audio context; bank 2/31 is explicit and channel-0 paired status
must agree with its source SFX ID. Refusal precedes clock advancement and leaves
the next instruction intact. Completion is cached as a result and executes
nothing on repeated calls. No native audio update is performed by this body.

The nested hardware-channel helpers describe CPU work and returned values only.
They do not timestamp physical source WRAM/APU writes. `wave_return_frequency`
is the DE returned by ApplyWavePatternAndFrequency, not an APU snapshot or a claim
that a rest has no register effects. Pending stream waveform/volume fields mean
that the source command wrote them, not that an initialized byte was observed.

Independent bounded test interpretation of the pinned original ROM covers:

* 310 channel-setup traces in both banks, including register-pointer, duty/length,
  stereo/panning, waveform copying and cry carry edges.
* 700 note-pitch continuations, including rests, paired sound channels, perfect
  pitch, octave and cry-modifier carry boundaries and global cry IDs.
* 538 complete music-note calls, from actual entry through final RET, including
  command controls, noise rests, tempo resets and paired/cry configurations.

Each instruction is refused, serialized/resumed, then accepted in tests. Its
T-cycles and final result fields are compared to the independent original trace.
All earlier timer/math/stream/preparation fixture files regenerate byte-identically
with the extended test-only interpreter. These are controlled scalar fixtures,
not actual gameplay recordings. Nothing renders fonts or display pixels.

The first whole-note fixture check exposed an oracle bookkeeping error: explicitly
initializing the wave instrument was being reported as a command write. The oracle
now identifies the pinned actual store instruction before returning a pending wave
write. All result-field assertions remain; no comparison was skipped. Initial
failure and corrected logs are retained in the audit workspace.

Earlier component validation JSON files record their historical checkpoints and
hashes. `audio-music-note-validation.json` describes this combined endpoint.

Still required before runtime hookup: terminal channel-end and finite PressAB
continuations, full frame composition with one native commit, actual shared
WRAM/APU ports and per-write timing, common IRQ/foreground ownership and its
canonical runtime producers. Then ordinary-input Game replays must establish
end-to-end behavior. Unsupported pitch slide, rotating-duty timer paths, drums,
other banks/mute states and other sounds remain explicit unsupported paths.
No component-only green check closes these gaps or the full fidelity goal.

```sh
python3 docs/audits/hof-pc-load/generate-channel-output-instructions.py \
  /path/to/pinned/reference.gbc /path/to/pinned/pokered.sym \
  crates/pokered-audio/src/source_clock/channel_output_instructions_golden.json
python3 docs/audits/hof-pc-load/generate-note-pitch-instructions.py \
  /path/to/pinned/reference.gbc /path/to/pinned/pokered.sym \
  crates/pokered-audio/src/source_clock/note_pitch_instructions_golden.json
python3 docs/audits/hof-pc-load/generate-note-stream-instructions.py \
  /path/to/pinned/reference.gbc /path/to/pinned/pokered.sym \
  crates/pokered-audio/src/source_clock/music_note_instructions_golden.json --whole-note
```
