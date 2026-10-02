# Original font and text metrics

The eleven pairs are captured from the preserved audit base and integrated fixes at the same initialized state and frame (trade pictures use the same phase-relative frame). `capture-manifest.json` records each image checksum and its actual source/build provenance:

| Captures | Before source | After source | Artifact |
| --- | --- | --- | --- |
| Main menu, options, naming, stats, shop, three NPC movie texts | `72ff719b39634c153cb82d3f3ece200bd413c4e0` | `cd9acde31a430e69768b39f598dc90c9fa536ccc` | Native debug binary |
| English/Chinese Town Map | `72ff719b39634c153cb82d3f3ece200bd413c4e0` | `8837a2a32c2ba8c345db16e97b01f49016e88950` | Native debug binary |
| Link movie peer name | `72ff719b39634c153cb82d3f3ece200bd413c4e0` | `8837a2a32c2ba8c345db16e97b01f49016e88950` | Identical fixture linked to each source's actual app/core/render libraries |

The base native binary SHA-256 is `fd832ed427cb33c0ea14a7fa10a97b6363381009a6a1a3ef037a0b1989ac1879`. The first eight after captures use `0dba5dc0ab6870244c01f42adea919ddb27689bf197a58dc086207f9712132de`; that binary was built at **cd9acde**, despite an old local filename containing `675`. The Town Map after captures use the immutable 8837a2a binary `b8ba782f6cb22151964722ae958dae32f9085b356a71c145c3a3b37bdf513847`. Source commits are not relabeled to a later final HEAD.

Individual CLI examples:

```sh
pokered-app screenshot --screen main-menu --frames 10 --lang en -o main-menu-after.png
pokered-app screenshot --screen options --frames 10 --lang en -o options-after.png
pokered-app screenshot --screen naming --frames 10 --lang en -o naming-after.png
```

The CLI naming fixture explicitly selects Chinese and initializes the pinyin IME; its `--lang en` argument does not turn it into an English naming capture. The screenshot tool reads saves beside its executable, so the reproduction script copies the executable to a temporary directory for every isolated run.

Original glyphs and opaque border tiles come from the pinned pret/pokered PNGs; conversion provenance is in `crates/pokered-renderer/fonts/POKERED.md`. The renderer chooses metrics by character: printable ASCII/original charmap characters use 8px advances in both UI languages; Chinese keeps its existing Fusion Pixel bitmap and 10px advance. No global language-dependent font switch is introduced.

Overworld and battle text use the same metrics for pagination and drawing. Authored newlines/page boundaries remain hard; expanded rows wrap at 144px before two-line paging. Tests cover exact eighteen-cell English rows, Chinese/ASCII number runs, overlong words, punctuation, placeholder growth, and pixels next to the right border. Chinese stats ID/OT values and shop name/price rows receive space for the wider ASCII cells. Original money/price fields suppress leading zeroes and remain right aligned; party level 100 omits its level prefix as in `PrintLevel`.

Focused validation: the renderer/data suites, all UI library/integration tests, and core dialogue typewriter/battle pagination tests. These captures document visible changes; they do not claim an emulator pixel match of every screen.

Additional preserved-base captures: `stats-zh-before.png` uses a level-100 Venusaur, maximum 5-digit trainer ID and a seven-letter OT at frame 512. `mart-buy-before.png` uses the actual Viridian Mart buy menu and maximum balance at frame 1024. Adjacent JSON files contain seeded input and all commands/responses. Both are debug-seeded targeted views, not a natural-playthrough assertion.

Reproduce all five pairs without Cargo using `python3 scripts/verify_font_fidelity.py --binary <verified-binary> --label before|after --output docs/screenshots/2026-10-02-original-font`. It isolates save sidecars, uses seed 49, and executes input timelines atomically. The script records the binary SHA-256 for subsequent captures.

The three `trade-*-40-*` pairs drive the actual Route 11 Youngster with a Nidorino and Pikachu, through YES, ConnectCableText and the movie. The fixed flow also selects the actual first party entry. Both runs sample `TextWentTo`, `TextForSends` and `TextFarewell` at phase-relative frame 40; their absolute movie frames differ because the base omits holds/slides. The preserved-base movie's frame-40 front-picture capture was independently compared byte for byte with the visual agent's base fixture, confirming the movie origin used for the later text samples. The driver finishes the movie and asserts Nidorina/Pikachu and `EVENT_TRADED_FOR_TERRY`.

Reproduce these text pairs with `python3 scripts/verify_trade_text_fidelity.py --binary <verified-binary> --label before|after --output docs/screenshots/2026-10-02-original-font`. The three paired samples show the formerly merged texts and literal angle brackets; separate regression tests cover the frame-79/80 second-text transition and all three window slides. These targeted seeded NPC runs do not assert a natural playthrough or physical/link-protocol interoperability.

The Town Map pairs both inspect Pallet Town at frame 10, with no adjacent save. English retains its original single-row name box. Chinese receives two interior tile rows so its taller glyph stays clear of the original bottom border; redraw damage and marker visibility use the same box geometry. The existing full-frame comparisons for every landmark and FLY selection remain, with an added Chinese interior-clearance assertion. Reproduce both with `python3 scripts/verify_font_fidelity.py --town-map-only --binary <verified-binary> --label before|after --output docs/screenshots/2026-10-02-original-font`.

`link-trade-went-40-before/after` uses the identical `crates/pokered-app/examples/fidelity_link_movie_capture.rs`, linked to the preserved base and immutable integrated build's app/core/render libraries. Two real `LinkTradeDriver`s exchange RED/GREEN names and Pikachu/Charmander through `ChannelTransport`, request, accept, selection and confirmation. The fixture routes the resulting `TradeExecute` into `CableClubFlow` and calls the public `PokemonGame::update` hook to start the movie. Both sides confirm a received peer name of GREEN, then sample `TextWentTo` at phase-relative frame 40: the base prints `to <TRAINER>.`, the fix prints `to GREEN.`. Adjacent JSON records fixture/source/library checksums and successful assertion output. The actual production frontend Cargo regression `completed_channel_trade_movie_uses_the_received_peer_name` passed in the final 132/132 app library run; it checks WentTo, Sends and Farewell text after the same complete driver exchange. This verifies the app integration with in-memory transport; it does not claim physical cable or TCP interoperability. The normal example invocation is `cargo run -p pokered-app --features debug-server --example fidelity_link_movie_capture -- <output-directory> after`.
