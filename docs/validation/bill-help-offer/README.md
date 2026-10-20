# Bill's help question

Base: literal master `abdb0ed7207a3ec8dc679fea24496fbfbc9414c8`.
Tested candidate game source: `92940b763e3d8169c30314bfabae4701e8fc74aa`.
The executable hashes and compressed records/logs are alongside this file.
Subsequent documentation commits do not change the tested production sources.

Original reference: pret/pokered
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`scripts/BillsHouse.asm` BillsHouseBillPokemonText and `text/BillsHouse.asm`:
the first question ends in DONE, returns from PrintText and immediately enters
YesNoChoice. The old generic speaker/choice sequence required another button
press after the question. The existing source-owned `askFieldYesNo` restores
that handoff and its menu input waits. English and Chinese strings are unchanged.

NO still prints the refusal, which ends in PROMPT, before the common teleporter
instructions. Those instructions keep their outer acknowledgement through
DisplayTextID (`home/text_script.asm`): a silent button wait followed by
HoldTextDisplayOpen. Held A keeps the completed text open until released; B
closes it. Bill then follows the existing path into the machine, hides, and
latches the separator request. This change does not alter the machine sequence,
ticket reward, return-PC list, or map coordinates.

The new AST regression fails on the old help handler. It covers borrowed and
owned loaders, YES/NO branching, the refusal's inner prompt, instructions and
the final walk/hide/request. Generic ShowChoice confirmation audio, B-over-A
and unowned window lifecycle tests now use a small explicit native DSL fixture:
Bill no longer exercises that generic path. Their assertions remain intact.
The existing complete NPC subplot now reads the help dialogue with actual
input, waits for an enabled choice and retains its flag/position assertions.

Complete local core: 3,580 pass / one existing ignored documentation example
across 41 targets. Complete default app: 884 pass / 88 existing ignores across
42 targets (including repeated library/binary suites). Agent: 61 pass. The
generated event graph is unchanged. No new ignores or production engine APIs.

Three native CLI cases run twice from an offline JSON fixture through normal
boot/CONTINUE, using actual buttons and read-only observations only:

- YES and NO: the question automatically opens the menu; acceptance at frame
  2539 keeps the menu through 2553 and returns at 2554, after fifteen frames.
  The confirmation plays sound ID 63. NO still displays its refusal.
- Held outer A: the final instructions stay open for twenty held frames;
  the request flag remains unset. Releasing A lets the instructions close
  and Bill walk into the machine.
- Each case checks the silent outer acknowledgement, Bill hidden at (6,2),
  only the request flag set, unchanged inventory/fixture and real walking
  afterwards. All recorded results and all eleven PNGs repeat identically
  after normalizing capture destination paths.

The paired base/candidate screenshots use exactly the same frame-advancing
commands and frame 2538. After the common boot/interaction prefix, each of six
intermediate pages receives 240 neutral frames, one A frame and eight neutral
frames, then the final page receives 240 neutral frames and no acknowledgement.
This allows printing and protected prompt waits to finish in both executables.
An earlier comparison reused the legacy adaptive tape: its early presses were
ignored by the restored waits and left the candidate on page five at frame
1914. That run is excluded; the final paired tape was rerun on literal master
and candidate without weakening page or frame assertions.

```sh
# Workspace root; native debug-server executable. Mode defaults to after.
python3 docs/validation/bill-help-offer/replay.py \
  /path/to/pokered-app /path/to/output docs/validation/bill-help-offer after
# The same driver accepts before when using the base executable.
```

These are native fixture checks against original source, not a fresh mainline
playthrough or original-ROM runtime timing proof. The RPC accepts one key per
frame; simultaneous-key priority remains covered by the existing FieldYesNo
unit suite, rather than claimed as a CLI case. Return-PC prompt/entry behavior
and broader fidelity work remain separate audit items.
