# Retained Audio1/3 timer instructions

This is an unused integration component. The Hall of Fame PC picture-loading
bug remains confirmed **unfixed and unwired**. No actual gameplay, IRQ completion,
APU write timestamps, or end-to-end frame parity is established by these tests.

`TimerFrameWork` retains each actual CPU instruction of the Audio1/3 update loop
and timer/vibrato bodies. Its input comes from the existing AudioManager sequencer;
there is no second channel owner. The interrupt owner must supply its actual bank
and mute byte. Supported banks are 2 and 31 with explicit mute zero, primed
Pokecenter/Silph/Dungeon3 channels, and supported timer-only PressAB states.

`begin_source_timer_frame` does not advance anything. Each
`step_source_timer_frame` invokes the owner's callback for at most one instruction.
Refusal must occur before the callback clocks that instruction. The cursor retains
the rejected instruction, accepted work survives suspension/serialization, and
changed source inputs reject before another instruction. The caller must retain
exclusive IRQ/audio ownership across suspension and must not also call the
ordinary frame update. After the final source RET, the manager performs its one
ordinary update and caches the result, including an advanced postcondition error.
Subsequent calls return that result without another callback or native tick.

The callback can feed an instruction duration to the common timeline. This does
**not** implement source WRAM/APU bus transactions: source instruction operands
are retained scalar CPU-work inputs, and canonical native channel/APU updates
still commit at the end. Correct per-write APU timing and live shared ports remain
required before runtime hookup. No early foreground wake or IRQ return is added.

Next-note parsing, duty rotation, pitch slide, nonzero mute, other sound banks,
unprimed/zero-delay channels, fade/alarm and deferred music resume remain explicit
unsupported states for this cursor. Existing ordinary and aggregate source-work
APIs retain their broader behavior; unsupported paths are not replaced by silence
or guessed instruction durations. `source-clock-serde` is optional; ordinary audio
builds acquire no serde runtime dependency.

Validation uses two independent forms of evidence:

* `generate-timer-instructions.py` interprets the pinned original ROM bytes with
  controlled scalar WRAM inputs. It renders nothing. The fixture records the PC
  and T-cycles of every actual instruction for 160 effect cases and 8 whole timer
  bodies across both banks. It covers early returns, paired SFX, noise/execute,
  vibrato direction/clamp/carry and source register-pointer branch boundaries.
  This is a bounded test oracle, not gameplay recording or a runtime GB emulator.
* Native manager tests run 96 updates in each of six song/confirmation configurations.
  Every supported timer instruction is first refused, then accepted, with serialized
  resumptions. Channel state remains unchanged until the final RET; the completed
  native update and real PCM match ordinary playback. Repeated completed calls
  consume nothing. Next-note frames use ordinary updates and are not counted as
  retained instruction coverage. Stale owner inputs and unsupported bank/mute/
  next-note/deferred-resume inputs reject without advancing.

The golden file can be reproduced with:

```sh
python3 docs/audits/hof-pc-load/generate-timer-instructions.py \
  /path/to/pinned/reference.gbc \
  crates/pokered-audio/src/source_clock/timer_instructions_golden.json
```

The generator requires ROM SHA-256
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`
from pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.
