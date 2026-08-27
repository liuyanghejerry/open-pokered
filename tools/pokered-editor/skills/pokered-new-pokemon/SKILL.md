---
name: pokered-new-pokemon
description: Add a new Pokémon species to the Pokémon Red/Blue reimplementation — create the species JSON (base stats, types, growth, moves, TM/HM flags, Pokédex entry, evolutions, learnset), understand the auto-assigned species id, handle sprites/cries/names, and make the species obtainable via wild encounters, gifts, or trainers. Use when the user wants to add, rebalance, or evolve a Pokémon.
---

# pokered-new-pokemon — Adding a New Pokémon

One species = one file: `crates/pokered-data/pokemon/<Species>.json`. At build time `pokered-data/build.rs::generate_species_enum` regenerates the `Species` enum from these files: the canonical 151 keep dex ids 1–151 and **new species are appended as id 152+ in filename order**. No table edits are needed for the data to build — but see "Names, sprites, cries" for the presentation layer.

## Naming rules

- PascalCase, `/^[A-Z][A-Za-z]+$/` — no digits, no underscores (the name becomes a Rust enum variant). `None` is reserved.
- The `species` field MUST equal the filename (`Bulbasaur.json` ↔ `"species": "Bulbasaur"`) — build.rs asserts this.
- Scene scripts reference it SCREAMING_SNAKE (`givePokemon("MR_MIME", 5)`); data files use PascalCase.

## Species JSON — full field guide

```json
{
  "$schema": "../schemas/pokemon.schema.json",
  "species": "Newmon",
  "baseStats": { "hp": 60, "attack": 65, "defense": 55, "speed": 70, "special": 60 },
  "type1": "Water",
  "type2": "Water",                       // single-typed: repeat type1 (matches the ROM layout)
  "catchRate": 190,                       // 0 = uncatchable … 255 = easiest
  "baseExp": 80,                          // EXP yield when defeated
  "growthRate": "MediumFast",             // MediumFast | SlightlyFast | SlightlySlow | MediumSlow | Fast | Slow
  "initialMoves": ["Tackle", "TailWhip", "None", "None"],   // EXACTLY 4, "None" pads empty slots
  "tmHmFlags": [164, 3, 56, 192, 3, 8, 6],
  "pokedex": {
    "category": "NEWT",                   // ≤ 11 chars, uppercase ASCII (the label box width)
    "heightFeet": 2, "heightInches": 0,
    "weightDecipounds": 190,              // tenths of a pound: 190 = 19.0 lbs
    "flavorTextPages": [ "A strange seed\nthat grows with\nlove.", "..." ]   // 0–4 pages, '\n' = line break
  },
  "evolutions": [
    { "method": "level", "species": "Newtitan", "level": 20 },
    { "method": "item",  "species": "Newtide",  "item": "WaterStone", "minLevel": 1 },
    { "method": "trade", "species": "Newlord",  "minLevel": 1 }
  ],
  "learnset": [
    { "level": 8,  "moveId": "Bubble" },
    { "level": 15, "moveId": "WaterGun" }
  ]
}
```

- **Types** — one of `Normal Fighting Flying Poison Ground Rock Bird Bug Ghost Fire Water Grass Electric Psychic Ice Dragon`.
- **`tmHmFlags`** — 7 bytes; bit *n* of byte *i* means TM/HM number `i*8 + n + 1` is learnable (TM01…TM50, then HM01…HM05). Easiest reliable method: copy the bytes of a species with similar coverage (`read_record(table: "pokemon", id: ...)` a few) and flip from there.
- **`initialMoves` vs `learnset`** — `initialMoves` are what a level-1 (or freshly generated low-level) mon knows; `learnset` is the level-up table, ordered by level. Move names are PascalCase `MoveId`s (`list_records(table: "moves")` to browse; note ROM casing like `Thunderpunch`, `PsychicM`).
- **Evolution targets** must themselves exist (or be created in the same batch). For an evolution *into* your new species, edit the base species' `evolutions` array too.

## Create it

`propose_data_edit(table: "pokemon", id: "<Species>", content: <COMPLETE JSON>)` — creates `pokemon/<Species>.json` (a `"_file"` key is never part of the record). To revise an existing species, `read_record` first and preserve untouched fields. The editor also has a dedicated Pokémon tab with a guided form, and `POST /api/pokemon {name}` creates a valid template if the user prefers clicking.

## Names, sprites, cries (the presentation layer)

Data builds without these, but the game shows fallbacks until they land:

- **Display name** — `crates/pokered-data/src/lang_data.rs`: add the English name to the `species_name_en` match and the Chinese name to `SPECIES_ZH` (indexed by species id). Fallback: `???`. (Rust edit — hand off.)
- **Battle sprites** — `gfx/pokemon/front/<name>.png` and `gfx/pokemon/back/<name>b.png`, where `<name>` is the species name lowercased with spaces/dashes/apostrophes stripped (`MrMime` → `mr.mime`). Missing files degrade gracefully (blank sprite), but ship them: draw in the editor's Pixel tab or use AI sprite generation.
- **Cry** — `crates/pokered-data/src/cries.rs` has a `_ =>` default, so new species get a generic cry; optionally add a `CryData` row (Rust edit).
- **Party icon** — `mon_party_icons.rs` fallback applies automatically.

## Make it obtainable

A species file alone changes nothing in-game. Wire at least one of:

- **Wild encounters** — add it to a map's `wild.red`/`wild.blue` grass/water slots in `map.json` (see the pokered-new-map skill).
- **Gift / event** — `givePokemon("NEWMON", 15)` in a `.scene` storyline.
- **Trainers** — add it to rosters in `trainers/*.json` (see the pokered-new-trainer skill).
- **Evolution** — listed in a base species' `evolutions`.
- **Debug** — `give_pokemon` over the TCP debug server for quick testing.

## Verify

- `cargo build -p pokered-data` (regenerates `Species` + base-stats + evos/learnsets + Pokédex tables; a malformed file fails here with a clear panic).
- In game: `cargo run --release --bin pokered-app --features debug-server -- run --headless --debug-port 9000 --skip-intro`, then `{"cmd":"give_pokemon","species":"Newmon","level":10}` and `{"cmd":"get_party"}`.
- Show it in the Pokédex by setting the seen/owned bits — or just battle it: `sample_battle.json` + `pokered-app battle --config ...`.
