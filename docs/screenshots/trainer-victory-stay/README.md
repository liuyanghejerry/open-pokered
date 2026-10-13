# Ordinary trainer victory: STAY/NONE

Base: master `5454eacb3081946adfc1a582884883ec4428eef6`.
Production/test fix: `ae6aa52ff4953083e7f4c9639d573c4912d3e688`.
Engine: v0.10.1, pinned `74e831c00467f028914f9ef6730a22e605ae2e04`.

Original source `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`:
`home/trainers.asm: PrintEndBattleText` calls
`engine/overworld/auto_movement.asm: SetEnemyTrainerToStayAndFaceAnyDirection`.
That routine skips all three rival classes and PokemonTower7F, then calls
`home/map_objects.asm: SetSpriteMovementBytesToFF` to write STAY/NONE.
The source map reload after battle preserves NPC sprite data.
This original policy is source-verified; a matched complete original battle
movie and original/native pixel/RNG parity are not claimed.

Both screenshots are absolute hardware frame **1616** (`frame-001615.png`).
Both native runs start at Mansion2F (3,18), give level100 Charizard through
an ordinary debug party command, wait60 frames, verify the trainer is idle,
and use controller-driven `interact_with npc:0` to approach/talk. Dialogue
and battle use the same real button sequence, followed by400 neutral frames.
No defeated flags, NPC positions, movement modes or RNG bytes are injected.
Seed0; original ASCII dialogue; audio disabled. Baseline was freshly built
from the named master. Two before and two after captures repeat exactly,
including every PNG and the frame manifest (counts in verification.json).
Their complete engagement records and battle-return frame1337 match.
Frame1358 before the baseline's autonomous restart is byte-identical.

Before: the defeated trainer walks from (4,17) to (1,17).
After: he stays (4,17), walk counter0, through all400 observed frames, while
turning from Up to Left. At screenshot frame1616 both face Left; only the
baseline has walked away. The original's pending stride fields are preserved
by the fix; the regression matrix covers wins, a loss, rivals and Tower7F.

Validation: all40 core suites / 3,241 tests and41 app test executables pass.
The broader mid-walk conversation destination issue is tracked separately;
this capture explicitly starts its conversation with an idle trainer.
