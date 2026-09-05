# pokered-save-editor — Save State Construction & Manipulation

Use this skill to construct, inspect, and manipulate Pokémon Red/Blue save states programmatically. Covers the JSON snapshot format, the Save Editor GUI, and headless save construction workflows.

## Quick Start

### Constructing saves from Python tests

For test-state construction (the playthrough/BDD stack), prefer
`scripts/save_builder.py` over hand-editing JSON: it exports a canonical
template from a freshly booted game, computes Gen-1 party stats with the
engine's own formula, and parses flag/item/map tables from this repo's
data files. `--preset champion` builds a finished-first-playthrough
state (story flags, 8 badges, full dex). See the playthrough-regression
skill, "Constructed saves". The GUI/CLI flows below remain the way to
inspect and hand-tune saves.

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
  "hall_of_fame": { ... }
}
```

> `script_flags` was removed from `SaveData` (fixed-memory refactor): named
> event flags live in `game_data.event_flags` (320-byte bitset) and
> runtime-only extras (`__OBJ_HIDDEN_*` etc.) are kept outside SaveData (native
> sidecar `pokered.script_flags.json`, editor localStorage key
> `pokered-editor-script-flags`). Old JSON with `script_flags` still loads
> (unknown fields are ignored) — flag edits made that way are silently dropped.

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
  "nickname": [129, 148, 139, 129, 128, 146, 128, 148, 145, 80, 80],
  "level": 5,
  "hp": 21,
  "max_hp": 21,
  "attack": 11,
  "defense": 11,
  "speed": 11,
  "special": 13,
  "type1": "Grass",
  "type2": "Poison",
  "moves": ["Tackle", "Growl", "Pound", "Pound"],
  "pp": [35, 40, 0, 0],
  "pp_ups": [0, 0, 0, 0],
  "status": "None",
  "dv_bytes": [154, 218],
  "stat_exp": [0, 0, 0, 0, 0],
  "total_exp": 125,
  "is_traded": false,
  "ot_id": 0,
  "ot_name": [80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80]
}
```

**Key fields:**
- `species`: PascalCase name (e.g., "Bulbasaur", "Charizard", "Mewtwo")
- `nickname` / `ot_name`: charmap-encoded byte arrays (`NameBytes`, max 10 chars,
  0x50 = no-name padding). On deserialize, legacy `null` or decoded-string
  values are still accepted.
- `moves`: Array of up to 4 MoveId variant names (e.g., "Tackle", "Thunderbolt")
- `dv_bytes`: two bytes packing the 4-bit DVs (high nybble = Atk/Spd, low nybble =
  Def/Spc; HP IV is derived from the low bits)
- `stat_exp`: [hp, atk, def, spd, spc] stat experience
- `ot_id`: 0 = unknown/own mon; a mon is "traded" iff `ot_id != 0 && ot_id != player_id`

### `event_flags` — Event Flags (bitset)

Named event flags live in `game_data.event_flags`: a **320-byte bit array**
(`EVENT_FLAGS_SIZE`, matching the original wEventFlags at SRAM $A00).
To set a flag in a snapshot, flip the right bit — the per-flag
`byte_offset`/`bit_mask` const tables live in `pokered-data/src/event_flags.rs`
(507 defined flags, max bit 0x9DA):

```python
# e.g. EVENT_GOT_STARTER — look up byte/bit from event_flags.rs, then:
flags = bytearray(save["game_data"]["event_flags"])
flags[0x1A] |= 0x08   # example: byte 26, bit 3
save["game_data"]["event_flags"] = list(flags)
```

Runtime-only keys (`__OBJ_HIDDEN_*`, `__OBJ_SHOWN_*`) are NOT in the bitset;
they live in the overworld's extras map, persisted by the native app to
`pokered.script_flags.json` and by the editor to its own localStorage key.

Key flag categories:

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

`script_flags` no longer exists in the snapshot — set named flags via the
`game_data.event_flags` bitset (see the `event_flags` section above), or use
the editor's FlagEditor / the debug-server `SetFlag` command. A Python helper
snippet:

```python
import json

FLAG_BITS = {  # byte_offset, bit_mask — derived from event_flags.rs enum values
    "EVENT_GOT_STARTER": (4, 0x04),          # bit 0x022
    "EVENT_GOT_POKEDEX": (4, 0x20),          # bit 0x025
    "EVENT_BEAT_BROCK": (0x0E, 0x80),        # bit 0x077
    "EVENT_BEAT_MISTY": (0x17, 0x80),        # bit 0x0BF
    "EVENT_BEAT_LT_SURGE": (0x2C, 0x80),     # bit 0x167
}
with open("snapshot.json") as f:
    save = json.load(f)
flags = bytearray(save["game_data"]["event_flags"])
for name, (byte, mask) in FLAG_BITS.items():
    flags[byte] |= mask
save["game_data"]["event_flags"] = list(flags)
with open("snapshot.json", "w") as f:
    json.dump(save, f)
```

> The `EventFlag` enum values ARE the bit indices (`byte = bit >> 3`,
> `mask = 1 << (bit & 7)`); recompute offsets from
> `pokered-data/src/event_flags.rs` — the enum is the source of truth.
> Gym badges are NOT event flags — they are the `game_data.obtained_badges`
> bitfield (bit0=Boulder … bit7=Earth).

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
