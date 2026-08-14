# hidden_events parity audit — pret/pokered vs open-pokered (dotzuki)

Audit-only report. Ref: `/Users/liuyanghejerry/develop/pokered` (pret/pokered master). Remake: this repo.
Scope: items, moves, pokemon, hidden events/map triggers, dialogue text. Excluded: fonts/glyphs,
Chinese translation content, battle animation ids (already verified by `scripts/verify_battle_anim_data.py`),
intentional adaptations already documented in `docs/fidelity-audit-2026-08.md` (marked `[documented]` /
`[scene-documented]`). Comparator scripts: `/tmp/audit_items_domain.py`, `/tmp/audit_moves.py`,
`/tmp/audit_pokemon.py`, `/tmp/audit_obj_events.py`, `/tmp/audit_coord_events.py`, `/tmp/audit_text.py`.

## Summary counts

| Domain | Group | Result |
|---|---|---|
| items | item order/index (83+5HM+50TM) | 0 diffs — verified identical |
| items | item names (83) | 0 diffs; TM/HM display names missing (1 gap) |
| items | prices (83 + 50 TM + vending) | 0 diffs; sell/purchase wiring diverges (3 gaps) |
| items | key-item flags | data equal; `is_key_item()` helper misses 10 ref-TRUE entries |
| items | hidden items (54) | 0 diffs — verified identical |
| items | hidden coins (12) | **12 missing (whole feature absent)** |
| items | Game Corner prizes + mon levels | items/prices/levels identical; 1 cancel-label text diff |
| items | slot wheels | 0 diffs — verified identical |
| moves | count/order, power, type, accuracy, PP, effect id (165) | 0 diffs — verified identical |
| moves | effect-chance thresholds | **3 diffs** (POISON 52→51, 103→102; CONFUSION 25→26) |
| moves | field-move display table | **1 structural diff** (unused ANIM_B4 entry absent) |
| moves | TM/HM mapping | 0 diffs — verified identical |
| pokemon | stats/types/catch/exp/growth/level-1/TMHM/evos/learnsets/dex (151) | 0 diffs — full value parity |
| pokemon | structural | 2 notes (sprite-dims byte not stored; fossil/ghost placeholder ids) |
| hidden events | `[no-sign]` seed lines | 119: 10 implemented in scene, 1 wrong coords, **108 missing** |
| hidden events | item-ball object events (104) | 0 data errors |
| hidden events | trainer object events | **1 systematic party off-by-one + 4 wrong scripted parties + 4 rival triplets wrong** |
| hidden events | coord-event triggers (59 ref) | **7 gaps** |
| hidden events | bench guys / card-key | bench guys missing entirely; SilphCo11F door missing |
| text | covered maps (113) | 1 map missing dialogue entry; 2 maps missing branches; **13 English content mismatches**; 2 real + 3 engine-equivalent structural flows missing; 10 far-text targets absent (9 documented, 1 unreferenced in ref) |

---

# 1. Items

## 1a. Item order / index — verified identical
- ref `constants/item_constants.asm:9-96` (NO_ITEM $00, 83 items $01–$53) + HM/TM block `:140-210`
  (HM_CUT $C4…HM_FLASH $C8, TM_MEGA_PUNCH $C9…TM_SUBSTITUTE $FA) == remake
  `crates/pokered-data/data/items/item_list.json:3-19` → generated enum
  (`NoItem=0x00, MasterBall=0x01…MaxElixer=0x53, Hm01=0xC4…Tm50=0xFA`).
  Index-for-index identity; `NUM_ITEMS=0x53` (`crates/pokered-data/src/items.rs:16`).
- TM/HM move lists identical: ref `add_tm`/`add_hm` order == remake `TM_MOVES`
  (`crates/pokered-data/src/items.rs:261-312`) + `HM_MOVES` (`:314-320`).

## 1b. Names — 0 diffs on the 83 items; TM/HM display names missing
- All 83 ref `li "…"` strings (`data/items/names.asm:3-85`) match remake JSON `name` byte-for-byte
  (incl. `POKé BALL`, `OAK's PARCEL`, `?????` placeholders, duplicate `PP UP` for $32/$4F).
- **DIFF**: ref generates `TM01`…`TM50`/`HM01`…`HM05` machine names at runtime
  (`home/names.asm:51-91`); remake `get_item_data` returns `None` for ids ≥ $C4 so every TM/HM
  renders as `"---"` (`crates/pokered-data/src/lang_data.rs:332`) or `""`
  (`crates/pokered-core/src/items/use_engine.rs:162-164`).

## 1c. Prices — table equal (83/83); sell/purchase behavior diverges
- All 83 `bcd3` prices (`data/items/prices.asm:3-85`) == remake JSON `price` exactly.
- **DIFF**: ref sells any non-key/non-HM item incl. ¥0 items (MASTER_BALL, MOON_STONE, ETHER,
  MAX_ETHER, ELIXER, MAX_ELIXER, EXP_ALL, PP_UP — `engine/events/pokemart.asm:70-77`); remake
  `can_sell = !key && price > 0` (`crates/pokered-core/src/items/shop.rs:676-682`) blocks those 8.
- **DIFF**: ref allows selling TMs (only HMs blocked, `engine/events/pokemart.asm:76-77`); remake
  cannot sell any TM (`shop.rs:676-682` + `get_item_data` None for ≥ $C4).
- **DIFF (TM shop broken)**: ref CeladonMart2F clerk 2 sells 9 TMs (`data/items/marts.asm:25-26`);
  remake scene passes the same 9 ids (`crates/pokered-data/maps/CeladonMart2F/script.scene:17`) but
  price/name resolution returns `None` for TM ids → the TM counter renders empty
  (`crates/pokered-ui/src/menus/mart.rs:71-75`) and `try_buy` yields `InvalidItem`.
- TM_PRICES 50/50 equal (`data/items/tm_prices.asm:4-53` == `crates/pokered-data/src/item_data.rs:32-83`).
- Vending prices equal: ref `data/items/vending_prices.asm:8-10` (200/300/350) ==
  `CeladonMartRoof/script.scene:113-143,165-195,217-247`; drink heal amounts match
  (`engine/items/item_effects.asm:1078-1083` == `crates/pokered-data/src/impl_traits.rs:1826-1828`).

## 1d. Key items — data equal; enum helper misses 10 entries
- Ref `dbit TRUE` set (`data/items/key_items.asm:3-86`, 31 items) == remake JSON `key_item:true` set
  (authoritative runtime data — `ITEM_DATA`/`can_sell` read it).
- **DIFF ×10**: `ItemId::is_key_item()` (`crates/pokered-data/src/items.rs:74-99`) omits
  BOULDERBADGE…EARTHBADGE (8), ITEM_2C, SAFARI_BALL (ref `key_items.asm:23-30,46,9`).
  Badges compensated by `is_badge()` at all call sites (behavior OK); ITEM_2C unobtainable (latent);
  SAFARI_BALL treated as a normal ball by toss/consume (latent — never a bag item).

## 1e. Hidden items (54) — verified identical
- ref `data/events/hidden_item_coords.asm:8-61` (54 map/x/y) + per-map item assignment in
  `data/events/hidden_events.asm` (54 `HiddenItems` events) == remake
  `crates/pokered-data/src/hidden_items.rs:36-126` index-for-index (map, x, y, item, flag order),
  incl. entry 16 SAFARI_ZONE_GATE (inaccessible) and entry 34 UNUSED_MAP_6F. `[documented C7]`.

## 1f. Hidden coins (12) — MISSING entirely
- ref has 12 GAME_CORNER floor-coin spots: `data/events/hidden_coins.asm:8-19` (coords) +
  `data/events/hidden_events.asm:287-298` (amounts 10,10,20,10,10,20,10,10,10,40→20,100,10;
  the 40 spot degrades to 20 via the original engine bug `engine/events/hidden_items.asm:78-80`).
- remake has no floor-coin pickups at all: `GameCorner/script_config.json:3` `coordEvents: []`,
  `GameCorner/script.scene` implements only NPC coin gifts (10/20/20) and the ¥1000→50 exchange.
  Spots: (0,8)+10 (1,16)+10 (3,11)+20 (3,14)+10 (4,12)+10 (9,12)+20 (9,15)+10 (16,14)+10
  (10,16)+10 (11,7)+40 (15,8)+100 (12,15)+10.
- `obtained_hidden_coins` exists in the save (`crates/pokered-core/src/save/game_data.rs:15,301`)
  but nothing sets/reads it.

## 1g. Game Corner prizes + mon levels — equal; 1 text diff
- ref prizes (`data/events/prizes.asm:9-66`, RED branch): ABRA 180, CLEFAIRY 500, NIDORINA 1200,
  DRATINI 2800, SCYTHER 5500, PORYGON 9999, TM23 3300, TM15 5500, TM50 7700; levels
  (`data/events/prize_mon_levels.asm:1-9`): 9/8/17, 18/25/26 — all equal to remake
  `GameCornerPrizeRoom/script.scene:43-189`.
- **DIFF (text)**: ref cancel label `NoThanksText` "NO THANKS" (`prizes.asm:6-7`); remake vendor-3
  cancel option reads "CANCEL" (`GameCornerPrizeRoom/script.scene:184`).

## 1h. Slot wheels — verified identical
- ref `data/events/slot_machine_wheels.asm:1-59` (3×18 symbols) == remake
  `SLOT_MACHINE_WHEEL1/2/3` (`crates/pokered-data/src/slot_machine.rs:36-97`) symbol-for-symbol;
  reward table + bet/payout constants also equal (locked partially by
  `crates/pokered-data/tests/test_slot_machine.rs`).

## 1i. Category/effect mapping — 0 data diffs; 3 label notes
- Every item has a registered category + non-null effect; heal/revive/PP/vitamin/stone values match
  ref `engine/items/item_effects.asm`. Label-only notes: `DireHit.json` effect label `"protect"`
  (ref DIRE_HIT = Focus Energy), `XAccuracy.json` `"focus"` (ref USING_X_ACCURACY),
  `GuardSpec.json` `"guard_spec"` (ref PROTECTED_BY_MIST) — runtime behavior faithful via
  `impl_traits.rs:1847-1849` + `battle_items.rs:20-39`.

---

# 2. Moves

## 2a. Base data — verified identical (0 diffs)
- 165 moves, same order (`constants/move_constants.asm:8-173` == `build.rs` MOVE_ORDER
  `crates/pokered-data/build.rs:1142-1171`); power, type, accuracy, PP, effect id all 165/165 equal
  (ref `data/moves/moves.asm` vs `crates/pokered-data/moves/*.json`). `MoveEffect` enum values
  match `move_effect_constants.asm:6-94` (incl. the 0x48-0x4B/0x4E gaps); `PokemonType` matches
  `type_constants.asm` (incl. 0x09-0x13 gap, BIRD=0x06).

## 2b. Effect-chance thresholds — 3 diffs
Same derivation model both sides (chance hardcoded per effect id, `roll < threshold`), but:
- ref `POISON_SIDE_EFFECT1` threshold 52 (`engine/battle/effects.asm:101`, `20 percent + 1`) vs
  remake **51** (`crates/pokered-core/src/battle/effects/mod.rs:141`).
- ref `POISON_SIDE_EFFECT2` threshold 103 (`effects.asm:104`, `40 percent + 1`) vs remake **102**
  (`effects/mod.rs:142`).
- ref `CONFUSION_SIDE_EFFECT` threshold 25 (`effects.asm:1116`, `10 percent`, no +1 — Gen-1 quirk)
  vs remake **26** (`effects/mod.rs:200`).
- Internal inconsistency: remake Twineedle path uses 52 (`multi_hit_effects.rs:75`) but its own
  `PoisonSideEffect1` dispatcher arm uses 51. All other thresholds (burn/freeze/paralyze 26/77,
  flinch 26/77, stat-down 85) equal.

## 2c. Field-move display table — 1 structural diff
- ref `FieldMoveDisplayData` has 9 entries + `-1` terminator (`data/moves/field_moves.asm:1-14`):
  CUT, FLY, ANIM_B4 (unused), SURF, STRENGTH, FLASH, DIG, TELEPORT, SOFTBOILED.
- remake `FIELD_MOVE_TABLE` (`crates/pokered-core/src/overworld/hm_effects.rs:41-51`) has 8 — the
  unused **ANIM_B4 entry (name_idx 3, leftmost $0C)** is absent (unreachable in-game, structural
  only). All 8 real entries match move id + leftmost tile; the dedicated `FieldMoveNames` table
  (incl. its empty index-3 string) has no remake equivalent (names come from `lang_data::move_name()`).

## 2d. TM/HM mapping — verified identical
- TM01–TM50 (`item_constants.asm:161-210` == `items.rs:261-312`), HM01–HM05
  (`hm_moves.asm:5-9` == `items.rs:314-320`), combined order matches `data/moves/tmhm_moves.asm`.

---

# 3. Pokemon — full value parity (0 diffs)

All 151 species bit-identical on every compared field; existing tests
(`crates/pokered-data/tests/test_pokemon_data.rs`, `test_evos_moves.rs`) only locked spot checks
and counts — this audit was the first full 151×all-fields comparison.

- base stats (hp/atk/def/spd/spc), types, catch rate, base exp, growth rate, level-1 learnset
  (4 moves, order), TM/HM flags (55-bit, byte-for-byte incl. Mew `FF FF FF FF FF FF 7F`),
  evolutions (70 species; method/level/item/target/order incl. min-level 1), level-up learnsets
  (139 species; (level, move) sequence equality), dex-entry presence (151/151), party icon kinds
  (151/151) — **all 0 diffs**.
- Ref: `data/pokemon/base_stats.asm:3-152` + `base_stats/*.asm`, `evos_moves.asm`,
  `dex_entries.asm`, `names.asm`, `menu_icons.asm`. Remake: `crates/pokered-data/pokemon/*.json`,
  `src/pokemon_data.rs`, `src/evos_moves.rs`, `src/pokedex.rs`, `src/mon_party_icons.rs`.

Structural notes (no value impact):
- The ref's per-species sprite-dims byte (`INCBIN …pic, 0, 1`, e.g.
  `data/pokemon/base_stats/bulbasaur.asm:10`) has no field in the remake data layer
  (`crates/pokered-data/src/pokemon_data.rs:7-21`); the renderer derives size from the PNG at
  runtime (`crates/pokered-renderer/src/resource.rs:298-336`). Cross-checked ref vs remake PNG
  sizes: all 151 identical.
- Ref placeholder internal ids $B6–$B8 (FOSSIL_KABUTOPS / FOSSIL_AERODACTYL / MON_GHOST,
  `constants/pokemon_constants.asm:191-193`) have no remake species data (PNG assets only).
- Ref tables (names/evos_moves/dex_entries) are in scrambled internal order; remake stores in dex
  order — consistent permutation, engine adaptation, not a data diff.

---

# 4. Hidden events / map triggers

Seed: `python3 tools/audit_parity.py --ref /Users/liuyanghejerry/develop/pokered --domain maps`
(119 `[no-sign]` lines).

## 4a. Missing hidden events (ref `data/events/hidden_events.asm`)
- **GymStatues — 14 missing, all gyms** (ref `hidden_events.asm:168-169,178-179,196-197,214-215,
  245-246,312-313,317,333`). Remake gym maps have no statue sign/text at all: ViridianGym (15,15)/(18,15),
  PewterGym (3,10)/(6,10), CeruleanGym (3,11)/(6,11), VermilionGym (3,14)/(6,14), CeladonGym (3,15)/(6,15),
  FuchsiaGym (3,15)/(6,15), CinnabarGym (17,13), SaffronGym (9,15).
- **PrintBenchGuyText — 15 missing** (ref `hidden_events.asm:155,186,191,204,210,240,303,308,328,337,342,398,496,501,506`):
  all 11 PokéCenters (0,4), CeladonHotel (0,4), SafariZone West/East/North Rest House (0,4).
  `data/events/bench_guys.asm` (12-entry table) has zero remake counterpart.
- **OpenPokemonCenterPC — 9 non-PokéCenter PCs missing**: CeladonMansion2F (0,5), CeladonHotel (13,3),
  IndigoPlateauLobby (15,7), CinnabarLabFossilRoom (0,4)/(2,4), 3× SafariZone Rest House (13,3),
  SilphCo11F (10,12).
- **PrintTrashText — 3 missing**: VermilionGym (6,1) (remake has 15 trash cans but not the 16th at
  (6,1); ref `:216`), SSAnneKitchen (13,5)/(13,7) (ref `:371-372`).
- **StartSlotMachine — 36 missing** (ref `hidden_events.asm:250-285`): remake slots playable only via
  Beauty NPC `openSlots()` (`GameCorner/script.scene:62-76`); the 36 machine tiles have no trigger, and
  the 3 special-text machines are missing — (18,10) SLOTS_SOMEONESKEYS, (13,12) SLOTS_OUTTOLUNCH,
  (6,12) SLOTS_OUTOFORDER (ref `:256,258,270`).
- **HiddenCoins — 12 missing** (ref `hidden_events.asm:286-297`; see §1f).
- **House/lab flavor texts missing** (all ref → remake signs `[]` or scene-only NPCs):
  RedsHouse2F (3,5) PrintRedSNESText (`:138`); BluesHouse (0,1)/(1,1)/(7,1) PrintBookcaseText
  (`:142-144`); OaksLab posters (4,0)/(5,0) + email (0,1)/(1,1) (`:148-151`); ViridianSchoolHouse
  notebook (3,4) + blackboard (3,0) (`:163-164`); CeladonMansionRoofHouse blackboard (3,0)/(4,0) +
  notebook (3,4) (`:521-523`); Museum1F AerodactylFossil (2,3) / KabutopsFossil (2,6) (`:173-174`);
  MrFujisHouse magazines (0,1)/(1,1)/(7,1) (`:515-517`); FightingDojo texts ×4 (`:527-530`);
  Route15Gate2F left binoculars (1,2) (`:511`); BikeShop PrintNewBikeText ×6 (`:543-548`);
  IndigoPlateau HQ texts (8,13)/(11,13) (`:357-358`, marked inaccessible in ref).
- **Implemented via scene (10 — not gaps)**: PokemonMansion1F switch (2,5), PokemonMansion3F (10,5),
  PokemonMansionB1F (20,3)/(18,25) — all as coordEvents; CinnabarGym PrintCinnabarQuiz ×6 re-routed
  through the 7 guard NPC talk handlers (machine tiles not interactable; quiz wording rewritten —
  `[scene-documented]` `CinnabarGym/script.scene:6-17`).
- **Wrong coords (1)**: PokemonMansion2F switch — ref (2,11) (`hidden_events.asm:458`); remake
  coordEvents at (3,8) and (4,8) (`PokemonMansion2F/script_config.json:5-8`). Neither tile is the
  ref switch; the toggled door blocks (4,2)/(9,4)/(3,11) are correct.

## 4b. Item balls (7-arg object_events) — 0 data errors
All 104 ref item balls have a remake npc at the same (x,y) with matching textId and item.
The 27 TM balls store `itemId = ref id + 5` (remake internal convention Tm01=0xC9,
`build.rs:1208-1236`) — semantically correct. Cosmetic: `items.rs:8-11` comment mislabels the TM/HM
id blocks (says 0xC4 TMs / 0xF0 HMs; build.rs is authoritative).

## 4c. Trainer object events (8-arg) — wrong parties
- **Systematic sight-trainer off-by-one (all maps, not documented)**: ref set N = Nth party
  (1-based, `engine/battle/read_trainer_party.asm:25-33`). Remake stores the ref set verbatim in
  `map.json trainerSet` (`scripts/parse_npcs.py:185`) but
  `make_trainer_id` emits `OPP_X{set+1}` (`crates/pokered-data/src/trainer_data.rs:356-358`), used
  by the sight path (`crates/pokered-core/src/overworld/update.rs:867-869`) and party lookup
  (`game.rs:2008-2030`). Net: every `isTrainer=true` NPC battles with party **N+1**
  (e.g. Route3 Youngster1 ref OPP_YOUNGSTER,1 → remake parties[1]).
- **4 scripted battles use ref set+1**: Route24 Rocket ref OPP_ROCKET,6
  (`objects/Route24.asm:19`) vs remake `startBattle("OPP_ROCKET7")` (`Route24/script.scene:33`);
  GameCorner Rocket ref 7 (`objects/GameCorner.asm:36`) vs `"OPP_ROCKET8"`
  (`GameCorner/script.scene:179`); MtMoonB2F SuperNerd ref 2 (`objects/MtMoonB2F.asm:29`) vs
  `"OPP_SUPER_NERD3"` (`MtMoonB2F/script.scene:31`); SilphCo11F Giovanni ref 2
  (`objects/SilphCo11F.asm:22`) vs `"OPP_GIOVANNI3"` (`SilphCo11F/script.scene:62`).
- **4 rival battles use the base-triplet party** (`game.rs:2015-2024` resolves starter triplets 0-2):
  CeruleanCity (ref Rival1 7/8/9), Route22 1st (ref Rival1 4/5/6), Route22 2nd + PokemonTower2F
  (ref Rival2 10/11/12 and 4/5/6) — `[scene-documented]` limitation
  (`Route22/script.scene:19-20`). OaksLab/SSAnne2F/ChampionsRoom base triplets correct.
- The other 21 talk-driven leaders/E4 (of 25) have correct class/set — `[scene-documented]` pattern.
- def_trainers (flags/sight ranges/text ids) vs `trainer_headers.rs` + map.json textIds: 0 diffs.
- SaffronGym cosmetic: map.json `trainerClass:"Psychic"` string vs enum `PsychicTr` (no runtime impact).

## 4d. Coord-event triggers (59 ref across 26 maps) — 7 gaps
- Route24 (10,15) bridge auto-trigger (`scripts/Route24.asm:48-50`) — remake Rocket is talk-driven;
  player can walk past (`Route24/script.scene:12-35`).
- Route5Gate (3,3)/(4,3), Route6Gate (3,2)/(4,2), Route7Gate (3,3)/(3,4) guard approach
  auto-engage (`scripts/Route5Gate.asm:19-47` etc.) — remake guards talk-driven.
- SafariZoneGate (3,2)/(4,2) worker faces player on approach — remake talk-driven, no facing trigger.
- VermilionCity (19,29)/(19,31) sailor front/behind greeting branch
  (`scripts/VermilionCity.asm:195-197`) — remake `talkSailor1` has no position branch
  (main ticket gate at (18,30) IS ported).
- LancesRoom Lance-approach coords (5,1)/(6,2)/(5,11)/(24,16)
  (`scripts/LancesRoom.asm:79-85`) — remake keeps only (6,11) door lock; approach walk collapsed
  into the talk handler.
- SilphCo11F (6,13)/(7,12) Giovanni approach — `[scene-documented]` talk-driven.
- VictoryRoad1F (17,13) boulder-on-switch + VictoryRoad3F (23,15) boulder-into-hole chain —
  `[scene-documented]` approximations (`docs/fidelity-audit-2026-08.md` Wave 9 "链式谜题差异").
- Ported correctly (no diffs): SilphCo2F-10F doors, SilphCo7F rival, SSAnne2F rival,
  PokemonTower2F, CeruleanCity, Route22, E4 rooms, CeladonGym/Erika, VermilionDock, RockTunnelB1F.

## 4e. Card-key doors
- SilphCo 2F–10F doors implemented via coord triggers + replaceTileBlock + flags; door-block sets
  verified equal to ref (`SilphCo{2-10}F/script.scene`).
- **SilphCo11F door (block 3,6, EVENT_SILPH_CO_11_UNLOCKED_DOOR) MISSING** — ref
  `scripts/SilphCo11F.asm:17-28`; remake `SilphCo11F/script_config.json` coordEvents `[]`.

---

# 5. Dialogue text (113 covered maps: first 30 ids + 8 gyms + IndigoPlateauLobby + 11 PokéCenters +
all Marts + all 69 def_trainers maps)

## 5a. Maps with missing dialogue entries
- **VIRIDIAN_GYM text id 14**: ref has `You do not have space for this!`
  (`text/ViridianGym.asm:82`) for the TM27 bag-full branch; remake `giveItem("TM27", 1)` +
  `setFlag(...)` unconditional (`ViridianGym/script.scene:16,43`) with no failure branch.
  (A reworded generic exists at `Museum1F/map.json:232` key `3_BagFull` but is not wired here.)

## 5b. Missing branches of a text id
- **CINNABAR_GYM ids 2–8**: ref quiz-guard EndBattleTexts (`text/CinnabarGym.asm:77,97,114,130,146,167,183`)
  dropped in the documented quiz rewrite (`CinnabarGym/script.scene:6-17`); wrong-answer lines replace them.
- **SILPH_CO_2F id 1**: ref TM36 no-room branch `You don't have any room for this.`
  (`text/SilphCo2F.asm:29`, `scripts/SilphCo2F.asm:115`); remake `giveItem("TM36", 1)` unconditional
  (`SilphCo2F/script.scene:19`).

## 5c. English content mismatches (13)
- **Shared PokéCenter nurse script, 3 wording diffs on all 12 nurse maps** (e.g.
  `ViridianPokecenter/script.scene:9,14,23,30`): ref `Welcome to our POKeMON CENTER!` → remake
  `Welcome to the POKEMON CENTER!`; ref `OK. We'll need your POKeMON.` → `OK, we will need your POKEMON.`;
  ref `Thank you! Your POKeMON are fighting fit!` → `Thank you for waiting. Your POKEMON are fully healed!`
  (ref `data/text/text_4.asm:159,168,173,178,184`).
- **SILPH_CO_5F ×2**: ref `We study POKé BALL technology on this floor!`
  (`text/SilphCo5F.asm:29`) → remake `…POKeMON BALL…` (`SilphCo5F/script.scene:122`); ref
  `We worked on the ultimate POKé BALL…` (`:40`) → remake `…POKeMON BALL…` (`script.scene:120`).
- **CINNABAR_GYM ×7**: quiz battle texts get `\n\nQUIZ! Answer to open the gate!` appended
  (`script.scene:82,118,154,190,226,262,298`) — question wording preserved, structure changed
  (`[scene-documented]` rewrite).
- **ROCKET_HIDEOUT_B1F data bug**: ref `Why...?` (`text/RocketHideoutB1F.asm:1`); remake stores
  `"Why...?@"` — the ref `@` text-terminator leaked into the string
  (`RocketHideoutB1F/map.json:115` npc `endBattleText`).

## 5d. Missing structural flows
- **ROUTE_24**: ref forces a 1-step player walk before the NUGGET reward
  (`scripts/Route24.asm:38-42`); remake `giveItem("NUGGET", 1)` with no movement
  (`Route24/script.scene:26`).
- **LANCES_ROOM**: ref force-walks the player down the hallway to Lance
  (`scripts/LancesRoom.asm:100-105`); remake documents the approximation with no movement call
  (`LancesRoom/script.scene:8-9`) — `[scene-documented]`.
- Engine-equivalent (not gaps): ViridianGym + RocketHideoutB2F/B3F spinner-tile joypad sequences are
  reproduced by the remake engine's spinner tiles.
- Verified present everywhere: yes/no `@choice`, multi-page `\n\n`, mart `openShop`, nurse `heal()`,
  gym leader battle/badge/TM flows, all `def_trainers` battle/end/after texts.

## 5e. Base `text_far` targets
All far targets referenced from covered maps resolved in the remake except 10:
9 = the CinnabarGym quiz texts of §5b/§5c (documented rewrite) and 1 = `_SilphCo10FPorygonText`
(marked `unreferenced` in the ref, `scripts/SilphCo11F.asm:385`) — not a reachable-content gap.

## 5f. Notes
- CINNABAR_MART_COPY has no ref-side dialogue files (glitch-copy map; remake empty text block) — informational.
- Ref text-id numbering ≠ remake sign-key numbering on some city maps (all ref sign texts present
  with matching content — no missing entries).
