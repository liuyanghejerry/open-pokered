# Content Parity Audit — items / moves / pokemon / hidden_events / text

Reference: pret/pokered master (`<pret/pokered checkout>`).
Remake: open-pokered (`<this repo checkout>`).
Audit scripts: `/tmp/parity/{items,moves,pokemon,objects,hidden_classify}.py`, `/tmp/parity/text.py` (full text list: `/tmp/parity/text_report.md`).

## Summary counts

| Domain | Compared | Mismatches |
|---|---|---|
| Items: order/names/prices/key-flags | 83 items | 0 |
| Items: TM/HM prices | 50 TMs + 5 HMs | 0 |
| Items: vending prices / guard drinks | 3 / 3 | 0 |
| Items: hidden items | 54 | 0 |
| Items: Game Corner prizes (RED) | 3+3+3 prizes, costs, levels | 0 (BLUE branch not ported, documented in scene) |
| Items: slot machine wheels | 3×18 | 0 |
| Items: hidden coins | 12 | **12 missing (feature absent)** |
| Moves: effect/power/type/acc/pp + order | 165 | 0 |
| Moves: field-move table | 8 moves | 0 (ref 9-slot table has duplicate `.surf` quirk) |
| Moves: effect chances (engine constants) | 9 checked | **3 off-by-one + 1 stat-down miss quirk absent** |
| Pokemon: stats/types/catch/exp/growth/initial/TMHM | 151 species | 0 |
| Pokemon: learnsets / evolutions | 151 | 0 |
| Pokemon: dex entries (height/weight/category presence) / party icons | 151 / 151 | 0 |
| Hidden events: ref hidden-event routines (no-sign) | 119 | **75 missing, 44 implemented/adapted** |
| Hidden events: item balls | 104 | **13 wrong/missing** |
| Hidden events: trainer class/set | 346 | **4 wrong trainer set** |
| Hidden events: coord events (rival/card-key/guards/holes/currents/switches) | 143 remake coordEvents | 0 gaps found |
| Text: ref text pointers | 915 | **178 missing** |
| Text: sign pointers | 145 | 0 |
| Text: structural flows (nurse/yesno/choices) | 12 nurse | 0 |

---

## 1. Items

### 1.1 Hidden coins — feature not implemented
- ref has 12 GameCorner hidden-coin pickups (`data/events/hidden_coins.asm:5-17`, each `hidden_coin GAME_CORNER, x, y`, detected via COIN CASE; MAX_HIDDEN_COINS = 16).
- remake has NO coordinate table and no detection logic. Only the save-format flag bytes exist and nothing reads/writes them: `crates/pokered-core/src/save/game_data.rs:301` (`obtained_hidden_coins`), `crates/pokered-core/src/save/sram_deser_game_data.rs:144`.
- Consequence: the 12 floor coins in Game Corner can never be found in the remake.

### 1.2 Everything else matches
- Item order/ids: `item_list.json` (83 entries) == `constants/item_constants.asm` $01–$53, verified positionally.
- Names: 83/83 == `data/items/names.asm`.
- Prices: 83/83 == `data/items/prices.asm` (`price` field in `data/items/{Id}.json`).
- TM prices: `src/item_data.rs:32` TM_PRICES == `data/items/tm_prices.asm` (nybble×1000).
- Key-item flags: JSON `key_item` == `data/items/key_items.asm` dbit, 83/83.
- Vending: FRESH WATER ¥200 / SODA POP ¥300 / LEMONADE ¥350 (`data/items/vending_prices.asm`) == `maps/CeladonMartRoof/script.scene:113-144` giveItem/takeMoney flows.
- Guard drinks: FRESH_WATER/SODA_POP/LEMONADE accepted by all four saffron gates == `data/items/guard_drink_items.asm` (Route5Gate/Route6Gate/Route7Gate/Route8Gate scenes).
- Hidden items: 54/54 == `hidden_items.rs` HIDDEN_ITEMS vs `data/events/hidden_item_coords.asm` + per-map `HiddenItems` entries in `data/events/hidden_events.asm` (order = save-flag index preserved).
- Game Corner prizes (RED): ABRA L9/180, CLEFAIRY L8/500, NIDORINA L17/1200; DRATINI L18/2800, SCYTHER L25/5500, PORYGON L26/9999; TM23 3300/TM15 5500/TM50 7700 == `data/events/prizes.asm` + `prize_mon_levels.asm` (maps/GameCornerPrizeRoom/script.scene). Note: BLUE-branch prizes (NIDORINO/PINSIR, different costs/levels) are not ported — the scene explicitly documents RED-only.
- Slot wheels: `slot_machine.rs` WHEEL1-3 == `data/events/slot_machine_wheels.asm` (3×18); payouts 300/100/8/15 match engine constants (tests/test_slot_machine.rs).

---

## 2. Moves

### 2.1 Move table — fully matches
- All 165 moves: effect / power / type / accuracy / pp == `data/moves/moves.asm` (ref) vs `crates/pokered-data/moves/*.json` (remake), in exact `constants/move_constants.asm` order (`MOVE_ORDER` in `crates/pokered-data/build.rs:1141`). 0 diffs.

### 2.2 Field move table — equivalent
- ref `engine/menus/start_sub_menus.asm:122-130` `.outOfBattleMovePointers`: cut/fly/surf/**surf**/strength/flash/dig/teleport/softboiled (9 slots, slot 3 duplicates `.surf` — engine dispatch quirk).
- remake `crates/pokered-core/src/overworld/hm_effects.rs:42-50`: Cut/Fly/Surf/Strength/Flash/Dig/Teleport/Softboiled — 8 entries, correct badge gates (Cascade/Thunder/Soul/Rainbow/Boulder). Behaviorally equivalent; the duplicate-surf slot is an unreachable ref quirk.

### 2.3 Effect chances (engine constants; the chance is not stored in the move table on either side)
- ref has POISON_SIDE_EFFECT1 chance `20 percent + 1` = **52**/256 (`engine/battle/effects.asm:101`), remake has **51** (`crates/pokered-core/src/battle/effects/mod.rs:142` `PoisonSideEffect1 => ... 51`). Remake's own Twineedle path uses the correct 52 (`crates/pokered-core/src/battle/effects/multi_hit_effects.rs:75`).
- ref has POISON_SIDE_EFFECT2 chance `40 percent + 1` = **103**/256 (`effects.asm:104`), remake has **102** (`mod.rs:143`).
- ref has ConfusionSideEffect chance `cp 10 percent` = **25**/256 (no `+1`, `effects.asm:1116`), remake has **26** (`mod.rs:200`).
- ref stat-down primary moves have an extra 25%-miss roll in regular battles: `cp 25 percent + 1` = 64/256 (`effects.asm:553`). Not found in the remake (`stat_effects.rs` applies `apply_stat_down` unconditionally; `accuracy.rs` special-cases only SwiftEffect). Likely unimplemented Gen-1 quirk.
- Verified matching: burn/freeze/paralyze side1 = 26, side2 = 77 (`effects.asm:215-273` vs `mod.rs:144-149`); flinch 26/77 (`effects.asm:984-986` vs `mod.rs:195-196`); stat-down side-effect 85 (`effects.asm:562` vs `stat_effects.rs:52`).

---

## 3. Pokemon — 0 diffs

- 151 species in dex order (`base_stats.asm` INCLUDE order + `mew.asm`) == `crates/pokered-data/pokemon/*.json` + SPECIES_ORDER (`build.rs:1130`).
- Per species: base stats, type1/type2, catch rate, base exp, growth rate, initial moves, TM/HM 55-bit flags — all match `data/pokemon/base_stats/{s}.asm`.
- Learnsets and evolutions (level/item/trade incl. min-level) — all match `data/pokemon/evos_moves.asm`.
- Dex entries: height ft/in + weight decipounds + category presence — all match `data/pokemon/dex_entries.asm`.
- Party icons: 151 == `data/pokemon/menu_icons.asm` (MonPartyData) vs `src/mon_party_icons.rs`.
- Existing tests only spot-check ~6 species; the JSON data itself is fully correct.

---

## 4. Hidden events / map triggers

Classification of all 119 `[no-sign]` ref hidden events (from `tools/audit_parity.py --domain maps`), grouped: 75 missing, 44 implemented or adapted.

### 4.1 Missing interactables (75)
- **Gym statues — 14 missing**: ref `hidden_event X, Y, GymStatues, SPRITE_FACING_UP` in `data/events/hidden_events.asm` (ViridianGym (15,15)/(18,15); PewterGym (3,10)/(6,10); CeruleanGym (3,11)/(6,11); VermilionGym (3,14)/(6,14); CeladonGym (3,15)/(6,15); FuchsiaGym (3,15)/(6,15); SaffronGym (9,15); CinnabarGym (17,13)). Remake has no "LEADER:"/"WINNING TRAINERS:" statue text anywhere in `crates/` — all 14 unmapped.
- **Bench guy text — 15 missing**: ref `hidden_event 0,4, PrintBenchGuyText, SPRITE_FACING_LEFT/UP` in every pokecenter + CeladonHotel + 3 Safari rest houses. Remake has no bench text (the bench NPC itself exists; the bench-tile interaction does not).
- **Pokecenter-style PCs — 9 missing**: ref `hidden_event ..., OpenPokemonCenterPC, SPRITE_FACING_UP` — SilphCo11F (10,12), SafariZoneWest/East/NorthRestHouse (13,3), CinnabarLabFossilRoom (0,4)/(2,4), IndigoPlateauLobby (15,7), CeladonHotel (13,3), CeladonMansion2F (0,5). (Pokecenters' own PCs at (13,3) ARE implemented as signs.)
- **Trash cans — 3 missing**: SSAnneKitchen (13,5)/(13,7), VermilionGym (6,1). (VermilionGym's 15 trash-can switch puzzle IS implemented as signs + scene.)
- **Bookcases/SNES/posters/mail/notebooks/blackboards/magazines/scrolls/fossil-plaques/bikes/HQ-signs — 31 missing**:
  - BluesHouse bookcases (0,1)/(1,1)/(7,1) PrintBookcaseText.
  - RedsHouse2F (3,5) PrintRedSNESText.
  - OaksLab posters (4,0)/(5,0) DisplayOakLabLeft/RightPoster; email (0,1)/(1,1) DisplayOakLabEmailText.
  - ViridianSchoolHouse (3,4) PrintNotebookText + (3,0) PrintBlackboardLinkCableText.
  - CeladonMansionRoofHouse (3,0)/(4,0) PrintBlackboardLinkCableText + (3,4) PrintNotebookText.
  - MrFujisHouse (0,1)/(1,1)/(7,1) PrintMagazinesText.
  - IndigoPlateau (8,13)/(11,13) PrintIndigoPlateauHQText.
  - BikeShop (1,0)/(2,1)/(1,2)/(3,2)/(0,4)/(1,5) PrintNewBikeText — displayed bikes not interactable.
  - FightingDojo (3,9)/(6,9) PrintFightingDojoText, (4,0) Text2, (5,0) Text3.
  - Museum1F (2,3) AerodactylFossil + (2,6) KabutopsFossil plaques.
  - Route15Gate2F (1,2) Route15GateLeftBinoculars (the right binoculars sign at (6,2) is ported).
- **Game Corner slot machines — 3 special machines missing**: ref has 36 `StartSlotMachine` hidden events (33 regular + `SLOTS_OUTOFORDER` (6,12), `SLOTS_OUTTOLUNCH` (13,12), `SLOTS_SOMEONESKEYS` (18,10)). Remake collapses all 33 regular machines into a single `openSlots()` via npc 4 (`maps/GameCorner/script.scene:76`) and has no out-of-order/out-to-lunch/someone's-keys texts.

### 4.2 Implemented / equivalent adaptations (44)
- PokemonMansion switches ×5 → coordEvents (`script_config.json`: Mansion1F [2,5] coordSwitch, Mansion2F, Mansion3F incl. holes, B1F ×2). ✓
- CinnabarGym quiz machines ×6 → talk-driven quiz guards (scene comment documents the adaptation). ✓
- GameCorner regular slot machines ×33 → single `openSlots()` NPC (all 33 machines collapsed; see 4.1 for the 3 special-state machines). ✓
- ViridianSchoolHouse blackboard, CeladonMansion2F/CeladonHotel/Safari/SilphCo11F PC texts — NOT implemented (see 4.1; keyword hits in scenes were dialogue-only false positives).
- BillsHouse extra sign (5,4): remake-added sign with no ref counterpart.

### 4.3 Item-ball mismatches (13 of 104)
ref `data/maps/objects/{Map}.asm` 7-arg `SPRITE_POKE_BALL, ..., ITEM` vs remake `giveItem` in `maps/{Map}/script.scene`:
- ref MtMoon1F (5,32) TM12 WATER_GUN — remake gives TM34 BIDE (`maps/MtMoon1F/script.scene:151`).
- ref PokemonMansionB1F (19,25) TM14 BLIZZARD — remake gives TM26 (`script.scene:105`); ref (5,4) TM22 SOLARBEAM — remake gives TM18 (`script.scene:92`).
- ref RocketHideoutB3F (26,17) TM10 DOUBLE_EDGE — remake gives TM44 (`script.scene:48`).
- ref SafariZoneEast (15,12) TM37 EGG_BOMB — remake gives TM39 (`script.scene:69`).
- ref SafariZoneWest (9,7) TM32 DOUBLE_TEAM — remake gives TM49 (`script.scene:39`).
- ref Route25 (22,2) TM19 SEISMIC_TOSS — remake gives TM39 (`script.scene:119`, scene comment repeats the wrong number).
- ref ViridianGym (16,9) REVIVE — no remake pickup.
- ref PowerPlant: CARBOS, HP_UP, RARE_CANDY, TM_THUNDER, TM_REFLECT (5 item balls) — remake PowerPlant scene has zero giveItem calls; all 5 missing.

### 4.4 Trainer class/set mismatches (4 of 346)
ref `data/maps/objects/{Map}.asm` 8-arg `..., OPP_CLASS, SET` vs remake `startBattle("OPP_CLASS{SET}")` (1-based set index, `trainer_data.rs:278`):
- ref SilphCo11F (6,9) OPP_GIOVANNI **2** (Silph Co. party) — remake `startBattle("OPP_GIOVANNI3")` = Viridian Gym party (`maps/SilphCo11F/script.scene:62`).
- ref Route24 (11,15) OPP_ROCKET **6** (L15 Ekans/Zubat) — remake `OPP_ROCKET7` = GameCorner party (`maps/Route24/script.scene:33`).
- ref GameCorner (9,5) OPP_ROCKET **7** (L20 Raticate/Zubat) — remake `OPP_ROCKET8` = Hideout B1F party (`maps/GameCorner/script.scene:179`).
- ref MtMoonB2F (12,8) OPP_SUPER_NERD **2** (L12 Grimer/Voltorb/Koffing) — remake `OPP_SUPER_NERD3` = Route 8 party (`maps/MtMoonB2F/script.scene:31`).
- All other scene-driven battles (rivals, Giovanni sets 1/3, Brock/Misty/LtSurge/Erika/Koga/Blaine/Sabrina/Lance, Voltorb/Electrode/Zapdos/Articuno/Moltres/Mewtwo wild battles) verified correct against ref scripts/parties.

### 4.5 Coord events
- 143 remake coordEvents (40 maps) cover the ref inline coord scripts: rival battles (Route22, CeruleanCity, SSAnne2F, PokemonTower2F, SilphCo7F), SilphCo card-key doors (ref `data/events/card_key_coords.asm` is documented UNUSED in the original; doors are per-map scripts, remake models them as cardKeyDoor coordEvents), Seafoam holes/currents, VictoryRoad switches, saffron/route gates, Museum ticket gate, Oak's lab, PewterCity/CinnabarIsland/ViridianCity gating. No missing ref coord-flow found.
- ref `data/events/hidden_events.asm` HiddenItems → all 54 ported via the generic engine table (see 1.2); HiddenCoins → missing (see 1.1).

---

## 5. Text

Target set: 118 maps (first 30 by map id + 8 gyms + 6 E4 rooms + 11 pokecenters + 16 marts + every trainer map). 915 ref text pointers checked, 145 sign pointers.

### 5.1 Missing dialogue entries (178 of 915)
- **Trainer defeat (end-battle) lines — 158 missing**: e.g. Route16 bikers "Don't you dare laugh!", "Knock out!", Route17 "Whoo!", Route18 "Tch!", Route19-21 swimmers/bikers/cooltrainers, plus gym trainers (Viridian/Celadon/Fuchsia/Cinnabar/SaffronGym), FightingDojo, RockTunnel, VictoryRoad, RocketHideout, SilphCo, PokemonMansion, Bruno, Agatha. These appear in ref `text/{Map}.asm` (`_XxxEndBattleText`) but nowhere in remake `map.json` text blocks, scenes, or `crates/pokered-data/trainers/*.json` (which store no dialogue). Full list: `/tmp/parity/text_report.md`.
- **Mart clerk greeting — 14 missing**: ref `_PokemartGreetingText` "Hi there! May I help you?" (from `data/items/marts.asm` clerk pointers) on 13 marts; remake mart scenes call `openShop(...)` directly with no greeting.
- **GameCorner rewording — 4**: ref "Oops! Your COIN CASE is full." → remake "But your COIN CASE is full." (`maps/GameCorner/script.scene`); "You don't need my coins!"; "You have lots of coins!"; "You've got your own coins!" — reworded/absent.
- **No-room-for-item branches — 2 missing**: ViridianGym Giovanni TM27 no-room text (`_ViridianGymGiovanniTM27NoRoomText`), SilphCo2F TM36 no-room text (`_SilphCo2FSilphWorkerFTM36NoRoomText`).
- **SilphCo5F scientist — reworded**: ref "We study POKe BALL technology on this floor." / "We worked on the ultimate POKé BALL which would contain anything!" — remake says "POKeMON BALL"/"catch anything".

### 5.2 Content mismatches (English)
- Only the GameCorner (4) and SilphCo5F (2) rewordings above; all other matched texts are content-equal modulo line-wrap (normalization: whitespace/punctuation-insensitive, `#MON`↔`POKeMON`).

### 5.3 Structural flows
- Nurse heal flows: all 12 nurse pointers have a heal flow in remake scenes (healParty/nurse storylines) — 0 structural mismatches.
- `yesorno` never occurs in pokered map texts, so the yes/no check is vacuous on the ref side; remake `@choice`/`@option` flows verified for vending machines, prize vendors, nurse, bike shop, game corner clerk (present).
- Sign texts: 145/145 covered (91 stored as `@t` in scenes rather than `map.json` `text.sign` blocks — storage-location difference, not missing content).
- 95 pointers skipped by design: engine-driven texts with no literal string (item PickUpItemText, elevator floor indicators, blank clipboard, PC/cable-club specials).
