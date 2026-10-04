# Save, storage and Cable Club split validation

Base: `cb131bb` (current `master`, including #110 Chinese pagination).
Source: #111 at `0e6c005`; only save/storage/NPC trade/link behavior is extracted.
Ordinary text remains Fusion Pixel. No renderer duplication refactor is included.

## Tests run on this split

- `cargo test -p pokered-core --lib --test fidelity_systems`: 2,622 library tests and 17 integration tests passed.
- `cargo test -p pokered-app --lib --test cable_club_flow`: 122 library tests and 8 integration tests passed before the final external-await guard was added.
- After that guard: `cargo test -p pokered-app --features debug-server --lib fidelity`: 12 passed; Cable Club integration: 8 passed.
- `cargo check -p pokered-tui`: passed, including the NPC party-selector rendering path.

The external-await guard regression reproduces an NPC trade suspended in the VM, then confirms its connect dialogue advances without emitting a second trade request. Storage tests cover current-box capture writeback, PC effort recalculation, daycare move/PP preservation, money limits, atomic bag additions, all ROM species IDs and original SRAM boundaries.

## Real processes, real TCP and SAVE/CONTINUE

Two split binaries ran headless in driven-only mode with debug ports 9121/9122 and link listen/connect port 9221. Each carried one level-15 Abra. They completed the TCP handshake, talked to the Cerulean receptionist, accepted the real save prompt, and wrote separate 32,768-byte `.sav` files. Both selected Trade Center, completed the room warp, requested/accepted trade through the table, selected and confirmed their Pokémon, ran the trade movies, and reached `TradeCompleted`.

A third process loaded the host `.sav` without `--skip-intro`, advanced language selection/intro/title, selected CONTINUE in the main menu and confirmed the save summary. It restored CeruleanPokecenter at (11,3) with the level-15 Abra. This validates the frontend save bridge rather than only offline SRAM round trips.

A separate process carrying only Abra completed Route2TradeHouse's NPC exchange: YES, party selection, connect dialogue, movie, then a level-15 Mr. Mime with the completion dialogue. The final external-await guard was present in this run.

## Matched screenshots

`npc-party-select-before/after.png`: current master versus this split; one level-15 Abra, Route2TradeHouse (4,2), interact with NPC 2, dismiss offer, choose YES, dismiss any immediate dialogue, then advance 40 neutral frames. Master immediately enters the movie; this split displays the required party selector. Same starting state and input sequence.

`trade-went-40-before/after.png`: same deterministic capture harness, Cubone/Machoke, player RED, English, `TextWentTo` phase frame 40. The literal control-marker brackets disappear from TRAINER and the split contains the corrected separate text holds/slides. Sprite rendering and ordinary font remain from master.

To reproduce the phase capture, copy `capture.rs` to `crates/pokered-app/tests/capture_save_link_fidelity.rs` in each checkout and run:

```sh
CAPTURE_DIR=/absolute/output CAPTURE_SIDE=before cargo test -p pokered-app --features debug-server --test capture_save_link_fidelity -- --ignored
# Use CAPTURE_SIDE=after on the split. Remove the temporary test file afterwards.
```

Screenshot images are committed under `docs/screenshots/fidelity-save-link-split/`. The PR must embed branch-pinned absolute raw URLs.

## Shared integration hunks

The story split shares the atomic inventory addition, overworld system-save-state helpers/gym-trash setters and external-await VM guard. The battle split shares named OT-ID-zero identity. These matching hunks are necessary for each PR to build and work independently; they merge identically. Engine dependencies remain at the tested `v0.8.2` in this PR.
