# Text-domain parity audit — open-pokered (dotzuki) vs pret/pokered

Date: 2026-08-14. Reference: `/Users/liuyanghejerry/develop/pokered` (master). Remake: `/Users/liuyanghejerry/develop/open-pokered`.
Audit only — no fixes made. `tools/audit_parity.py` untouched; standalone comparators and raw extracts under `/tmp/audit_text/`.
Excluded per scope: fonts/glyphs, Chinese translation content (English presence/structure audited), battle ANIMATION data (already 100% via `scripts/verify_battle_anim_data.py`), cries, and intentional adaptations already documented in `docs/fidelity-audit-2026-08.md`.

## Summary counts

| domain | sub-area | result |
|---|---|---|
| items | order / index parity (item_constants.asm) | 0 index mismatches (11 cosmetic variant names, same id) |
| items | names (83) | 0 diffs |
| items | prices (83) | 0 diffs |
| items | TM prices (50) | 0 diffs |
| items | vending prices (3) | 0 diffs |
| items | key-item flags (83 JSON) | 0 diffs; engine `is_key_item()` omits SAFARI_BALL (1 behavioral gap) |
| items | guard drinks list | 0 diffs |
| items | hidden items (54) | 0 diffs (map/coords/item/flag-index) |
| items | hidden coins | **12 missing** (entire Game Corner feature absent) |
| items | Game Corner prizes | 0 diffs vs RED; BLUE variant not modeled (note) |
| items | slot machine wheels | 0 diffs (3×18 symbols + byte values) |
| moves | order (165) / power / type / accuracy / PP / effect / effect-chance | 0 diffs |
| moves | field-move table (9 entries incl. Softboiled) | 0 diffs (dead 9th ref row = behaviorally equivalent omission) |
| moves | engine effect-chance thresholds | **3 × 1/256 roll deltas** (PoisonSideEffect1/2, Confusion) — engine note |
| pokemon | base stats / types / catch / exp / growth (151) | 0 diffs |
| pokemon | TM/HM flags | 1 species diff — Mew bit 55 (unused, inert) |
| pokemon | level-up learnsets / evolutions (level/item/trade) | 0 diffs |
| pokemon | dex entries (presence) / category / height / weight / names / icons / order | 0 diffs |
| hidden events | hidden-text triggers | **73 missing** |
| hidden events | hidden coins | **12 missing** |
| hidden events | item balls (104 ref 7-arg incl. 27 TM balls) | 0 missing / 0 wrong |
| hidden events | trainer object events (346 ref 8-arg) | 0 missing; **4 wrong-party scene calls** + **2 systemic party bugs** |
| hidden events | coord events | ref checkout has no coord-event system; 1 remake coord mismatch (PokemonMansion2F) |
| text A | 56 maps (29 cities/routes + 27 PokéCenter/Mart) | 13 maps w/ missing entries · 8 wording issues · 7 structural issues |
| text B | 61 maps (8 gyms, E4/league, 44 trainer maps) | 4 maps w/ missing entries · 16 wording lines · 3 structural issues · FISHER class name |

---

# 1. Items

## 1.1 Clean (0 diffs)

- Order/index: ref `constants/item_constants.asm:9-95` (84 consts incl. NO_ITEM, NUM_ITEMS=$53) vs remake `crates/pokered-data/data/items/item_list.json:2-20` → `ItemId` enum (build.rs:1206-1242). All 83 items at identical indices; `test_item_data.rs:43-52` locks `ITEM_DATA[i].id == i+1`.
- Names: ref `data/items/names.asm:3-85` vs `data/items/*.json` `name`. All 83 identical incl. "POKé BALL" glyph, "?????" placeholders, original misspellings ("PARLYZ HEAL", "ELIXER", "EXP.ALL", "OAK's PARCEL", "GUARD SPEC.").
- Prices: ref `data/items/prices.asm:3-85` (bcd3) vs JSON `price`. 83/83 match.
- TM prices: ref `data/items/tm_prices.asm:4-53` (nybble thousands) vs `src/item_data.rs:32-83` `TM_PRICES`. 50/50 match. TM/HM id layout $C9–$FA / $C4–$C8 and move mapping (`items.rs:261-320`) match ref `add_tm`/`add_hm` order.
- Vending: ref `data/items/vending_prices.asm:8-10` (200/300/350) vs `maps/CeladonMartRoof/script.scene:104-263` — all 3 signs match, menu order == ref VendingMachineMenu.
- Key flags (JSON): ref `data/items/key_items.asm:3-85` vs per-item JSON `key_item` — 83/83 match.
- Guard drinks: ref `data/items/guard_drink_items.asm:2-4` vs `maps/Route{5,6,7,8}Gate/script.scene` — same 3 drinks, same priority.
- Hidden items: ref `data/events/hidden_item_coords.asm:8-61` (54) + `data/events/hidden_events.asm:346-599` items vs `src/hidden_items.rs:36-126` — 54/54 match on map/x/y/item/flag-index, original order.
- Prizes (RED): ref `data/events/prizes.asm:9-67` + `data/events/prize_mon_levels.asm` vs `maps/GameCornerPrizeRoom/script.scene:41-189` — Abra9/180, Clefairy8/500, Nidorina17/1200; Dratini18/2800, Scyther25/5500, Porygon26/9999; TM23/3300, TM15/5500, TM50/7700. All match.
- Slot wheels: ref `data/events/slot_machine_wheels.asm` vs `src/slot_machine.rs:36-97` — 3×18 symbol sequences and symbol byte values identical; `tests/test_slot_machine.rs` pins the ref values. Payouts/flash counts also match.

## 1.2 Diffs / gaps

- **Hidden coins — 12/12 missing.** ref has GAME_CORNER HiddenCoins pickups (`data/events/hidden_coins.asm:8-20`, amounts in `hidden_events.asm:287-298`): (0,8)+10, (1,16)+10, (3,11)+20, (3,14)+10, (4,12)+10, (9,12)+20, (9,15)+10, (16,14)+10, (10,16)+10, (11,7)+40, (15,8)+100, (12,15)+10 — remake has nothing: no coords, no pickup logic anywhere (only an unused save field `obtained_hidden_coins: [u8; 2]` at `crates/pokered-core/src/save/game_data.rs:301`; `maps/GameCorner/map.json`/`script.scene` have no floor-coin pickups). Pressing A at these tiles does nothing in the remake.
- **SAFARI_BALL key flag (engine).** ref has SAFARI_BALL key=TRUE (`data/items/key_items.asm:10`), remake JSON says key but `ItemId::is_key_item()` at `crates/pokered-data/src/items.rs:74-99` omits `SafariBall` → Safari Balls are tossable in the remake bag (used by toss/sell/PC guards: `crates/pokered-core/src/items/inventory.rs:27`, `pc_screen.rs:1112`). Same omission for ITEM_2C (ref TRUE, `key_items.asm:46`) — unobtainable, no practical impact.
- **BLUE prize variant not modeled.** ref `data/events/prizes.asm` BLUE table (Nidorino 120/750, Pinsir/Dratini/Porygon 2500/4600/6500, levels 6/12/17/20/24/18) has no remake counterpart — `GameCornerPrizeRoom/script.scene:12` hardcodes RED with a comment. Remake is RED-only; noted, not counted as a data error.
- Cosmetic variant names (same index/id, not parity breaks): `BOULDERBADGE`→`BoulderBadge` (item_list.json:6-8), `ITEM_2C`→`Unused2C` (:11), `ITEM_32`→`Unused32` (:12), `S_S_TICKET`→`SsTicket` (:15). Knock-on: `ItemId::const_name()` yields `UNUSED_2C` (vs ref `ITEM_2C`), unmatched by `from_const_name` — irrelevant, unused item.

---

# 2. Moves

## 2.1 Clean (0 diffs)

- Order: ref `constants/move_constants.asm:8-173` (165) == remake `build.rs:1142-1171` `MOVE_ORDER`; ids emitted as 0x01–0xA5 (`build.rs:1288-1293`); locked by `test_move_data.rs`. 165/165 JSON files, no extras.
- Per-move data: ref `data/moves/moves.asm:14-178` vs `crates/pokered-data/moves/*.json` — power/type/accuracy/pp all 165 match; effect string == PascalCase of ref `EFFECT_*` id in all 165 (valid `MoveEffect` variants, ids 0x00–0x56 incl. the four `const_skip` gaps $48-$4B). Effect chance is encoded in the effect name on BOTH sides (remake has no separate chance field) → per-move chance parity follows from name parity.
- Field moves: ref `data/moves/field_moves.asm:5-13` vs `crates/pokered-core/src/overworld/hm_effects.rs:41-51` — CUT/FLY/SURF/STRENGTH/FLASH/DIG/TELEPORT/SOFTBOILED, same order, same leftmost tiles (0x0C/0x0C/0x0C/0x0A/0x0C/0x0C/0x0A/0x08), same badge gates (Cascade/Thunder/Soul/Rainbow/Boulder, none×3). Ref's 9th `ANIM_B4` row and duplicated `.surf` handler are dead entries (an anim id, never matched by `GetMonFieldMoves`) — remake's omission is behaviorally equivalent.
- HM/TM association: ref `constants/item_constants.asm:141-210` vs `src/items.rs:261-320` — all 55 match in order.

## 2.2 Engine-level notes (not move data; 1/256 deltas)

Effect-chance thresholds keyed by effect id differ from ref by one roll value (ref success set `roll < T`; remake `roll < T` on full-range u8, `crates/pokered-core/src/battle/mod.rs:1463-1467`):
- ref has `POISON_SIDE_EFFECT1` threshold 52 (`engine/battle/effects.asm:100-101`), remake has 51 (`crates/pokered-core/src/battle/effects/mod.rs:141`; also `stack_parity/mod.rs:1577`)
- ref has `POISON_SIDE_EFFECT2` threshold 103 (`effects.asm:103-104`), remake has 102 (`effects/mod.rs:142`; also `stack_parity/mod.rs:1578`)
- ref has `CONFUSION_SIDE_EFFECT` threshold 25 (`effects.asm:1114-1116`), remake has 26 (`effects/mod.rs:200`)

Affects PoisonSting/Smog/Sludge/Psybeam/Confusion at the 1/256 level. Not mentioned in `docs/fidelity-audit-2026-08.md` §B1. All other side-effect thresholds (26/77/85/52) match exactly.

---

# 3. Pokemon

## 3.1 Clean (0 diffs)

- Base stats: all 151 species match HP/ATK/DEF/SPD/SPC, type1/type2, catch rate, base exp, growth rate (remake growth-value set == the set actually used by RB; `GROWTH_SLIGHTLY_FAST/_SLOW` unused in RB and in remake alike). Initial (level-1) moves incl. NO_MOVE padding: 0 diffs.
- TM/HM flags: 150/151 species byte-for-byte on all 7 bytes.
- Learnsets: all 151 `learnset` arrays == ref `evos_moves.asm` `db level, move` lists, element-wise order compared. 70 species with evolutions / 139 with learnsets / 12 with none / 81 without evolutions — same counts both sides.
- Evolutions: every `EVOLVE_LEVEL`/`EVOLVE_ITEM`/`EVOLVE_TRADE` matches (method, level or item, target; minLevel 1 everywhere as in ref).
- Dex: all 151 remake JSONs have non-empty `flavorTextPages`; category/height/weight match incl. quirks (Gyarados 21'4"/5180, Gastly 0.2 lb, Diglett 0'8"). Flavor-text page *content* not compared (presence-only per scope).
- English species names (`lang_data.rs::species_name_en`) == ref `names.asm` via `dex_order.asm` (FARFETCH'D, MR.MIME, NIDORAN♂/♀). Party icons: `menu_icons.asm` nybble table == `mon_party_icons.rs` all 151. Dex order: 151/151.

## 3.2 Diffs

- **Mew TM/HM bit 55 (inert).** ref has flags [255,255,255,255,255,255,255] (bit 55 set by `UNUSED` sentinel, `data/pokemon/base_stats/mew.asm:17`), remake has [255,255,255,255,255,255,127] (`crates/pokered-data/pokemon/Mew.json:22`). `UNUSED_TMNUM` bit 55 is never read by any engine code → behaviorally inert.

Notes: ref icon table is `menu_icons.asm` (no `icon_pointers.asm` exists in this checkout); Mew's stats live outside the BaseStats table (`data/pokemon/mew.asm`) — both handled. Sprite-dimension nibbles are not represented in the remake data layer (renderer derives dims from PNGs) — not comparable, not a diff.

---

# 4. Hidden events / map triggers / object events

## 4.1 Missing hidden-text triggers — 73

Per-map (ref line = `data/events/hidden_events.asm` unless noted). Format: ref has X, remake has Y (path:line).

- **IndigoPlateau** — ref has PrintIndigoPlateauHQText hidden events at (8,13) and (11,13) facing UP/DOWN (`hidden_events.asm:357-358`), remake has no trigger — scene is an 11-line stub with only warps (`maps/IndigoPlateau/script.scene:1-11`).
- **RedsHouse2F** — ref has PrintRedSNESText at (3,5) ANY_FACING (`:138`), remake has only the bedroom PC sign at (0,1) (`maps/RedsHouse2F/map.json` signs).
- **BluesHouse** — ref has 3× PrintBookcaseText at (0,1),(1,1),(7,1) facing UP (`:142-144`), remake has no signs and no bookcase handler (`maps/BluesHouse/script.scene`).
- **OaksLab** — ref has DisplayOakLabLeftPoster (4,0), DisplayOakLabRightPoster (5,0), 2× DisplayOakLabEmailText (0,1),(1,1) facing UP (`:148-151`), remake has nothing at these coords (`maps/OaksLab/map.json`, `script_config.json`).
- **Bench guys (15)** — ref has PrintBenchGuyText at (0,4) in Viridian (155), Pewter (186), Cerulean (191), MtMoon (204), RockTunnel, Vermilion (210), Celadon (240), CeladonHotel (0,4), Lavender, Fuchsia, Cinnabar, Saffron PokéCenters + SafariZone West/East/North Rest Houses; remake has no sign/npc at (0,4) in any of these (map.json signs contain only the PC sign at (13,3)).
- **OpenPokemonCenterPC (9)** — ref has PC hidden events at: CeladonMansion2F (0,5) (236), CeladonHotel (13,3), CinnabarLabFossilRoom (0,4)+(2,4) (538-539), IndigoPlateauLobby (15,7) (534), SafariZone West/East/North Rest House (13,3), SilphCo11F (10,12) (560); remake has NO PC sign/trigger in any of these maps. (The 12 main PokéCenters + RedsHouse2F + BillsHouse PCs ARE implemented — not in this list.)
- **ViridianSchoolHouse** — ref has PrintNotebookText (3,4) + PrintBlackboardLinkCableText (3,0) (`:163-164`), remake has neither (`script.scene:22` only has a trainer's "read the blackboard" quip).
- **Gym statues (14)** — ref has 2 per gym facing UP (Viridian (15,15)+(18,15) `:168-169`, Pewter (3,10)+(6,10) `:178-179`, Cerulean (3,11)+(6,11) `:196-197`, Vermilion (3,14)+(6,14) `:214-215`, Celadon (3,15)+(6,15) `:245-246`, Fuchsia, Cinnabar (17,13), Saffron (9,15)); remake has no statue text in any of the 8 gym scenes or map.json ("statue"/"LEADER:" absent).
- **Museum1F** — ref has AerodactylFossil (2,3) + KabutopsFossil (2,6) facing UP (`:173-174`), remake has neither (npcs are 4 scientists + OldAmber at (16,2) only).
- **BikeShop** — ref has 6× PrintNewBikeText ANY_FACING on bike-display tiles (1,0),(2,1),(1,2),(3,2),(0,4),(1,5) (`:543-548`), remake has no per-tile trigger ("It's a cool BIKE" only inside clerk dialogue, `script.scene:32`).
- **VermilionGym** — ref has PrintTrashText at (6,1) facing DOWN (`:216`, the 16th can behind Lt.Surge), remake has 15 GymTrashScript cans as signs but nothing at (6,1) (map.json signs are (1,7)…(9,11) only).
- **SSAnneKitchen** — ref has 2× PrintTrashText at (13,5),(13,7) facing DOWN (`:371-372`), remake has no trash trigger (only an NPC line mentioning trash, `script.scene:20`).
- **CeladonMansionRoofHouse** — ref has 2× PrintBlackboardLinkCableText LinkCableHelp (3,0),(4,0) (`:521-522`) + PrintNotebookText TMNotebook (3,4) (`:523`), remake has none.
- **MrFujisHouse** — ref has 3× PrintMagazinesText at (0,1),(1,1),(7,1) facing DOWN (`:515-517`), remake has none.
- **FightingDojo** — ref has PrintFightingDojoText (3,9),(6,9), PrintFightingDojoText2 (4,0), PrintFightingDojoText3 (5,0) facing UP (`:527-530`), remake has none (scene only has Karate Master/blackbelt dialogues).
- **Route15Gate2F** — ref has Route15GateLeftBinoculars at (1,2) facing UP (`:511`), remake only has the right-window binocular sign at (6,2). Left window trigger missing.
- **PokemonMansion2F** — ref has Mansion2Script_Switches at (2,11) facing UP (`:458`), remake has coord events at (3,8) and (4,8) instead — coordinate mismatch (`maps/PokemonMansion2F/script_config.json` coordEvents mansionSwitch1/2).
- **GameCorner hidden coins** — 12 entries, see §4.2.

## 4.2 Hidden coins — 12 missing

ref has 12 `HiddenCoins` in GameCorner (`hidden_events.asm:287-298`): (0,8)+10, (1,16)+10, (3,11)+20, (3,14)+10, (4,12)+10, (9,12)+20, (9,15)+10, (16,14)+10, (10,16)+10, (11,7)+40, (15,8)+100, (12,15)+10 — remake has NO pickup anywhere (no coordEvents/signs at those coords, nothing in `script.scene`; only SRAM bytes reserved at `crates/pokered-core/src/save/game_data.rs:301`). Itemfinder correctly does not detect coins (matches ref, doc C7).

## 4.3 Item balls — 0 diffs (104/104)

All 104 ref 7-arg `object_event` balls (77 regular + 27 TM balls; ref has no HM balls) match remake map.json npcs: same coords, `spriteName: PokeBall`, `itemId` = raw Gen-1 item index. BluesHouse's two NO_ITEM toggle events (Daisy, Town Map) are modeled as npcs with toggle flags (`BLUESHOUSE_DAISY2`/`BLUESHOUSE_TOWN_MAP`) — equivalent. No extra remake item-ball npcs.

## 4.4 Trainer object events — 0 missing; 4 wrong-party scenes + 2 systemic bugs

346 ref 8-arg trainer events → 346 remake counterparts (308 sight-engaged `isTrainer=true`, class/set/coords all match; 25 script-driven with correct class/set, `isTrainer=false`; 13 static/wild implemented via `startWildBattle`/`startBattle` with correct species/level).

**Wrong-party scene calls (4):**
- ref has GameCorner OPP_ROCKET set 7 (Raticate20/Zubat20; `data/maps/objects/GameCorner.asm:36`), remake has `startBattle("OPP_ROCKET8")` → Drowzee21/Machop21 (`maps/GameCorner/script.scene:179`)
- ref has MtMoonB2F OPP_SUPER_NERD set 2 (Grimer12/Voltorb12/Koffing12; `objects/MtMoonB2F.asm:24`), remake has `startBattle("OPP_SUPER_NERD3")` → Route8 Voltorb20 group (`maps/MtMoonB2F/script.scene:31`)
- ref has Route24 OPP_ROCKET set 6 (Ekans15/Zubat15; `objects/Route24.asm:19`), remake has `startBattle("OPP_ROCKET7")` → GameCorner Raticate20/Zubat20 (`maps/Route24/script.scene:33`)
- ref has SilphCo11F OPP_GIOVANNI set 2 (Nidorino37/Kangaskhan35/Rhyhorn37/Nidoqueen41; `objects/SilphCo11F.asm:22`), remake has `startBattle("OPP_GIOVANNI3")` → Viridian Gym L45 team (`maps/SilphCo11F/script.scene:62`)

**S1 — sight-trainer party off-by-one (systemic, affects all 308 sight trainers).** map.json `trainerSet` stores the ref's 1-based trainer number (generator `scripts/parse_npcs.py:185`), but the engine treats it as a 0-based party index: `make_trainer_id(tc, set)` emits `OPP_CLASS{set+1}` (`crates/pokered-data/src/trainer_data.rs:356-359`), `parse_trainer_id` returns `index−1` (`:278-300`), and `crates/pokered-core/src/overworld/update.rs:869/1009` pass `trainer_set` raw → every sight-engaged trainer fights party[set] instead of party[set−1] (e.g. MtMoonB2F Rocket1 fights Sandshrew11/Rattata11/Zubat11 instead of Rattata13/Zubat13). No test covers it.

**S2 — rival battles resolve to the L5 starter party (systemic).** `parse_trainer_id("OPP_RIVAL2")` strips trailing digits → class `Rival1`, index 1; the Rival2/Rival3 match arms are unreachable. The app then remaps by player starter into Rival1's L5 lab group (`crates/pokered-app/src/game.rs:2008-2030`) → every post-lab rival battle (CeruleanCity:61, Route22:57/128, SSAnne2F:57, PokemonTower2F:52, SilphCo7F:118, ChampionsRoom:43) uses the L5 starter party. Remake party JSONs themselves match ref exactly (Rival1.json 9 / Rival2.json 12 / Rival3.json 3 parties) — the correct data exists; the id parsing is broken.

## 4.5 Coord events

Ref checkout has NO coord-event/trigger system (no `coord_event`/`def_coord_events` macro, no trigger loader — Gen-1 style) → the ref-side comparison is vacuous; 0 ref coord-event blocks. Remake's 41-map `coordEvents` were cross-checked against ref hidden-event anchors where one exists: mansion switches 1F (2,5)✓, 3F (10,5)✓, B1F (20,3)+(18,25)✓, **2F mismatch** — ref (2,11) vs remake (3,8)/(4,8) (`PokemonMansion2F/script_config.json`). SilphCo card-key doors have no ref hidden-event counterpart (ref uses flag-checked scripts) — not a gap.

## 4.6 Trainer-header sanity

`trainer_headers.rs` tables: `tools/audit_parity.py --domain maps` reports 0 diffs (counts, event flags, sight ranges). TrainerClass enum order == `trainer_constants.asm` exactly (0=NOBODY…47=LANCE); every map.json `trainerClass` string resolves (`map_data_loading.rs:233-260`). Remake trainer party JSONs match ref `parties.asm` order.

---

# 5. Dialogue text

Method: ref `text/{Map}.asm` labels resolved through `scripts/{Map}.asm` def_text_pointers/def_trainers + base texts; compared (wrap/glyph normalized, `#MON`↔`POKéMON`, dynamic item names via token matching) against remake map.json text blocks + `script.scene` `@t`/`@say`/`@choice` + npc `endBattleText`. Chinese 2nd args of `@t` ignored.

## 5.1 Missing dialogue entries (by map)

Part A (13 maps):
- **All 11 PokéCenters** — ref cable-club receptionist texts missing ("This area is reserved for 2 friends…", "Please apply here. Before opening the link, we have to save the game.", "Please wait." — `_CableClubNPCPleaseWaitText`); remake has only "Welcome to the Cable Club!". Flow implemented app-side with different texts (`crates/pokered-app/src/render/link.rs:31-35`, doc W1). Affects Viridian/Pewter/Cerulean/MtMoon/RockTunnel/Vermilion/Celadon/Lavender/Fuchsia/Cinnabar/Saffron PokéCenters.
- **Route2** — ref has "No more room for items!" for MOON STONE / HP UP item balls, remake has no bag-full branch (unconditional giveItem, `script.scene:8-29`).
- **Route4** — ref has "No more room for items!" for TM WHIRLWIND, remake has none (`script.scene:34`).
- **MtMoonPokecenter** — ref `TEXT_MTMOONPOKECENTER_CLIPBOARD` is an empty text box (intentional ref quirk), remake has no clipboard dialogue at all — behaviorally equivalent, noted.

Part B (4 maps):
- **ViridianGym** — ref has `TEXT_VIRIDIANGYM_GIOVANNI_TM27_NO_ROOM` "You do not have space for this!" (`text/ViridianGym.asm:83`), remake has no bag-full branch on the TM27 give (`script.scene:14-53`).
- **CinnabarGym** — ref has 7 quiz-trainer end-battle texts, remake has none: "Yow! Hot, hot, hot!" (`text/CinnabarGym.asm:78`), "I surrender!" (:88), "Waah! My studies!" (:97), "Too hot to handle!" (:113), "Ow!" (:120), "Yowza! Too hot!" (:129), "Oh! Snuffed out!" (:184). Remake `talkSuperNerd1..7` show only AfterBattleText after the `@choice` (ref prints end-battle text when the quiz is answered correctly, `scripts/CinnabarGym.asm:264-277` BIT_PRINT_END_BATTLE_TEXT).
- **ChampionsRoom** — ref has `_RivalVictoryText` "Hahaha! I won, I won!…" (`text/ChampionsRoom.asm:53`), the lose-branch text of the final battle; remake has only `@if (result == "win")` (`script.scene:52`), no lose text. NOTE: this text is global in ref (all rival battles) and absent from the entire remake corpus.
- **SilphCo2F** — ref has `_SilphCo2FSilphWorkerFTM36NoRoomText` "You don't have any room for this." (`text/SilphCo2F.asm:30`), remake has no bag-full branch (`script.scene:12-26`).

## 5.2 English content mismatches

Part A:
- **Nurse heal flow reworded on all 11 PokéCenters**: ref "Welcome to **our** POKéMON CENTER!" vs remake "Welcome to **the** POKEMON CENTER!" (`maps/SaffronPokecenter/script.scene:9`); ref "OK. We'll need your POKéMON." vs remake "OK, we will need your POKEMON." (`PewterPokecenter/script.scene:17`); ref "Thank you! Your POKéMON are **fighting fit**!" vs remake "Thank you for waiting. Your POKEMON are fully **healed**!" (`PewterPokecenter/script.scene:23`). Note: `maps/shared/pokecenter.scene` keeps ref-faithful wording but is shadowed by per-map scenes, and its literal `#MON` has no runtime replacement.
- **CeruleanCity CooltrainerF**: remake English literally contains `#MON` twice ("It's so hard to control #MON!… Your #MON's obedience…", `script.scene:194`) — renders as "#MON" (ref "POKéMON").
- **CeladonMart4F**: ref "POKé DOLL" (with space) vs remake "POKeDOLL" (`script.scene:18,26`).
- **Route9/12/15 item balls**: ref "No more room for items!" vs remake "You have too much stuff already!" (`Route9/script.scene:111`, `Route12/script.scene:126,141`, `Route15/script.scene:121`).
- **Route2 item names**: remake "<PLAYER> found MOON_STONE!" / "HP_UP!" with const-name underscores (`script.scene:16,29`); ref prints display names "MOON STONE" / "HP UP".
- **ViridianCity Fisher**: ref `_ViridianCityFisherTM42NoRoomText` exists only as unreachable map.json fallback (`text.npc.6_NoRoom`); `talkFisher` ignores the giveItem result (`script.scene:120`; engine `crates/pokered-app/src/game.rs:2769` `let _ = bag.add_item(...)`).
- **CeruleanCity Rocket**: ref `_CeruleanCityRocketTM28NoRoomText` unreachable same way (`script.scene:134,149`; map.json `text.npc.2_NoRoom` only).
- **CeladonMartRoof Little Girl**: ref `_CeladonMartRoofLittleGirlNoRoomText` missing entirely (`script.scene:45,63,81` no bag-full branch).
- **PewterPokecenter**: remake-only "JIGGLYPUFF sang its SONG. Everyone started to feel sleepy..." (`script.scene:49`); ref has only "JIGGLYPUFF: Puu pupuu!".
- **Route2**: remake-only "The item ball is empty." (`script.scene:11,24`); ref hides picked balls with no message.

Part B:
- **SilphCo5F**: ref "We study # BALL technology on this floor!" → remake "We study POKEMON BALL technology…" (`script.scene:122`); ref "…the ultimate # BALL which would catch anything!" → remake "…ultimate POKEMON BALL…" (`script.scene:120`). (Ref `#` renders as "POKé".)
- **ViridianForest signs (5 replaced + 1 extra)**: ref sign (16,32) "For poison, use ANTIDOTE!…" (`text/ViridianForest.asm:84`) → remake map.json sign:2 "TRAINER TIPS If a POKeMON is poisoned…"; ref (26,17) "Contact PROF.OAK via PC…" (:90) → remake sign:3 "…Contact with the outside world has been made through the Pc system."; ref (4,24) "No stealing of #MON…" (:99) → remake sign:4 "…POKeMON attacks are physical or special…"; ref (18,45) "LEAVING VIRIDIAN FOREST PEWTER CITY AHEAD" (:120) → remake sign:5 "Now leaving VIRIDIAN FOREST"; remake sign:6 "VIRIDIAN FOREST PEWTER CITY - VIRIDIAN CITY" is extra (ref has 6 bg_events, `objects/ViridianForest.asm:23-28`). Only TrainerTips1 (24,40) matches.
- **CinnabarGym quiz merge**: remake merged ref's 6 hidden gate quizzes into the 7 Super Nerd flows and appended "CINNABAR GYM's QUIZ! Answer to open the gate!" to each question (`script.scene:82,118,154,190,226,262,298`); gate questions reworded ("CATERPIE evolves into BUTTERFREE?" → "A CATERPIE evolves straight into BUTTERFREE! YES or NO?"); correct text ref "You're absolutely correct! Go on through!" → remake "Correct! You know your types! The gate is open!" (`scene:305`); ref "Sorry! Bad call!" → remake per-question wrong-answer lines; remake-only "Argh! The gate is all yours!" (×7) and six per-question explanations.
- **FISHER class display name**: ref "FISHERMAN" (`data/trainers/names.asm`) vs remake `TrainerClass::Fisher => "FISHER"` (`crates/pokered-data/src/trainer_data.rs:138`) — battle intros read "FISHER wants to fight!". Affects 4 FISHER trainers on Route21 + FISHERs on Route12/13.
- Remake-only "empty item ball" interactions across many maps (VictoryRoad1F/2F, MtMoon1F, PokemonMansion1F/2F/B1F, PokemonTower4F/6F, RocketHideoutB2F, SilphCo3F/4F/5F/10F, SSAnne2FRooms/B1FRooms) — ref picked balls vanish.
- Bag-full wording drift: remake "You have too much stuff already!" vs ref "But, <PLAYER> has no more room for other items!" (`data/text/text_2.asm:758`).

## 5.3 Missing structural flows (yes/no, multi-page, item handouts)

Part A:
- **PalletTown page breaks**: ref `_PalletTownOakItsUnsafeText` has 3 pages, remake joins with `\n` into 1 (`script.scene:51,101`); `_PalletTownGirlText` 2 pages → 1 (`:109`); `_PalletTownFisherText` 2 pages → 1 (`:116`).
- **CeruleanCity CooltrainerF**: ref 3 pages, remake 2 — page break between "That's wrong!" and "It's so hard to control" missing (`script.scene:194`).
- **Route2/Route4/Route9/12/15/CeladonMartRoof/ViridianCity/CeruleanCity bag-full branches**: giveItem results ignored — no no-room branch (see §5.2 lines).
- **PokéCenter receptionist flow** — see §5.1 (cable club).

Part B:
- **ViridianGym TM27**: ref `ViridianGymReceiveTM27` guards GiveItem carry (`scripts/ViridianGym.asm:143-153`), remake unconditional `giveItem("TM27", 1)`.
- **SilphCo2F TM36**: ref loads `.TM36NoRoomText` on GiveItem carry (`scripts/SilphCo2F.asm:122-126`), remake unconditional.
- **ChampionsRoom lose branch**: ref SaveEndBattleTextPointers shows `_RivalVictoryText` on loss (`scripts/ChampionsRoom.asm:66`), remake has no lose text.

Verified clean: all 105 trainer battle/end/after texts (part A) + 217 def_trainers entries (part B), all 414 def_text_pointers entries, ViridianMart parcel cutscene, vending flow, elevator flow, Snorlax/POkéFLUTE flows, all yes/no flows, badge gives, item-give flows (MtMoonB2F fossil choice, Route24 NUGGET, SilphCo11F MASTER_BALL), card-key door lines, secret-switch lines, all gym leader/E4/champion/Hall-of-Fame texts.

## 5.4 Notes (not gaps)

- False positives excluded: `_SilphCo10FPorygonText` is marked unreferenced in ref itself (`scripts/SilphCo11F.asm:385`); `TEXT_MTMOONB1F_UNUSED` is an empty ref text.
- Glyph-class drops (out of scope): JR.TRAINER♂/♀ → "JR.TRAINER", COOLTRAINER♂/♀ → "COOLTRAINER"; RIVAL name substituted at runtime (note only).
- CHIEF: ref name "CHIEF" vs remake "SCIENTIST" — OPP_CHIEF is unused in the original, non-issue.
- Legendary/Voltorb "trainers" (13 NPCs) are script-driven wild battles with correct species/level (Voltorb 40, Electrode 43, birds 50, Mewtwo 70) and ref pre-battle lines present — equivalent adaptation.
- Stray/duplicate map.json blocks: VermilionGym npc:1 bracket placeholder; PokemonTower3F npc:4 "[Purified zone heal script]" (stray — 3F has no purified zone); PokemonTower2F/5F/6F bracket duplicates; Route24 npc:7 truncated duplicate (full text in `script.scene:103`) — dead data, content OK.
- Mansion switch text + BoulderText + Card-key texts match ref word-for-word; "It looks like the boulder is settled on the switch." (VictoryRoad1F scene:81) is remake-only (ref always shows BoulderText).
- Pagination timing / waitbutton timing are engine-level, not diffed.
