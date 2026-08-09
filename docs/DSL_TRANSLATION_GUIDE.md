# Translating a Pokémon Red map: disassembly → Game DSL `.scene`

You are given ONE map name `<Map>`. Faithfully reproduce its original story/events
in our Game DSL, using ONLY the real API surface below. Then self-verify.

## Sources to read
- Original event logic: `<original-disassembly>/scripts/<Map>.asm`
- Original dialogue: `<original-disassembly>/text/<Map>.asm` (and `<Map>_2.asm` if present; some texts are in shared files — grep `text/` for a `_<Map>...Text::` label if missing)
- Map objects (npc/sign ids, trainers, coords): `crates/pokered-data/maps/<Map>/map.json`
- Current (usually BROKEN) port to replace: `crates/pokered-data/maps/<Map>/script.scene`

The original uses a `w<Map>CurScript` script-pointer state machine + `CheckEvent`/
`SetEvent` flags + `*_TextPointers` (npc dialogue) + trainer headers (sight battles).

## ⚠️ The current .scene files are BROKEN — do not trust them
The earlier auto-converter emitted a HALLUCINATED API that never existed and throws
at runtime: `startGymBattle`, `giveBadge(...)` as `giveBadge("X")` is fine but
`startTrainerBattle`, `giveTM34`, `showTextChoice`, `startScriptedBattle`, and the
`EVENT.X` member-access form (`getFlag(EVENT.BEAT_BROCK)` → `EVENT` undefined). You
REWRITE to the real surface. Reuse the original's dialogue text and event logic, not
the broken .scene's API calls.

## DSL syntax
```
game_scene <Map> {
  @storyline("talkName") {
    @trigger(map = "<Map>", npc = <id>)          // bind to an npc (map.json npcs[].textId)
    @if (getFlag("EVENT_X") && !getFlag("EVENT_Y")) {
      @speaker("") { "Line one\nLine two" }       // Gen1 dialogue: ALWAYS empty speaker (no name prefix)
      setFlag("EVENT_Z")
      giveItem("POTION", 1)
    } @else {
      @speaker("") { "Other text" }
    }
  }
  @storyline("signName") { @trigger(map="<Map>", sign = <id>) @speaker(""){ "sign text" } }
  @storyline("coordName") { @trigger(map="<Map>", name = "coordTriggerName") ... }
  @load { ... runs on map entry ... }              // only if the original has on-enter logic
}
```
Rules:
- **Assignment is `name = expr`** (NO `let` — `let` parses as a stray command). Use it
  to capture a battle result: `result = startBattle("OPP_X")`.
- **Conditions/args calls auto-namespace to `game.`** — write `getFlag("X")`, not `game.getFlag`.
- **Flags are `"EVENT_*"` STRING LITERALS.** No member access, no `EVENT.X`.
- **`@speaker("")`** is the narrator/no-prefix form — use it for ALL Gen1 dialogue
  (the name is part of the text, e.g. "I'm BROCK!"). Join `line`/`cont` with `\n`,
  separate `para` pages with `\n\n` (or a new `@speaker("")` block). Convert `#MON`→`POKeMON`,
  `<PLAYER>`/`<RIVAL>` stay as-is (supported tokens).
- **`@speaker` is fixed to player-initiated dialogue** — talking to an NPC
  (`@trigger` `npc` binding), A-advanced. **Cutscene speech uses `@say("Name")`**:
  an auto-triggered storyline (`@load` / coord) where NPCs talk in sequence,
  e.g. `@say("OAK") { "Hey!" }`. Both compile to `game.showText(...)` — the
  distinction is semantic, so a scripted storyline never looks like a talk
  handler (and vice versa).
- **`@choice { @option("YES"){...} @option("NO"){...} }`** for YesNoChoice / showChoice.
- Bare commands are awaited automatically. A `@storyline` body is a sequence of statements.

## Real API surface (ONLY these exist; anything else ReferenceErrors)
Queries (return a value, usable in `@if`/assignment):
`getFlag(name)`→bool, `hasItem(constName)`→bool, `getMoney()`→num, `hasMoney(n)`→bool,
`getPokedexOwnedCount()`→num, `getPlayerFacing()`→"up"|"down"|"left"|"right", `getRivalStarter()`→0|1|2,
`getPlayerPosition()`→{x,y},
`getPlayerX()`→num, `getPlayerY()`→num (scalar accessors — the DSL has no member access).
Effects (await):
`setFlag(name)`, `resetFlag(name)`, `giveItem(constName, qty)`, `takeItem(constName, qty)`,
`giveMoney(n)`, `takeMoney(n)`, `giveBadge("BOULDERBADGE".."EARTHBADGE")`,
`givePokemon(species, level)`, `startBattle("OPP_<CLASS><N>")`→"win"|"lose"|"draw",
`heal()`, `warpTo(map, x, y)`, `openShop(...)`, `openNamingScreen(...)`,
`showPokedexEntry(species)`, `showEmotionBubble(...)`, `playMusic(id)`, `playSound(id)`,
`stopMusic()`, `fadeOutMusic()`, `fadeScreen(...)`, `playCry(species)`,
`moveNpc(toggleId, [[x,y],...])`, `moveNpcTo(...)`, `movePlayer([[x,y],...])`, `movePlayerTo(x,y)`,
`movePlayerRelative([[dx,dy],...]|["down",...])` (relative steps; direction strings allowed),
`faceNpc(toggleId, dir)`, `facePlayer(dir)`, `followNpc(...)`,
`showObject(idx)`, `hideObject(idx)`, `showObjectByName(toggleId)`, `hideObjectByName(toggleId)`,
`setNpcFrame(...)`, `setNpcPosition(...)`, `setJoyIgnore(mask)`, `clearJoyIgnore()`, `delay(frames)`.
Item const names are SCREAMING_SNAKE (POTION, POKE_BALL, OAKS_PARCEL, SILPH_SCOPE, LIFT_KEY,
TM34, HM01, ...). Trainer ids: `OPP_<SCREAMING_CLASS><party+1>` (e.g. `OPP_BROCK1`,
`OPP_JR_TRAINER_M3`, `OPP_RIVAL1`).

## Map-type patterns
- **Gym leader (talk-driven battle + reward):** the leader npc must be talk-driven, so set
  that npc's `"isTrainer": false` in `map.json` (find it by trainerClass = the leader, e.g.
  Misty/LtSurge/Erika/Koga/Sabrina/Blaine/Giovanni). Handler: if beaten → post-battle text;
  else → pre-battle text → `result = startBattle("OPP_<LEADER>1")` → `@if (result == "win")`
  → badge text → `giveBadge("<BADGE>")` + `setFlag("EVENT_BEAT_<LEADER>")` + TM `giveItem` +
  set the gym-trainers-beaten event. (See PewterGym for the full worked example.)
- **Sight-engaged trainer (gym grunts, route trainers):** map.json keeps `isTrainer:true`; the
  runtime drives the battle. The .scene talk handler is DIALOGUE ONLY: if beaten → after-battle
  text; else → the pre-battle taunt. Do NOT `startBattle` in a sight-trainer's talk handler.
- **Rival / scripted battle (coord/onLoad triggered):** use a coord `@trigger` or `@load`;
  `result = startBattle("OPP_RIVAL<n>")` and branch; set the rival event flags.
  Rival party-by-starter: `s = getRivalStarter()` then pick the OPP id variant.
- **Item gate (key item / hidden item):** `@if (hasItem("SILPH_SCOPE")) { ... }`; ground items:
  coord/onLoad → giveItem + setFlag(picked).
- **NPC dialogue / signs:** straightforward @speaker, branch on story flags as the original does.
- **Doors / switches / Card-Key gates (`replaceTileBlock` — NOW SUPPORTED).** The original
  opens a door by swapping a map block: a routine (often `<Map>SetDoorTile` / a gate or
  coord callback, re-run on map load) does `CheckEvent FLAG` → pick a block id (closed vs
  open) → `ld [wNewTileBlockID], a` → `lb bc, Y, X` → `predef ReplaceTileBlock`. Translate it:
  - **`@load` re-applies the open state from the flag** (so a solved door stays open on
    re-entry): `@if (getFlag("EVENT_DOOR_FLAG")) { replaceTileBlock(X, Y, OPEN_ID) }`. (You
    usually only need to open — the map.blk already has the door closed.)
  - **The switch/trigger** that solves it sets the flag AND opens immediately:
    `setFlag("EVENT_DOOR_FLAG")`, `replaceTileBlock(X, Y, OPEN_ID)`, optionally
    `playSound("SFX_GO_INSIDE")`.
  - **API:** `replaceTileBlock(blockX, blockY, blockId)` takes BLOCK coords, X FIRST. The asm
    `lb bc, B, C` is `(Y=B, X=C)`, so an asm `lb bc, 3, 5` → `replaceTileBlock(5, 3, …)`.
  - **Block ids are decimal:** convert the asm hex (`$5`→5, `$24`→36, `$0A`→10).
  - Extract the exact coords + open/closed block ids + gating flag from THIS map's asm.
  - Examples to mine: VermilionGym `SetDoorTile` (door at X2,Y2, open=5/closed=36, flag
    EVENT_2ND_LOCK_OPENED), Silph Co Card-Key gate callbacks, Victory Road Strength-boulder
    switches, Elite-Four room exit-locks, Game Corner hidden-stairs poster.
  - The forced-movement / spinner / teleport / invisible-wall puzzles still have NO API —
    keep those stubbed; only the BLOCK-swap doors are now wireable.

## Stub policy (per the user's "stub side minigames" decision)
For mechanics with no API yet — elevator floor menu, Game Corner slots/coins, Safari Zone
game, in-game trades, fossil revive, daycare, Silph Scope ghost reveal, forced-movement
spinner tiles, cross-map object/flag writes, link/cable-club — do NOT call a non-existent API
(it ReferenceErrors). Instead: translate the DIALOGUE faithfully and either (a) approximate
with real primitives (a trade → `givePokemon` + dialogue; an elevator → `warpTo` a fixed
floor; a received-item → `giveItem`), or (b) skip the mechanic with a `// TODO: <mechanic>
needs <api>` comment. A faithful-dialogue + working-approximation + TODO is the goal; a
broken call is not.

## Output + self-verify (REQUIRED)
1. Write the new `maps/<Map>/script.scene` (replace the broken one). Keep a 1-2 line header
   comment noting it was translated from the disassembly.
2. If the gym-leader pattern applies, edit `maps/<Map>/map.json` to set the leader npc
   `isTrainer:false`.
3. Run the PRE-BUILT binary directly (do NOT use `cargo run` — concurrent cargo
   invocations contend on the build lock and hang): from the workspace root
   (`workspace/`), run
   `./target/debug/scene_apply <Map>` (if that binary is missing, build it once with
   `cargo build -p jrpg-engine-dsl --bin scene_apply`, then use `./target/debug/scene_apply <Map>`).
   It compiles the .scene and regenerates `script_config.json`. ITERATE until it prints `<Map>: ok` (fix any COMPILE
   ERROR it reports — usually a typo, an unknown construct, or a missing `@trigger`).
4. Do NOT edit other maps, the DSL compiler, or engine code.
