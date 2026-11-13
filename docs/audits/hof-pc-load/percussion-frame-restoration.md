# Field-restoration percussion work (2026-10-26)

> 存储说明：逐帧录制、压缩日志和配套运行报告已移出版本库。下文的录制文件名及哈希仅描述历史验证；复现输出请放在 `.task-tmp/` 或仓库之外，完成后清理。见[审计材料约定](../README.md)。

This documents an unfinished local FLASH candidate, not a merged game fix or an
ordinary-input gameplay verification. The game branch is
`fix/flash-restoration-current-master`, based on `cabaa9af`; game master was
`ea3b2624` at this initial checkpoint, and the overall field-music regression was
still **failing**. The completed pitch-slide integration below now passes that
regression, but broader gameplay and GBA validation remain open.

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

At that checkpoint, these children were **not yet connected to the stream command
and complete frame owner**. They did not establish complete Safari playback,
actual FLASH behavior, or physical timing. After updating the private engine pin,
default audio tests reported **214 passing and one failing**, with no ignored or
filtered tests. The same SafariZone `$EB` command at update 5 was the only
failure.

Regenerate the isolated slide evidence with:

```sh
python3 docs/audits/hof-pc-load/generate-pitch-slide.py \
  path/to/original.gbc path/to/pokered.sym \
  crates/pokered-audio/src/source_clock/pitch_slide_golden.json \
  --engine-cases path/to/pitch_slide_onsets.rs
```

## Pitch-slide frame integration (audit 1359)

The `$EB` stream command is now connected through note preparation, slide
initialization, aggregate work, timer continuation, and complete-frame native
state checks. The full field-music regression passes every one of the 26 songs
for 4,096 updates, both with and without PressAB every 37 updates. Default and
`source-clock-serde` audio test runs each report **217 passing and no failures**,
with no ignored or filtered tests.

Independent whole-frame fixtures cover 128 ordinary Safari starting states and
three independently traced port contexts per state: 384 bus cases and 744 APU
accesses. They check original instruction order and cycle counts, APU values and
phases, refusal and serialized continuation, and exactly one native commit.

Regenerate the complete-frame slide evidence with:

```sh
cargo run -p pokered-audio --example export_source_slide_frames \
  --features source-clock-serde -- path/to/native-slide-inputs.json
python3 docs/audits/hof-pc-load/generate-audio-frame-instructions.py \
  path/to/original.gbc path/to/pokered.sym path/to/native-slide-inputs.json \
  crates/pokered-audio/src/source_clock/slide_frame_work_golden.json
python3 docs/audits/hof-pc-load/generate-audio-frame-instructions.py \
  path/to/original.gbc path/to/pokered.sym path/to/native-slide-inputs.json \
  crates/pokered-audio/src/source_clock/slide_frame_bus_golden.json --apu
```

## Pre-integration target validation (2026-11-12–13)

Native checks now pass 217 audio tests in both feature modes, 3,457 core tests,
440 application tests, and all 11 subsystem scenarios. Two runs replay the same
ordinary SAVE and retained 849-command tape through normal CONTINUE and real
START/Pokemon/FLASH inputs. Each has 240 consecutive captures after the initial
image; all state records and PNG hashes repeat exactly. The current build returns
to RockTunnel1F at capture 206, executes source audio work on 45 captures, and
finishes with the correct map and player position. It preserves 235 of the old
candidate's 240 images, including the final image; transfer-phase differences
are not treated as original physical-timing evidence.

The default GBA ROM builds successfully. All seven performance scenarios pass
the unchanged 15%-or-25-tick regression budget. All 72 sprite ranges (23,808
bytes) match the original ROM through both native resources and embedded GBA
assets. Two seeded diagnostic runs use real START/Pokemon/FLASH inputs twice and
return after 46 logical updates each, with 1,495,685 and 1,572,527 hardware timer
ticks. Both finish with a 50,296-byte largest free block and 9,368 bytes of
painted stack margin; the running heap low is 7,008 bytes. These are constrained
GBA fixture results, not NEW GAME or ordinary-SAVE playthrough evidence.
Route22 completes all 40 encounters with a 26,784-byte minimum contiguous heap
and 9,392-byte stack margin. The 13 broader graphics/map/animation/PC/SRAM
scenarios also pass, with a 7,832-byte minimum contiguous heap and 8,632-byte
stack margin. All 23 connection, encounter and field-move frame-timing scenarios
pass as well, together with six slow-cartridge hardware scenarios and three
unskipped opening scenarios.

OAM instructions now use a compact three-byte representation and count before
one exact allocation. The native UI sampler uses a bounded 256-entry scratch
array and grows its retained pattern pool by the exact number of new entries.
Short moving sprite sheets resolve their real adjacent ROM resources instead
of rejecting the upper copy.

A fresh m01–m49 run now reaches `PLAYTHROUGH REACHED REQUESTED MILESTONE`.
It completes the Champion, Hall of Fame, credits and TheEnd, retains eight badges
and one Hall of Fame record, and lets the game write its own 32,768-byte save.
A separate process continues that save to PalletTown `(5,6)` with three party
members and 30,330 money. No debug save is used for this verdict.

At this checkpoint the remaining work was integration with current master,
which already contained the separate Party-to-START restoration work from PR
#439. No earlier recording or performance figure is relabeled as the result of
the later integrated build.

## Master integration validation (2026-11-13)

The candidate was merged with `origin/master` at `d17f9742`. One automatically
duplicated `noise_next_note` body was removed. The final runtime sources match
the integrated master; this follow-up retains the validation record rather than
reapplying fixes that master already contains.

Both audio feature modes pass **225 tests** each. Core passes **3,495 tests**;
the application library passes **494 tests**, with 40 existing ignored tests.
All 11 subsystem scenarios and all 27 GBA performance/memory/frame-timing
tool tests pass. The default GBA ROM builds successfully at approximately
5.8 MiB, and the production core/app Clippy correctness check passes.

A separate fresh m01–m49 run reaches `PLAYTHROUGH REACHED REQUESTED MILESTONE`.
It completes the Champion, Hall of Fame, credits and TheEnd with eight badges
and one Hall of Fame record. The game writes a 32,768-byte save; an independent
process continues it to PalletTown `(5,6)` with three party members and 33,044
money. This run starts from power-on and uses button input plus observation,
not a seeded save or resumed checkpoint.

The pre-integration GBA timing and memory figures above remain scoped to that
candidate. Original physical audio/display timing parity is still not claimed.
