# Slot refusal and loss acknowledgement

Depends on slot entry PR #336. Native base game source: `642668cb329bcc2537d233046a13fe85db6e1264`.
Candidate game source: `9d51723896488eab574426fdf6edf1b9f38c5a31`.
The candidate includes the entry PR's regenerated agent graph; its game-flow
changes were tested at `a150e1cf` before that metadata-only merge.

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/slots/slot_machine.asm` MainSlotMachineLoop and
SlotMachine_CheckForMatches.noMatch; `data/text/text_2.asm`
_NotEnoughCoinsSlotMachineText and _NotThisTimeText both end in PROMPT.

Previously both messages were overwritten on the next update. A loss could
reach OneMoreGo or complete the zero-coin automatic exit without the player
acknowledging the loss. The fix adds explicit refusal and loss phases. Neither
accepts directions; A/B acknowledge the current message without also accepting
a bet, answering OneMoreGo or exiting. A refused bet returns to .loop, resetting
the cursor to x3 while retaining the refusal text (the original does not
reprint BetHowMany at that jump). A loss reaches the balance check only after
acknowledgement; the existing zero-coin 60-frame exit then runs.

Two new core regressions fail before the fix. They exercise A and B, directions,
idle persistence, unchanged coins/reels, cursor reset and the zero-balance exit
boundary. A renderer regression verifies stable prompt frames in both languages
and unlit payline lamps for refused bets. The shared runtime lifecycle continues
through the loss acknowledgement instead of treating that new intermediate
phase as a completed game; its coin persistence and exit assertions remain.
Full core: 3,160 pass; app default: 381 pass / 38 existing ignores;
app debug-server: 392 pass / 40 existing ignores.

The native harness uses a compressed offline fixture from the entry validation,
normal boot/CONTINUE, read-only observation and actual buttons only. No live
setters, warp, skip-intro or mid-run restore. Four candidate cases run twice:
refused bet acknowledged by A/B, loss with remaining coins, loss with no coins.
All recorded observations and all PNGs repeat identically except destination
paths. Actual walking after leaving is also checked. The zero-coin case remains
on slots through 59 frames after acknowledgement and leaves on frame 60.

Three base/candidate screenshot pairs have identical command prefixes and frame
numbers (944 for refused bet, 3082 for funded/zero-coin losses). Baseline ran
once, candidate twice. Records and logs are losslessly compressed. The baseline
Down+A observation additionally records the still-unfixed OneMoreGo selection
problem; that behavior is not part of this change.

```sh
# Run from the workspace root; needs the entry validation fixture and debug-server executable.
python3 docs/validation/slots-message-prompts/replay.py /path/to/pokered-app /path/to/output
```

This establishes acknowledgement behavior in the native implementation, with
original assembly as the reference. It does not prove original-ROM rendering,
complete text/animation/audio timing, all minigame behavior or whole-game parity.
OneMoreGo selection and presentation/audio/RNG follow-ups remain separate work.
