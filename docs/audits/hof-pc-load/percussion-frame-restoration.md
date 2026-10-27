# Field-restoration percussion work (2026-10-26)

This is an unfinished local FLASH candidate, not a merged game fix or an
ordinary-input gameplay verification. The game branch is
`fix/flash-restoration-current-master`, based on `cabaa9af`; game master remains
`ea3b2624`. The overall field-music regression remains **failing**.

The previous source-frame admission rejected an already active CHAN8 and the
music channel's `drum_note` command. The candidate now retains the original
Audio1/Audio3 sound-start instructions, drops a drum when CHAN8 is busy, and
processes a newly started noise voice later in the same update. The native
sequencer still owns playback and commits once after the retained source call;
there is no speculative native tick or duplicate batch of APU writes.

Canonical noise data is checked before admission and again on resume. Changing
the instrument table after accepted output creates a retained terminal failure,
without more clock work, writes, or a native commit. CHAN8 uses its fixed tempo;
the current CHAN5 ID still controls the common ending helpers, including when
PressAB ends earlier in the same update.

Evidence:

- 76 original-ROM PlaySound calls cover all 19 instruments, free/busy CHAN8,
  and both audio banks. Every instruction supports refusal and serialized resume.
- 184 original noise note/end cases compare 864 actual APU accesses and the
  independent aggregate work estimate. Existing 200 PressAB cases remain intact.
- 106 complete-frame starting states compare 115,633 original CPU instructions.
  Each state has three independently traced port contexts: 318 cases and 5,586
  APU accesses. Tests check instruction order, reads, writes, APU phase, refusal,
  serialization, one native commit, and completed-call replay protection.
- Of those states, 102 come from ordinary sequencer playback of 12 percussion
  songs. Four explicitly override CHAN8 with a pending canonical instrument 19
  to exercise busy drops. No busy overlap occurred in the sampled ordinary
  trajectories; the controlled rows are not ordinary gameplay evidence.
- The oracle's added sound-start opcodes and relocation of unconditional loops
  preserve both existing golden files byte for byte: 311 CPU cases / 205,206
  instructions and 957 bus cases / 10,101 accesses. Unknown instructions and
  unspecified memory reads still fail the oracle.

The pre-slide baseline (audit 1357) has **210 passing audio tests and one failure**,
with no ignored or filtered tests. The
failure is `every_field_song_keeps_source_frames_and_native_channel_updates_in_sync`:
25 of 26 songs complete 4,096 updates with PressAB every 37 updates; SafariZone
stops on channel 0's `$EB` pitch-slide command at update 5. This test has not been
waived. Both default and source-clock-serde GBA audio builds and the Clippy
correctness check pass.

The private game dependency is now pinned to dotzuki `b8cdd36e`. Engine PRs
#114, #115, and #116 each passed all 12 CI jobs before merging. #115 preserves
the noise LFSR-width bit; #116 corrects sliding-note onset registers. Updating
this private dependency does not update game master.

Regenerate the new complete-frame fixtures with:

```sh
cargo run -p pokered-audio --example export_source_percussion_frames \
  --features source-clock-serde -- path/to/native-inputs.json
python3 docs/audits/hof-pc-load/generate-audio-frame-instructions.py \
  path/to/original.gbc path/to/pokered.sym path/to/native-inputs.json \
  crates/pokered-audio/src/source_clock/percussion_frame_work_golden.json
python3 docs/audits/hof-pc-load/generate-audio-frame-instructions.py \
  path/to/original.gbc path/to/pokered.sym path/to/native-inputs.json \
  crates/pokered-audio/src/source_clock/percussion_frame_bus_golden.json --apu
```

The generators require the pinned original ROM SHA-256
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
The source reference is pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.

## Pitch-slide follow-up (audit 1358)

Independent original-ROM execution found that slide initialization returns its
last division temporaries as the note's first frequency. It separately retains
the ordinary note frequency for later slide ticks. The Safari intro also exposes
the high-byte length-counter bit, and perfect pitch wraps the low byte without a
carry. Engine #116 preserves those onset values on all three tonal voices; its
96 engine regression combinations include paired SFX suppression. All-feature
engine checks pass (224 unit, one allocation, two doc tests), with ARM no_std,
formatting, and Clippy checks.

The candidate now has retained initialization and timer cursors, and integrates
initialization into the complete note-pitch/output child before the paired-SFX
gate. Independent source fixtures cover 48 init calls, 72 timer calls, and 192
note onsets: 108,978 instructions and 1,344 APU accesses. Every instruction is
checked for refusal without progress and serialized continuation; target
crossing suppresses writes, and a zero source divisor is explicitly rejected.
The old full-frame CPU and bus fixtures remain byte-identical after the offline
oracle's opcode extension.

These children are **not yet connected to the stream command and complete frame
owner**. They do not establish complete Safari playback, actual FLASH behavior,
or physical timing. After updating the private engine pin, default audio tests report **214 passing
and one failing**, with no ignored or filtered tests. The same SafariZone `$EB`
command at update 5 remains the only failure. The full field-music test still
has to pass without skips.

Regenerate the slide evidence with:

```sh
python3 docs/audits/hof-pc-load/generate-pitch-slide.py \
  path/to/original.gbc path/to/pokered.sym \
  crates/pokered-audio/src/source_clock/pitch_slide_golden.json \
  --engine-cases path/to/pitch_slide_onsets.rs
```

Remaining: pitch-slide stream decoding, frame-state/aggregate/timer wiring and
whole-frame original comparisons; broader ordinary-input FLASH
verification, current-combination GBA timing/performance/memory, fresh m49 and
independent CONTINUE, then the separate Party-to-START restoration gap. The
last native Game recording and GBA performance figure belong to earlier builds;
this change does not relabel them as current evidence.
