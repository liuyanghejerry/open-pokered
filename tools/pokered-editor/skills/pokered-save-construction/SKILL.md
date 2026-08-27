---
name: pokered-save-construction
description: Construct, inspect, and manipulate Pokémon Red/Blue save states — the editor's Save tab, the full JSON snapshot schema (party, bag, badges, event-flag bitset), headless save construction via pokered-app export/import-snapshot, and live save surgery through the TCP debug server. Use when the user wants a save at a specific story point, a test save for new content, party/item/flag edits, or save debugging.
---

# pokered-save-construction — Building Save States

Three layers, pick per goal:

| Goal | Tool |
|---|---|
| Quick edits + playtest inside the editor | **Save tab** (Info / Party / Flags / Items editors, JSON import/export) |
| A full-fidelity save file for the real game | **JSON snapshot** via `pokered-app export-snapshot` / `import-snapshot` |
| Tweak a *running* game | **TCP debug server** (`set_flag`, `give_item`, `give_pokemon`, `warp`, `save`) |

The assistant has no save-file propose tool — produce JSON in chat for the user to import, or walk them through the CLI below.

## Layer 1: the editor Save tab

The Save activity edits a simplified snapshot (`SaveDataSnapshot`): `player` (name, rival, mapName, x/y, facing, money, coins, play time), `badges` (8 booleans, Boulder→Earth order), `party` (up to 6 × `{species, level, currentHp, maxHp, moves[4], nickname}`), `items` (`{name, quantity}`), and `flags` (name → bool). **Import JSON** accepts that shape; **Export** downloads it. Pair it with the Playtest activity to spawn the game at that state.

## Layer 2: the full JSON snapshot

The game binary's own format (mirrors `SaveData` in `crates/pokered-core/src/save/`):

```bash
# .sav → editable JSON, and back
cargo run --release --bin pokered-app -- export-snapshot --input pokered.sav -o snapshot.json
cargo run --release --bin pokered-app -- import-snapshot --input pokered.sav          # writes back into the .sav
# launch on it
cargo run --release --bin pokered-app -- run --snapshot snapshot.json --skip-intro --warp PalletTown,10,14
```

Key fields:

- **`game_data.position`** — `{map_id, x, y, x_block, y_block}`; **`player_direction`** 0=Down 4=Up 8=Left 12=Right.
- **`game_data.obtained_badges`** — u8 bitfield, bit0=Boulder … bit7=Earth. (Badges are NOT event flags.)
- **`game_data.player_money`** (≤999999), **`player_coins`**, **`game_data.bag.items`** as `[[item_id, qty], …]` — numeric item ids from `crates/pokered-data/src/items.rs` (1=Master Ball, 15=Ultra Ball, 20=Rare Candy, 68=Bicycle…).
- **`party`** — up to 6 full Pokémon records: `species` (PascalCase), `level`, `hp`/`max_hp`/`attack`/`defense`/`speed`/`special`, `type1`/`type2`, `moves` (4 MoveId names, pad with a repeat), `pp[4]`, `dv_bytes[2]` (Atk/Spd high nybbles, Def/Spc low), `stat_exp[5]`, `total_exp`, `status`, `ot_id` (0 = own), plus charmap-encoded `nickname`/`ot_name` byte arrays (`0x50` = padding; see `pokered_data::charmap`).
- **`game_data.event_flags`** — the story flags: a **320-byte bitset** (507 named flags in `crates/pokered-data/src/event_flags.rs`; the enum value IS the bit index). Flip bits, don't append:

```python
import json
save = json.load(open('snapshot.json'))
flags = bytearray(save["game_data"]["event_flags"])
bit = 0x025                        # e.g. EVENT_GOT_POKEDEX — look the value up in event_flags.rs
flags[bit >> 3] |= 1 << (bit & 7)  # byte = bit >> 3, mask = 1 << (bit & 7)
save["game_data"]["event_flags"] = list(flags)
json.dump(save, open('snapshot.json', 'w'))
```

Common progression flags: `EVENT_GOT_STARTER`, `EVENT_GOT_POKEDEX`, `EVENT_BEAT_BROCK/MISTY/LT_SURGE/…`, `EVENT_GOT_SS_TICKET`, `EVENT_BEAT_*_GYM_TRAINER_*`. The enum in `event_flags.rs` is the source of truth.

## Layer 3: live surgery via the debug server

```bash
cargo run --release --bin pokered-app --features debug-server -- run --headless --debug-port 9000 --skip-intro
```

JSON-lines over TCP (`scripts/debug_drive.py` is a minimal client):

```python
from debug_drive import DebugClient
d = DebugClient(9000)
d.cmd(cmd="give_pokemon", species="Mewtwo", level=70)
d.cmd(cmd="give_item", item="MASTER_BALL", quantity=3)
d.cmd(cmd="set_flag", name="EVENT_BEAT_BROCK", value=True)
d.cmd(cmd="warp", map="CeruleanCity", x=14, y=8)
d.cmd(cmd="save")                      # persist from inside the running game
print(d.cmd(cmd="get_state")["data"])  # position, party, flags, dialogue, battle phase
```

## Recipe: a save that exercises NEW content

To test-drive a new map / species / trainer (the other skills):

1. Export a template snapshot (or take one from a playthrough near the target).
2. Set `position` to a map adjacent to the new one (or warp in-game with `--warp <NewMap>,x,y` — the new map needs its Rust registration done, see the pokered-new-map skill; in the editor Playtest the runtime override covers it without a rebuild).
3. Put the new species in `party` (or plan to `give_pokemon` it live) with coherent stats: recompute `max_hp`/stats for its level from its `baseStats`, set `total_exp` on its `growthRate` curve, `pp` from the moves' base PP.
4. Set only the event flags your test needs — prefer the debug server's `set_flag` for iteration speed.

## Gotchas

- `script_flags` was removed from `SaveData` — old snapshots carrying it still load, but those edits are silently dropped. Named flags = `event_flags` bitset only; runtime-only `__OBJ_HIDDEN_*`/`__OBJ_SHOWN_*` keys live outside the save (`pokered.script_flags.json` next to the native app).
- `event_flags` must stay a 320-byte array — the loader sizes it exactly (`EVENT_FLAGS_SIZE`).
- A party mon's `hp` may not exceed `max_hp`; keep `moves`/`pp` lengths at 4.
- charmap name fields are byte arrays, not strings — decode with `pokered_data::charmap` if you need to read them.
