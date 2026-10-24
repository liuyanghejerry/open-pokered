# Mist and damaging stat drops

Original `MoveHitTest` applies Mist only to the primary stat-down effect ranges.
The damaging secondary path in `StatModifierDownEffect` still rolls its chance
and lowers the stat. Production instead used the same unconditional Mist veto
for primary and damaging moves.

Keep the nested veto for primary moves, and remove only Mist's handler when
resolving a secondary stat drop. Other protection, including Substitute, stays
in force. An older stack test incorrectly expected Mist to block Aurora Beam;
its assertion now follows the original source and independent turn-caller test.

Prepared production turns cover Acid, Aurora Beam, Bubble, Bubblebeam, Constrict
and Psychic on both sides, chance bytes 0/84/85/255, and absent Mist / Mist /
Mist with Substitute (144 cases). Growl, Leer and Screech retain their primary
protection (12 controls). Before the fix, 24 qualifying secondary drops were
incorrectly blocked. All 3,685 core tests now pass, with one existing ignored
test. The initial test-only `Psychic` enum typo was corrected before the retained
runtime red reproduction.

This is pure game logic, exempt from screenshots under AGENTS.md. The tests use
prepared link battle states, not a fresh playthrough or an original-ROM recording.
The separate question of primary Mist's early-return RNG order remains unaudited.
GBA and final remote CI validation are pending. See `verification.json` for hashes.
