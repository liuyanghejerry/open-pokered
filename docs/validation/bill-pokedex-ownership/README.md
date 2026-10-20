# Ownership in Bill's PC entries

Base: literal master `d8427ea8393488f2ba9b27ba2728b589745bc4eb`.
Tested candidate game source: `cbdc234cbbb2024b26ef3f7171fd146a806c8873`.
Hashes, compressed observations/logs, fixtures and replay scripts are included.
Later documentation commits do not change the tested production sources.

Original reference: pret/pokered
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/menus/pokedex.asm` ShowPokedexDataInternal / IsPokemonBitSet. The
original checks wPokedexOwned and skips height, weight and description when
the species has not been caught. Name, category and picture remain visible.
Bill's PC uses that same display through DisplayPokedex. The native field
overlay instead passed a constant `true` to the shared entry renderer.

The host now projects the persistent owned bit into the requested field entry
after its normal field update. The renderer uses that bit, including when a
species has been seen but not caught. The new UI field defaults to false when
deserializing older snapshots. Reading a PC entry still registers the species
as seen and does not grant ownership. Existing full-description visual
fixtures in app/TUI explicitly identify their entries as owned; their text
and expected visual content are unchanged.

Validation:

- A cold ordinary SRAM/CONTINUE test drives the actual PC with buttons. It
  compares full and cached field rendering to the existing ordinary Pokédex
  display, for owned, unseen and seen-only saves. It also checks description
  page count, only-seen registration, unchanged ownership and unchanged SRAM
  file. On the old renderer the owned positive control passes and the unowned
  pixel assertion fails. Initial test-code import/constructor compilation
  mistakes are excluded from that negative control.
- Complete core: 3,579 pass / one existing ignored documentation example;
  complete default app: 886 pass / 88 existing ignores, including repeated
  library/binary suites. These full runs tested the implementation on the
  previous master, before merging PR #340. After that merge, 16 Bill core
  tests and the cold SRAM regression pass; the later seen-only extension
  also passes all three preboot cases. All TUI test targets compile. No new
  ignores or weakened behavioral assertions. See verification.json for the
  exact validation source commits.
- Nine native cases cover all four Eevee-family species, each owned/unseen,
  plus Eevee seen-only. The same fixture and frame-advancing input precede
  each paired capture at frame 1437 + 9 × list index. Both candidate runs
  have identical records after output-path normalization and identical nine
  PNGs. All four owned control PNGs are byte-identical before/after.
- Before: all nine entries reveal the description and require two B presses
  to return. After: the five unowned entries hide the description and close
  with one B; the four owned entries retain their full data and pagination.
  Live read-only export_fixture observations check that only the selected
  seen bit is added, with no owned-bit changes. Inventory stays unchanged,
  the list cursor is retained and actual field walking works afterwards.
- An additional single-session route starts before helping Bill, then drives
  the help question, machine, ticket receipt, exit to Route25, return with
  pointing Bill visible and viewing unowned Eevee. All six Bill flags are
  set by actual story actions. It ends with the hidden-data view and working
  field control. Its first attempt assumed UP could re-enter directly from
  the outside arrival tile; the successful script steps off and back onto
  the actual warp. Movement assertions were retained, and no game movement
  code changed.

```sh
# Workspace root; native debug-server executable. Mode is before or after.
python3 docs/validation/bill-pokedex-ownership/replay.py \
  /path/to/pokered-app /path/to/output \
  docs/validation/bill-pokedex-ownership/fixtures after

# Complete help-to-return-PC route using the previously committed help fixture.
python3 docs/validation/bill-pokedex-ownership/subplot-replay.py \
  /path/to/pokered-app /path/to/subplot-output
```

These are offline-fixture/native checks against original source, not a fresh
mainline run or original-ROM runtime proof. Return-PC prompt/menu ownership,
entry presentation/audio and its return waits remain separate work. The
sampled first 96 entry frames have the confirmation sound (ID 63) but no
Eevee cry (ID 45); this change addresses data availability, not that audio
gap. Full original Bill-PC equivalence is not claimed.
