# Story Completion Audit — Rust rewrite vs. original Red disassembly

**Baseline:** `pokered-worktree` (branch `pure-red`, pret/pokered disassembly)
**Target:** `crates/pokered-data/maps/*/script.scene`
**Method:** 223 maps (every Rust map with an original `scripts/<Map>.asm` counterpart) diffed
against the original `scripts/*.asm` + `text/*.asm` by 16 parallel agents.
**Excluded:** 26 maps with no original counterpart — `*Copy` duplicates, `UnusedMap*`
stubs, and `shared/`.

Every divergence is classed **A** (pure dialogue/text — fillable now by editing only
`.scene`, no engine change), **B** (a mechanic needing a new engine/DSL primitive), or
**C** (faithful). Headline: the ports are overwhelmingly faithful. The gaps cluster into a
small number of repeating patterns.

---

## TL;DR

- **~55 Class A dialogue/text fixes** across ~20 maps — all restorable verbatim from the
  baseline with no engine work. This is the core of "补全剧情".
- **~1 confirmed engine bug** (`giveItem` never reports success) that is worth fixing on
  its own and that unlocks ~10 "bag-full" dialogue branches.
- **~15 distinct missing engine primitives** behind the rest of the Class B stubs; most are
  already `// TODO`-annotated in the scenes. A handful are **story-critical**.

---

## Class A — pure dialogue/text gaps (fillable now, no engine change)

### A1. Pokécenter placeholder dialogue — literal `"TODO: dialog"`

Ten Pokécenters ship the two flavor NPCs and the Cable-Club receptionist as
`@speaker("") { "TODO: dialog" }`. Original text exists verbatim in each
`text/<Map>.asm`; the receptionist gets the same canned line every ported Pokécenter
already uses (`"Welcome to the\nCABLE CLUB!"`).

| Map | NPCs to fill |
|-----|--------------|
| CeruleanPokecenter | npc2 SuperNerd, npc3 Gentleman, npc4 Link |
| CinnabarPokecenter | npc2 CooltrainerF, npc3 Gentleman, npc4 Link |
| FuchsiaPokecenter | npc2 Rocker, npc3 CooltrainerF, npc4 Link |
| LavenderPokecenter | npc2 Gentleman, npc3 LittleGirl, npc4 Link |
| MtMoonPokecenter | npc2 Youngster, npc3 Gentleman, npc5 Clipboard (→ empty), npc6 Link |
| PewterPokecenter | npc2 Gentleman, npc3 Jigglypuff, npc4 Link |
| RockTunnelPokecenter | npc2 Gentleman, npc3 Fisher, npc4 Link |
| SaffronPokecenter | npc2 Beauty, npc3 Gentleman, npc4 Link |
| VermilionPokecenter | npc2 FishingGuru, npc3 Sailor, npc4 Link |
| ViridianPokecenter | npc2 Gentleman, npc3 CooltrainerM, npc4 Link |

(MtMoonPokecenter npc4 "Magikarp salesman" is a priced Pokémon-purchase flow — Class B.)

### A2. Wrong / duplicated / swapped sign & NPC text (real content bugs)

- **VermilionCity** — sign triggers 3,5,6,7 are wired to the *wrong* text bodies (shifted by
  one slot) and the MART_SIGN body is dropped entirely. Correct mapping: 3=MART, 5=FAN_CLUB,
  6=GYM (`LEADER: LT.SURGE / The Lightning American!`), 7=HARBOR.
- **ViridianCity** — sign 4 shows a duplicate gym sign instead of MART_SIGN; sign 5 shows a
  duplicate city sign instead of POKECENTER_SIGN. Also two on-enter gates missing (see A3).
- **WardensHouse** — sign 1 and sign 2 text are swapped (merchandise ↔ photos/fossils).
- **CeladonPokecenter** — npc2 (Gentleman) and npc3 (Beauty) carry unrelated invented flavor
  text instead of the original POKé FLUTE / Cycling-Road lines. (File header admits it's a
  hand-authored "template".)
- **SaffronPidgeyHouse** — npc3 says "POKéMON DOLL"; the `#DOLL` macro expands to "POKé DOLL".
- **SafariZoneEast** — npc2 item ball calls `giveItem("MAX_RESTORE", …)`; `MAX_RESTORE` is not
  a real item id in `pokered-data`. Original awards **MAX_POTION**. (Data fix — VERIFY runtime.)

### A3. Dropped / inverted / truncated passages

- **OaksLab** — ⚠️ Rival end-of-battle win/lose text is **inverted** (win shows the "I picked
  the wrong POKéMON" line, lose shows the "Am I great or what?" brag — backwards vs. the
  `SaveEndBattleTextPointers` convention, cross-checked against Route22). Also Oak's
  post-Pokédex dialogue tree is truncated: three original states are skipped, **including the
  grant of 5 free Poké Balls** (`giveItem`, gated on `EVENT_BEAT_ROUTE22_RIVAL_1ST_BATTLE`).
  Restorable with existing primitives.
- **PewterGym** — Brock's `"Wait! Take this\nwith you!"` line is dropped from the first-win
  path (present only in the retry path), and the TM34 explanation is missing 1 of its 4
  paragraphs (the "a TM is good only once" one).
- **Museum1F** — the third dialogue branch of the ticket clerk (`"Please go to the\nother
  side!"`, shown when approached from the wrong tile) is dropped.
- **CinnabarLabFossilRoom** — the revived species name is dropped from the "back to life" line
  (species is statically known per fossil branch).

### A4. Silph Co. card-key door text (8 floors)

Silph Co. 2F/3F/4F/5F/6F/8F/9F/10F drop or reword the card-key door text. Original shows two
success boxes (`"Bingo!"` → `"The CARD KEY\nopened the door!"`) and a fail box
(`"Darn! It needs a\nCARD KEY!"`). Several floors show only box 2, some show nothing, 3F
invents a different fail line. (The door-open *mechanic* is approximated via a coord trigger —
that part is Class B; the text is Class A. 7F and 11F omit the door entirely — Class B.)

---

## Class B — missing engine / DSL primitives

Grouped by the primitive that's missing. Most are already `// TODO`/`// STUB` annotated in the
scenes. **Bold = story-critical** (affects main-plot progression or completion state).

### B1. `giveItem` success/failure result — ⚠️ CONFIRMED ENGINE BUG (highest value)

`finish_effect()` in `pokered-core/src/overworld/update.rs` has no `ScriptEffect::GiveItem`
arm → returns `CommandResult::Void` → JS `undefined` → every `@if (given = giveItem(...))`
takes the **false** branch. Independently found by 3 agents (RocketHideoutB3F/B4F, Route12,
Route15, Route16FlyHouse). Effects on affected pickups:
- The item *is* still added, but the got-flag is **never set** and the "found ITEM!" text
  never shows — instead the bag-full line shows on every playthrough, and the ball can be
  re-picked/duplicated.
- Affects TM16/IRON (Route12), TM20 (Route15), TM44/RARE CANDY (RocketHideoutB3F),
  HP_UP/TM02/IRON/**SILPH SCOPE**/**LIFT KEY** (RocketHideoutB4F), **HM02 FLY** (Route16FlyHouse).
- Fixing this also makes every **bag-full dialogue branch** work: BillsHouse, CeladonGym
  (Erika TM21), **SaffronGym** (Sabrina TM46), **SafariZoneSecretHouse** (HM03/Surf — currently
  can set `EVENT_GOT_HM03` without the item on a full bag), MrFujisHouse, MrPsychicsHouse,
  CeladonMart3F, CeladonMartRoof, FuchsiaGoodRodHouse, FuchsiaGym, VermilionOldRodHouse,
  Route1, Route24, Route2Gate, ChampionsRoom-adjacent, etc.

### B2. Per-trainer `EndBattleText` for sight battles (systemic, cosmetic)

Engine has `TrainerHeader::end_battle_text_id` / `ShowEndBattleText` but it is **dead code** —
trainer headers carry no text ids and sight-triggered battles cut silently overworld→battle→
overworld (`resume_script_after_battle` is a no-op for non-script battles). Every gym/sight
trainer's one-shot victory quip is lost: AgathasRoom, BrunosRoom, CeladonGym (7), CinnabarGym
(7), FuchsiaGym (6), SS Anne rooms (2FRooms/B1FRooms/Bow = 12), Pokémon Tower channelers
(3F/4F), Pokémon Mansion 1F, and more.

### B3. Static / legendary wild battles

No wild/legendary-battle-from-script primitive; all approximated as instant `givePokemon` +
flag. **Route12 SNORLAX** and **Route16 SNORLAX** are route blockers (story-critical, though
currently walk-through-ok). **PokemonTower6F Ghost Marowak** (gated on Silph Scope) sits on the
path to the Mr. Fuji rescue (story-critical). Optional: Articuno (SeafoamB4F), Zapdos +
8×VOLTORB/ELECTRODE (PowerPlant), Moltres (VictoryRoad2F), Mewtwo (CeruleanCaveB1F).

### B4. Forced / scripted movement (simulated joypad)

Gate push-backs (Route5/6/7/8 Gate, Route16Gate1F, Route18Gate1F, Route22Gate, Route23),
spinner-tile mazes (RocketHideout B2F/B3F, **ViridianGym**, **SaffronGym** teleport maze),
escort/cutscene walks (PalletTown Oak, Pewter guides, Lance's Room, Lorelei's Room, SS Anne
dock/VermilionDock, Silph Giovanni walk-up, MtMoonB2F, PokemonTower7F, Bill's House). Mostly
cosmetic; the two gym mazes are structural but non-blocking.

### B5. Cross-map object / script toggles

- **PokemonTower7F → SaffronCity** — hide the Rocket guard blocking Silph Co. entry. **Story-critical.**
- **HallOfFame → CeruleanCave** — hide the cave guard to unlock Mewtwo. **Story-critical** (VERIFY the
  substitute flag is actually wired).
- **OaksLab → Route22/PalletTown** — show Route22 rival, set Daisy script. Story-relevant.
- SilphCo11F team-rocket-leaves (~37 objects), BillsHouse↔Cerulean guards, Route20↔Seafoam
  boulders, Route23/Route25 boulder & Bill toggles — cosmetic re-sync.

### B6. Other primitives (mostly optional/cosmetic)

- **In-game trades** (no ownership-check/removal/animation): CeruleanTradeHouse, VermilionTradeHouse,
  Route2TradeHouse, Route11Gate2F, Route18Gate2F, UndergroundPathRoute5, CinnabarLabFossilRoom/TradeRoom.
- **Elevator floor menu + shake**: CeladonMartElevator, RocketHideoutElevator, SilphCoElevator (approx via `@choice`).
- **Game Corner coins + slot machine**: GameCorner, GameCornerPrizeRoom.
- **Safari Zone timer/step counter**: SafariZoneGate + all Safari maps.
- **Daycare** level-growth/fee/party-picker: Daycare.
- **Card-key door "use item on facing tile"**: all Silph Co floors (text is A4).
- **Strength/boulder puzzles**: VictoryRoad 1F/2F/3F switches, Seafoam boulder-in-hole + forced current.
- **Cinnabar Gym quiz-maze**, **Vermilion Gym trash-can RNG puzzle**.
- **RNG flavor-text pools**: CeruleanCity NPCs, SSAnneKitchen cook.
- **Screens / misc**: DisplayDiploma (CeladonMansion3F), **HallOfFamePC + SaveGameData/reset** (HallOfFame —
  story-critical for completion persistence), DisplayDexRating (OaksLab), Name Rater rename, numeric
  text token (Route2Gate owned-count), starter-name token (ChampionsRoom), Jigglypuff song/spin
  (PewterPokecenter), PokemonMansion2F switch coord fix.

---

## Recommended fill order

1. **Class A (all)** — restore the ~55 dialogue/text items above. Pure `.scene` edits from the
   baseline; the heart of the story completion.
2. **B1 `giveItem` result** — one engine fix that removes a real playthrough bug and lights up
   ~10 bag-full branches.
3. **Story-critical Class B** — B3 Snorlax/Ghost-Marowak, B5 Saffron/Mewtwo guard toggles,
   HallOfFame save/reset — if we want true main-plot parity.
4. **Optional Class B** — trades, elevators, Game Corner, Safari, daycare, puzzles, EndBattleText —
   large surface, each its own primitive; schedule as wanted.

---

## Implementation status — branch `feat/story-completion`

### Done (build + `pokered-core` tests green)

**Class A — dialogue/text (scene-only):**
- 10 Pokécenters: filled Beauty/Gentleman/Rocker/etc. flavor NPCs + Cable-Club receptionist
  (Cerulean, Cinnabar, Fuchsia, Lavender, MtMoon, Pewter, RockTunnel, Saffron, Vermilion, Viridian);
  CeladonPokecenter's two invented NPC lines corrected to the real POKé FLUTE / Cycling-Road lines.
- Sign/text bugs: VermilionCity signs 3/5/6/7 + MART_SIGN; ViridianCity signs 4/5; WardensHouse
  signs 1/2 un-swapped; SaffronPidgeyHouse "POKé DOLL"; SafariZoneEast `MAX_RESTORE`→`MAX_POTION`.
- Dropped/inverted passages: **OaksLab** rival win/lose text un-inverted + full post-Pokédex Oak
  chain restored (PokemonAroundTheWorld, the **5 Poké Balls** grant, ComeSeeMeSometimes);
  PewterGym Brock "Wait! Take this" + TM34 4th paragraph; CinnabarLabFossilRoom revived-species name.
- Silph Co. 2F/3F/4F/5F/6F/8F/9F/10F card-key door text (Bingo! / opened / Darn!) restored (18 doors).
- ViridianCity private-property on-enter gate added (coordEvent + storyline).

**Engine fixes:**
- **`giveItem` success/failure result** — the completion path now returns `CommandResult::Bool`
  computed from the frame-seeded bag snapshot (`script_bag_names`, cap `BAG_ITEM_CAPACITY`), so
  `@if (given = giveItem(...))` "no room" branches work; every affected ground-item pickup now shows
  the correct "found ITEM!" text, sets its got-flag, and can't be re-picked. Fixes the Surf/TM46
  bag-full softlock class too.
- **`startWildBattle(species, level)`** — new script command (generic `command.rs` → pokered
  `script_api.rs` → `script_bridge` → `update.rs` sets `pending_wild_encounter` + suspends like
  `startBattle` → app resumes with the outcome). App outcome strings extended: `Captured`→`"caught"`,
  `Escaped`→`"fled"`. Wired into all 7 static/legendary encounters (Route12 & Route16 SNORLAX,
  PokemonTower6F Ghost MAROWAK, SeafoamIslandsB4F ARTICUNO, PowerPlant ZAPDOS, VictoryRoad2F MOLTRES,
  CeruleanCaveB1F MEWTWO) — real catchable battles; the beat-flag is set only on win/catch.
  NOTE: needs an in-game playtest to confirm the runtime encounter→resume→flag flow end-to-end;
  `pokered-tui` (which resumes no scripted battle) is unaffected/consistent with existing `startBattle`.

## Overworld bag menu  ✅ v1 done

`StartMenu → ITEM` was a stub; now opens a real bag screen (`bag_screen::BagScreenState` +
`GameScreen::Bag` + `draw_bag`): item list with cursor/scroll, USE / TOSS / CANCEL per item, and a
TOSS-quantity selector. Verified live (Start → ITEM → `screen=bag`, action menu, B backs out).

**✅ Field-use dispatch done** — `USE` now routes through `OverworldScreen::use_field_item(item)`:
- **POKé FLUTE** — on Route 12 / 16 (before the Snorlax is beaten) sets `EVENT_FIGHT_ROUTE<n>_SNORLAX`
  so talking to the SNORLAX starts the battle. This is the real trigger that replaces the earlier
  talk-driven approximation and satisfies **Phase 1a** below. Elsewhere it just plays. ★
- **BICYCLE** — toggles the real `player.transport` between `Walking` and `Biking`, which halves
  `frames_per_step` (Biking = 4 vs Walking = 8) so the player actually moves faster; refused while
  `Surfing`. (Bike-restricted-tileset gating + the on-bike sprite are cosmetic follow-ups.)
- **TOWN MAP** — opens a real full-screen KANTO map viewer (`GameScreen::TownMap` +
  `town_map_screen::TownMapScreenState` + `render/town_map.rs`): the 20×18 map is rebuilt from a new
  `town_map.rle` decoder (high-nibble tile / low-nibble run), a flashing "you are here" marker sits
  on the player's current landmark, and Up/Down scroll a selection reticle through the fly-order
  landmarks showing each name; A/B closes. A `town_map_position(MapId)` resolver maps any map
  (dungeon floors included) to its marker via `map_to_name_id`. Verified by screenshot + a live TCP
  run (bag → USE → `screen=town-map`, Down scrolls, B → overworld).
- **ESCAPE ROPE** — warps out of a dungeon/interior (non-outside tileset) to the recorded outside
  entrance and is consumed. New `last_map_entry: Option<(u8,u8)>` records the outside tile in
  `commit_pending_warp` whenever `save_last_map` fires (player x/y there still hold the entrance
  tile). Refused (not consumed) on outside maps or before any entrance is recorded.
- default — "This isn't the time to use that!"

Each use sets `pending_dialogue` (shown on return to the field), except the ESCAPE ROPE warp which
skips it; consumed items are removed from the bag. Also fixed `DebugCommand::GetFlags` to read the
**live** `unified_flags` store, not the `save_data` snapshot. Verified: 7 `tests_field_items` unit
tests (bike toggle/surf-guard, flute flag on/off Route 12, escape-rope warp + both refusals) plus a
live TCP run flipping `EVENT_FIGHT_ROUTE12_SNORLAX` unset → true through the real bag UI.

## ⚠️ Game-wide infrastructure fix — `.scene` functions were never executing ★★

While wiring the Seafoam puzzle we found that **no `.scene` npc/coord/sign handler was actually
running at runtime** — every one silently fell back to the map.json JSON text. Root cause: the DSL
compiles `@storyline("talkX")` to an exported JS function named **`storyline_talkX`**, but the
runtime (configs + trigger bindings) looks up the **bare** name `talkX`, so the lookup always
missed. Simple NPCs still showed dialogue only because that text is *also* in `map.json`; any
conditional logic (trade gates, legendary-battle gates, bag-full `@else`, the Seafoam gate, …) never
executed. Fixed in `jrpg-engine-script/src/engine.rs`: `call_function`/`has_function` now resolve the
exact name first (so `onLoad`/bare `.js` names still win) then fall back to `storyline_<name>`. This
retroactively activates **all** the `.scene` conditional logic added across the earlier phases.
Two supporting fixes: `OverworldScreen::new` now also loads each map's `script_config.json` in the
`--scripts-dir` path (disk scenes had no triggers otherwise), and the debug `Warp` command now goes
through the real warp-commit path so scripts/triggers reload. The dead `script.js.bak` migration
leftovers (248 files) were deleted.

## ✅ Verification sweep of the now-activated `.scene` features

Once the runtime fix landed, a 12-agent parallel sweep live-verified the previously-inert conditional
logic. **10/12 confirmed working** (each runs the `.scene`, not the JSON fallback): all five static
legendaries **do start the battle** with a party — Articuno, Zapdos, Moltres, Mewtwo (Lv70), Snorlax
(the earlier "Gyaoo! but no battle" was just an empty-party debug artifact) — plus PowerPlant Voltorb
item-balls, the Pokémon Tower ghost MAROWAK (Silph-Scope coord trigger), the Poké Center heal, the
in-game trade (both wrong-mon `@else` and success), and the bag-full `@else`. **2/12 were genuinely
broken and are now fixed** (below).

## ⚠️ + ✅ Second game-wide fix — `hideObjectByName`/`showObjectByName` were undefined ★★

The sweep caught the SaffronCity (Silph Co. Rocket guard) and CeruleanCity (Unknown-Dungeon guard)
`@load` object-gates not hiding their guard even with the flag set — both **story-critical** (Silph Co.
is main-plot; the Cerulean guard gates MEWTWO). Root cause was two-fold: (1) the engine registered only
`game.hideObject`/`showObject`, but **35 + 7 scene files call the `…ByName` variants**, which were
`undefined` → the `@load` threw before hiding anything; (2) SaffronCity/CeruleanCity's committed
`script_config.json` were **missing the `onLoad` binding** (stale — predated the `@load`), so the load
handler never even ran. Fixes: registered `hideObjectByName`/`showObjectByName` in
`jrpg-engine-script`, and regenerated both configs via `scene_apply` (which also required declaring the
Cerulean rival-battle coord positions in the `.scene` `@trigger(... coords=[[20,6],[21,6]])` so they
survive regeneration). Verified end-to-end: entering SaffronCity via the real `run_on_load` path hides
`SAFFRON_CITY_OBJ_14/15`; `hideObjectByName` sets `__OBJ_HIDDEN_*` + `npc.visible=false` (collision
already filters invisible NPCs, so the path opens). This unblocks all 42 scenes using the `…ByName`
object toggles.

## ✅ Tier-2/3 parallel implementation batch

Eight features implemented in parallel (isolated worktrees → patches → integrated), all building + tested
(pokered-core/data/dsl/engine 2462 passed; app + tui compile):

- **SilphCo 7F card-key doors** — the only Silph floor missing them; 3 doors gated on `hasItem("CARD_KEY")`.
- **Cinnabar Gym quiz gates** — 7 Super Nerd guards ask a yes/no quiz (@choice); correct opens, wrong battles.
- **Vermilion Gym trash-can puzzle** — real two-switch search (random first can + orthogonal-neighbor second).
- **Safari Zone timer** — 500-step / 30-Ball economy + eject-to-gate on zero (bait/rock deferred).
- **RNG flavor-text pools** — new Rust-sourced `game.showRandomText([...])` (scripts have no `Math.random`);
  wired into CeruleanCity (CooltrainerF/Slowbro) + SSAnneKitchen. Engine gets a seedable xorshift + `mix_rng`.
- **Oak's Dex-rating** — `getPokedexSeenCount()`/`getPokedexOwnedCount()` query fns + Oak's full owned-count
  rating tiers in OaksLab (DexRatingsTable thresholds).
- **Name Rater** — talk → pick a party mon (new `party_select` selector + `ChoosePartyPokemon`/`SetPartyNickname`
  script effects + naming screen) → rename it, in NameRatersHouse.
- **Jigglypuff song** — PewterPokecenter cutscene: cry + MUSIC_JIGGLYPUFF_SONG + "everyone feels sleepy" text.

**Live verification sweep of these 8** (parallel debug-server agents): **7/8 confirmed working in-game** —
card-key doors (flag flips only with the CARD KEY), Cinnabar quiz (correct→open, wrong→battle), Safari
timer (walked ~500 steps → "PA: Ding-ding! Your SAFARI GAME is over!" → eject to gate), RNG flavor text
(lines vary per talk), Oak dex-rating (shows seen/owned counts + tier), Name Rater, Jigglypuff. The sweep
caught **Vermilion trash cans broken** (coordEvents on impassable can tiles → never fire in real play; a
pre-existing bug) — **fixed**: the cans are now A-press signs and the handler reads the faced can via
`getPlayerFacing`.

## ✅ Game Corner slot machine

`GameScreen::Slots` + a SlotsScreen state machine wrapping the existing tested `SlotMachineState` (BetSelect
1–3 → Spinning, press A to stop each reel → Result payout), coin economy (bet deducted, payout capped 9999),
framebuffer rendering, and a GameCorner slot-machine interaction gated on the COIN CASE.

## ✅ Game Corner prize room + coin economy

Added a **coin balance script API** (`giveCoins`/`takeCoins` async + `getCoins`/`hasCoins` sync) threaded
through the same 6 layers as the money API, backed by `GameData::give_coins`/`take_coins` (cap 9999,
saturating). **GameCornerPrizeRoom** vendors 1 & 2 are now a playable coin-for-POKeMON exchange
(pick prize → `hasCoins` → `givePokemon` + `takeCoins`; RED prices/levels from `data/events/prizes.asm`).
The **GameCorner** coin-gift NPCs (Fishing Guru +10, Clerk 2 +20, Gentleman +20, one-time; Clerk 1 buy 50 for
¥1000) now actually credit coins, with a coin-case-full guard so Clerk 1 can't take ¥1000 for <50 coins at the
cap. Vendor 3 (TM prizes) stays a documented stub: TMs aren't bag-storable yet (`ItemId` has no TM variants),
so it defers rather than charge coins for an undeliverable prize.

## ✅ Daycare (Route 5 Day Care)

Real deposit/withdraw backed by the save's existing day-care box struct: `SaveData::deposit_daycare(index)`
(MoveMon PARTY_TO_DAYCARE — removes the mon, rejects HM holders + the last party mon), `withdraw_daycare()`
(re-derives level/stats/moves from accumulated EXP, restores HP), and `GameData::tick_daycare_exp()` (+1 EXP
per overworld step, capped at level-100). Scene primitives: `depositDaycare`/`withdrawDaycare` +
`isDaycareInUse`/`getDaycareMonName`/`getDaycareLevelsGrown`/`getDaycareCost`/`getPartyCount`/`getPartyMonName`/
`partyMonKnowsHm`. Fee = ¥100·(levels grown + 1), matching `.calcPriceLoop`. The `Daycare` scene is rewritten
to the real flow; state persists via `game_data.daycare`. 7 unit tests in `save/daycare_tests.rs`.

## ✅ Per-trainer EndBattleText

Wired the previously-dead one-shot victory quip for **sight/talk trainer battles**: `end_battle_text` flows
`map.json` → `NpcJson`/`StaticNpcJson` → `PokemonNpcData` → `PendingTrainerBattle` → `BattleScreen`, spliced
into the `VictoryPhase::EndBattleText` arm before the prize-money text (original `PrintEndBattleText`). Data:
**all 308 sight-trainer quips across 64 maps** converted from the baseline via `tools/endbattle_text/` (parses
each `scripts/<Map>.asm` `trainer` header → `text/<Map>*.asm` string; ordinal join to `isTrainer` NPCs,
verified against VermilionGym). Script-driven gym leaders keep their own scene reward text (`end_battle_text:
None`). English-only (the battle text layer isn't i18n).

Deferred (genuinely large / blocked): cable-club/link multiplayer (needs real networking; not single-player
completable); TM-in-bag storage (blocks the Game Corner TM vendor + all TM gifts); per-trainer *AfterBattleText*
+ distinct loss-text.

## Remaining work — phased roadmap

Every remaining gap is grouped by the **missing primitive** that blocks it, because one primitive
usually unlocks a whole cluster of maps. Ordered roughly by leverage ÷ effort. `★` = story-critical
(gates main-plot progression or completion state); everything else is side/cosmetic.

### Phase 0 — Enable live playtest verification (debug tooling)  ✅ mostly done
Done this pass: fixed the `load_full_map_data` signature + double-bind that stopped the server
launching; implemented `DebugCommand::GiveItem` (→ `bag.add_item`) and `GivePokemon`
(→ `create_pokemon` + `party.add`); fixed the input path so `Press`/`PressSequence` actually reach
the player and `RunFrames` advances the game instead of freezing it (was an early-return);
enriched `GetState` with the live dialogue text + party count. All verified live over TCP.
**Remaining:** `SetFlag` still doesn't propagate to a running scene's `getFlag` (the scene
re-seeds its own flag bridge on each run) — so playtest setup must use the real gameplay mechanism
(e.g. actually using an item), not `set_flag`, for scene-gating flags.

### Phase 1a — ✅ DONE: overworld POKé FLUTE use (Snorlax trigger)
Previously nothing **set** `EVENT_FIGHT_ROUTE12/16_SNORLAX` (only the *in-battle* `use_poke_flute`
existed), so the Route 12 / 16 SNORLAX couldn't be triggered in real play. Now the bag "use POKé
FLUTE" overworld path (`use_field_item`) sets the FIGHT flag on Route 12 / 16, making the static
Snorlax battles reachable via real gameplay — verified live (see the bag-menu section above). ★
**Follow-up:** wake Pokémon Tower channelers with the flute (currently the tower ghost path uses the
SILPH SCOPE reveal; the flute's channeler-calming is not yet wired).

### Phase 1 — Quick wins (small, high fidelity, low risk)
- ✅ **PowerPlant 8× VOLTORB/ELECTRODE** item-ball battles → now use `startWildBattle`
  (VOLTORB Lv40 ×6, ELECTRODE Lv43 ×2; beat-flag set only on win/catch).
- ⏸ **Per-trainer `EndBattleText`** — DEFERRED. The engine has a `ShowEndBattleText` state, but the
  production `TrainerHeaderData` struct carries only `event_flag` + `sight_range` — **no per-trainer
  text exists in the Rust data**. Wiring it means bulk-converting hundreds of `*EndBattleText`
  bodies from the disassembly (a data-generation tooling job) for a purely-cosmetic one-line quip.
  Best done as a dedicated data pass, not hand-entry.
- ⏸ **PokemonMansion2F** hidden-switch coord + **Museum1F** clerk branch — skipped (map-data /
  position-query nuance, low value).

### Phase 2 — Cross-map object toggles ★  ✅ story-critical done
Implemented as robust local `@load` flag-gates in the target maps (more reliable than cross-map
toggling), after verifying the blocking geometry + trigger flags against the disassembly:
- ✅ **SaffronCity** `@load` — on `EVENT_RESCUED_MR_FUJI`, hide `SAFFRON_CITY_OBJ_14/15` (the Rocket
  at the Silph door's approach tile (18,22), confirmed vs the (18,21) Silph warp). Unblocks Silph Co.
- ✅ **CeruleanCity** `@load` — on `EVENT_HALL_OF_FAME_DEX_RATING` (the champion flag HallOfFame sets),
  hide `CERULEAN_CAVE_GUY` (the "champion only" SuperNerd). Unlocks the Unknown Dungeon (MEWTWO).
- ✔ **OaksLab → Route22 / Daisy** — already covered by existing flag-gates (Route22 reads
  `EVENT_ROUTE22_RIVAL_WANTS_BATTLE`; PalletTown gates Daisy on `EVENT_GOT_TOWN_MAP` +
  `EVENT_ENTERED_BLUES_HOUSE`). No work needed.
- ⏸ Cosmetic-only, deferred: SilphCo11F mass Rocket-clear across floors (beaten trainers stay
  visible with post-battle text), Route20↔Seafoam / Route23,25 boulder re-syncs, BillsHouse↔Cerulean.

### Phase 3 — Forced / scripted movement  ✅ already implemented (audit was pessimistic)
The audit flagged these "stubbed" from stale scene *comments*, but the engine already handles the
mechanics (the audit agents read scenes, not `overworld/special_terrain.rs`):
- ✅ **Spinner / arrow-tile mazes** — fully wired & tested: `update.rs` overrides player input to
  force rotation+slide while standing on a spin tile (`is_spinner_tile`/`handle_spinner_rotation`,
  `SPINNER_TILESETS = [Facility, Gym]`). Covers **ViridianGym**, Rocket Hideout B2F/B3F, Silph Co —
  map-tile-driven, needs no scene code. (Scene TODO comments there are now stale.)
- ✅ **Teleport pads** — `teleport_spin_direction` exists (SaffronGym maze).
- ✔ **Gate push-backs** (Route16/18 Gate, Route22Gate) and
  **escort/cutscene walks** (Pewter guides, Lance's/Lorelei's Rooms, Vermilion/SS-Anne dock,
  Giovanni walk-up, Bill's House) — functional `movePlayer`/`moveNpc` approximations; only exact
  simulated-joypad RLE fidelity is missing (cosmetic).
  ⚠️→✅ This line over-claimed: Route5/6/7Gate, Route23 and SafariZoneGate had **no coord
  interception at all** (only Route8Gate did) — the guards never stopped the player. Fixed in
  the 2026-09 blocking-NPC pass below.
- Seafoam forced-surf current is the one genuine remaining piece (bundled with Phase 5 boulders).

### Phase 4 — In-game trade primitive  ✅ done
New `tradePokemon(offered, received, nickname)` DSL command: gates on the party actually holding
`offered` (reported synchronously from a new frame-seeded party-species snapshot, mirroring the
`giveItem` result path), removes it, and adds `received` nicknamed at the offered mon's level; the
`@else` shows the dialogset-correct wrong-mon text. Wired into all 9 trades across 8 maps
(CeruleanTradeHouse, VermilionTradeHouse, Route2TradeHouse, Route11Gate2F, Route18Gate2F,
UndergroundPathRoute5, CinnabarLabFossilRoom, CinnabarLabTradeRoom ×2). Added
`Species::from_scene_name` / `scene_species_to_pascal` (with tests) so multi-word tokens like
`MR_MIME` / `NIDORAN_F` parse to the strum PascalCase variant names — this also fixes a latent
`givePokemon("MR_MIME")` parse failure. The live cable-trade animation remains cosmetic/omitted.

### Phase 5 — Standalone side-content subsystems  ⏸ deferred (functional approximations exist)
These are minigame/economy **systems**, not story, and the main game is completable without full
fidelity. Each is a major standalone build; deferred rather than rushed:
- **Elevator floor menu + shake**: CeladonMart/RocketHideout/SilphCo elevators — already functional
  (a `@choice` warps to each floor; only the exact floor-menu UI + shake animation are cosmetic).
- **Game Corner coins + slot machine**: `player_coins` exists in game data, but the slot minigame +
  coin-credit/prize-picker is a full subsystem. (Prize-room Pokémon are the only gated content.)
- **Safari Zone** 500-step/30-ball timer: wild encounters currently fall through to normal battles
  (HM03/Surf gift still obtainable, so not progression-blocking).
- **Daycare** EXP/level-growth + fee/party-picker sim.
- **Strength boulder-on-switch** (Victory Road) — already functional via walk-to-switch auto-solve.
- ✅ **Seafoam Islands boulder-in-hole + forced current** — DONE (SeafoamIslandsB4F). The two boulders
  are pushed by interacting with them (STRENGTH), each setting `EVENT_SEAFOAM4_BOULDER{1,2}_DOWN_HOLE`
  ("You pushed the BOULDER into a hole… The current slows."). A forced-current coord event at (7,5) —
  the only water approach up to ARTICUNO — sweeps the surfing player back down (`movePlayerTo`) while
  either boulder is still up, and ARTICUNO itself is gated on both flags, so it's unreachable until
  the puzzle is solved. Coord position is declared in the `.scene` `@trigger(..., coord=[7,5])` so
  `scene_apply` keeps it in lock-step. Verified live: boulders-up → "The current is too fast!";
  boulders-down → "Gyaoo!" (the ARTICUNO battle branch). ★
- **Cinnabar Gym quiz-maze**, **Vermilion Gym trash-can** — already completable (approximated).

### Phase 6 — Screens / misc APIs
- ✅ **Item-give bag-full `@else` texts** — restored for the specific NPC gifts that have a
  `*NoRoomText` in the original (unblocked by the `giveItem` fix). *(this pass)*
- ✔ **HallOfFame ★** — the story-critical outcome already works: ChampionsRoom (beat Champion →
  `warpTo("HallOfFame")`) → HallOfFame `@load` sets `EVENT_HALL_OF_FAME_DEX_RATING` → the Phase-2
  CeruleanCity gate unlocks the Mewtwo cave. The remaining `HallOfFamePC` roster screen, Elite-4
  reset (post-game rematch), and auto-save/restart-after-credits are cosmetic/QoL, not blockers.
- ⏸ Deferred cosmetic: card-key doors on SilphCo 7F/11F (gate side-rooms, don't block; 11F is
  already open in `map.blk`), RNG flavor-text pools (CeruleanCity, SSAnneKitchen), DisplayDiploma,
  DisplayDexRating, Name Rater rename, numeric/starter text tokens, Jigglypuff song/spin,
  per-trainer EndBattleText (needs bulk data), and **cable-club/link** multiplayer.

## ✅ 2026-09 — Missing "block-the-player" coord interceptions (Route5/6/7Gate, Route23, SafariZoneGate)

Live playtesting found several NPCs that should stop the player from passing never fired:
the original maps run the interception from a `DefaultScript` coord check
(`ArePlayerCoordsInArray` / the Route23 guard-row scan), but the ports only had the guard's
*talk* handler — walking past did nothing. Route8Gate was the one correct reference port.

- **Route5Gate / Route6Gate / Route7Gate** — added the `gateBlock` coord storyline on the
  original trigger tiles ((3,3)/(4,3), (3,2)/(4,2), (3,3)/(3,4) respectively): while
  `EVENT_GAVE_SAFFRON_GUARDS_DRINK` is unset, stepping onto the gate row takes a carried
  drink (FRESH_WATER→SODA_POP→LEMONADE) and sets the shared flag, else shows the thirsty
  line and shoves the player back (original PAD_UP / PAD_DOWN / PAD_LEFT directions).
- **Route23** — added the seven `guardRow*` on-step badge checks (CASCADE→EARTH, south to
  north) the original `Route23DefaultScript` runs: the guards stand *beside* the corridor,
  so without the row check every checkpoint was bypassable (live-verified sidestep exploit).
  Trigger tiles enumerate exactly the reachable tiles of each guard row (derived from the
  real blockset collision data; rows 85/96 are surf-only tiles; row 35 stops at x<14 like
  the original). Uses the real `hasBadge()` API (the old "no badge-query API" comment was
  stale). DENIED/SFX + one-step `movePlayerRelative(["down"])` on failure; sets the original
  `EVENT_PASSED_*_CHECK` flags on success.
- **SafariZoneGate** — added the `gateRow` coord storyline at (3,2)/(4,2): walking in runs
  the join pitch (pay ¥500 → 30 SAFARI BALLs + `warpTo` into the ZONE; refuse/no money →
  one-step shove down), walking back out of the ZONE runs "Leaving early?" (YES returns the
  balls + clears `EVENT_IN_SAFARI_ZONE`/`EVENT_SAFARI_GAME_OVER` + shove down; NO →
  "Good Luck!" + one step back up toward the door).

All verified live over the debug server (fresh state and a money/all-badges snapshot):
blocked without drink/badge/money (incl. sidestep probes), correct pass-through with
drink/badge/payment, the shared gate flag opening all Saffron gates, and both
leaving-Safari branches. `pokered-core` + `pokered-data` suites green (incl. the
`.scene`↔config round-trip).
## ✅ 2026-09 — Remaining cutscene/interception fidelity gaps (LancesRoom, CeruleanCity, SilphCo11F, Bruno)

Follow-up to the blocking-NPC pass: a full sweep of the maps whose original
`DefaultScript` runs coord checks but whose ports had fewer coord events at — or
different — tiles. What was genuinely missing (each verified live over the debug
server where the flow is reachable):

- **LancesRoom** — (a) the entrance seal now covers BOTH original tiles
  `(5,11)/(6,11)` (was (6,11) only); (b) standing next to Lance (`(5,1)/(6,2)`)
  now auto-triggers his trainer text + scripted battle — `lanceStep` storyline
  gated on `!EVENT_BEAT_LANCE` (the original's `coordIndex<3` case; the talk
  handler still works as before); (c) the arrival nudge at `(24,16)` replicates
  the original `WalkToLance` RLE as a short movePlayerRelative path (the maze
  walls bump the simulated-walk short — the live port of the original RLE
  sequence ends ~(22,15)). Note: `lanceStep`'s win flow was verified only by
  construction (identical binding + pipeline to the verified Cerulean/Silph
  intercepts) — the room's maze makes the (5,1) approach impractical to drive
  blind.
- **CeruleanCity** — the Rocket-thief on-step interception at `(30,7)/(30,9)`
  (`rocketStep`: his text + auto-battle + TM28 return, gated on
  `!EVENT_BEAT_CERULEAN_ROCKET_THIEF`). Verified live: full win flow + post-beat
  silence.
- **SilphCo11F** — the Giovanni on-step interception at `(6,13)/(7,12)`
  (`giovanniStep`, gated on `!EVENT_BEAT_SILPH_CO_GIOVANNI`); the card-key door
  story collapsed to the single traffic tile `(6,12)` (the door block
  (3,6)'s second traffic tile is Giovanni's intercept tile) and its "Darn!"
  branch now only fires while the door is still locked. Verified live: door
  open → walk-on intercept → win → post-beat silence.
- **BrunosRoom** — its "Don't run away!" shove used `movePlayer([[0,-1]])`,
  which is an ABSOLUTE position (→ (0,-1), a no-op): the room could actually be
  escaped. Now `movePlayerRelative`, matching Lorelei/Agatha; the beat gate was
  removed so the guard is unconditional like the original.
- **LoreleisRoom / AgathasRoom / the (4,11),(5,11) question** — investigated,
  no change needed: those two tiles ARE the exit warps, and the port's warp
  check runs before the coord check on the same step, so coord events there can
  never fire; the (4,10)/(5,10) shove is what keeps the player off the warp
  (the original's DefaultScript runs pre-movement so its y=10 shove pre-empts
  the down-step — same gameplay result). Documented in the scenes.
