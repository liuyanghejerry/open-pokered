# Next-note arithmetic instruction progress

The Hall of Fame PC picture-loading issue remains **confirmed unfixed / unwired**.
These arithmetic cursors are a component of the pending full audio body. They are
not a complete next-note update, Game hookup, IRQ completion, or APU timing proof.

The existing aggregate multiply, frequency and music note-length helpers now
consume the same retained instruction cursors. Temporary registers and branch
progress can be serialized; native sequencer and APU state are never changed by
these helpers. Music note-length work stops at the actual note-pitch entry or the
noise channel's early RET, rather than claiming the remainder of the note or an
interrupt return. There is no expanded instruction queue or runtime ROM emulator.

`generate-note-math-instructions.py` independently interprets pinned original ROM
bytes under controlled scalar inputs. The test-only bounded interpreter was
extended for these instructions; regenerating the earlier timer fixture remains
byte-identical. No font, picture or display is decoded/rendered. Across Audio1/3:

* 1,044 multiply cases cover source wrapping arithmetic and branch order.
* 264 frequency cases cover actual pitch tables, octave wraparound, each shift
  instruction, and the returned carry flag.
* 778 note-length cases cover both multiply calls, fractional accumulation,
  every instruction boundary, refusals/serialized resumptions, and the exact
  note-pitch/early-return boundary.

All previously recorded aggregate arithmetic/note-length/source-song tests remain
in force. The full audio suite passes 147 tests; default and optional-serde GBA
no_std checks pass. These are controlled CPU/codec checks, not actual gameplay.

Still required: retained stream command parsing and call/loop work, note-pitch
and channel output instructions, full native frame composition, actual source
ports and per-write APU timing, common IRQ/foreground ownership, and Game runtime
integration followed by ordinary-input end-to-end validation. No timing table or
captured source state is substituted for these missing producers.

Reproduce the golden fixture with the same pinned ROM and its symbols:

```sh
python3 docs/audits/hof-pc-load/generate-note-math-instructions.py \
  /path/to/pinned/reference.gbc /path/to/pinned/pokered.sym \
  crates/pokered-audio/src/source_clock/note_math_instructions_golden.json
```

ROM SHA-256: `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
