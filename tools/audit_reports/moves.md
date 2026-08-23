# Content parity audit — items · moves · pokemon · hidden events · text

Date: 2026-08-14 · Audit only, no fixes applied.
Reference: pret/pokered master (`<pret/pokered checkout>`, abbreviated **REF**).
Remake: open-pokered dotzuki engine (`<this repo checkout>`, abbreviated **RMK**).
Method: standalone comparators for items/moves/pokemon; hidden events seeded from
`tools/audit_parity.py --domain maps` (`[no-sign]` lines) and cross-checked per map against
`script.scene` / `script_config.json` / `hidden_items.rs`; text compared per-map from
`def_text_pointers` + `text/*.asm` vs `map.json` text blocks + scene `@t` English strings.
Exclusions applied: fonts/glyphs, Chinese strings, battle animation data (already 100% verified),
and deviations documented in `docs/fidelity-audit-2026-08.md` (listed per section as "documented").

## Summary

| Domain | Verified equal | Diffs (concrete) |
|---|---|---|
| Items | item order/index (83+5 HM+50 TM), 83 names, 83 prices, 50 TM prices, vending, key-item JSON set, guard drinks, hidden items 54/54, Game Corner prizes+levels, slot wheels | **26** (6 root causes) |
| Moves | 165/165 moves × effect/power/type/accuracy/pp, effect-id constants, field-move table, TM/HM mapping | **3** effect-chance thresholds |
| Pokemon | 151/151 species × all value fields (stats, types, catch rate, exp, growth, TM/HM, initial+level moves, evolutions, dex presence, icons) | **0** value diffs; 2 structural gaps |
| Hidden events | hidden items 54/54, item balls 106/106, trainers 346 (0 content gaps) | **119** `[no-sign]` events (103 missing / 9 approximated / 5 coord-adapted / 2 ref-inaccessible) + **12** hidden coins missing + 2 movement diffs |
| Text | 111 maps checked; 12 nurse heals, 12 marts, 9 yes/no maps, para/page structure all present | **9** missing entries (3 maps), **7** content mismatches (4 maps), **2** missing no-room flows |

≈ **182 concrete diff entries** across the five domains.

---

## 1. Items

### 1.1 Verified equal (0 diffs)

- Order/index: all 83 base items `$01–$53` + `HM01–05 $C4–C8` + `TM01–50 $C9–FA` index-identical
  (REF `constants/item_constants.asm:9-210` vs RMK `crates/pokered-data/data/items/item_list.json:3-19`
  → enum `crates/pokered-data/src/items.rs:16`). TM/HM move lists match
  (`items.rs:261-320` vs `item_constants.asm:141-210`).
- Names: all 83 `li "..."` strings byte-identical incl. `POKé BALL`, `OAK's PARCEL`, `?????`
  (SURFBOARD), duplicate `PP UP` (ITEM_32 vs PP_UP) — REF `data/items/names.asm:3-85`.
- Prices: all 83 bcd3 prices equal — REF `data/items/prices.asm:3-85`.
- TM/HM prices: 50/50 equal — REF `data/items/tm_prices.asm:4-53` vs RMK
  `crates/pokered-data/src/item_data.rs:32-83`.
- Vending: 200/300/350 — REF `data/items/vending_prices.asm:8-10` vs RMK
  `maps/CeladonMartRoof/script.scene:113-247`; drink heal amounts 50/60/80 equal.
- Key items: the 31-item JSON `key_item:true` set == REF dbit set
  (`data/items/key_items.asm:3-86`); runtime sellability follows the JSON set.
- Guard drinks: member set + priority equal (`data/items/guard_drink_items.asm:2-4` vs
  `maps/Route5Gate/script.scene:19-31`, `Route7Gate/script.scene:19-31`).
- Hidden items: **54/54 exact** (map id, x, y, item, order=flag index) — REF
  `data/events/hidden_item_coords.asm:8-61` vs RMK `crates/pokered-data/src/hidden_items.rs:36-126`;
  pickup wired at `crates/pokered-core/src/overworld/update.rs:921`.
- Game Corner prizes: RED tables equal (ABRA 180/L9, CLEFAIRY 500/L8, NIDORINA 1200/L17,
  DRATINI 2800/L18, SCYTHER 5500/L25, PORYGON 9999/L26, TM23 3300, TM15 5500, TM50 7700) — REF
  `data/events/prizes.asm:9-66` + `prize_mon_levels.asm:1-9` vs RMK
  `maps/GameCornerPrizeRoom/script.scene:43-189`.
- Slot wheels: 3×18 symbols + reward/flash tables equal — REF
  `data/events/slot_machine_wheels.asm:1-59` vs RMK `crates/pokered-data/src/slot_machine.rs:36-143`
  (and `tests/test_slot_machine.rs`).

### 1.2 Diffs

1. **Hidden coins — 12 floor-coin pickups entirely missing.**
   REF has 12 `HiddenCoins` spots in GAME_CORNER (`data/events/hidden_coins.asm:8-19` +
   `data/events/hidden_events.asm:287-298`): (0,8)+10, (1,16)+10, (3,11)+20, (3,14)+10,
   (4,12)+10, (9,12)+20, (9,15)+10, (16,14)+10, (10,16)+10, (11,7)+40→20 (original engine bug:
   `engine/events/hidden_items.asm:78-80` converts 40 to 20), (15,8)+100, (12,15)+10.
   RMK has no pickup path at all: `maps/GameCorner/script_config.json:3` `coordEvents: []`,
   the A-button chain (`crates/pokered-core/src/overworld/update.rs:882-940`) never consults
   coins, and `obtained_hidden_coins` (`crates/pokered-core/src/save/game_data.rs:301`) is
   ser/deser-only dead storage. (Also reported in §4.)

2. **TM/HM display names missing.**
   REF has runtime-generated names `TM01`…`TM50` / `HM01`…`HM05`
   (`home/names.asm:51-91`, `GetMachineName`); RMK has `get_item_data` return `None` for ids
   `≥ $C4`, so TM/HM names fall back to `"---"` (`crates/pokered-data/src/lang_data.rs:332`) or
   `""` (`crates/pokered-core/src/items/use_engine.rs:162-164`) — blank rows in bag/PC/mart UIs.

3. **CeladonMart2F TM counter is broken.**
   REF clerk 2 sells 9 TMs (`data/items/marts.asm:25-26`); RMK scene passes the same 9 items in
   the same order (`maps/CeladonMart2F/script.scene:17`), but shop resolution goes through
   `get_item_data` → `buy_price` (`crates/pokered-core/src/items/shop.rs:666-669`), which is
   `None` for TM ids → `mart.rs:71-75` renders no rows and `try_buy` returns `InvalidItem`.

4. **¥0 items are unsellable.**
   REF only guards `IsKeyItem`/`IsItemHM` on sell (`engine/events/pokemart.asm:70-77`), so
   MASTER_BALL, MOON_STONE, ETHER, MAX_ETHER, ELIXER, MAX_ELIXER, EXP_ALL, PP_UP (price 0, not
   key) are sellable for ¥0; RMK requires `price > 0`
   (`crates/pokered-core/src/items/shop.rs:676-682`) → all 8 return `Unsellable`.

5. **TMs are unsellable.**
   REF blocks only HMs on the sell path (`engine/events/pokemart.asm:76-77`) — TMs sell at half
   `TechnicalMachinePrices`; RMK `can_sell(Tm01..Tm50) == false` (same `shop.rs:676-682` +
   `item_data.rs:14-27`).

6. **`ItemId::is_key_item()` enum helper omits 10 ref key items.**
   REF marks BOULDERBADGE, CASCADEBADGE, THUNDERBADGE, RAINBOWBADGE, SOULBADGE, MARSHBADGE,
   VOLCANOBADGE, EARTHBADGE, ITEM_2C, SAFARI_BALL as key (`data/items/key_items.asm:23-30,46,9`);
   RMK `crates/pokered-data/src/items.rs:74-99` omits them. The 8 badges are compensated by
   `is_badge()` at call sites (`inventory.rs:27`, `impl_traits.rs:1898`, `pc_screen.rs:1112`);
   SAFARI_BALL is the only latent gap (unreachable as a bag item in practice); ITEM_2C is latent.

### 1.3 Documented deviations (excluded)

- C7 (`docs/fidelity-audit-2026-08.md:205-212`): hidden-item table + pickup semantics ported
  (verified equal in 1.1); "TUI does not seed flags" limitation.
- Prize vendor: RED branch chosen; confirm flow merged into menu choice
  (`GameCornerPrizeRoom/script.scene:1-22` header).
- Vending menus as scene `@choice` approximations (`CeladonMartRoof/script.scene:104-107`).
- Guard-drink priority via `hasItem`/`takeItem` + event flag (`Route5Gate`/`Route7Gate` headers).
- Itemfinder not detecting hidden coins matches the original (but does NOT excuse diff 1).

### 1.4 Test coverage note

`test_item_data.rs` locks order/prices shape + spot checks; `test_slot_machine.rs` locks wheel
numbers, not the full 54-symbol sequence. Not locked by any test: TM/HM names, the CeladonMart2F
TM shop, `is_key_item` set, hidden coins.

---

## 2. Moves

### 2.1 Verified equal (0 diffs)

- Count/order: 165 == 165, ids `0x01–0xA5` identical (REF `constants/move_constants.asm:9-173`
  vs RMK `crates/pokered-data/build.rs:1142-1171`).
- Per move (165/165): effect id, power, type, accuracy, pp all identical vs REF
  `data/moves/moves.asm`. Effect-id enum values (82 constants) identical
  (`move_effect_constants.asm:6-94` vs `crates/pokered-data/src/moves.rs:45-127`, incl. the
  `0x48-0x4B`/`0x4E` gaps).
- Field-move table: RMK `FIELD_MOVE_TABLE` (`crates/pokered-core/src/overworld/hm_effects.rs:41-51`)
  == the 8 real REF entries in order with identical leftmost tiles (`data/moves/field_moves.asm:1-14`).
- TM/HM move mapping: TM01-50 + HM01-05 identical (`item_constants.asm:141-210`,
  `data/moves/hm_moves.asm:5-9` vs `items.rs:261-320`).
- Note: neither side stores a per-move 7th byte — REF hardcodes side-effect chances per effect id
  (`engine/battle/effects.asm`); RMK does the same (`crates/pokered-core/src/battle/effects/mod.rs`).
  REF `FieldMoveDisplayData` has an inert `ANIM_B4` placeholder (`field_moves.asm:7`) and a
  `db -1` terminator with no RMK counterparts — structural only.

### 2.2 Diffs — 3 effect-chance thresholds off by one

1. **POISON_SIDE_EFFECT1**: REF poisons iff `rand < 52` (`engine/battle/effects.asm:101`,
   `ld b, 20 percent + 1`); RMK uses 51 (`crates/pokered-core/src/battle/effects/mod.rs:141`) —
   poisons 1/256 less often.
2. **POISON_SIDE_EFFECT2**: REF `rand < 103` (`effects.asm:104`, `40 percent + 1`); RMK uses 102
   (`effects/mod.rs:142`) — 1/256 less often.
3. **CONFUSION_SIDE_EFFECT**: REF `rand < 25` (`effects.asm:1116`, `cp 10 percent`, no `+1`);
   RMK uses 26 (`effects/mod.rs:200`) — confuses 1/256 **more** often.
   - RMK-internal inconsistency: the Twineedle poison path uses 52
     (`crates/pokered-core/src/battle/multi_hit_effects.rs:75`) while the PoisonSideEffect1
     dispatcher arm uses 51 — same ref effect id, two values.
   - Undocumented: not in `fidelity-audit-2026-08.md` nor `FIDELITY_GAPS.md`; the "bit-identical
     to legacy" comment at `crates/pokered-rules/rules.ron:137-138` only holds within the remake.

### 2.3 Test coverage note

`test_move_data.rs` locks count/order/ranges + 3 spot checks, not per-move parity;
`tests_hm_effects.rs:700-770` locks the field table; no test pins the 3 chance thresholds.

---

## 3. Pokemon

### 3.1 Verified equal — 0 value diffs across all 151 species

Base stats (5), types (incl. mono-type double encoding), catch rate, base exp, growth rate,
initial moves (level-1 ×4), TM/HM flags (7 bytes — Mew is `FF FF FF FF FF FF 7F` on both sides,
the trailing `UNUSED` token in REF `data/pokemon/base_stats/mew.asm:17-20` sets no bit), level-up
learnsets (139 species, `data/pokemon/evos_moves.asm` vs JSON `learnset`), evolutions (70 species:
level/item/trade, params, order — `evos_moves.asm` vs JSON `evolutions`), dex-entry presence
(names, height/weight, category/description pointers: 151/151), party icon kinds
(`data/pokemon/menu_icons.asm` vs `mon_party_icons.rs`). Ref internal-id vs remake dex-order
storage is an engine adaptation; decoded data is identical.

### 3.2 Structural gaps (no value diff)

1. **Sprite-dimensions byte not stored.**
   REF stores a dims byte per species (first byte of the front pic, e.g.
   `data/pokemon/base_stats/bulbasaur.asm:10`, `INCBIN ...,0,1`); RMK `BaseStats`
   (`crates/pokered-data/src/pokemon_data.rs:7-21`) has no dims field anywhere (JSON schema
   included) — the renderer derives size from the PNG
   (`crates/pokered-renderer/src/resource.rs:298-336`). Cross-checked: ref-vs-remake PNG tile
   dims agree for all 151, so no value is wrong, the byte just isn't modeled.
2. **Fossil/ghost placeholder species absent.**
   REF internal ids `$B6–$B8` = FOSSIL_KABUTOPS / FOSSIL_AERODACTYL / MON_GHOST
   (`constants/pokemon_constants.asm:191-193`); RMK has the PNG assets but no `Species`
   variants/data — fossil-revival/ghost events would need event-scoped handling.

### 3.3 Test coverage note

`test_pokemon_data.rs` locks 4 stat spot checks; `test_evos_moves.rs` locks 4 evo spot checks +
counts. Types/exp/growth/initialMoves/TM-HM bytes/learnset values/evo params for the other
~147 species were untested until this audit — now verified 0 diffs.

---

## 4. Hidden events / map triggers

Seed: `tools/audit_parity.py --domain maps` `[no-sign]` lines (hidden events whose coords have no
remake sign) — **119**. Resolution per map (scene + script_config + hidden_items + signs):

| Category | Count |
|---|---|
| MISSING (no remake implementation anywhere) | 103 |
| Approximated (talk-driven / reworded / ref-no-op) | 9 |
| Coord-event adaptation (step-on instead of A-face / moved tiles) | 5 |
| Inaccessible in ref (ref comments: unreachable) | 2 |
| Hidden coins (outside the 119; separate audit) | 12 missing |
| Hidden items | 54/54 exact |
| Item-ball object events (7-arg) | 106/106 exact |
| Trainer object events (8-arg) | 346: 304 exact, 42 adapted, 0 content gaps |

### 4.1 Missing hidden events (103)

- **14 gym statues** — REF `GymStatues` plaque texts ("<CITY> #MON GYM / LEADER: … /
  WINNING TRAINERS: …", `engine/events/hidden_events/gym_statues.asm:1-25`) at
  ViridianGym (15,15)/(18,15), PewterGym (3,10)/(6,10), CeruleanGym (3,11)/(6,11),
  VermilionGym (3,14)/(6,14), CeladonGym (3,15)/(6,15), FuchsiaGym (3,15)/(6,15),
  SaffronGym (9,15), CinnabarGym (17,13) — REF `data/events/hidden_events.asm:168-333`.
  RMK: no statue text/sign/coordEvent in any gym (`signs: []`, `coordEvents: []`).
- **12 bench-guy texts** — REF `PrintBenchGuyText` on A at (0,4)
  (`engine/events/hidden_events/bench_guys.asm:1-35`, `data/events/bench_guys.asm:5-21`) in the
  11 Pokécenters + CeladonHotel. RMK centers have only the PC sign at (13,3); no bench trigger.
- **9 playable PCs** — REF `OpenPokemonCenterPC` with no RMK sign:
  CeladonMansion2F (0,5), CeladonHotel (13,3), IndigoPlateauLobby (15,7), SilphCo11F (10,12),
  CinnabarLabFossilRoom (0,4)+(2,4), SafariZoneWest/East/NorthRestHouse (13,3)
  (`hidden_events.asm:236,302,534,560,538-539,497-507`).
- **36 GameCorner slot-machine A-interactions** — REF `StartSlotMachine` grid
  (`hidden_events.asm:250-285`), incl. 3 special machines: (6,12)=SLOTS_OUTOFORDER,
  (13,12)=SLOTS_OUTTOLUNCH, (18,10)=SLOTS_SOMEONESKEYS. RMK: `GameCorner/script_config.json:3`
  `coordEvents: []`, no machine signs; slots only reachable by talking to Beauty 2
  (`GameCorner/script.scene:76`); the three special-machine constants
  (`src/slot_machine.rs:152-154`) have zero references.
- **12 GameCorner hidden coin spots** — see §1.2 diff 1 (no pickup code at all).
- **32 flavor-text hidden events**, REF → RMK:
  - RedsHouse2F (3,5) SNES text ("<PLAYER> is playing the SNES!") — REF `hidden_events.asm:138`;
    RMK `RedsHouse2F/script.scene:3` "no NPCs, no signs, no items, no dialogue".
  - BluesHouse (0,1)/(1,1)/(7,1) bookcase texts — REF `:142-144`; RMK no signs.
  - OaksLab (4,0)/(5,0) posters + (0,1)/(1,1) e-mail — REF `:148-151`; RMK scene has no such texts.
  - ViridianSchoolHouse (3,4) notebook + (3,0) blackboard — REF `:163-164`; RMK scene has only
    the two student NPCs.
  - Museum1F (2,3)/(2,6) Aerodactyl/Kabutops fossil displays (sprite popup + text) — REF
    `:173-174`; RMK fossil lines are NPC talks only.
  - BikeShop 6 "A shiny new BICYCLE!" displays — REF `:543-548`; RMK clerk trade works, displays
    have no A-text.
  - VermilionGym (6,1) trash can ("Nope, there's only trash here.") — REF `:216`; RMK's 15
    trash-can signs cover only the GymTrashScript rows y=7/9/11.
  - SSAnneKitchen (13,5)/(13,7) trash cans — REF `:371-372`; RMK no trash signs.
  - CeladonMansionRoofHouse (3,0)/(4,0) blackboard + (3,4) TM notebook — REF `:521-523`; RMK
    scene has only Hiker + EEVEE ball.
  - MrFujisHouse (0,1)/(1,1)/(7,1) magazine texts — REF `:515-517`; RMK NPC dialogue only.
  - FightingDojo (3,9)/(6,9) "FIGHTING DOJO" + (4,0)/(5,0) wall texts — REF `:527-530`; RMK has
    the string only inside the master's trespassing line.
  - Route15Gate2F (1,2) left binoculars (Articuno cry + sprite + "shining bird" text) — REF
    `:511`; RMK has only the right binoculars sign at (6,2)
    (`Route15Gate2F/script.scene:56-61`).

### 4.2 Approximated (9)

- **CinnabarGym 6 quiz machines** (`PrintCinnabarQuiz`, REF `hidden_events.asm:319-324`): RMK
  folds the quiz into the 7 guard NPC talk handlers (`CinnabarGym/script.scene:6-17,72-310`);
  machine coords + quiz-intro text dropped, question wording partly rewritten. The scene header's
  claim that the original wording is "not recoverable" is wrong — it is in REF
  `data/text/text_2.asm` (see §5.1).
- **3 Safari-zone rest-house bench triggers** (REF `hidden_events.asm:496,501,506`): REF maps are
  absent from `BenchGuyTextPointers`, so the original prints nothing there — behaviorally
  equivalent omission.

### 4.3 Coord-event adaptations (5) — Pokemon Mansion secret switches

REF = A-press facing the tile; RMK = step-on `coordEvents`. PokemonMansion1F (2,5) ✓ tile +
facing gate (`script_config.json`, `scene:78-103`); Mansion3F (10,5) ✓; MansionB1F (20,3)/(18,25) ✓;
**Mansion2F bound to (3,8)/(4,8) instead of REF (2,11)** (`script.scene:77-100` — comment documents
binding to "the documented switch-block region"). Step-on semantics risk: switches unreachable if
the tile is collision-blocked.

### 4.4 Inaccessible in ref (2)

- IndigoPlateau (8,13)/(11,13) `PrintIndigoPlateauHQText` — REF `hidden_events.asm:357-358`,
  ref comments mark both unreachable behind the HQ building; RMK omits them — no observable diff.

### 4.5 Item balls / trainers / coord triggers

- **Item balls: 106/106 exact** — all 7-arg `object_event …, ITEM` lines (incl. TM/HM balls) match
  RMK map.json npcs at the same (x,y) with equal `itemId` (e.g. ViridianForest ANTIDOTE/POTION/
  POKE_BALL at (25,11)/(12,29)/(1,31)).
- **Trainers: 346 ref 8-arg object events, 0 content gaps.** 304 exact in map.json
  (`isTrainer`/`trainerClass`/`trainerSet`); 42 adaptations: 12 static legendaries via
  `startWildBattle` (PowerPlant 8 Voltorb/Electrode + Zapdos, Articuno, Moltres, Mewtwo),
  25 talk-driven bosses (class/set stored, `isTrainer=false`, `startBattle` from talk handler),
  1 class-less talk-driven Rocket (CeruleanCity TM28 thief,
  `CeruleanCity/script.scene:118-131`), 4 OPP_PSYCHIC_TR → "Psychic" naming-only.
  Event flags + sight ranges: 0 diffs (`trainer_headers.rs` vs `def_trainers`).
- **Coord triggers: ref has no `coord_event` blocks** (inline `wXCoord`/`wYCoord` checks);
  14 sites checked — 9 ported (PalletTown north exit, CinnabarIsland door, Museum1F gates,
  ViridianCity, SSAnne2F rival, CeruleanCity bridge, SilphCo5F/9F card doors ×28, Route22Gate,
  OaksLab), 2 talk-driven approximations (**MtMoonB2F Super Nerd (13,8)** and **Route23 badge
  checkpoints** — you can walk past without being stopped), 1 documented not-reproduced
  (SeafoamIslandsB3F forced-surf current, `scene:10-11`), spinner tiles engine-native.
- **2 movement diffs**: TradeCenter + Colosseum npc#0 (Red) — REF `STAY` → Stationary; RMK
  `Wander` (`TradeCenter/map.json`, `Colosseum/map.json`). Cable Club Red should stand still.

### 4.6 Documented deviations (excluded)

- C7: 54 hidden items (verified equal, §1.1). C2/E2: Pokécenter PC signs (13,3), RedsHouse2F PC
  (0,1), BillsHouse PC (1,4), cable-club desk invisible signs (5,4)/(4,4).
- VermilionGym 15 GymTrashScript cans as 15 signs (`VermilionGym/script.scene:50-130`).
- CinnabarGym quiz moved to guard dialogue; static legendaries via `startWildBattle`;
  Route23 badge checks approximated by event flags (scene headers document each).

---

## 5. Text (English structure)

111 maps checked (first 30 by id ∪ 8 gyms ∪ 7 elite-4/HoF ∪ 12 Pokécenters ∪ 12 marts ∪
all 69 trainer maps). Normalization: `é→e`, `#→poke`, whitespace/punctuation collapsed;
line-wrap differences treated as OK; Chinese strings ignored.

### 5.1 Missing dialogue entries (9 entries, 3 maps)

- **CinnabarGym — 7 end-battle defeat lines missing.** REF `text/CinnabarGym.asm:77-183`
  ("Yow! Hot, hot, hot!", "I surrender!", "Waah! My studies!", "Too hot to handle!", "Ow!",
  "Yowza! Too hot!", "Oh! Snuffed out!") have no RMK counterpart — the quiz-gate rework replaces
  them with authored wrong-answer lines. Pre-battle and after-battle lines ARE present.
- **ViridianGym — 1:** REF "You do not have space for this!" (`text/ViridianGym.asm:82`) missing;
  RMK `ViridianGym/script.scene:16,43` hands TM27 unconditionally.
- **SilphCo2F — 1:** REF "You don't have any room for this." (`text/SilphCo2F.asm:29`) missing;
  RMK `SilphCo2F/script.scene:19` hands TM36 unconditionally.

### 5.2 Content mismatches (7 lines, 4 maps)

- **CeruleanCity — sign texts swapped between positions.** REF (26,25) = MART sign
  (`data/maps/objects/CeruleanCity.asm:32`) vs RMK sign 3 at (26,25) shows "…BIKE SHOP"
  (`maps/CeruleanCity/map.json:400` + `script.scene:246`); REF (11,25) = BIKE SHOP sign
  (`objects/CeruleanCity.asm:34`) vs RMK sign 5 at (11,25) shows "…POKeMON MART"
  (`map.json:424` + `script.scene:260`).
- **SSAnne2FRooms — two gentlemen's dialogue swapped.** REF (21,2) says "Ah yes, I have seen
  some #MON ferry people…" (`text/SSAnne2FRooms.asm:11`) vs RMK npc 7 at (21,2) says the SAFARI
  ZONE line (`map.json:364`); REF (12,12) says the SAFARI ZONE line (`:23`) vs RMK npc 10 at
  (12,12) says the ferry line (`map.json:388`).
- **SilphCo5F — 2 lines:** REF renders "…POKé BALL…" (`text/SilphCo5F.asm:29,40` `# BALL`) vs
  RMK "…POKeMON BALL…" (`SilphCo5F/script.scene:120,122`).
- **Route1 — 1 line:** REF "<PLAYER> got POTION!" (`text/Route1.asm:15`, item name engine-printed)
  vs RMK "<PLAYER> got a POTION!" (`maps/Route1/script.scene:25`).

### 5.3 Missing structural flows

- **ViridianGym**: REF `.BagFull` branch for TM27 → missing (see 5.1).
- **SilphCo2F**: REF bag-full branch for TM36 → missing (see 5.1).
- **CinnabarGym**: REF = 7 sight-triggered battles; RMK = talk-driven yes/no quiz gates
  (documented in `CinnabarGym/script.scene:10-20`); end-battle lines dropped (see 5.1).

### 5.4 Verified present (no flags)

- Nurse heals: all 12 Pokécenter maps have `heal()` + `@choice` (ref `script_pokecenter_nurse`).
- Marts: all 12 have `openShop` (ref `script_mart`); Pokécenter PCs use `openPC()`.
- Yes/no: all 9 ref `YesNoChoice` maps have `@choice`; no lost `para`/`page` breaks in 111 maps.
- Item handouts: every ref `GiveItem` has ≥ the same number of `giveItem(...)` calls.
- Engine adaptation (content-equivalent): 10 ref dynamic item-name texts are hardcoded in RMK
  (TM41/TM24/TM21/TM06/TM38/TM36/NUGGET/MASTER BALL…).

### 5.5 Method caveat

64 ref cutscene text ids (gym "Received TM" / "No Room", Snorlax, SilphCo rival, elite-4 lines)
have no ref object anchor — matched to RMK by normalized content, so their 1:1 mapping is
best-effort. NPC/sign association assumes remake `npc i`/`sign i` = ref object order; the swap
detection pass found exactly the 2 swap bugs reported in 5.2.

---

## Top action items (severity order)

1. GameCorner: 36 slot-machine interactions + 12 hidden coin pickups (coins have zero pickup code).
2. 14 gym statues, 9 playable PCs, 12 bench-guy texts missing.
3. 3 battle effect-chance thresholds off by one (2× poison 1/256 low, 1× confusion 1/256 high).
4. CeladonMart2F TM counter broken (empty shop) + TM/HM items have no display names + TMs/¥0
   items unsellable.
5. ~32 flavor-text hidden events missing (OaksLab posters, BikeShop, FightingDojo, trash cans,
   fossils, SNES, binoculars…).
6. Text: CeruleanCity + SSAnne2FRooms swaps, 2 missing bag-full flows, CinnabarGym defeat lines.
7. Pokemon: sprite-dims byte not modeled; fossil/ghost placeholder species absent (both inert).
