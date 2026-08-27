---
name: new-map
description: Add a new map to the Pokémon Red/Blue reimplementation — create the map directory (map.json / map.blk / script.scene / script_config.json), place warps/NPCs/signs/wild encounters, wire map connections, script events in the .scene DSL, and register the map in the Rust tables. Use when the user wants to create, extend, or connect a map, route, city, building, or dungeon.
---

# new-map — Adding a New Map

A map in this project is a directory under `crates/pokered-data/maps/<MapName>/` with four files. `pokered-data/build.rs` scans that directory at compile time and embeds every `map.json` + compiled `script.scene`, so **data work is file work** — no engine fork.

| File | Purpose |
|---|---|
| `map.json` | Header (tileset/music/size), connections, warps, NPCs, signs, per-map text, wild encounters |
| `map.blk` | Raw block layout: exactly `width * height` bytes, one block id per byte, row-major |
| `script.scene` | Event script in the Game DSL (`game_scene`, `@storyline`, `@trigger`, …) |
| `script_config.json` | Binds `textId`s to scene handlers (`talk`), coord events, `onLoad` |

## Workflow

Publish a plan with `update_plan` first — this is a multi-step task. Then:

1. **Create the directory** — call `propose_map_create` with `name` (PascalCase, e.g. `"CinnabarLab"`) plus `tileset`, `width`, `height` (in 2×2-tile blocks), `music`, `borderBlock`, and optionally `townMap: {x, y}` + `displayName`. This scaffolds a valid map dir: `map.json` with the next free numeric `id`, a `map.blk` filled with the border block, an empty `script.scene`, and an empty `script_config.json`.
2. **Flesh out `map.json`** — `read_file` the new `crates/pokered-data/maps/<Name>/map.json`, then `propose_map_file` (file `map.json`) with the complete JSON: warps, npcs, signs, text, wild, connections.
3. **Script the events** — draft `script.scene` following the DSL rules below, run `check_scene` on the draft and fix every FAIL, then `propose_scene_write`. Mirror the handler bindings in `script_config.json` via `propose_map_file` (file `script_config.json`).
4. **Connect to the world** — connections and warps are two-sided; edit the *other* map's `map.json` too (its `connections` entry and/or a return warp).
5. **Hand off the Rust registration** — the propose tools only write data files; the Rust tables below must be edited in the repo before a production `cargo build` sees the map. List them explicitly in your final message.
6. **Verify** — tell the user how to check it (see Verification).

## map.json reference

Field invariants you must respect (the example below is clean JSON — copy it, then fill in):

- `id` — auto-assigned by map creation (max existing id + 1); never reuse an id.
- `name` — MUST equal the directory name.
- `header.width` / `header.height` — in 2×2-tile blocks (the screen is 10×9 blocks = 20×18 tiles).
- `header.connectionFlags` — bit0=East, bit1=West, bit2=South, bit3=North (e.g. 12 = N+S). The `connections` object below is what the engine reads — keep the two in sync.
- `header.borderBlock` — block id used to pad around the map; also the map.blk fill.
- `connections` — omit empty directions.
- NPC `spriteName` — from `crates/pokered-data/src/sprites.rs`. `x`,`y` are TILE coordinates (2×2 tiles per block step). `movement`: Stationary | Wander | FixedPath | FacePlayer; `facing`: Down|Up|Left|Right; `range` = wander radius (sight range for trainers). `textId` is 1-based and matches `text.npc["1"]` and `script_config.json` `npcs[].id`. Set `isTrainer: true` for trainer NPCs (see the new-trainer skill).
- `wild` — `null` when the map has no encounters (shape below).

```json
{
  "$schema": "../../schemas/map.schema.json",
  "id": 248,
  "name": "CinnabarLab",
  "header": {
    "tileset": "Lab",
    "music": "Cinnabar",
    "connectionFlags": 0,
    "width": 10,
    "height": 9,
    "borderBlock": 3
  },
  "connections": {
    "north": { "targetMap": "Route21", "offset": 0 }
  },
  "warps": [
    { "x": 5, "y": 5, "destMap": "PalletTown", "destWarpId": 0 }
  ],
  "npcs": [
    {
      "spriteId": 3,
      "spriteName": "Oak",
      "x": 8,
      "y": 5,
      "movement": "Stationary",
      "facing": "Down",
      "range": 0,
      "textId": 1,
      "isTrainer": false
    }
  ],
  "signs": [ { "x": 13, "y": 13, "textId": 1 } ],
  "text": {
    "npc":  { "1": [ { "line1": "...", "line2": "..." } ] },
    "sign": { "1": [ { "line1": "...", "line2": "..." } ] }
  },
  "wild": null
}
```

**Coordinates.** NPC/sign/warp `x`,`y` are *tile* coordinates (the map is `width*2` × `height*2` tiles). `map.blk` has one byte per *block* (`width*height` bytes).

**Warps are paired by index.** `destWarpId` is the 0-based index into the *destination* map's `warps` array. If the player should be able to walk back, add the return warp to the destination map and make its `destWarpId` point back at this warp's index.

**Connections are reciprocal.** A north connection here needs a south connection on the target map; `offset` shifts the alignment in blocks. Keep widths/heights compatible along the connected edge, and update `connectionFlags` on both maps.

**Wild encounters** (routes/dungeons only; `null` for towns and most interiors). Provide BOTH `red` and `blue` — each with a `grass` and a `water` habitat, and exactly 10 mon slots per habitat (slot order is the encounter-probability table; early slots are most common). Species names are PascalCase and must exist in `crates/pokered-data/pokemon/`.

```json
{
  "red": {
    "grass": { "encounterRate": 25, "mons": [ { "level": 3, "species": "Pidgey" } ] },
    "water": { "encounterRate": 4, "mons": [ { "level": 15, "species": "Psyduck" } ] }
  },
  "blue": {
    "grass": { "encounterRate": 25, "mons": [ { "level": 3, "species": "Pidgey" } ] },
    "water": { "encounterRate": 0, "mons": [] }
  }
}
```

**Tilesets** (`header.tileset`): `Overworld`, `RedsHouse1`, `Mart`, `Forest`, `RedsHouse2`, `Dojo`, `Pokecenter`, `Gym`, `House`, `ForestGate`, `Museum`, `Underground`, `Gate`, `Ship`, `ShipPort`, `Cemetery`, `Interior`, `Cavern`, `Lobby`, `Mansion`, `Lab`, `Club`, `Facility`, `Plateau`. Pick the one whose blockset gives you the walls/floors you need (`Overworld` for outdoor, `House`/`Interior` for homes, `Cavern` for caves, `Gym` for gyms…).

**Music** (`header.music`): `PalletTown`, `Pokecenter`, `Gym`, `Cities1`, `Cities2`, `Celadon`, `Cinnabar`, `Vermilion`, `Lavender`, `SSAnne`, `Routes1`–`Routes4`, `IndigoPlateau`, `SafariZone`, `Dungeon1`–`Dungeon3`, `CinnabarMansion`, `PokemonTower`, `SilphCo`, `OaksLab`, `BikeRiding`, `Surfing`, … (full list: `MusicId` in `crates/pokered-data/src/music.rs`).

## Scripting the map

Minimal `script.scene`:

```
game_scene CinnabarLab {
  @storyline("talkScientist") {
    @trigger(map = "CinnabarLab", npc = 1)
    @speaker("") { @t("Welcome to the lab!", "欢迎来到研究所！") }
  }
}
```

Rules that matter here (full reference: `docs/DSL_TRANSLATION_GUIDE.md`; API typings: `scripts/types/game.d.ts`):

- Every line of player-facing text is bilingual: `@t("english", "中文")`. Gen-1 dialogue uses `@speaker("")` (no name prefix; the name goes in the text).
- `@trigger(map = "<Map>", npc = <textId>)` binds a handler to an NPC; `sign = <textId>` for signs; `@load { }` runs on map entry.
- No `let` — assign with `result = startBattle("OPP_BROCK1")`. Flags are `"EVENT_*"` string literals; check with `getFlag`, set with `setFlag`, and reuse existing flags (`scan_flags`) or follow the `EVENT_*` naming conventions when inventing new ones.
- Useful calls: `giveItem("POTION", 1)`, `givePokemon("EEVEE", 25)`, `startBattle(...)`, `warpTo(map, x, y)`, `heal()`, `showObject(i)` / `hideObject(i)`, `replaceTileBlock(x, y, blockId)` (BLOCK coordinates, X first — not tile coords).
- **Always run `check_scene` on your draft and iterate to PASS before `propose_scene_write`.** Then write `script_config.json` binding every NPC/sign `textId` to its `@storyline` name (`"talk": "talkScientist"`); coord events go in `coordEvents` with a `name` + tile `position` + `trigger`.

## Rust registration (hand-off to the developer — propose tools cannot edit `.rs`)

A new map dir alone is enough for the **editor** (the Map tab lists it; the Play preview injects data via runtime overrides, no rebuild). For the **game binary**, three tables are keyed by the hand-written `MapId` enum and must be extended:

1. `crates/pokered-data/src/maps.rs` — add `MapId::CinnabarLab = 0xF8` (next free value), bump `NUM_MAPS`, append `(width, height)` to `MAP_DIMENSIONS` **in id order**.
2. `crates/pokered-data/src/map_data_loader.rs` — add `("CinnabarLab", include_bytes!("../maps/CinnabarLab/map.blk"))` to `embedded_blk_sources()`.
3. `crates/pokered-data/src/map_names.rs` — add a `map_to_name_id` arm. For a brand-new location name also add a `MapNameId` variant + entries in `MAP_NAME_STRINGS` and `MAP_NAME_STRINGS_ZH`; an indoor map can reuse its parent location's name id.

Optional: `fly_warp_data.rs` (Fly destination) and `town_map_data.rs` (town-map dot). The editor also maintains `crates/pokered-data/town_map_extras.json` for the town-map dot — an editor-side sidecar created on demand (via the New Map dialog or the `townMap` param); it is not read by the Rust build.

## Verification

- In the editor: the map appears in the Map tab immediately after the proposal is applied; open it, paint the blocks, and use Playtest to walk it.
- CLI: `cargo build -p pokered-data` (must succeed — build.rs re-embeds the maps), then
  `cargo run --release --bin pokered-app -- run --skip-intro --warp CinnabarLab,4,4` and
  `cargo run --release --bin pokered-app -- screenshot --screen overworld -o map.png -f 30`.
- `map.blk` must be exactly `width*height` bytes or the map renders garbage.
