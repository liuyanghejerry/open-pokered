# Bill's Pokémon list: text, input, cry and return flow

Bill's PC previously printed both introduction strings as ordinary blocking
field dialogue. The question needed an extra ACK before the list appeared;
returning from an entry lost the question. Entries also skipped the cry and
accepted input during what should have been the original display/return waits.

`billPokemonList` now owns the complete original caller. It retains the field
Joypad ledger through favorite/PROMPT, question/DONE, direct HandleMenuInput,
ShowPokedexData, ReloadMapData, the repeated question and outer A-release close.
It does not grant ownership; #341's actual saved owned bit still determines
whether height, weight and description are available.

## Original source

References are pinned to pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`:

- [BillsHousePokemonList](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/events/hidden_events/bills_house_pc.asm): favorite PrintText, question in the loop, NO_TEXT_DELAY, five clamped choices, B before A, retained cursor and DoNotWait outer close.
- [Text terminators](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/data/text/text_2.asm): favorite ends PROMPT; question ends DONE.
- [HandleMenuInput](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/menu.asm): three VBlanks, direction before acceptance, no YesNoChoice fifteen-frame completion wait.
- [ShowPokedexData](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/menus/pokedex.asm): external/internal white Delay3, cry before owned data, silent final wait and volume restore.
- [PlayCry](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/pokemon.asm) waits for actual sound completion. [Audio1](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/audio/engine_1.asm) temporarily restores cry volume.
- [PAGE](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/text.asm) protects its prompt, plays the scroll sound, clears the description and waits twenty VBlanks.
- [DisplayPokedex](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/events/display_pokedex.asm) returns to the map for ten VBlanks before marking seen and restoring the caller.

## Verification

- Core full suite: **3,585 passed**, one existing ignored doc test.
- App full default suite: **886 passed**, 88 existing ignored tests (includes both library and binary targets).
- Protocol validation test passes. The cold SRAM test covers owned, unseen and
  seen-only Eevee using actual muted APU/PCM progression. It observes the real
  cry before data and asserts NR50 `$77` during cry, `$33` for data, `$77` on exit.
- TUI test targets compile (`--no-run`); no claim of TUI runtime playtesting.
- Native baseline is actual merged master #341. The existing canonical binary
  was built from `cbdc234`; `git diff cbdc234 a79508cc -- crates` is empty.
- Nine offline preboot fixtures: four species each owned/unseen, plus seen-only
  Eevee. Each boots normally and uses CONTINUE, atomic physical-button timelines
  and read-only observations. No runtime setters, warps, dialogue skips or
  restored game states. Fixture files and owned bits remain unchanged; only
  viewing the selected entry sets its seen bit.
- All nine cases pass twice on `b594bf52`. All **27 candidate PNGs** and normalized
  records are identical between runs. The per-frame Eevee trace observes six
  white frames and actual cry SFX 45 playing before data. It also checks that
  seen remains unchanged through the ten map-return frames and changes after
  the tenth frame.
- The question screenshots share the **same advancing input prefix and frame
  1105**. Later entry navigation deliberately omits the obsolete extra question
  ACK on the candidate; do not describe the entire tapes as identical. Final
  entry captures are at the same frame and all nine entry PNGs match baseline
  byte-for-byte, including owned descriptions and unowned placeholders.
- A separate actual-input route starts from one pre-help fixture, helps Bill,
  operates the separator, receives the ticket, leaves/re-enters the house,
  observes pointing Bill, views unowned Eevee, exits the PC and walks again.
  All six Bill event flags arise from the actual story actions.

These are source comparison and fixture-assisted native checks. They are not
original-ROM runtime comparison or a fresh mainline playthrough. Global
scan/FLASH work and latest-master fresh NEW GAME → credits → independent
CONTINUE verification remain outside this PR.

## Catalog-order CI follow-up

CI caught the new capability outside alphabetical order: the focused protocol
run had omitted the pre-existing catalog-order test. The entry is now correctly
positioned after `bikeShopOffer`. The capability set and game logic are unchanged.
On the corrected source recorded in `catalog-followup.json`, the **full data
suite passes 448 tests across 17 targets**, the three-case cold SRAM/real-cry/
volume test passes, and a freshly rebuilt native binary passes all nine cases.
All 27 images and normalized case records match the preceding candidate runs.
`after-catalog.json.gz` and the follow-up logs preserve these exact-source results.
The full core/app suites and integrated subplot above belong to `b594bf52`;
they are not relabeled as full-suite reruns on the catalog-order follow-up.

## Event-analyzer CI follow-up

The workspace coverage job also caught the new capability missing from the
quest semantic extractor. It is now explicitly recognized as presentation
outside quest-event effects, matching the prior `pokemonMenu` classification.
The strict unknown-construct gate and its allowance list are unchanged.

Two added tests fail before registration, then pass with it: bare/`game.` calls
in both statement/expression form, and the actual BillsHouse PC/ticket branch
reads and effects. The full agent suite passes **63 tests**. On the corrected
source in `agent-followup.json`, `cargo test --workspace --tests` passes **5,712
tests with 95 existing ignores across 126 targets**. This is the coverage job's
workspace test scope without LLVM instrumentation; it is not a claim that the
remote coverage job has passed.

A freshly built debug-server game on that source also boots normally, CONTINUEs,
queries actual BillsHouse semantics and views/exits unowned Eevee with actual
input. The response retains all three PC flag reads and the separator effect,
with no unknown effect; the read-only query leaves frame 804 unchanged. Its
question, entry and returned-menu PNGs match the prior candidate byte-for-byte.
This is one fresh native API case, not a relabeling of the prior nine-case runs.
`agent-api.json.gz`, its replay and logs pin the exact scope and build.

## Text-preservation check

All **20 ordered bilingual `@t` literal pairs** exactly match master `a79508cc`,
including the two strings moved from `@speaker` blocks to typed-call arguments.
Run `python3 docs/validation/bill-pc-list-flow/verify-texts.py` to reproduce this.
The older `verify_scene_translations.py --base a79508cc` compares only speaker
blocks and reports 9 → 7 blocks; its structural mismatch output is retained in
`speaker-check.log.gz`. It is not reported as a passing check. No English or
Chinese wording or layout was changed.

## Replay

From the source checkout root, with its matching debug-server native binary:

```sh
python3 docs/validation/bill-pc-list-flow/replay-after.py \
  /path/to/pokered-app /path/to/output \
  docs/validation/bill-pokedex-ownership/fixtures after
python3 docs/validation/bill-pc-list-flow/subplot-replay.py \
  /path/to/pokered-app /path/to/output-subplot
```

Use `replay-before.py` on master `a79508cc` with final argument `before`.
All outputs must use a new output directory. `*.json.gz` stores complete commands,
observations and audio/return traces. `verification.json` pins source/binary
identities and scope; `repeatability.json` records capture hashes. Logs are
compressed to keep the evidence small.
