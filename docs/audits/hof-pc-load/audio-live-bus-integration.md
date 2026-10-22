# Source audio access propagation and evidence

The supported source audio frame now routes individual APU accesses through
`SourceAudioFrame` to the `AudioManager` APU. After the original frame RET, the
manager calls `advance_frame_without_output` on its existing sequencer once.
It does not run native batch register output a second time. The dependency is
pinned to `dotzuki@a4f500c831aa7a2eaa725a471e9e225d29b271d5`.

The callback admits a complete instruction before advancing time, performs its
read or write at the supplied machine-cycle offset, and returns the actual
register byte for a read. Accepted source time and APU effects are retained
across suspension. NR51 reads belong to the original helper/rest instructions;
they are not an entry snapshot. Commands such as consecutive `F0` each retain
their own NR50 write.

This is the audio-call path for admitted PokeCenter, Silph Co and Dungeon3 music
after an ordinary native update, including the finite PressAB SFX. The tests
start music and perform that ordinary update before beginning source frames;
they do not establish a source-driven music start or boot. Bank/mute and native
state must match the cursor. Begin requires no active fade, installed manager
post-hook, low-health alarm, queued output (`pending_nr50`), or deferred
music-after-SFX action. Broader SFX remain unsupported. The manager path does not
create the live Game/IRQ owner,
produce its missing source runtime bytes, or connect ordinary boot/CONTINUE and
physical player input to the Hall of Fame picture loader. These audio proofs do
not establish a player-visible HoF fix, whole-VBlank execution or physical PCM
equivalence.

## Suspension and terminal failure

The manager distinguishes a rejected pending instruction from a frame that has
already executed source work. A caller must keep the same cursor and exclusive
owner across suspension; restarting a partially executed frame would repeat
its earlier APU accesses.

| Outcome | Meaning and permitted continuation |
| --- | --- |
| `Preflight` | A validation check failed with zero accepted source cycles. No instruction ran; the owner may correct the rejected input before beginning/resuming. |
| `WrongMode` | A CPU cursor was offered to the bus API, or vice versa. No new instruction runs and the cursor is unchanged; it is not poisoned. Earlier source effects may exist, so continue with the original API without discarding/replaying the frame or running an ordinary update. |
| `Admission` | The pending instruction was not accepted. In particular, callback refusal retains that instruction without consuming a native tick. Earlier accepted source time/accesses remain, so any retry continues the retained cursor rather than restarting. |
| `SourceFailed` | Source cycles were already accepted and a later resume guard or final accounting invariant failed before native commit. The terminal error is retained, including across serialization; restoring a previously mismatched input does not make the frame replayable. |
| `Advanced` | The single native logical update already occurred and its final validation failed. Its outcome is cached; it cannot be retried as another native update. |

Once a bus frame has source progress, a changed native wave instrument or a newly
pending native NR50 write is a failed frame, not a retryable preflight. A missing
wave at the standalone pitch component remains its own pre-instruction rejection:
the component can accept a real table while preserving its cursor. That local
contract does not roll back earlier instructions in an enclosing manager frame.
A completed successful frame returns its cached result with no additional
callback, APU access or native tick.

## Evidence sources

All original-call fixtures below use `pret/pokered` revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c` and ROM SHA-256
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.
The bounded Python oracle reads the pinned original instructions and records
actual APU accesses with strict memory-read checks. It is test tooling, not a
runtime GB interpreter. Fixtures use controlled explicit scalar state; they are
not records of original boot or a live player run. No original fonts are rendered.

| Layer | Original-call fixture coverage | Source of the comparison |
| --- | --- | --- |
| Helper and pitch | 930 helper calls, 2,100 pitch continuations, 25,218 accesses | `output_live_bus_golden.json` refers to the unchanged 310 helper and 700 pitch CPU traces; the independent access wrapper supplies distinct values for successive NR51 reads. |
| Complete music note | 548 calls, 5,074 accesses | `music_note_bus_golden.json` reuses 538 complete CPU traces and adds ten consecutive-F0/noise-ending cases. Wave data comes from each source bank's pointer table. |
| Finite PressAB | 200 calls, 60,000 CPU instructions, 1,120 accesses | `press_ab_bus_golden.json` covers both banks, all five supported boundaries and five NR51 inputs, with actual original reads/writes. |
| Complete audio frame | 957 contexts, 10,101 accesses | `frame_bus_golden.json` expands 311 existing native scalar contexts plus eight explicit wave-instrument 5–8 contexts across three initial NR50/NR51 settings. The 311 original CPU traces, containing 205,206 instructions, regenerate unchanged. |

`generate-audio-frame-instructions.py` produces the frame evidence. The new bus
fixture references the existing CPU case identity rather than copying its full
instruction trace. The complete-frame test refuses every instruction before
acceptance, compares its actual source access, and serializes around hardware
accesses and frame boundaries. A second test runs 1,536 consecutive frames with
1/17/31-instruction suspension slices, retaining the cursor and committing one
native logical update without replaying APU writes.

The bus generator also asserts equality of the entire CPU trace against its
baseline for every port setting and wave override, rather than accepting equal
aggregate cycle counts. Its 319-row input is
`.task-tmp/fidelity-39-plus/audit-1275-native-frame-wave-inputs.json` (311 ordinary
rows plus eight explicit wave variants), whose hash matches `input_sha256` in
the bus fixture. The older 311-row export is not this bus fixture's input.

Reproduce with the existing cache, target and original ROM/symbol files:

```sh
CARGO_HOME=/home/jerry/.cache/open-pokered-fidelity/1030-tile-candidate-cargo-home \
CARGO_TARGET_DIR=/home/jerry/develop/open-pokered/.task-tmp/fidelity-1064-payday-target \
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_TEST_OPT_LEVEL=0 \
cargo run -p pokered-audio --features source-clock-serde --locked --offline \
  --example export_source_audio_frames -- \
  .task-tmp/fidelity-39-plus/audit-1275-native-frame-wave-inputs.json --wave-variants

python3 docs/audits/hof-pc-load/generate-audio-frame-instructions.py \
  .task-tmp/fidelity-39-plus/victory-95-source-setup-1f/reference.gbc \
  .task-tmp/stats-59-reference-sgvaaq/pokered.sym \
  .task-tmp/fidelity-39-plus/audit-1275-native-frame-wave-inputs.json \
  crates/pokered-audio/src/source_clock/frame_bus_golden.json --apu
```

## Validation records

Counts in this table overlap: a name filter may include a test listed by another
row. They must not be added into a distinct-test total. Exact commands, fixture
hashes and locally retained log hashes are in the adjacent validation JSON files.
The six component logs were moved byte-for-byte out of the documentation tree to
the existing `.task-tmp/fidelity-39-plus/audit-1275-*` location; they are not
committed artifacts.

| Check / cargo test filter | Result | Evidence |
| --- | --- | --- |
| `source_clock::channel_output_instructions` | 7 passed | `output-live-bus-validation.json`; includes three new live helper/pitch tests and prior CPU/store-only helper tests. |
| `source_clock::note_pitch_instructions` | 2 passed | `output-live-bus-validation.json`; prior 700-case CPU-only pitch fixture. |
| `music_note_instructions` | 5 passed | `music-note-bus-validation.json`; complete music bus, missing wave, instrument selection, prior CPU note and ending checks. |
| `source_clock::note_` | 9 passed | `music-note-bus-validation.json`; command, preparation, pitch and arithmetic regressions, including the two pitch tests above. |
| `press_ab_instructions` | 6 passed | `audio-press-ab-bus.md`; successful tool-session output, no separate log captured. |
| `audio_manager::source_frame_bus_tests` | 2 passed | `.task-tmp/fidelity-39-plus/audit-1275-frame-bus-tests.log`; 957-context source proof and consecutive-frame owner checks. |
| `source_bus_guard_tests` | 10 passed | `audio-live-bus-integration-validation.json`; mode separation, source-failed caching, pending-volume guards, real APU-state preservation and ordinary frame boundaries. |
| `changed_native_program_and_unsupported_source_context_reject_before_clock_or_tick` | 1 passed | `audio-live-bus-integration-validation.json`; prior CPU guard regression. |
| Full `cargo test -p pokered-audio --offline` | 191 passed, 0 failed/ignored; doctests 0 | `audio-live-bus-integration-validation.json`; default features, 101.78s. |
| GBA `pokered-audio`, default features | check passed, 3.85s | `audio-live-bus-integration-validation.json`; existing target/cache and nightly-2025-12-07. |
| GBA `pokered-audio`, `source-clock-serde` | check passed, 4.60s | `audio-live-bus-integration-validation.json`; existing target/cache and nightly-2025-12-07. |

The earlier broad `live_` filter is retained only as preliminary evidence in
`output-live-bus-validation.json`. It selected three new tests and four existing
tests with matching names; it is not an additional feature-complete regression
suite. The final whole-crate test command used default features without
`--features` or `--locked`; serialization assertions are enabled by `cfg(test)`.
It must not be described as a full `source-clock-serde` feature test run. Both GBA
checks completed with the existing `dead_code` warning and no errors. Their exact
commands, working directories and shared build environment are recorded in
`audio-live-bus-integration-validation.json`.

`rustfmt --check` passed on the 23 changed Rust files. A whole-crate
`cargo fmt -p pokered-audio --check` still reports pre-existing formatting in six
unchanged files: `audio_format_tests.rs`, `audio_manager_tests.rs`, `lib.rs`,
`output.rs`, `output_gba.rs` and `sfx_data.rs`. They are outside this patch and
were not reformatted. The validation record therefore reports the scoped format
pass and the whole-crate baseline failure separately. The retained whole-crate
format log was captured before a module-order correction in
`source_clock/mod.rs`; that changed file passed the later 23-file check. These
records describe the verified audio changes in the shared checkout, rather than
claiming equality of every checkout file with a later isolated PR.
