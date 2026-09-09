---
name: key-animation-differential
description: Compare original Pokémon Red/Blue and current pokered-app visual animation behavior by recording matched emulator scenarios, aligning frames, and producing evidence-backed differential reports. Use for battle-entry, field-move, movement, transition, and overlay animation audits; not for generic gameplay QA or implementing fixes.
---

# Key Animation Differential

Use this skill when the user wants to verify that a visual animation in the current game matches the original ROM. The deliverable is a reproducible pair of recordings, frame evidence, and a report that separates visual differences from setup or asset differences.

## Scope and boundaries

- Audit behavior only unless the user explicitly asks for a fix. Do not modify game logic while collecting evidence.
- Prefer the normal Red ROM for the reference. An official DEBUG build is acceptable when it makes a deterministic setup possible, but record the exact ROM variant and do not call it pixel-identical to Red.
- Compare the same semantic event, not raw file indexes from unrelated emulator runs. Keep the input trace, pre-trigger state, trigger frame, and first stable post-animation state.
- Never grant `PASS` from representative frames, a contact sheet, or matching source constants alone. `PASS` requires a contiguous raw-time window and quantitative timing/trajectory evidence from both implementations.
- Treat species, player name, language, debug labels, palette, and save-data differences as confounders. Either normalize them or state that the comparison is limited to animation geometry/timing.
- Use temporary directories created with `mktemp -d`; preserve only the compact evidence needed by the repository.

## Required context before acting

1. Read [`pokered-debug`](../pokered-debug/SKILL.md) for the debug-server protocol and recording flags.
2. Read [`playthrough-regression`](../playthrough-regression/SKILL.md) when choosing milestone/scenario coverage or when the audit is part of a regression investigation.
3. If the repository has `.codegraph/`, use CodeGraph before `rg`, `find`, or opening source files to locate the relevant symbols and call paths.
4. Inspect `AGENTS.md` and `CLAUDE.md` for build, screenshot, and artifact conventions. Check `git status --short` and preserve unrelated user changes.

## Workflow

### 1. Establish the reference/current pair

Record these in the report before interpreting frames:

- current repository commit and build command/binary;
- reference ROM source commit, ROM filename/hash, and whether it is Red, Blue, or DEBUG;
- emulator/renderer, output resolution, capture FPS, and input method;
- save/snapshot identifiers and the exact map/coordinates used.

Build the current debug binary with the repository's documented command. If a dependency or cache prevents a rebuild, do not patch unrelated caches just to make the audit pass: use an existing binary only after recording its path, timestamp, and commit relationship, and mark the limitation in the report.

### 2. Prepare deterministic scenarios

Use a save editor or a controlled debug snapshot for the current game when setup through ordinary play would introduce timing noise. For reference ROMs, use a deterministic test/debug entry if available. The scenario must specify:

- starting map, coordinates, facing, party/moves/items/badges;
- input sequence up to the trigger;
- the semantic trigger (for example, “confirm Viridian on Town Map”);
- the expected state transition and stopping condition.

Use the standard cases in [`references/scene-matrix.md`](references/scene-matrix.md) as the starting matrix. Add a new case there when a scenario will be reused.

### 3. Record both implementations

For the current game, the usual headless shape is:

```bash
target/debug/pokered-app run \
  --headless --no-audio --debug-port 23456 \
  --snapshot "$SNAPSHOT" \
  --record-frames "$OUT/frames" \
  --record-video "$OUT/clip.mp4"
```

Drive it through the debug server using the commands from `pokered-debug` (`get_state`, `press_sequence`, `step_frames`, `warp`, and the relevant battle/party helpers). Capture a pre-trigger frame, the trigger state, the full animation, and enough post-animation frames to prove the final state. When using a reference emulator such as PyBoy, apply the same semantic input trace and save a machine-readable manifest alongside the frames.

For any intended `PASS`, prove that each PNG corresponds to one consecutive emulated frame. The manifest must map image number → emulator/debug frame and include the trigger, phase changes, movement state/counter when observable, and first stable final frame. If the debug loop advances between observations or the mapping is unknown, timing is unverified and the maximum verdict is `PARTIAL`.

If the recorder and debug loop run at different cadences, use the emulator/debug frame counter or state transition as the anchor. Do not infer timing from a contact-sheet label alone. Assemble a video only after confirming the input frame numbering; inspect the input filename pattern (`%04d` vs `%06d`) before invoking `ffmpeg`.

### 4. Align and judge the frames

Read [`references/quantitative-alignment.md`](references/quantitative-alignment.md) before judging a capture. First trim both recordings to the same semantic window without stretching, dropping, duplicating, or resampling frames. Contact sheets may only show identical elapsed-frame offsets (`t+N`); phase-normalized views are supplemental and cannot support a verdict.

For each scenario, align on all applicable anchors:

- trigger input;
- first visible animation frame;
- first map/screen transition;
- first stable final state.

Evaluate independently:

1. visibility and ordering of animation phases;
2. sprite/OAM geometry, trajectory, shadow, and screen coordinates;
3. background/camera trajectory, including per-frame scroll and landing discontinuities;
4. phase duration and cadence in emulator frames;
5. state transitions, lock/input behavior, and final map/transport/battle state;
6. confounders that make an apparent pixel difference non-actionable.

Run the bundled sequence analyzer on at least one stable background ROI for every moving-camera scene:

```bash
python3 .agents/skills/key-animation-differential/scripts/compare_sequences.py \
  --reference-dir "$REF_FRAMES" --reference-range "$REF_START:$REF_END" \
  --current-dir "$CUR_FRAMES" --current-range "$CUR_START:$CUR_END" \
  --roi X,Y,WIDTH,HEIGHT --max-dx N --max-dy N \
  --output "$OUT/metrics.json" --diagnostic-image "$OUT/raw-time.png"
```

The ROI must contain stable map landmarks and exclude the player, UI, water, flowers, and other animated tiles. Use a separate sprite/OAM measurement or per-frame state trace for the actor path; background motion cannot substitute for actor motion.

Use these verdicts: `PASS` (all mandatory raw-time, phase, actor, background, and final-state checks match), `PARTIAL` (a phase exists but one or more required quantitative channels are unavailable), `FAIL` (a phase, duration, trajectory, cadence, or transition is wrong), and `BLOCKED` (the pair could not be reproduced). A missing departure phase is a failure even if the arrival phase passes. Matching the original source table is corroboration only; the composed rendered motion remains the oracle.

Repeat deterministic captures at least twice before a final `PASS`. A mismatch reproduced twice is a finding; inconsistent runs indicate capture instability and must be fixed or reported before judging the animation.

### 5. Persist compact evidence and report

Use the repository convention:

```text
docs/audits/YYYY-MM-DD/key-animation-differential.md
docs/screenshots/visual-key-animations/
```

Keep one or more of the following per scenario:

- a raw-time original/current diagnostic sheet using the same `t+N` columns;
- a labeled overview contact sheet (presentation only, not verdict evidence);
- a short original/current MP4;
- a machine-readable manifest with trigger frame, coordinates, state, and hashes;
- a source cross-check naming the current implementation and original routine.

Follow [`references/report-template.md`](references/report-template.md). The report must state the conclusion first, include the exact scenario and ROM/build variants, link the evidence, and distinguish observed frame evidence from source-level inference.

If the audit leads to a visual code change, follow the repository's before/after screenshot policy and capture the same screen/frame on the base and changed revisions. Do not silently turn an analysis request into an implementation request.
