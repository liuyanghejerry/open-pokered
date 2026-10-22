# Live music-note APU propagation

`StreamInstructions`, `NotePreparation`, and `MusicNoteInstructions` offer one
`ApuInstruction` at a time to the existing clock/APU owner. Their CPU-only APIs
remain available and share the same retained command, length, and note state.
The cursors do not own an APU, tick a second clock, or commit a native frame.

Each original `F0` master-volume command offers its own NR50 write, using the
parameter just read from the program. The write is at T+8 of its 12T instruction.
The final retained `master_volume` value does not replace earlier writes.
Preparation forwards those accesses unchanged. Music supplies pitch/output
with the actual prepared delay, volume, and duty, the caller's stereo panning,
and either the stream's newly selected wave instrument or the caller's initial
instrument. Wave bytes come from the explicit caller-owned table.

An audible wave note requires its selected pattern. Without it, the pitch child
rejects before accepting an instruction; its cursor and cycle count are unchanged
and it can resume after the owner provides the missing table. Wave rests and
notes covered by paired SFX do not need a pattern. A successful hardware read
must return its real byte; the owner must reject before making clock/APU effects
when it cannot accept an instruction. Finishing a cursor never replays accesses.
The existing narrow noise/non-cry ending remains the only supported music end
handoff; this change does not invent inputs for broader ending paths.

## Original-ROM evidence

`generate-music-note-bus.py` runs the bounded original-instruction oracle against
the pinned original ROM (SHA-256
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`). It records actual
APU reads and writes, including their values, instruction indices and order.
The fixture contains 548 complete calls and 5,074 accesses from banks 2 and 31.
Its first 538 calls must retain the existing complete original CPU traces; ten
additional calls exercise consecutive distinct F0 writes across all music
voices and the supported noise ending. Wave patterns are read from the original
bank's pointer table and explicitly supplied to the test cursor.

The tests compare every instruction duration and access, deny every instruction
once without changing the cursor, serialize/restore every continuation, and
verify final original note results. Focused cases cover missing-wave rejection
and retry, stream instrument override, rests, paired-SFX suppression and the
noise ending. These are controlled source-call proofs, not a gameplay or
rendering claim.

Regenerate the fixture from the existing reference assets:

```sh
python3 docs/audits/hof-pc-load/generate-music-note-bus.py \
  .task-tmp/fidelity-39-plus/victory-95-source-setup-1f/reference.gbc \
  .task-tmp/stats-59-reference-sgvaaq/pokered.sym \
  crates/pokered-audio/src/source_clock/music_note_bus_golden.json
```

Run the music CPU and live-bus tests with the existing shared cache and target:

```sh
CARGO_HOME=/home/jerry/.cache/open-pokered-fidelity/1030-tile-candidate-cargo-home \
CARGO_TARGET_DIR=/home/jerry/develop/open-pokered/.task-tmp/fidelity-1064-payday-target \
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_TEST_OPT_LEVEL=0 \
cargo test -p pokered-audio --features source-clock-serde --locked --offline \
  music_note_instructions -- --nocapture
```

Validation: all five music CPU/live-bus tests passed (17.42s), and the nine
`source_clock::note_` command, preparation, pitch and arithmetic regression
tests passed (7.13s). Their logs are retained outside the repository at
`.task-tmp/fidelity-39-plus/audit-1275-music-note-bus-tests.log` and
`.task-tmp/fidelity-39-plus/audit-1275-music-note-bus-regression.log`.
`music-note-bus-validation.json` records the exact filters, paths and hashes.
The existing unrelated `sfx_data_tests.rs` unused-import warning remains.

These component results do not establish ordinary Game boot/CONTINUE input,
Hall of Fame picture loading, or complete IRQ and device-output ownership. The
audio-frame manager's separate validation is summarized in
`audio-live-bus-integration.md`.
