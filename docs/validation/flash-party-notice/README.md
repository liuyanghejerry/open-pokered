# FLASH success text retains the party parent

Original `engine/menus/start_sub_menus.asm::StartMenu_Pokemon` saves the party
screen before opening its action menu and restores that screen before dispatch.
The `.flash` branch clears wMapPalOffset and calls PrintText before whiteout
and `.goBackToMap`. Reference: retail Red disassembly
fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c. The previously recorded ordinary retail
FLASH pair (audit1096) independently showed the party backing during this text.

The previous native runtime returned to the map immediately after selecting
FLASH, so the success message appeared over the cave. It now retains the actual
party framebuffer and drives PrintText through the shared foreground input
owner. While it prints or waits for a fresh A/B acknowledgement, neither map
frames, NPCs nor player movement tick. Confirmation resumes the existing
whiteout guard. Missing-badge refusals still return to the selected party member.
Ordinary load, soft reset and snapshot restore discard an abandoned notice.

This is one independently verified correction extracted from the larger FLASH
restoration work. The existing three-frame whiteout is retained as that code's
current behavior. Full CloseTextDisplay, loader, VBlank/IRQ, audio and equal-cycle
restoration timing are still unfinished; this proof makes no whole-FLASH parity
claim. No aggregate scene duration has been substituted for those operations.

Validation includes a native shared-Game regression that fails on master because
it transitions to Overworld before the text acknowledgement. Controls exercise
both RockTunnel1F and an already lit outside map, hold movement for600 updates,
verify the map frame counter and all actor coordinates stay frozen, acknowledge,
and verify the existing whiteout continues to block movement. Snapshot, ordinary
SRAM load and held soft-reset chord cover abandoned flow cleanup. All3129 core
library tests and10 field-move shared-Game tests pass. Core text controls cover
held A/B history and never synthesise a neutral menu predecessor.

The ordinary CLI replay uses the same32KiB persistent SRAM fixture as the earlier
retail FLASH audit: RockTunnel1F(15,3), map palette offset6, a FLASH user and the
Boulder Badge. Its SHA256 is
6513e8fa9993de3478d535536f590e47b6ea2a74fdf0116615cc2a74ca463f1c.
That fixture originated from an actual retail SAVE; palette offset and checksum
were changed offline for the dark-cave setup. No runtime state setters, debug
SAVE, skip-intro or warp are allowed. It boots through ordinary Continue and
START/Pokemon/FLASH, then captures120 frames after the action-menu confirmation.
The owner stops the game after assertions (exit-15), which is diagnostic cleanup,
not natural game completion or a fresh-route result.

Build ordinary base and candidate CLIs with desktop,debug-server, then run from
a checkout containing this proof:

```bash
python3 docs/validation/flash-party-notice/replay.py before /absolute/base/pokered-app /new/before-dir
python3 docs/validation/flash-party-notice/replay.py after /absolute/fixed/pokered-app /new/after-dir
```

The initial private replay mistakenly submitted neutral entries to press_sequence,
which rejected that setup tape and left the action menu untouched. That diagnostic
is excluded. The packaged controller uses press_timeline and checks its response.
