# OneMoreGo result menu

Local candidate following PR #337. Tested game source:
`d745fb2040cd36dc690e824ec9d6e2037535641d`.
The current native before record is `9d51723896488eab574426fdf6edf1b9f38c5a31`,
the tested refusal/loss candidate. Before publishing, wait for PR #337 to merge,
merge the actual master, and refresh the literal-master baseline and identities.

Original source: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/slots/slot_machine.asm` MainSlotMachineLoop / OneMoreGoSlotMachineText /
SlotMachine_PayCoinsToPlayer; `engine/menus/text_box.asm` DisplayTwoOptionMenu;
`home/window.asm` HandleMenuInput. The old result screen only accepted A=YES,
B=NO, ignored directions, preferred A over B and returned immediately.

The candidate draws a two-option menu, starts YES, clamps directions with UP
priority, moves before accepting A/B and gives B priority over A. The previous
drawn cursor remains during the fifteen-frame acceptance wait. YES resets the
bet to x3; NO returns to the live field. Accepted A/B produces SFX_PRESS_AB.
The retained framebuffer key includes the displayed result cursor.

The hosted adapter supplies all eight held buttons. Initial/unwatched-key
Delay3 and acceptance DelayFrames15 do not sample input history. A held new
press survives the initial delay; a short pulse entirely inside it is missed.
Holding the parent's acknowledgement does not answer the next question.
The original coin transfer has no Joypad polls: its last acknowledgement poll
must survive the transfer, even when raw held buttons change meanwhile.
An additional regression caught and corrected a first implementation that
incorrectly seeded this history from the final payout frame instead.

Two menu regressions fail on the old result handler; the payout-history
regression fails on the intermediate implementation. Final full core: 3,164
pass. Full app default: 383 pass / 38 existing ignores; debug-server: 394 pass /
40 existing ignores. Shared runtime checks B-over-A and the delayed exit with
coin persistence. Both language render caches are checked against a full redraw
for both cursor transitions. No new ignores or weakened assertions.

Five CLI cases run twice, from an offline fixture through normal boot and
CONTINUE, then buttons/read-only observation only: Down+A NO, B cancellation,
YES/default-three bet, early-held A, and clamped/ignored directions. All states
and PNGs repeat identically except capture destination paths. Each field exit
also verifies real walking and unchanged coins. The three paired before/after
captures use identical frame-advancing commands and frames 3204/3222/3344;
read-only observation query counts may differ.

The early-held case uses atomic per-frame input. Parent ACK is frame 3083,
menu accepts held A on frame 3086, remains visible at frame 3100 and returns
on frame 3101. Continued held A does not insert coins; a fresh A spends three.
The observed confirmation sound is ID 63 (PressAB), playing on acceptance.
An initial harness run used legacy tap(B,0), which actually advanced two frames;
that failed boundary assertion is excluded. The strict boundary was retained.

```sh
# From workspace root; uses the committed entry fixture and debug-server binary.
python3 docs/validation/slots-one-more-menu/replay.py /path/to/pokered-app /path/to/output
```

This is native fixture evidence, not original-ROM runtime or audio-waveform
proof. The CLI cases use real losing spins; winning-payout input inheritance is
covered by the core's real payout stages. Full slots foreground ownership,
entry presentation, reroll animation, payout audio and RNG audit remain separate
work. No whole-minigame or whole-game equivalence is claimed.
