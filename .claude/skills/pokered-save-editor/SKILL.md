# pokered-save-editor — Save State Construction & Manipulation

Use this skill to construct, inspect, and manipulate Pokémon Red/Blue save states programmatically. Covers the JSON snapshot format, the Save Editor GUI, and headless save construction workflows.

## Quick Start

### Launch the Save Editor (GUI)

```bash
cd workspace/tools/pokered-editor
pnpm install   # first time only
pnpm dev       # opens at http://localhost:5173
```

Then click the **"Save"** tab in the top bar to access:
- **Party Editor** — Edit your Pokémon team (species, level, moves, stats)
- **Flag Editor** — Toggle event flags (story progression, badges, hidden items)
- **Item Editor** — Manage bag items and quantities
- **Info Editor** — Set map position, play time, badges, money

### Headless Save Manipulation (CLI)

```bash
cd workspace

# Convert .sav to JSON for inspection/editing
cargo run --release --bin pokered-app -- export-snapshot --input pokered.sav -o snapshot.json

# Edit snapshot.json with any text editor, then convert back
# (import-snapshot goes the other direction: .sav → JSON)
cargo run --release --bin pokered-app -- import-snapshot --input pokered.sav -o snapshot.json

# Launch game with edited snapshot
cargo run --release --bin pokered-app -- run --snapshot snapshot.json --skip-intro
```

## Save Data JSON Schema

The JSON snapshot format mirrors the `SaveData` struct. Here's the complete schema:

```json
{
  "player_name": [82, 69, 68, 0, ...],
  "game_data": { ... },
  "party": [ ... ],
  "current_box": { ... },
  "pc_storage": { ... },
  "hall_of_fame": { ... },
  "script_flags": { ... }
}
```

### `player_name`

Array of u8 bytes in the game's custom charmap encoding. Use `pokered_data::charmap` to encode/decode.

### `game_data` (selected fields)

| Field | Type | Description |
|-------|------|-------------|
| `position` | `{map_id, x, y, x_block, y_block}` | Player's map and coordinates |
| `player_direction` | u8 | 0=Down, 4=Up, 8=Left, 12=Right |
| `play_time` | `{hours, minutes, seconds, frames, maxed}` | Game play time |
| `obtained_badges` | u8 | Bitfield: bit0=Boulder, bit1=Cascade, ..., bit7=Earth |
| `player_money` | u32 | Player money (max 999,999) |
| `player_coins` | u16 | Casino coins |
| `bag` | `{items: [[item_id, qty], ...]}` | Bag inventory |
| `pokedex` | Pokedex | Seen/owned bitfields |
| `rival_name` | Vec<u8> | Rival's name (charmap encoded) |
| `player_starter` | u8 | Starter species ID |
| `rival_starter` | u8 | Rival's starter species ID |
| `event_flags` | Vec<u8> | Raw event flag bytes |
| `game_progress_flags` | Vec<u8> | Game progress flags |

### `party` — Pokémon Team

Array of up to 6 Pokémon:

```json
{
  "species": "Bulbasaur",
  "level": 5,
  "current_hp": 21,
  "status": "None",
  "type1": "Grass",
  "type2": "Poison",
  "catch_rate": 45,
  "moves": [
    {"MoveId": "Tackle"},
    {"MoveId": "Growl"}
  ],
  "original_trainer_id": 12345,
  "experience": 125,
  "hp_exp": 0,
  "attack_exp": 0,
  "defense_exp": 0,
  "speed_exp": 0,
  "special_exp": 0,
  "dvs": {"raw": 39480},
  "pp": [35, 40, 0, 0],
  "stats": {
    "max_hp": 21,
    "attack": 11,
    "defense": 11,
    "speed": 11,
    "special": 13
  }
}
```

**Key fields:**
- `species`: PascalCase name (e.g., "Bulbasaur", "Charizard", "Mewtwo")
- `level`: 1–100
- `moves`: Array of up to 4 moves. Use the MoveId variant name (e.g., "Tackle", "Thunderbolt")
- `dvs.raw`: Determinant Values encoded as u16 (atk/def/spd/spc, 4 bits each)
- `stats`: Calculated stats (max_hp, attack, defense, speed, special)

### `script_flags` — Event Flags

HashMap of flag names to booleans:

```json
{
  "EVENT_GOT_STARTER": true,
  "EVENT_GOT_POKEDEX": false,
  "EVENT_GOT_BOULDERBADGE": true,
  "EVENT_BEAT_BROCK": true,
  "EVENT_GOT_HM01": false
}
```

There are 507 defined event flags. Key categories:

| Prefix | Category |
|--------|----------|
| `EVENT_GOT_*` | Received items, badges, HMs |
| `EVENT_BEAT_*` | Defeated trainers/gym leaders |
| `EVENT_*_APPEARED` | NPC appearance triggers |
| `EVENT_*_GONE` | NPC disappearance triggers |
| `EVENT_FOLLOWED_*` | Follow sequences (e.g., Oak's lab) |
| `EVENT_*_DOOR_OPEN` | Door/gate states |

For the complete flag list, see `pokered-data/src/event_flags.rs`.

## Common Headless Workflows

### Construct a test save with specific party

```bash
# 1. Export a template save
cargo run --release --bin pokered-app -- export-snapshot -o template.json

# 2. Edit template.json to set up your party
# Example: give player 6 level 100 Mewtwos with Psychic

# 3. Launch with the constructed save
cargo run --release --bin pokered-app -- run --snapshot template.json --skip-intro --warp CeruleanCity
```

### Set specific event flags for story testing

```json
// In snapshot.json, add to script_flags:
{
  "EVENT_GOT_STARTER": true,
  "EVENT_GOT_POKEDEX": true,
  "EVENT_GOT_BOULDERBADGE": true,
  "EVENT_GOT_CASCADEBADGE": true,
  "EVENT_GOT_THUNDERBADGE": true,
  "EVENT_BEAT_BROCK": true,
  "EVENT_BEAT_MISTY": true,
  "EVENT_BEAT_LT_SURGE": true
}
```

### Create a save with specific items

```json
// In snapshot.json, under game_data.bag.items:
[
  [1, 99],    // 99 Master Balls (item_id=1)
  [15, 50],   // 50 Ultra Balls
  [20, 99],   // 99 Rare Candies
  [51, 10],   // 10 Full Restores
  [68, 1]     // 1 Bicycle
]
```

Item IDs are defined in `pokered-data/src/items.rs`. The `ItemId` enum maps numeric IDs to names.

## TCP Debug Server (Feature 4)

When the game is running with `--debug-port 9876`, you can send JSON commands over TCP:

```bash
# Get player position
echo '{"cmd":"get_position"}' | nc localhost 9876
# → {"ok":true,"data":{"map_id":0,"map_name":"PalletTown","x":10,"y":14,"facing":"Down"}}

# Warp to a map
echo '{"cmd":"warp","map":"CeruleanCity","x":14,"y":8}' | nc localhost 9876
# → {"ok":true}

# Get party info
echo '{"cmd":"get_party"}' | nc localhost 9876
# → {"ok":true,"data":[...]}

# Set event flag
echo '{"cmd":"set_flag","name":"EVENT_GOT_POKEDEX","value":true}' | nc localhost 9876
# → {"ok":true}
```

Requires building with: `cargo run --release --bin pokered-app --features debug-server -- run --debug-port 9876`

## Reference Files

- Save struct definition: `crates/pokered-core/src/save/mod.rs`
- GameData struct: `crates/pokered-core/src/save/game_data.rs`
- Party struct: `crates/pokered-core/src/pokemon/party.rs`
- Event flags: `crates/pokered-data/src/event_flags.rs`
- Item IDs: `crates/pokered-data/src/items.rs`
- Species IDs: `crates/pokered-data/src/species.rs`
- Move IDs: `crates/pokered-data/src/moves.rs`
