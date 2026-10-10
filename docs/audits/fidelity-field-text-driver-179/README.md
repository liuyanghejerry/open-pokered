# Field text input ownership / Bill regression (179)

The actual new-game run 176 stopped at m14 after Bill's cell separator: `PrintFieldParagraph` was waiting for A while the legacy `dialogue_state` was null. The playthrough driver therefore supplied only neutral frames; `skip_dialogue` also falsely reported closure after advancing zero frames. This is a debug/driver input ownership defect, not proof of a player runtime softlock.

Both debug completion and driver acknowledgement now recognize inner field text effects. Existing choice ownership remains explicit: skipping stops when a choice opens and never selects its answer. Normal player input, scripts, text rendering and timing are unchanged. No on-screen change is introduced, so no new visual captures are required.

A real Game fixture reaches Bill's authored paragraph with held A, invokes the actual debug command, then verifies one SS TICKET, the ticket flag and return of player control. The before command advances 0 frames; the fixed first call advances 298 to GiveItem; subsequent idle/skip completes the receipt. This is a controlled fixture, not a claim of complete fresh-game validation.

Validation: 72 Python navigation/recovery cases; full debug native suites (core 2750, data 263, app 229 with 34 ignored, agent 54, audio 99). Final comment-only source cleanup was rebuilt; both Bill and the existing gift-question skip tests pass on the frozen final binary. Source/binary hashes and raw before/after responses are in verification.json and evidence.zip. The initial test compile typo and failed diagnostic logs are retained.

Fresh-game m01..m49 and independent SAVE/CONTINUE remain open. Original GBA budgets also remain open: outlining PC key construction did not eliminate the two failures and was not adopted. Other open audit gates are documented in the preceding batches; this change does not establish final game fidelity.
