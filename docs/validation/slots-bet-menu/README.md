# Slot-machine bet menu

Base: `c05ccb0f5397e94e86673262e89f1eb3dafd8f07`. Tested game source:
`73250644bfd7adc4b615601babea715fb0496ef1`.

The original starts the bet menu on the top row, x3, every time. Returning YES
from OneMoreGo jumps to MainSlotMachineLoop, which clears the cursor again.
HandleMenuInput checks Up before Down and ignores horizontal keys for this menu.
The old implementation started at x1, remembered the last bet across games,
and treated Left/Right as bet controls. Simultaneous Up+Down ran both branches.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/slots/slot_machine.asm` MainSlotMachineLoop / CoinMultiplierSlotMachineText,
and `home/window.asm` HandleMenuInput. The fix changes the pure screen state only.

Two new regressions fail on the old code and pass on the candidate. 73 related
core tests and four slot-renderer tests pass, including cached cursor redraws.
After integrating PR #334, all 3,155 core tests and the four renderer tests pass.
The new native before/after recordings match every earlier paired PNG.
The intended failure logs are from d037; PR #334 does not change the bet source.
The shared runtime lifecycle now asserts the original x3 default and 100 -> 97
coin persistence. Full app tests pass in both configurations: default 380 passed /
38 existing ignores; debug-server 391 passed / 40 existing ignores.
The payout test now explicitly selects x1; it keeps its original one-payline
payout assertions rather than depending on the former incorrect default.

Actual CLI checks run from the compressed offline native fixture through normal
boot and CONTINUE, then buttons only. Three cases, each side repeated twice:

- Default entry: x1/debit 1 before; x3/debit 3 after.
- Select x2 then Right: x3/debit 3 before; x2/debit 2 after.
- Select x2, complete a losing spin, answer YES to OneMoreGo: x2 before; x3 after.
  Both sides retain 97 coins and have byte-identical OneMoreGo screenshots.

All before/after command tapes match after normalizing output paths; every
comparison uses the same frame number. Each side's two runs have identical
observations and PNGs. The screenshot observer counts the existing Fusion Pixel
cursor in its three known menu rows, and the images were also inspected.

```sh
# Run from workspace root. Needs Pillow and a debug-server native executable.
python3 docs/validation/slots-bet-menu/replay.py after /path/to/pokered-app /path/to/new-output
```

The four compressed JSON records retain commands and observations. Test logs
are losslessly compressed; `unit-before.log.gz` contains the intended failures.
`verification.json` records source/executable/fixture hashes and repeated results.

This does not verify original-ROM runtime timing, audio, or every minigame path.
Missing entry gates/confirmation, OneMoreGo NO selection, insufficient-bet text
acknowledgment, and the native AST lucky-machine command require separate work.
No whole-minigame or whole-game equivalence is claimed.
