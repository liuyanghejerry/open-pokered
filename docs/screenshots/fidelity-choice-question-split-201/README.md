# Retain questions during choices: standalone split 201

Directly based on master506e19b. Seven production files cover completed field
text display ownership, direct/nested choice detection, rendering/cache gates,
debug/FrameRecorder display observation and backwards-compatible snapshot state. Incoming
movement, NPC sprite/font gates and field-clock dependencies were excluded.
The thin update wrapper only clears retained state at conversation completion.

Original Safari information worker calls PrintText then YesNoChoice. The latter
saves/restores its own menu tiles, leaving the question in the lower window.
The repair keeps the final text page through that choice, without allowing the
retained display text to consume A/B as an active dialogue. New dialogue,
answered/cancelled choice and completed scripts clear it. CUT's existing retained
text is still handled independently. Legacy JSON defaults the new field to None;
missing historical question content cannot be recreated from an older snapshot.

Actual Safari YES/A and NO/B branches verify preserved question, pre-answer JSON
restore, replacement text, final cleanup and every-frame cached/full pixel equality.
Museum verifies its multipage question survives the money-box command. Final
native suites pass core2707/app192 (15app intentionally ignored). Core sources
and frozen core binary remain unchanged from a1; final appa2 reruns its full
suite/captures after adding the matching FrameRecorder observation. GBA must pass
this independent PR's exact-head build/performance CI without budget changes.

Screenshot baseline is actual master506e19b with identical capture helper appended
and production prefix verified unchanged. Both sides load the original controlled
SRAM through real CONTINUE, explicitly end that fixture's Safari game and warp to
information worker position(3,4), reseed field RNG1 after load, faceLeft, then use
A at t0/80 and release at2/82. Frame0121/t120 has the same YES/NO choice and selected0:
master has no question; repair displays “Hi! Is it your first time / here?”. Each
side records152 frames and repeats twice. Every PNG, full raw frames.json and
visible observations.json matches byte-for-byte within each side, without masks.
All608PNG, all raw JSON, logs, source/binary hashes and differences reports retained.
Observed repeat equality does not prove globally seeded RNG or original CPU timing.

Historical157 original recordings/probe and primary assembly establish source
behavior; original footage uses a different interaction timeline, not an absolute
same-frame original/native comparison. Older broad-branch Native/GBA claims excluded.
This controlled interaction is not paid admission, a last-ball or fresh mainline
proof. General PrintText initialization/closing timing, NPC presentation and further
story callers remain separate audits. No ROM or executable is published.
