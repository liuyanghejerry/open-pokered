# Delayed trainer field-owner handoff (183)

Frozen parent179 NEW GAME passed m01..m21, then the m22 driver stopped at
RocketHideoutB4F (11,3): `face(up) failed: Left`. The lift-key trainer is at
(11,2). The logical destination is already reached, but the trainer intro owns
field input before its dialogue becomes observable. A two-frame UP pulse is
ignored during this interval; the old helper raises after twelve idle frames.
This is a debug-driver failure, not evidence of an unplayable player interaction.

`face` now observes a bounded sequence of pulses and returns when it sees the
requested facing or a dialogue/battle/choice owner. It sends no acknowledgement
or choice selection. A persistent ignored turn still raises. Object approach
also drains inner field text and script owners before planning again. Existing
map, NPC, battle and completion-flag assertions remain.

Two controlled original-helper/final-helper runs start with identical full
state at frame416. Original helper raises at430. Final helper observes the
ShowDialogue handoff at448, still at (11,3) facing Left, and the subsequent
approach completes the real trainer battle, verifies the trainer2 event flag,
and returns in RocketHideoutB4F. Every raw screenshot/JSON repeats byte-exactly
within each pair. The stronger continuation fixture uses a constructed
Venusaur38 party; it is subsystem evidence. The earlier Bulbasaur25 fixture also
repeats the handoff, but its continuation blacked out and is retained separately.
Neither fixture is claimed as a fresh mainline pass.

75 navigation/recovery tests pass. New tests guard delayed turn, delayed trainer
text, untouched choice, bounded failure and draining inner text before planning.
The first test-fixture BFS error and low-level blackout remain in the archive.
`verification.json` pins source/binary/archive hashes. Only Python tooling and
tests changed; normal on-screen rendering is unchanged, so no new visual PR
comparison is required. Final-source NEW GAME/independent CONTINUE, remaining
fidelity audits, and latest-head green CI remain open.
