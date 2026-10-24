# Ordinary secondary effects after target KO

Original `core.asm` checks the defender's remaining HP after
`AlwaysHappenSideEffects` and returns before ordinary secondary dispatch when
it is zero. Both the player and enemy paths have this guard. Production still
rolled secondary chances and could poison, burn, freeze, paralyze, confuse or
lower a stat on an already-fainted target.

The chance-bearing data bridge and the separately registered foe-stat-down
bridge now return before any secondary random draw on target KO. This includes
the ordinary enemy's extra stat-drop miss roll. Recoil and self-KO still run.
Twineedle's nested final-hit rider and multi-hit KO termination remain separate
audit candidates, outside this change.

Prepared real BattleScreen turns cover seven effect families on both sides,
surviving/fainted targets and chance bytes 0/255 (56 cases), plus four recoil /
self-KO controls and four ordinary-mode stat-drop KO controls. The original
60-case reproduction showed 28 excess RNG consumptions and 12 effects applied
after KO. All 64 final cases and the full 3,691-test core suite pass, with one
existing ignored test.

The initial implementation missed the stat-drop bridge's direct registration;
the full suite exposed that and an older test explicitly expecting a fainted
defender to freeze. The final guards cover both bridges. That test now asserts
the original HP-zero early return and three consumed bytes; the legacy test
oracle is not used as authority for this behavior.

Product source `57f9a3c69e88afe27b873da2e97dedaf2ac1171d` passes all seven original
GBA performance scenarios and 13 memory scenarios, including 330 move animations.
Minimum contiguous heap is 7,816 bytes and untouched stack is 8,708 bytes. No
gate, workload or timeout changed. Source and log identities are recorded in
`verification.json`. The canonical target is reused without another ROM copy,
and the completed memory log is compressed losslessly.

This is prepared-state production-path evidence, not an original-ROM recording
or a complete NEW GAME run. Pure game logic is exempt from screenshots under
AGENTS.md. All final remote workflows and six actual GBA runtime gates must pass
before merge.
