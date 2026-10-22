# Shared native audio admission

The complete source ISR needs to refuse work before a native audio update, then
advance its one existing sequencer once after admission. The legacy
`update_frame_with_source_work` intentionally ticks even when accounting reports
an unsupported sound; it cannot serve as a retryable instruction.

`AudioManager::try_update_frame_with_source_work` shares that method's readonly
preparation. It prepares supported work without mutation, invokes the owner's
`FnOnce` admission callback, then calls the ordinary manager update once and
checks the existing source postconditions. The exclusive manager borrow keeps
sound state stable across preparation/admission/commit. Hosted PCM output
forwards under its existing manager lock. No second sequencer, clock, input
shadow or snapshot token is introduced.

The result distinguishes whether a native update happened:

- `Preflight(error)`: unsupported state or unavailable hosted output; callback
  was not called, native manager unchanged.
- `Admission(error)`: callback refused; native manager unchanged. The callback's
  own source-clock/progress effects are outside this API and are not rolled back.
- `Advanced(error)`: the native update already happened and its postcondition
  failed. The owner must record that outcome and must not retry the update.
- Success: exactly one native update happened.

The legacy always-update API keeps its existing semantics. Support remains the
existing primed POKECENTER/SILPH_CO/DUNGEON3 music and finite PressAB paths.
Unsupported sounds/fades/alarms/constructor states are not converted to silence
or assigned invented work. The new admission path refuses them before updating.

[Validation](audio-admission-validation.json) records 138 audio tests passing,
including the existing 10,000-update original Pokecenter corpus, plus the GBA
no_std audio-package check. Four new tests compare complete channel state and
real PCM over denied/admitted updates, unsupported preflights and later ordinary
updates, a deliberately conflicting pending-resume unit fixture that reports
Advanced after one update, and hosted output forwarding.

This is a pure unused interface. The callback is not a source instruction
continuation, and aggregate work cycles do not establish precise port-write/APU
timing or cross-budget execution. No clock/IRQ/wake or serializable instruction
progress is owned here. The GBA AudioOutput still uses its existing physical
VBlank catch-up; its source-owner bridge remains unfinished. Nothing is wired
into PokemonGame and the actual Hall of Fame loading gap remains unfixed.

Next work must provide FadeOutAudio/audio dispatch and real retained instruction
work/once-only outcomes, playtime/input/IRQ return, canonical common runtime
producers and picture/renderer handoff. Ordinary boot/CONTINUE player input,
matched visual evidence and native/GBA regressions remain required.
