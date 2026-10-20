# Bill's cell separator

Base: literal master `de704e839d3b2742c66202048806ea3a84a70cd9`.
Tested game source: `5bd4cb457c088c6a84b56886ad01e686462b739b`.
Executable identities, compressed observations and test logs are alongside this
file. Later documentation commits do not change the tested production sources.

Original reference: pret/pokered
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`:

- `engine/events/hidden_events/bills_house_pc.asm`, BillsHousePC and
  BillsHouseInitiatedText: UP-only eligibility; returned-house priority;
  initiated text with button acknowledgement; stop music; wait 16; SWITCH and
  sound completion; wait 60; close the retained text window; wait 32; TINK;
  wait 80; SHRINK; wait 48; TINK; wait 32; GET_ITEM_1. Each cue finishes before
  continuing. Restore map music, then set USED.
- `scripts/BillsHouse.asm`, BillsHouseBillExitsMachineScript and cleanup:
  show human Bill at actual map (1,2), wait eight frames, walk DOWN, RIGHT,
  RIGHT, RIGHT, DOWN to (4,4), then set MET_BILL_2 and MET_BILL.
- `data/text/text_2.asm`, _BillsHouseInitiatedText: player-name substitution
  and the original initiated / TELEPORTER's Cell / Separator text.

The previous script used invented three-page running/back-to-normal dialogue,
omitted the machine's audio and waits, delayed before showing Bill, and also
started the event when facing LEFT from the side. This change keeps the map's
existing PC binding and restores the script above. The favorite-Pokemon list
and initial help conversation are separate audit surfaces.

Validation:

- Both AST loaders check all four facings and eight combinations of the three
  eligibility flags. The command-order test also checks flags at each yield:
  USED follows the final sound/music, and MET follows the walk. These two new
  tests fail on the old script.
- Ordinary preboot SRAM fixtures, real title/CONTINUE, direction buttons and A
  run through PokemonGame with its muted PCM backend. All five actual sound
  sequencers own active channels and produce nonzero PCM (including both
  TINKs); music stops and CITIES2 resumes before USED. Bill appears before
  the eight-frame delay, walks to (4,4), and the other forms remain hidden.
  A physical LEFT approach is silent. No runtime setters or script injection.
- Core library: 3,166 pass; app library debug-server: 396 pass / 40 existing ignores;
  app library default: 385 pass / 38 existing ignores; agent: 61 pass. No new ignores.
  The complete `cargo test -p pokered-core` subsequently passes 3,579 tests
  across 41 targets (one existing ignored documentation example).
  The existing full NPC subplot still verifies its flags and positions; its
  separator phase now uses actual text input and allows the restored waits.
- Two CLI cases, UP and LEFT, run twice. They boot offline JSON fixtures via
  normal CONTINUE and use only actual buttons/read-only observations. The
  exact baseline command prefix is replayed through frame 804, yielding the
  paired screenshots. All recorded results and all five PNGs repeat exactly
  after normalizing capture destination paths. UP checks actual cue IDs
  72,59,71,59,57, exactly eight visible stationary frames, blocked START/RIGHT
  during the sequence, final flags/position, unchanged bag and real walking
  after control returns. LEFT remains silent, leaves flags unset and permits
  walking. Both before cases demonstrate the previous incorrect dialogue.

```sh
# Run from the repository root, using a debug-server native binary.
python3 docs/validation/bill-cell-separator/replay.py \
  /path/to/pokered-app /path/to/output docs/validation/bill-cell-separator
```

This is native fixture verification against original source, not a fresh
mainline playthrough or original-ROM runtime timing proof. CLI observations
show sound activity; PCM and music restoration are checked by the cold SRAM
app tests. The first repeat invocation from an unrelated working directory
failed at import before launching a game; the two archived runs use the
workspace root and both complete successfully.

The first CI run exposed three old `tests/pc_trigger.rs` fixtures that set the
player direction but did not seed synchronous script queries. Production seeds
these before input. The new UP guard correctly saw an empty query in those
fixtures. The same three failures were reproduced locally, then the fixtures
were corrected to seed queries without changing their behavioral assertions.
The complete core command above covers integration targets omitted by the
initial library-only run. These test-only corrections do not change the
executable or invalidate the native frame pairs.
