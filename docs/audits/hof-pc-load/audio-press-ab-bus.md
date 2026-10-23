# Finite PressAB APU instruction propagation

`PressAbInstructions::try_step_with_bus` forwards the actual APU accesses of the
already supported finite PressAB program through its existing retained CPU cursor.
The clock-only API remains available. No channel-ending support has been widened:
the supported non-cry channel 4 ending has no APU access.

## Source behavior

The source is pret/pokered revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, `audio/engine_1.asm` and
`audio/engine_3.asm` at `Audio*_sfx_note`, `Audio*_EnableChannelOutput`,
`Audio*_ApplyDutyCycleAndSoundLength`, and
`Audio*_ApplyWavePatternAndFrequency`. Both banks perform these accesses for each
supported note, in this order:

| Access | Value | Instruction / access position |
| --- | --- | --- |
| NR11 write | computed delay byte OR actual duty byte | `ld [hl],d`, 8 T / +4 T |
| NR12 write | next program envelope byte | `ld [hl],d`, 8 T / +4 T |
| NR11 write | (computed delay byte AND $3f) OR duty | `ld [hl],d`, 8 T / +4 T |
| NR51 read | byte returned by the actual bus owner | `ldh a,[rAUDTERM]`, 12 T / +8 T |
| NR51 write | retained read byte OR $11 | `ldh [rAUDTERM],a`, 12 T / +8 T |
| NR13 write | program frequency low byte | `ld [hl],e`, 8 T / +4 T |
| NR14 write | source helper's frequency high/control byte | `ld [hl],d`, 8 T / +4 T |

The first NR11 write intentionally does not apply the later helper's `$3f` mask.
Channel 4 takes the SFX branch that skips stereo panning; it does not read the
wave instrument. The unused `NoteBusInput` panning/wave fields therefore do not
replace any source read. Its NR51 operand always comes from the execution
callback and is retained in the output child's existing cursor.

Rejection must happen before the owner clocks or changes its bus. A rejected
instruction does not advance the CPU cursor, its cycle count, or its child.
Successful read callbacks must return `Some(actual_byte)`. Program changes are
refused before invoking the callback. Mixing the clock-only and bus API within
one active continuation is not supported: the clock-only path does not acquire
the live read operands required by later bus stores.

## Independent evidence and tests

`generate-press-ab-bus.py` executes the pinned original ROM bytes with the
existing bounded test oracle. It imports `source_apu_access_oracle.py` to observe
the original reads/writes and derive each access position from the original
opcode, independently of Rust's instruction constructors. The ROM SHA-256 is
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
No ROM or rendering artifact is created by this generator.

The fixture covers both banks, all five supported command boundaries, existing
fraction/duty variants, and five actual NR51 inputs. Its 200 cases contain
60,000 original CPU instructions and 1,120 APU accesses. Each ordinary note has
six writes and one read; every supported ending has zero accesses.

The three added tests compare every instruction and access against that fixture,
reject each instruction twice before accepting it, and serialize/restore at every
instruction boundary. They also verify that an accepted NR51 byte survives
serialization and a later bus change, and that an unavailable NR51 read refuses
without repeating preceding NR11/NR12 writes. Final logical results and final
bus bytes are checked separately.

Validation on 2026-10-22:

- All six `press_ab_instructions` tests passed with `source-clock-serde`.
- GBA-target release `pokered-audio` check with `source-clock-serde` passed using
  the existing GBA target configuration and nightly-2025-12-07.
- The fixture regenerated successfully from the existing original ROM.

These checks establish the finite component's bus contract. They do not by
themselves prove whole-frame routing or the once-only native commit; the current
manager path has separate validation summarized in `audio-live-bus-integration.md`.
They also do not establish Game/HoF or IRQ-owner integration, hardware PCM
equivalence, or support for other SFX programs. The component test and GBA
commands returned successfully in the tool session; no separate log file was
captured, so no log hash is asserted for these two observations.
