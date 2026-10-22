# Retained command stream and note preparation

The Hall of Fame PC picture-loading issue remains **confirmed unfixed / unwired**.
These components are not complete notes, audio frames, IRQ returns, APU write
scheduling, or Game runtime integration.

`StreamInstructions` retains each Audio1/3 instruction from PlayNextNote entry
through the note-length entry. It reads the existing owner's converted stream
without retaining a copy, and keeps CPU temporaries for real command fetches,
call/return pointers, finite/infinite loops, speed/volume/duty/octave commands,
perfect pitch, vibrato, tempo, and master volume. Refusal does not consume an
instruction or input byte. A byte already read survives serialized resumption.
No source ROM instructions are interpreted at runtime; there is no expanded
instruction queue or second canonical sequencer.

The legacy linear dispatch helper uses this cursor while preserving its contract
that control commands are handled by the separate stream helper. The existing
source-work and native/PCM tests remain in force.

`NotePreparation` composes that stream cursor with the retained note-length
arithmetic. It inserts no CALL/RET at the handoff: music falls directly into note
length. A tempo command clears the channel's fractional accumulator even when
it sets the existing tempo value. The component stops at actual note-pitch entry,
or after the noise music channel's early RET. The enclosing PlayNextNote RET,
pitch handling and hardware output still require their own instructions.

The caller must preserve exclusive ownership of stream data/native audio and
clock each complete instruction or reject it before clocking. These are CPU-work
cursors: native channel writes and physical source WRAM/APU accesses are not
performed at their original timestamps by this interface. Result fields for
master volume, waveform and vibrato are pending effects, not evidence of live
APU parity. Ordinary game audio continues to use its existing owner.

Unsupported channel-end bodies, pitch slide, drums, other command families and
more than 100 immediate commands stop explicitly without inventing a note or
return. The valid original channel-ending `sound_ret` path is named
`ChannelEndUnsupported`, not treated as an invalid original script. These paths
must be implemented before runtime hookup.

The independent pinned-ROM scalar interpreter generates 478 command-stream
instruction traces across both banks, then 478 whole stream-plus-length traces.
Tests refuse and serialize every instruction boundary, compare the exact next
T-cycle duration, verify pointers/loop counters/all result fields, and verify that
completed boundaries do not execute extra instructions. These are controlled
fixtures, not gameplay recordings. No graphics or font patterns are rendered.
The older timer and arithmetic fixtures remain byte-identical after extending
the test-only interpreter. All 150 audio tests and default/serde GBA checks pass.

```sh
python3 docs/audits/hof-pc-load/generate-note-stream-instructions.py \
  /path/to/pinned/reference.gbc /path/to/pinned/pokered.sym \
  crates/pokered-audio/src/source_clock/note_stream_instructions_golden.json
python3 docs/audits/hof-pc-load/generate-note-stream-instructions.py \
  /path/to/pinned/reference.gbc /path/to/pinned/pokered.sym \
  crates/pokered-audio/src/source_clock/note_preparation_golden.json --through-length
```

Next: source note-pitch/channel output instructions, terminal noise/PressAB
paths, full frame composition with one native update, actual shared ports/APU
write timing and IRQ/foreground ownership, then Game hookup and ordinary-input
end-to-end verification. No component-only check closes the HoF bug.
