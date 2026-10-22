# Supported whole audio update continuation

HoF PC picture loading remains **confirmed unfixed / unwired**. This change
composes the supported Audio1/3 update CPU body and commits the existing native
sequencer once. It does not supply physical APU write timestamps, the enclosing
IRQ return, canonical Game state producers or Game integration.

`SourceAudioFrame` retains the real outer eight-channel loop, timer/effect
instructions, complete supported music notes and endings, and finite PressAB
calls through the update's final RET. Music tempo changes reset all four music
fractions before later channels run. Endings clear their provisional source IDs.
Noise-channel endings account for the actual PressAB ID's cry-check branches.
Caller-owned native programs remain immutable; the cursor stores scalars and
stable content fingerprints, without copying or ticking a second sequencer.

Only Pokecenter (bank 2), Dungeon3 and SilphCo (bank 31), mute zero and the
supported finite PressAB are admitted. Source bank and mute are explicit owner
inputs. Original header offsets independently establish raw song IDs 189, 232
and 243 and PressAB ID 144; native enum values are not source IDs. Unknown songs,
banks, other active SFX, fades, alarm or deferred music reject explicitly. A zero
native delay is accepted only for the guarded newly constructed PressAB state.
Unprimed music rejects rather than inventing a source delay.

`AudioManager::resume_source_audio_frame` validates the unchanged native state,
clocks at most one instruction per callback, and commits its ordinary native
update only after the complete CPU body and its aggregate postcondition agree.
A refused instruction changes neither cursor nor native state. Completion caches
its success or advanced-error outcome, so repeated or serialized resumption
never repeats a native update. The callback must refuse before clocking and must
not reenter the manager/output; the caller owns exclusive IRQ/foreground access.
Physical native APU writes still occur at the ordinary update, not at the source
instruction timestamps. Future integration must avoid applying those writes twice.

The independent original-ROM interpreter walks 311 controlled whole updates,
205,206 instructions, covering all note-ready frames across 256 native updates
per song with/without periodic confirmation sounds. This includes Dungeon3's
initial 120-frame delayed wave/noise channels. Every non-ROM read must have an
explicit controlled fixture value or a preceding source store; missing reads
fail. Converted command targets are relocated only in these test fixtures.
Native remaining loop counts are translated to original ascending counts only
when a reachable finite loop provides the actual count. Instrument zero and
NR50/NR51 initial values are explicit controlled ports, not live APU observations.
No original graphics or fonts are decoded or rendered.

Tests recreate each native fixture using normal AudioManager music/SFX/update
APIs, then compare each accepted cycle duration to the corresponding original
PC's instruction. Every boundary is also refused and serialized. Separate
one-owner tests compare all channel states, tempos and rendered PCM against the
ordinary native manager for 1,536 frames, and verify repeated completion and
changed-program refusal. These are controlled tests, not Game playthroughs.

Reproduce with the pinned ROM and symbols documented by the other audio audits:

```sh
cargo run -p pokered-audio --example export_source_audio_frames \
  --features source-clock-serde --offline -- /path/to/native-inputs.json
python3 docs/audits/hof-pc-load/generate-audio-frame-instructions.py \
  /path/to/reference.gbc /path/to/pokered.sym /path/to/native-inputs.json \
  crates/pokered-audio/src/source_clock/frame_work_golden.json
cargo test -p pokered-audio --offline
```

Remaining: actual source WRAM/port producers, timestamped APU writes, enclosing
IRQ/foreground ownership, runtime hookup and ordinary boot/CONTINUE/physical
input end-to-end verification. CPU continuation evidence alone cannot close the
HoF issue or the overall fidelity goal.
