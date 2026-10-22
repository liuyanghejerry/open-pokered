# Retained finite PressAB body

HoF PC picture loading remains **confirmed unfixed / unwired**. This component
executes CPU work only: no native tick, APU timestamp scheduling, complete audio
frame, IRQ return or Game hookup is provided.

`PressAbInstructions` retains the complete supported confirmation-tone
PlayNextNote call, including optional duty command, real parameter fetches,
SFX note-length calls, two multiply routines, SetSfxTempo/IsCry, duty/envelope/
frequency writes' CPU work, channel setup and both actual RETs. It reuses the
existing arithmetic, hardware-channel and ending continuations. It never copies
or independently ticks a native channel/sequencer.

The context explicitly selects bank 2/31 and the original PressAB ID 144, exact
finite stream, flags1=4, flags2=0, note speed 1 and the supported native SFX tempo.
The caller must select this entry only when its note is actually ready; the
native constructor's ready-zero exception still requires the existing explicit
constructor-origin guard. Starting offsets are 0, 6, 10, 14 and 18. Stream data
remains caller-owned and must match the canonical program throughout suspension.
Refusal advances no instruction; serialized resumption retains fetched bytes and
pending multiplies/calls. Completion repeats no RET or native tick.

The ending outcome contains actual pending ending actions and preserves fraction,
duty and reload state. It does not pretend that the original ending zeroed delay,
envelope or frequency RAM. `Note` separately returns computed note properties
and the wave/frequency helper's actual DE return, not a complete APU snapshot.

The independent pinned-ROM generator verifies that both source header tables
place PressAB at ID 144, point at channel 5, and contain the exact 19-byte native
program. It generates 40 whole calls across both banks. For Audio3 it additionally
matches every earlier recorded original PC/opcode/T-cycle and register result in
`press_ab_work_golden.json`, rather than validating only an aggregate duration.
Each instruction boundary is refused and serialized/resumed in tests. Changed
program/context variants reject without replacing the voice with silence.

All 161 audio tests pass; default and optional-serde GBA no_std checks pass. These
are controlled source/codec tests, not actual Game input recordings. Nothing
renders fonts or graphics. No worktree, target, game binary or ROM copy is added.

```sh
python3 docs/audits/hof-pc-load/generate-press-ab-instructions.py \
  /path/to/pinned/reference.gbc /path/to/pinned/pokered.sym \
  crates/pokered-audio/src/source_clock/press_ab_instructions_golden.json
```

Next: assemble supported timer/music/PressAB paths into an entire source update
with exactly one existing native commit, then supply actual shared ports/APU
write timing, IRQ/foreground ownership and runtime state producers. Broader
sounds/mute/banks and unsupported paths still require implementation. Ordinary
input end-to-end verification remains required before closing the HoF issue.
