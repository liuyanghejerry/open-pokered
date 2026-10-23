# Source timer/effect APU stores

`timer_work::bus` attaches the supported Audio1/3 timer/effect APU store to the
existing `EffectInstructionWork` and `TimerFrameWork` instruction cursors. It
does not own an APU, advance a clock, update a second sequencer, or connect the
Hall of Fame picture loader to `Game`.

## Contract and supported paths

- `EffectInstructionWork::try_step_with_apu` accepts one instruction's T-cycle
  count and optional `channel_output_instructions::bus::ApuWrite` together.
  The caller must refuse before performing either clock or write side effects.
  Refusal retains the cursor, including the pending store. The enclosing source
  frame supplies and validates bank 2/31.
- `EffectBusInstructions` adds explicit bank validation and accepted-cycle
  accounting for an individual effect call. `TimerFrameBusInstructions` wraps
  the existing complete timer-only frame, retaining its context and cycle
  counter. Neither wrapper owns another instruction timeline.
- Inputs are immutable, explicit `ChannelEffectWorkInput` values. The complete
  frame also requires the actual bank, zero mute byte, sound-ID table and an
  input for each active channel. It rejects a paired-SFX flag inconsistent with
  that table, missing channels and unsupported paths before executing anything.
- Existing source branches remain supported: paired-music suppression, the
  execute-music/noise gate, vibrato delay decay, zero extent, rate countdown,
  and both saturated vibrato directions. Individual effects support source
  note-delay underflow from zero; a complete timer-only frame requires delays
  above one, as before. A next-note parse, reachable duty rotation and reachable
  pitch slide remain unsupported rather than supplying absent payloads.

For these paths, the only APU write is the final `ld [hl], d` in
`Audio1_ApplyMusicAffects` / `Audio3_ApplyMusicAffects`, after
`GetFrequencyRegister`. It writes the saved frequency low byte plus the upper
extent nibble, or minus the lower nibble, saturated to 255 or zero. The flags
select the direction. Channels 0/4, 1/5, 2/6 and 3/7 target `$FF13`, `$FF18`,
`$FF1D` and `$FF22`, respectively. The following `ret` is a separate instruction.
No current APU register is read by these supported branches, so no optional port
default is needed or invented.

The reported store belongs to the actual source instruction. Its callback is
not a claim about the write's hardware subinstruction timestamp. The owner must
also perform the once-only native sequencer state commit without replaying
native batch APU output. That owner and the live Game/IRQ connection remain
separate integration work.

## Independent source evidence

`generate-timer-bus.py` uses the pinned ROM interpreter in
`generate-timer-instructions.py`, with strict read validation and actual APU
store recording. It does not reproduce the Rust write-value formula. The new
fixture preserves all 160 existing effect instruction traces and eight complete
timer traces, then extends them to:

- 492 individual calls in banks 2 and 31;
- 72 complete timer updates with empty, sparse, paired and full channel tables;
- 448 actual APU stores, recorded as instruction index, source PC, address and
  value;
- all nibble magnitudes, both directions, arithmetic boundaries, all four port
  addresses, delay/rate/extent exits, and flags skipped by earlier source gates.

Reference: `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`;
ROM SHA-256:
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
The generator only executes controlled audio routines. It performs no graphics
or font rendering and records no player run.

Regeneration, using an existing locally built reference ROM and symbol file:

```sh
python3 docs/audits/hof-pc-load/generate-timer-bus.py \
  /path/to/reference.gbc /path/to/pokered.sym \
  crates/pokered-audio/src/source_clock/timer_bus_golden.json
```

Tests compare every instruction and its optional store, refuse each twice,
serialize and resume at every accepted boundary, and require exactly one final
completion. They cover both the direct existing effect cursor API and its
banked wrapper, plus invalid/missing context. The original CPU-only tests and
fixtures remain unchanged.
