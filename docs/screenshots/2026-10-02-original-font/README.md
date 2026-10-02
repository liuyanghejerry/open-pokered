# Project font retained; independent UI and trade captures

The user requested that the project's existing font be preserved. Ordinary English, Chinese and numeric text now uses **Fusion Pixel** again. The original-game alphabet experiment was withdrawn. The five main-menu/options/naming/stats/shop font-experiment pairs remain historical files and are **excluded from final PR acceptance**; they have not been relabeled or recaptured as final font evidence.

The **six independent functional pairs** below were recaptured after that withdrawal, with every before PNG preserved:

| Captures | Before source | Final after source | State/frame |
| --- | --- | --- | --- |
| English/Chinese Town Map | `72ff719` | `eebde2f` | Pallet Town, frame 10 |
| NPC movie WentTo/Sends/Farewell | `72ff719` | `eebde2f` | Named movie phase, relative frame 40 |
| Link movie received peer name | `72ff719` | `eebde2f` | TextWentTo, relative frame 40 |

[capture-manifest.json](capture-manifest.json) labels historical and final cases separately, and records each checksum and actual build source. The final full source is `eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235`, using engine `92ba95f`. The preserved base app SHA-256 is `fd832ed427cb33c0ea14a7fa10a97b6363381009a6a1a3ef037a0b1989ac1879`; the final native app is `ea717f1c2d6332f4608fc13194296cdb166ea20662d8d0f7c379bd17ba005e52`. Every CLI/debug capture uses an isolated executable directory without a default save. These are software-renderer comparisons, **not new original-ROM frame or PCM comparisons**.

Both Town Map languages inspect Pallet Town at frame 10. The final name box reserves space for the retained 10px-high project glyphs above the corrected bottom border; full and partial redraws use the same box geometry. Reproduce both with `python3 scripts/verify_font_fidelity.py --town-map-only --binary <verified-binary> --label before|after --output docs/screenshots/2026-10-02-original-font`.

The three `trade-*-40-*` pairs drive the actual Route 11 Youngster with Nidorino/Pikachu, through YES, actual party selection, ConnectCableText and the movie. Both runs sample `TextWentTo`, `TextForSends` and `TextFarewell` at relative frame 40. Their absolute movie frames differ because the base omits holds/slides. The driver finishes the movie and asserts Nidorina/Pikachu and `EVENT_TRADED_FOR_TERRY`. Adjacent JSON preserves inputs, commands, movie origin and binary hash. Reproduce with `python3 scripts/verify_trade_text_fidelity.py --binary <verified-binary> --label before|after --output docs/screenshots/2026-10-02-original-font`. These targeted NPC runs are not natural-playthrough or link-transport evidence.

`link-trade-went-40-before/after` uses the identical [fixture](../../../crates/pokered-app/examples/fidelity_link_movie_capture.rs), with two real `LinkTradeDriver`s exchanging RED/GREEN names and Pikachu/Charmander through `ChannelTransport`. It routes the resulting `TradeExecute` into `CableClubFlow` and calls the production `PokemonGame::update` movie hook. Both drivers receive GREEN; the movie formerly printed `to <TRAINER>.` and now prints `to GREEN.`. The frozen final Cargo example SHA-256 is `ec563fc073d915a511c6cd80b9f947c54f0fb25ec642bb34403430200bf943c1`; adjacent JSON records its exact source/library hashes and assertion output. This pair verifies app integration with in-memory transport. Separate runtime audits cover actual TCP; this image pair does not make that claim.

Dedicated border, PK/MN and naming-underscore UI graphics remain documented in [POKERED.md](../../../crates/pokered-renderer/fonts/POKERED.md). The ordinary alphabet and numeric font bank have been removed, while pagination still preserves authored lines and wraps expanded text using the retained project's actual glyph advances.

The shared [final capture ledger](../2026-10-02-full-fidelity/after-capture-ledger.json) records this run's 65 newly written after PNGs, including all six functional pairs here. Their before checksums remain unchanged. The earlier `c51209a` capture run is archived separately as evidence before the final dialogue-row spacing repair and is not claimed as the final after source.

## 商店补验

原有商店after曾属于已撤回字体实验。此归档后补同输入、seed49、frame1024的
`eebde2f` 最终普通Fusion Pixel版本，before保持原hash。实际输入、逐步命令及二进制
来源见 [补验记录](mart-buy-final-capture.json)。其余被标注的纯字体实验仍为历史资料。
