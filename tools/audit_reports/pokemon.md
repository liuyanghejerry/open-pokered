# Pokémon Content Parity Audit — items / moves / pokemon / hidden_events / text

Date: 2026-08-14. Reference: pret/pokered master (`/Users/liuyanghejerry/develop/pokered`). Remake: open-pokered (this repo). Method: per-domain comparator scripts under `/tmp/pokemon_audit/<domain>/` (audit only; no files in either repo were modified). Excluded per scope: fonts/glyphs, Chinese text content, battle animation data (already verified 100%), and engine-architecture adaptations documented in `docs/fidelity-audit-2026-08.md`.

## Summary counts

| Domain | Findings | Breakdown |
|---|---|---|
| items | **12** | 10 key-item classifier mismatches, 2 not-ported data sets (hidden coins, BLUE prize lists) |
| moves | **3** | 3 effect-chance thresholds off by 1 (PoisonSting, Smog/Sludge, Psybeam/Confusion) |
| pokemon | **0 diffs** | all 151 species value-identical on every field; 4 structural notes |
| hidden_events | **98** | 86 missing/not-ported triggers (70 hidden events, 12 coin spots, 3 special slots, 1 card-key door) + 12 coord-script gaps (11 missing, 1 wrong position) |
| text | **23** | 7 missing entries (2 maps), 11 content mismatches (7 maps), 1 missing TM bag-full branch, 4 global-text findings; 0 core-flow gaps; 19 paging-fidelity notes |

**Total substantive findings: 136** (12 items + 3 moves + 86 hidden-event triggers + 12 coord scripts + 7 missing texts + 11 content mismatches + 1 branch + 4 global texts), plus 4 pokemon structural notes and 19 paging notes.

---

## items

### Index/order parity — OK

- Ref `constants/item_constants.asm` const sequence vs remake `crates/pokered-data/data/items/item_list.json` + generated `OUT_DIR/items_gen.rs` discriminants: base items `$01–$53` identical in order (spot-checks: `POTION`=$14, `BOULDERBADGE`=$15, `MAX_ELIXER`=$53).
- Fixed ids: ref `HM01=$C4`…`HM05=$C8`, `TM01=$C9`…`TM50=$FA` == generated enum `Hm01=0xC4`, `Tm01=0xC9`, `Tm50=0xFA`.
- TM/HM move mapping: ref `add_tm`/`add_hm` order == `crates/pokered-data/src/items.rs` `TM_MOVES` (50) / `HM_MOVES` (5).
- Info (not a diff): ref `FLOOR_B2F`–`FLOOR_B4F` ($54–$61) have no remake `ItemId` variants — elevator floors are not modeled as item ids (`NUM_ITEMS = 0x53`).

### Names / prices / TM prices / vending / guard drink / hidden items / RED prizes / slot wheels — all OK (0 diffs)

- NAMES: all 83 display names identical, exact (é and apostrophes preserved in remake, e.g. `POKé BALL`, `OAK's PARCEL`).
- PRICES: all 83 `bcd3` prices match JSON `price` (e.g. POTION 300 `data/items/prices.asm:22` = `Potion.json:1`; NUGGET 10000 = `Nugget.json`).
- TM_PRICES: ref `data/items/tm_prices.asm` `nybble N`×1000 == `crates/pokered-data/src/item_data.rs:33-82` `TM_PRICES` (50/50).
- VENDING: ref `data/items/vending_prices.asm` (FRESH WATER 200 / SODA POP 300 / LEMONADE 350) == `crates/pokered-data/maps/CeladonMartRoof/script.scene:113-144` (all 3 vending signs).
- KEY_ITEMS primary check: ref `data/items/key_items.asm` `dbit` flags == remake `data/items/*.json` `key_item` field for all 83 items (31 TRUE flags match).
- GUARD_DRINK: ref `data/items/guard_drink_items.asm` (FRESH WATER → SODA POP → LEMONADE) == all four Saffron gate scenes (`Route5Gate/script.scene:19`, `Route6Gate:17`, `Route7Gate:19`, `Route8Gate:33,45,56`) + roof girl `CeladonMartRoof/script.scene:38`.
- HIDDEN_ITEMS: all 54 entries identical (map, x, y, item, order=flag index). Ref `data/events/hidden_item_coords.asm` + `hidden_events.asm` == `crates/pokered-data/src/hidden_items.rs` (spot: entry 0 VIRIDIAN_FOREST (1,18) POTION = `hidden_items.rs:38`; entry 53 ROUTE_4 (40,3) GREAT_BALL = `hidden_items.rs:125`).
- PRIZES RED: 6 mon prizes + 3 TM prizes with prices/levels identical (`GameCornerPrizeRoom/script.scene` vendors 1–3 vs `data/events/prizes.asm` RED + `prize_mon_levels.asm` RED: ABRA L9/180 … PORYGON L26/9999, TM23/3300, TM15/5500, TM50/7700).
- SLOT_WHEELS: 3 wheels × 18 symbols identical (`data/events/slot_machine_wheels.asm` == `crates/pokered-data/src/slot_machine.rs` `SLOT_MACHINE_WHEEL1/2/3`); payouts match `engine/slots/slot_machine.asm` `SlotRewardPointers` (300/100/8/15/15/15); consistent with `crates/pokered-data/tests/test_slot_machine.rs`.

### KEY_ITEMS — 10 classifier mismatches

Primary data (JSON `key_item`) is correct; the hand-written bag classifier disagrees with both ref and the remake's own JSON (likely intentional — badges/Safari Ball managed outside the bag — but the two layers diverge):

- `ref data/items/key_items.asm:10 has dbit TRUE for SAFARI_BALL, remake crates/pokered-data/src/items.rs:74 is_key_item() excludes SafariBall`
- `ref data/items/key_items.asm:23-30 has dbit TRUE for BOULDERBADGE/CASCADEBADGE/THUNDERBADGE/RAINBOWBADGE/SOULBADGE/MARSHBADGE/VOLCANOBADGE/EARTHBADGE, remake items.rs:74 is_key_item() excludes all 8 badges`
- `ref data/items/key_items.asm:46 has dbit TRUE for ITEM_2C, remake items.rs:74 is_key_item() excludes Unused2C`

### HIDDEN_COINS — not ported

- `ref data/events/hidden_coins.asm has 12 Game Corner hidden coin spots (amounts 10/20/40/100; data/events/hidden_events.asm:287-298), remake has no hidden-coin coordinate/amount data — only the SRAM-format flag bytes obtained_hidden_coins (crates/pokered-core/src/save/game_data.rs) exist for save compat; they are never written in gameplay.`

### PRIZES — BLUE lists not ported

- `ref data/events/prizes.asm IF DEF(_BLUE) blocks and data/events/prize_mon_levels.asm BLUE block (NIDORINO, PINSIR, different prices/levels) have no remake counterpart — GameCornerPrizeRoom/script.scene implements RED only.`

---

## moves

### ORDER — OK (0 diffs)

- `constants/move_constants.asm` ids 1..165 (POUND..STRUGGLE) == remake `MOVE_ORDER` (`crates/pokered-data/build.rs:1142-1171`) and the 165 `moves/*.json` filenames; every JSON `id` matches its filename.

### MOVE_DATA — OK (0 diffs)

- All 165 rows identical for effect, power, type, accuracy, pp (`data/moves/moves.asm` vs `crates/pokered-data/moves/<Move>.json`). `MoveEffect` enum ids (`src/moves.rs:45-128`) match `constants/move_effect_constants.asm` ($00-$47, $4C, $4D, $4F-$56; unused ids omitted on both sides).
- Note: this checkout's `move` macro has only 6 args (no per-move chance byte); effect chance is hardcoded per effect in `engine/battle/effects.asm` — the remake stores it the same way (per effect, 256-based thresholds).

### EFFECT_CHANCE — 3 off-by-one thresholds (ref roll bound is `BattleRandom < b`, 256-based)

- `ref engine/battle/effects.asm:101 has ld b, 20 percent + 1 (=52) for POISON_SIDE_EFFECT1, remake crates/pokered-core/src/battle/effects/mod.rs:141 has 51` → POISON_STING triggers at 19.9% vs ref 20.3%.
- `ref engine/battle/effects.asm:104 has ld b, 40 percent + 1 (=103) for POISON_SIDE_EFFECT2, remake mod.rs:142 has 102` → SMOG, SLUDGE trigger at 39.8% vs ref 40.2%.
- `ref engine/battle/effects.asm:1116 has cp 10 percent (=25) for CONFUSION_SIDE_EFFECT, remake mod.rs:200 has 26` → PSYBEAM, CONFUSION trigger at 10.2% vs ref 9.8%.
- Verified identical (12 effects): PARALYZE/BURN/FREEZE/FLINCH side effects 1/2 = 26/77 (`mod.rs:143-148, 195-196`), stat-down side effects = 85 (`stat_effects.rs:52`), Twineedle's roll 52/256 (`multi_hit_effects.rs:75`; only its line-74 comment "20% = 51/255" is stale).

### FIELD_MOVES — 8/8 real entries OK, 1 structural note

- CUT($0C), FLY($0C), SURF($0C), STRENGTH($0A), FLASH($0C), DIG($0C), TELEPORT($0A), SOFTBOILED($08) — same order and leftmost tiles: ref `data/moves/field_moves.asm` == `crates/pokered-core/src/overworld/hm_effects.rs:41-51`.
- `ref data/moves/field_moves.asm:7 has a 9th placeholder entry ANIM_B4, remake drops it` — unreachable in the original too (no move id can equal $B4), behaviorally identical.
- Badge gating matches per move (Cut=Cascade, Fly=Thunder, Surf=Soul, Strength=Rainbow, Flash=Boulder).

### FIELD_MOVE_NAMES / HM_MOVES / TMHM_MOVES — OK

- 8/8 field-move display names identical (`data/moves/field_move_names.asm` == `crates/pokered-data/src/lang_data.rs:183-222`); ref name slot 2 is `"@"` paired with ANIM_B4, dropped with it.
- `data/moves/hm_moves.asm` (CUT, FLY, SURF, STRENGTH, FLASH) == `items.rs:314-320` `HM_MOVES`.
- `constants/item_constants.asm` add_tm/add_hm sequences == `TM_MOVES` (50, `items.rs:261-312`) + `HM_MOVES` (5) — 55/55 exact.

---

## pokemon

**All 151 species are value-identical on every compared field: 0 diffs.** Per-group results (ref `data/pokemon/base_stats/*.asm`, `data/pokemon/mew.asm`, `data/pokemon/evos_moves.asm`, `data/pokemon/dex_entries.asm`/`dex_text.asm`, `data/pokemon/menu_icons.asm` vs remake `crates/pokered-data/pokemon/*.json`, `src/pokemon_data.rs`, `src/evos_moves.rs`, `src/pokedex.rs`, `src/mon_party_icons.rs`):

- ORDER: 151/151 (ref `base_stats.asm:3-152` INCLUDE list + Mew == remake `build.rs:1112-1139` `SPECIES_ORDER`, dex order; 151 JSONs, filename == `species`).
- BASE_STATS (hp/atk/def/spd/spc): 0 diffs, all 151.
- TYPES (type1/type2): 0 diffs (incl. `PSYCHIC_TYPE`→`Psychic`).
- CATCH_RATE / BASE_EXP: 0 diffs, all 151.
- GROWTH_RATE: 0 diffs (ref `GROWTH_*` 0..5 == enum `GrowthRate`, `src/species.rs:81-88`).
- TMHM_FLAGS: 0 diffs — all 151 × 7 bytes byte-identical, including Mew (`data/pokemon/base_stats/mew.asm:17-28` = `FF FF FF FF FF FF 7F` == `Mew.json` `[255,255,255,255,255,255,127]`; the trailing `UNUSED` token sets no bit on either side).
- INITIAL_MOVES (level-1 learnset): 0 diffs, all 151.
- LEARNSETS: 0 diffs, all 139 species with level-up moves ((level, move) sequences exact, ascending).
- EVOLUTIONS: 0 diffs, all 70 evolving species (method/level/item/target exact, incl. trade and item evos; min level 1 for item/trade on both sides).
- EVO_MOVES: no data table on either side (both derive from the evolved form's learnset).
- POKEDEX: 0 diffs — category/height/weight present and matching for all 151; 2 flavor-text pages each (presence + normalized English text identical 151/151).
- ICONS: 0 diffs — `menu_icons.asm` 151 kinds == `mon_party_icons.rs` `MON_PARTY_DATA`.

Structural notes (no value impact):

- `ref constants/pokemon_constants.asm:8-199 stores species tables in scrambled internal-id order (RHYDON=$01..VICTREEBEL=$BE), remake stores everything in dex order (Bulbasaur=1..Mew=151)` — engine adaptation, same per-species data.
- `ref data/pokemon/base_stats/*.asm:10 stores a per-species sprite-dims byte (INCBIN first byte), remake stores no dims anywhere (BaseStats in src/pokemon_data.rs:7-21, build.rs:827-906, pokemon.schema.json) and derives size from PNGs at runtime (crates/pokered-renderer/src/resource.rs:298-336)` — all 151 PNG sizes still match the ref byte values; not documented in `docs/fidelity-audit-2026-08.md`.
- `ref pokemon_constants.asm:191-193 has event placeholder species FOSSIL_KABUTOPS/FOSSIL_AERODACTYL/MON_GHOST ($B6-$B8), remake has no species data for them` (only PNG assets exist).
- `ref data/pokemon/dex_order.asm (pokedex display-order map) and data/icon_pointers.asm (per-icon-kind frame table) have no remake counterparts` (dex display order is trivial in the remake; icon frames not needed by its renderer).

Test coverage note: `tests/test_pokemon_data.rs` and `test_evos_moves.rs` lock only presence/order/spot checks (4 stats, 4 evos); everything else (stats/types/exp/growth/tmhm/learnsets/evolutions for the other ~147 species, pokedex 149 species) was verified by this audit — all parity OK.

---

## hidden_events

### (a) Missing / not-ported triggers — 86

Gym statues — 14 (ref `hidden_event x,y GymStatues` examine-text "<CITY> POKéMON GYM / LEADER… / WINNING TRAINERS…" not ported anywhere; grep "WINNING TRAINERS" finds nothing):

- `ref hidden_events.asm has GymStatues at ViridianGym (15,15),(18,15); PewterGym (3,10),(6,10); CeruleanGym (3,11),(6,11); VermilionGym (3,14),(6,14); CeladonGym (3,15),(6,15); FuchsiaGym (3,15),(6,15); CinnabarGym (17,13); SaffronGym (9,15), remake has no signs/coordEvents/statue text in any of these maps' script.scene/map.json`

Pokecenter bench guy — 12 (ref `hidden_event 0,4 PrintBenchGuyText`):

- `ref has bench-guy text at ViridianPokecenter, PewterPokecenter, CeruleanPokecenter, MtMoonPokecenter, RockTunnelPokecenter, VermilionPokecenter, CeladonPokecenter, LavenderPokecenter, FuchsiaPokecenter, CinnabarPokecenter, SaffronPokecenter, CeladonHotel (0,4), remake has no bench interactable in any of them`

Safari Zone rest houses — 6 (bench guy + PC, per house):

- `ref has PrintBenchGuyText (0,4) + OpenPokemonCenterPC (13,3) in SafariZoneWestRestHouse, SafariZoneEastRestHouse, SafariZoneNorthRestHouse, remake rest-house scenes are NPC-dialogue only, no signs`

PCs outside pokecenters — 6:

- `ref has OpenPokemonCenterPC at CeladonMansion2F (0,5), CeladonHotel (13,3), CinnabarLabFossilRoom (0,4),(2,4), IndigoPlateauLobby (15,7), SilphCo11F (10,12), remake has none of these (e.g. CeladonHotel map.json signs: [])`

House furniture hidden texts — 28:

- `ref has PrintBookcaseText at BluesHouse (0,1),(1,1),(7,1), remake has none`
- `ref has PrintRedSNESText at RedsHouse2F (3,5), remake has only the PC sign at (0,1)`
- `ref has DisplayOakLabLeftPoster (4,0), DisplayOakLabRightPoster (5,0), DisplayOakLabEmailText (0,1),(1,1) in OaksLab, remake has none`
- `ref has PrintNotebookText (3,4) + PrintBlackboardLinkCableText (3,0) in ViridianSchoolHouse, remake has only an NPC line quoting the blackboard — not interactable`
- `ref has PrintBlackboardLinkCableText (3,0),(4,0) + PrintNotebookText (3,4) in CeladonMansionRoofHouse, remake has none`
- `ref has PrintMagazinesText (0,1),(1,1),(7,1) in MrFujisHouse, remake has none`
- `ref has AerodactylFossil (2,3) + KabutopsFossil (2,6) in Museum1F, remake mentions fossils only in NPC dialog — no signs`
- `ref has PrintNewBikeText ×6 (1,0),(2,1),(1,2),(3,2),(0,4),(1,5) in BikeShop, remake has only the clerk dialog — no bike interactables`
- `ref has PrintFightingDojoText (3,9),(6,9) + PrintFightingDojoText2/3 (4,0),(5,0) in FightingDojo, remake has the Karate Master talk only`

Trash cans — 3:

- `ref has PrintTrashText at SSAnneKitchen (13,5),(13,7), remake has no trash interactables`
- `ref has PrintTrashText at VermilionGym (6,1) (decorative can, distinct from the 15 puzzle cans), remake has signs only at the 15 GymTrashScript positions — the (6,1) can is missing`

Binoculars — 1:

- `ref has Route15GateLeftBinoculars at Route15Gate2F (1,2) (Articuno sprite + cry easter egg), remake has only the right-window sign at (6,2)`

Game Corner hidden coins — 12:

- `ref hidden_events.asm GameCorner block has HiddenCoins at (0,8)+10, (1,16)+10, (3,11)+20, (3,14)+10, (4,12)+10, (9,12)+20, (9,15)+10, (16,14)+10, (10,16)+10, (11,7)+40, (15,8)+100, (12,15)+10, remake has no floor-coin pickup anywhere (coins only via NPC one-time gifts and the ¥1000 exchange)`

Game Corner special slot machines — 3:

- `ref has StartSlotMachine with SLOTS_SOMEONESKEYS (18,10), SLOTS_OUTTOLUNCH (13,12), SLOTS_OUTOFORDER (6,12), remake slot minigame is uniform via openSlots() (Beauty 2) — special-machine messages not ported`

Card-key door — 1:

- `ref has SilphCo11F card-key door (SilphCo11GateCoords dbmapcoord 3,6, engine special-case, EVENT_SILPH_CO_11_UNLOCKED_DOOR), remake SilphCo11F has coordEvents: [] and the flag is never set/read — door to the president's office not implemented (the other 10 SilphCo floors are OK)`

(Accepted skip: IndigoPlateau `PrintIndigoPlateauHQText` (8,13),(11,13) — ref marks both "inaccessible"; remake documents the empty map.)

### (b) Wrong item/trainer data — 0

- Item balls: 104/104 match (remake npc `itemId` == ref item index; 77 plain items + 27 TM/HM balls, e.g. MtMoonB2F TM_MEGA_PUNCH→201, PowerPlant TM_THUNDER→225).
- Species-disguised balls (VOLTORB/ELECTRODE/ZAPDOS/ARTICUNO/MOLTRES/MEWTWO) are scripted wild battles with correct levels + `EVENT_BEAT_*` flags — correct adaptation of ref 7-arg object events with a species as arg7.
- Trainers (8-arg): 25/25 scripted-battle npcs keep exact `trainerClass` + `trainerSet` (pascal of `OPP_*` + set no.; `OPP_PSYCHIC_TR`→`Psychic`).
- Nit: `ref CeruleanCity object event (30,8) OPP_ROCKET,5, remake npc#1 has no trainerClass/trainerSet fields (battle lives entirely in scene startBattle("OPP_ROCKET5"), script.scene:119-130)` — behavior present, npc data field absent.

### (c) Coord-event scripts not ported — 11 missing + 1 wrong position

- `ref VictoryRoad3F .SwitchOrHoleCoords (23,15) warps to VictoryRoad2F (wDungeonWarpDestinationMap), remake has only switch1 (3,5); scene comment acknowledges the hole but leaves it unwired — falling through not implemented`
- `ref SeafoamIslandsB3F forced-surf current checks (15,8) + (18/19) (DecodeRLEList), remake scene documents "not reproduced" — deliberate omission (hole warps are implemented)`
- `ref MtMoonB2F walk-up auto-text at (13,8) (SuperNerd dialog opens on approach), remake SuperNerd is talk-driven; MtMoonB2F has no coordEvents`
- `ref FightingDojo (4,3) step-trigger turns the Karate Master to face the player, remake master is talk-driven`
- `ref LoreleisRoom/BrunosRoom/AgathasRoom entrance coords include row 11 (4,11),(5,11) per room, remake has only (4,10),(5,10) — the second tile row per room is uncovered`
- `ref LancesRoom LanceTriggerMovementCoords (5,1),(6,2),(5,11),(24,16) + forced-walk hallway (DecodeRLEList), remake documents the forced walk as approximated — entrance lock at (6,11) IS implemented`
- `ref PokemonTower7F NPCCoordMovementTable (3 Rockets walk away/hide on approach), remake documents the choreography as approximated by rescue dialogue + flags — movement not ported`
- `ref Route23 walk-up badge gates (Route23GuardsYCoords auto stop/push-back + Victory Road bypass), remake gates are talk-driven (badges approximated by EVENT_BEAT_* flags)`
- `ref GameCorner wXCoord==8 rocket-approach movement branch, remake rocket is talk-driven scripted battle + hideObject`
- `ref MtMoonB2F MtMoonB2FPlayerNearDome/HelixFossilCoords (SuperNerd walks to the unchosen fossil), remake hides the other fossil instead`
- `ref OaksLab wXCoord==9 / wXCoord==4 Blue movement variants during the starter sequence, remake covers the exit gate (dontGoAway1-3) but simplifies the movement variants`
- WRONG POSITION: `ref PokemonMansion2F switch hidden event at (2,11) (hidden_events.asm:458, map data byte-identical), remake binds mansionSwitch triggers to (3,8),(4,8) (script_config.json) — 3+ tiles from the switch tile; the scene toggles the correct door blocks (replaceTileBlock (4,2)/(9,4)/(3,11)), only the trigger location is wrong. Other mansion floors use exact ref coords (1F (2,5), 3F (10,5), B1F (20,3),(18,25))`

Engine-implemented (ref script checks now live in the engine — counted OK): MtMoonB2F no-battles zone, ViridianGym/RocketHideout arrow tiles, Seafoam hole warps, PokemonMansion3F holes, VictoryRoad2F switches + VR3F switch (boulder approximated), VR1F switch (talkBoulder1 approximation).

### (d) Text ids missing — 0

- All 1206 ref `def_text_pointers` ids checked; object/sign-referenced ids: 0 missing. 7 apparent gaps were false positives: 6 item-ball pickups use the engine-generic "<PLAYER> found X!" text (same as ref's shared `PickUpItemText`) and HallOfFame's Oak text lives in the on-enter cutscene. 57 script-driven ids were deferred to the text audit, which found 7 actually missing (see below).

### (e) OK / adaptations (verified identical)

- Hidden items 54/54 (`src/hidden_items.rs` == ref, incl. the 2 intentionally inaccessible entries).
- Trainer headers: all maps match `src/trainer_headers.rs` (count, event flag, sight range) — 0 diffs.
- Pokecenter PCs → signs (12 maps); RedsHouse2F/BillsHouse PCs → signs; TradeCenter/Colosseum Cable Club gameboys → signs.
- VermilionGym 15-can trash puzzle (signs 1-15 + scene port incl. documented RNG approximation).
- GameCorner 36 slot machines → openSlots() adaptation; poster switch → replaceTileBlock + posterSign.
- CinnabarGym 6 quiz machines → talk-driven quiz on 7 guard NPCs (mechanic + flags reproduced; original wording not used — see text).
- Card-key doors SilphCo2F–10F: all coordEvent triggers at 2×ref block coords with CARD_KEY gates and UNLOCKED_DOOR flags (10/11 floors; 11F missing).

---

## text

Representative set: **107 maps** (first 30 by map id + all 8 gyms + 7 elite-four/champion/HoF + 11 pokecenters + 11 marts + all trainer-NPC maps). **1296 ref text entries compared; 1264 exact content matches (97.5%); 99/107 maps fully covered.** Normalization: whitespace collapse, é→e, curly quotes, `#`→POKe, `<PC>/<TM>/<TRAINER>/<ROCKET>` expansion. Chinese strings excluded.

### (a) Missing dialogue entries — 7 across 2 maps

- `ref scripts/ViridianGym.asm has ViridianGymGiovanniTM27NoRoomText "You do not have space for this!", remake has no matching English string (ViridianGym/script.scene)`
- `ref scripts/CinnabarGym.asm has CinnabarGymSuperNerd1/2/3/4/6/7 (quiz-gate texts, e.g. "Do you know how hot #MON fire breath can get?..."), remake replaces all 6 with re-implemented quiz wording (CinnabarGym/script.scene) — the scene comment claims the original wording is "not recoverable", but text/CinnabarGym.asm contains all seven texts verbatim (SuperNerd5 alone was kept)`

### (b) Content mismatches (English) — 11 across 7 maps

Case-only (remake uses plain "POKEMON"/"POKE" where ref has the trademark POKéMON casing — cosmetic, renderer-level):

- `ref scripts/CeladonPokecenter.asm has "# FLUTE awakens #MON with a sound…", remake has "POKE FLUTE awakens POKEMON…" (CeladonPokecenter/script.scene:50)`
- `ref scripts/FuchsiaPokecenter.asm has "…raise them evenly." with #MON, remake has "POKEMON" (FuchsiaPokecenter/script.scene:36,41)`
- `ref scripts/CinnabarPokecenter.asm has #MON twice, remake has "POKEMON" (CinnabarPokecenter/script.scene:36,41)`
- `ref scripts/SaffronPokecenter.asm has "#MON growth rates differ…", remake has "POKEMON" (SaffronPokecenter/script.scene:36)`

Wording:

- `ref scripts/CeladonMart4F.asm has "I'm getting a # DOLL for my girl friend!", remake has "I'm getting a POKeDOLL for my girl friend!" (CeladonMart4F/script.scene:18,26)` — ref prints POKéDOLL via the `#`→POKé token; remake hardcodes the expanded word.
- `ref scripts/SilphCo5F.asm has "We study # BALL technology on this floor!" / "the ultimate # BALL", remake has "POKeMON BALL" (SilphCo5F/script.scene:120,122)` — ref token is `#` (POKé) alone, remake expands to "POKeMON BALL" (different word).
- `ref scripts/CinnabarGym.asm CinnabarGymSuperNerd5 has "I know why BLAINE became a trainer! Ow! …", remake has the same story minus the "I know why… Ow!" intro (CinnabarGym/script.scene:222)`

### (c) Structural flows — core flows 0 missing; 1 branch + 4 global-text findings

Core flows — all present: yes/no on all 9 maps (`@choice`), item handouts on all 17 maps (`giveItem`), nurse heal on all 12 maps (`heal()` + `@choice` + `animateHealingMachine()`), marts on all 12 maps (`openShop`).

- `ref scripts/ViridianGym.asm Giovanni TM27 flow checks the GiveItem result and prints ViridianGymGiovanniTM27NoRoomText when the bag is full, remake talkGiovanni (ViridianGym/script.scene:14-19) calls giveItem("TM27",1) unconditionally and sets EVENT_GOT_TM27 — no bag-full branch (the scene even comments about the bag-full case but doesn't handle it)`
- `ref text.asm _PokemartGreetingText "Hi there! May I help you?" / _PokemartBuyingGreetingText "Take your time." / sell greeting "What would you like to sell?" have no occurrence anywhere in the remake — openShop (crates/pokered-core/src/items/shop.rs) opens the menu with no clerk greeting; price-confirm/result lines are reworded in the app layer (crates/pokered-app/src/render/menu.rs:205,261)`
- `ref text.asm nurse lines are reworded in the remake: "Welcome to our POKéMON CENTER!"→"Welcome to the POKEMON CENTER!", "OK. We'll need your POKéMON."→"OK, we will need your POKEMON.", "Thank you! Your POKéMON are fighting fit!"→"Thank you for waiting. Your POKéMON are fully healed!" — present in all 12 heal locations, wording differs`
- `ref _PlayerBlackedOutText is two boxes ("<PLAYER> is out of useable POKéMON!" + "<PLAYER> blacked out!"), remake has only "Player blacked out!" (crates/pokered-core/src/battle/mod.rs:2232, crates/pokered-app/src/render/battle_i18n.rs:31) — first box missing`
- `ref script_cable_club_receptionist link menu (12 maps) is adapted: pokecenter scenes print "Welcome to the CABLE CLUB!" and the link flow lives in the app layer — dialogue present, flow relocated (documented adaptation)`

Paging fidelity — 19 notes (ref `para`-separated pages rendered as fewer remake pages; in-game paging may still match since scenes split pages across `@speaker` blocks and map.json stores one textbox per page). Notable: PewterGym Brock 6 pages→1, VermilionGym Surge 7→1, ViridianCity OldMan 6→1, CeladonGym Erika 9→6, FuchsiaGym Koga 6→4. Dynamic ref texts (`text_ram`/`text_bcd`, 14 entries) are structurally adapted to static item names in scenes — not comparable, not counted as diffs.

### Maps with findings (all others fully covered)

| map | ref texts | missing | mismatch |
|---|---|---|---|
| ViridianGym | 28 | 1 | 0 |
| CinnabarGym | 12 | 6 | 1 |
| CeladonMart4F | 3 | 0 | 2 |
| CeladonPokecenter | 2 | 0 | 1 |
| FuchsiaPokecenter | 2 | 0 | 2 |
| CinnabarPokecenter | 2 | 0 | 2 |
| SaffronPokecenter | 2 | 0 | 1 |
| SilphCo5F | 16 | 0 | 2 |

---

## Appendix: audit scripts

- `/tmp/pokemon_audit/items/audit_items.py`, `/tmp/pokemon_audit/moves/audit_moves.py`, `/tmp/pokemon_audit/pokemon/audit_pokemon.py`, `/tmp/pokemon_audit/hidden_events/audit_hidden.py`, `/tmp/pokemon_audit/text/audit_text.py` (reuse `tools/audit_parity.py` helpers via import; no repo files modified). Raw per-domain reports: `/tmp/pokemon_audit/<domain>/<domain>.md`.
