# Retained channel endings and noise-music handoff

The HoF PC picture-loading issue remains **confirmed unfixed / unwired**. This
work adds CPU continuations, not physical APU timing, a native tick, complete
frame/IRQ ownership, or a Game runtime fix.

`ChannelEndInstructions` starts at the actual sound_ret tail after command fetch,
return comparison, SOUND_CALL check and untaken JR nz. It retains music/noise/SFX
ending paths, wave-channel restart, conditional channel-output disabling, cry
classification, saved-volume restoration and the cry companion's command-pointer
rewind. The source conditional RET and its actual 20 T-cycles are retained.

Inputs for panning, disable-output, saved volume and the original source pointer
word are optional. Construction requires a value exactly when its path reads it;
unused fields may remain unknown. No zero/default byte or pointer is substituted.
Results report actual pending writes and whether the voice ends, rather than
inventing values for untouched fields. The pointer is an original 16-bit source
word, not a converted-stream offset or a captured runtime address. A genuine
runtime producer is still needed on paths that read it.

`StreamInstructions` and `NotePreparation` expose the exact pending ending
context. `MusicNoteInstructions` now hands ordinary non-cry noise endings to the
new tail immediately after the accepted branch, and includes the enclosing RET.
Every step still clocks exactly one source instruction; there is no zero-clock
normalization between children. The result is `MusicNoteOutcome::End` with
pending actions, distinct from `::Note`; it does not fabricate a note length or
inactive delay/frequency values. Broader endings remain explicit unsupported
boundaries until their required source state can be provided.

Independent controlled original-ROM checks cover 660 ending tails across both
banks, all channel kinds, cry boundaries, disable-output variants and source
pointer wraparound. Another 88 whole noise-ending calls include real prefixes,
commands, call/return/loop control and final RET under non-cry SFX IDs. Each
instruction is refused, serialized/resumed, then checked against its original
PC/T-cycles and final result. All earlier normal-note comparisons remain in force.
No fonts or graphics are rendered; these are not gameplay recordings.

Full audio tests: 158 pass. Default and optional-serde GBA no_std checks pass.
All nine fixture outputs regenerate byte-identically with the expanded test-only
ROM interpreter. No new worktree, target, game binary or ROM copy is needed.

Still required: finite PressAB next-note continuation, complete frame composition
with one native commit, genuine source-state/port producers and per-write APU
scheduling, shared IRQ/foreground ownership, then ordinary-input Game validation.
No component-only check closes the HoF issue or the wider fidelity goal.
