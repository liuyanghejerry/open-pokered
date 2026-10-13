# NPC conversation pixel alignment

Base: master `ea0d93de54c3df1e9c93b650b323e38e135cdda3`.
Production/test fix: `81225e62eaeb163242c7a73897a0d277d65fc97d`.
Engine: v0.10.1, pinned `74e831c00467f028914f9ef6730a22e605ae2e04`.

Original source `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`home/overworld.asm: IsSpriteOrSignInFrontOfPlayer / IsSpriteInFrontOfPlayer2`,
compares exact sprite pixel coordinates, at distance16 or distance32 when
facing a counter. The counter selects the extended target directly.
The native engine retains the origin tile during a stride. Accepting that
tile while counter1..15 permits dialogue with a sprite between tiles.
Counter16 remains aligned before its first pixel; counter0 is idle.

`original-probe.json` contains204 controlled original-ROM cases, each
repeated twice: four directions,17 counters,normal16/extended32 targets,
and a16-pixel object excluded by the32-pixel counter target. Pixel geometry,
NPC counter/player facing and the distance register are seeded at the
original routine entry. These are original policy probes driven by real A
input from a natural Center emulator save, not natural walking movies or
claims of original/native hardware/OAM/RNG parity. ROM SHA1 is recorded.

Both PNGs are native absolute hardware frame **40** (`frame-000039.png`).
Start at Mansion2F (2,17), seed0, give level100 Charizard through the debug
party command, neutral12, physicalRight3, physicalA3, neutral22. No NPC
positions, movement state or defeated flags are injected. At pretrigger
frame15 the NPC is (3,17), movingRight,counter9; state and PNG are identical.
Before opens the trainer dialogue and changes facingLeft; after has no
interaction and completes the original Right step to (4,17).

Two before and two after runs repeat all40 PNGs and their frame manifest
exactly. Both branches were freshly built after #217 merged; all41 artifacts
per run and states also match the older pre-merge captures byte-for-byte.
The regression matrix preserves the aligned initial counter16 and counter
extension, and ignores hidden NPCs. The empty-dialogue test keeps all its
assertions while placing the clipboard on a reachable floor instead of
teleporting the player onto an impassable counter; production map data is
unchanged.

Validation: all40 core suites / 3,244 tests and41 app test executables pass.
The separate facing-versus-step-vector issue at an aligned stride start
remains under audit. This change does not claim to resolve it.
