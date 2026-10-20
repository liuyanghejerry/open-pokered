# Slot-machine entry

Base: `7624055a37b9fcd734932bcda116ce96b426f19a`. Tested candidate game source:
`642668cb329bcc2537d233046a13fe85db6e1264`.

The old machine handler entered slots unconditionally. It also passed an omitted
`openSlots()` argument as explicit false, bypassing the per-visit lucky sign.
The map contained four same-name storylines; the loaders retain the last one.
This change keeps one triggered handler, checks COIN CASE before coin balance,
then asks YES/NO. Only YES opens slots. An omitted lucky argument remains null
until the actual triggering sign is resolved; explicit true/false still override.

Original source: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`StartSlotMachine` / `AbleToPlaySlotsCheck`, and
`engine/slots/slot_machine.asm` `PromptUserToPlaySlots`.
`data/text/text_2.asm` `_PlaySlotMachineText` ends in DONE, which returns straight
to YesNoChoice; it does not require a separate dialogue acknowledgement.
The implementation reuses the existing original field question and menu flow.

The guard/consent matrix covers borrowed and owned scene loading, both bag
states, zero/one/many coins, and YES/NO. A real-map-AST integration test follows
the question without A/B until the menu accepts input, retains the triggering
sign through the fifteen-frame restoration, and dispatches matching/nonmatching/
absent lucky signs. An argument contract test covers omitted/true/false.
Two intended unit failures are retained against the old guards/argument behavior;
a separate intended failure catches the intermediate extra-acknowledgement bug.
Full core: 3,158 passed. Shared app: default 380 passed / 38 existing ignores;
debug-server 391 passed / 40 existing ignores. No new ignores or relaxed tests.

The native replay starts with an offline-constructed fixture, boots normally,
and chooses CONTINUE. It uses actual buttons and read-only observation from
there, with no live mutation, warp, skip-intro or mid-run restore.
Three base cases and eight candidate cases each ran twice. Repeated observations
and PNGs are identical after ignoring capture destination paths. The three image
pairs use identical command tapes through capture and frame 803:

- No case, 99 coins: directly enters slots before; case error after.
- Case, zero coins: directly enters slots before; no-coins error after.
- Case, 99 coins: directly enters slots before; original question/menu after.

Candidate replay also checks case-error precedence with no case/zero coins,
NO, B, YES, one coin and 9,999 coins. Error/NO/B returns restore actual movement;
inventory and coins are unchanged. Holding confirmation A across the screen
handoff does not place a bet. A fresh A then bets three coins. One coin permits
entry and B exit without spending it. All six images were visually inspected.

```sh
# Run from the workspace root, supplying a debug-server native executable.
python3 docs/validation/slots-entry-flow/replay.py after /path/to/pokered-app /path/to/output
```

Compressed records preserve complete command/state evidence and executable
hashes. `verification.json` records source identities and scope. The workspace
HEAD in a record is context, not independent proof of executable origin.

This is native fixture evidence and original assembly comparison. It does not
verify original-ROM entry emotion/fades/audio timing, the whole minigame, or
whole-game equivalence. Loss/insufficient-bet acknowledgement and OneMoreGo
selection remain separate work.
