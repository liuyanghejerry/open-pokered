# pokered-debug — Headless Debug & Testing CLI

Use this skill to rapidly test and debug the workspace game engine via the command line. Covers all CLI subcommands including warp-to-map, skip-intro, save/snapshot manipulation, direct battles, screenshots, and state dumping.

## Quick Start

```bash
cd workspace

# Normal game launch
cargo run --release --bin pokered-app

# Skip intro, warp directly to a map
cargo run --release --bin pokered-app -- run --skip-intro --warp PalletTown,10,14

# Warp to Cerulean City with save file
cargo run --release --bin pokered-app -- run --save path/to/pokered.sav --warp CeruleanCity,14,8

# Load a JSON snapshot instead of .sav
cargo run --release --bin pokered-app -- run --snapshot path/to/snapshot.json --skip-intro
```

## CLI Reference

### `pokered run` — Launch the game

```
pokered run [OPTIONS]
```

| Flag | Type | Description |
|------|------|-------------|
| `--save <PATH>` | PathBuf | Load SRAM .sav file at startup |
| `--snapshot <PATH>` | PathBuf | Load JSON snapshot at startup |
| `--skip-intro` | bool | Skip Copyright→Title→MainMenu→OakSpeech, start at Overworld |
| `--warp <STRING>` | String | Warp to map/coordinates. Format: `"MapName[,x,y]"`. Examples: `PalletTown,10,14`, `CeruleanCity` |

**Map names** use PascalCase identifiers: `PalletTown`, `ViridianCity`, `PewterCity`, `CeruleanCity`, `VermilionCity`, `LavenderTown`, `CeladonCity`, `FuchsiaCity`, `SaffronCity`, `CinnabarIsland`, `IndigoPlateau`, `Route1`–`Route25`, etc. (all 248 maps supported).

**Warp coordinates** are optional. If omitted, the player spawns at the default position for that map (first warp point or center).

**Combined usage**: `--skip-intro --warp` skips the intro AND positions the player at the specified map/coordinates.

### `pokered export-snapshot` — Save → JSON

```
pokered export-snapshot [OPTIONS]

Options:
  --input <PATH>   Input .sav file (defaults to auto-detected pokered.sav)
  -o, --output <PATH>  Output JSON file [default: snapshot.json]
```

Converts a SRAM `.sav` file into a human-readable JSON snapshot. Useful for inspecting save state or preparing test fixtures.

### `pokered import-snapshot` — JSON → Save

```
pokered import-snapshot --input <PATH> [-o <PATH>]

Options:
  -i, --input <PATH>   Input .sav file (required)
  -o, --output <PATH>  Output JSON file [default: snapshot.json]
```

Reverse of `export-snapshot` — reads a `.sav` file and produces a JSON snapshot. Use this to convert game saves into the JSON format that the Save Editor and `--snapshot` flag understand.

### `pokered battle` — Direct Battle Mode

```
pokered battle --config <PATH> [OPTIONS]

Options:
  -c, --config <PATH>    Battle config JSON file
  -s, --screenshot <PATH>  Save screenshot to PNG instead of opening window
  --frames <N>           Frames to advance before screenshot [default: 5]
```

Launches a battle directly from a JSON configuration, bypassing all menus and story. The config format is documented in `sample_battle.json`.

### `pokered screenshot` — Capture Screen

```
pokered screenshot --screen <TARGET> [-o <PATH>] [-f <N>]

Options:
  -s, --screen <TARGET>  Screen to capture (see below)
  -o, --output <PATH>    Output PNG [default: screenshot.png]
  -f, --frames <N>       Frames to advance [default: 5]
```

Screen targets: `copyright`, `title`, `main-menu`, `oak`, `overworld`, `battle`, `start-menu`, `options`, `save`

### `pokered screenshot-all` — Capture All Screens

```
pokered screenshot-all [-o <DIR>] [-f <N>]

Options:
  -o, --output-dir <DIR>  Output directory [default: screenshots]
  -f, --frames <N>        Frames per screen [default: 5]
```

Captures PNG screenshots of all game screens in one run.

### `pokered dump-state` — JSON State Dump

```
pokered dump-state --screen <TARGET> [-f <N>]

Options:
  -s, --screen <TARGET>  Screen to dump
  -f, --frames <N>       Frames to advance [default: 0]
```

Dumps game state as JSON to stdout. Includes: screen, map_id, map_name, player position, battle status, player/rival names, frame count.

## Common Workflows

### Test a specific map location

```bash
# Jump to Viridian City without going through intro
cargo run --release --bin pokered-app -- run --skip-intro --warp ViridianCity,16,20
```

### Debug save state

```bash
# Export save to inspect
cargo run --release --bin pokered-app -- export-snapshot --input pokered.sav -o debug.json

# Edit debug.json manually, then load it
cargo run --release --bin pokered-app -- run --snapshot debug.json --skip-intro
```

### Capture battle screenshots

```bash
# Capture the battle screen
cargo run --release --bin pokered-app -- screenshot --screen battle -o battle.png -f 10

# Run a configured battle
cargo run --release --bin pokered-app -- battle --config my_battle.json --screenshot result.png
```

### Headless state inspection

```bash
# Dump overworld state to JSON
cargo run --release --bin pokered-app -- dump-state --screen overworld -f 30 > state.json

# Parse with jq
cat state.json | jq '.player_x, .player_y, .map_name'
```

## Advanced: Debug Logging

Enable per-module debug logging:

```bash
cargo run --release --bin pokered-app -- --debug-modules save,overworld,battle run
```

Available modules: `save`, `overworld`, `battle`, `menu`, `audio`, `warp`, `event`, `render`, `all`.

Logs are written to `pokered-debug.log` in the current directory.

## Global Flags

| Flag | Description |
|------|-------------|
| `--debug-modules <MODULES>` | Enable debug logging (comma-separated modules) |
| `--scripts-dir <PATH>` | Custom scripts directory path |

## TCP Debug Server (scripted driving)

Build with the `debug-server` feature and start the game with `--debug-port`. For CI/scripted tests prefer `--headless` (no window; the game loop runs without rendering):

```bash
cargo run --release --bin pokered-app --features debug-server -- \
  run --headless --debug-port 9000 --skip-intro --warp PalletTown,10,5
```

Commands are JSON lines over TCP (`{"cmd": "..."}`). Key commands for testing:

| Command | Purpose |
|---------|---------|
| `step_frames {count}` | **Synchronously** advance N frames before responding — deterministic frame control. Queued `press`/`press_sequence` inputs are consumed one per stepped frame |
| `run_frames {count}` | Schedule N frames on the real-time loop (legacy; window throttling makes it unreliable for tests) |
| `wait_until {condition,max_frames}` | **Synchronously** step frames until a condition holds (checked after every frame) or the budget elapses — one round trip replaces a poll-every-N-frames loop. Response: `{reached, stepped, state}`. Conditions: `dialogue_done`, `dialogue_ready`, `choice_open`, `choice_closed`, `script_idle`, `control_ready` (control back after a cutscene), `not_battle`, and the generic `screen=<name>` / `battle_phase=<name>` / `script_effect=<name>` forms (Debug variant names). Unknown conditions error immediately |
| `skip_dialogue` | Advance the active dialogue box to completion with engine-internal A taps (skip typing, advance all pages, close) so a script suspended on the text resumes; no-op when no dialogue is showing. Queued `press` inputs are dropped first. Response: `{stepped, dialogue_closed, state}` |
| `get_state` | Screen, map, player pos/facing, `active_script_effect` (label), full `script_effect` payload, `dialogue_state` (page/total/char progress/waiting-for-input), `choice` (menu options+cursor), `script_running`, `script_awaiting_battle`, `player_movement_state`, `battle_phase`/`battle_message`, `party` (species/level/hp/status/moves/pp per mon), money/coins, `pc_phase`, `text_speed_delay_frames`, `warp_fade` |
| `get_npcs` | Every NPC on the map: position, home, visibility, facing, `scripted_path_remaining`, walk counter |
| `press` / `press_sequence` | Inject button input (one button per frame) |
| `warp {map,x,y}` | Warp through the real warp path (reloads scripts/triggers) |
| `set_flag` / `give_item` / `give_pokemon` / `start_wild_battle` | State seeding |

**Driving dialogue/cutscenes deterministically:** `dialogue_state.waiting_for_input` tells
you exactly when the current page is fully revealed (press A once per page), and
`script_effect` carries progress fields (e.g. `Delay.frames_remaining`,
`FollowNpc.phase`, `ShowChoice.selected`). For long flavor text, `skip_dialogue`
collapses whole conversations into one round trip; `wait_until("control_ready")`
returns once the cutscene has finished and the player has control again.

Recommended driving pattern (deterministic):

```python
import sys; sys.path.insert(0, "workspace/scripts")
from debug_drive import DebugClient

d = DebugClient(9000)
d.drive(["up"] * 40)            # queue 40 ups, step exactly 40 frames
st = d.state()                  # {'player_x': 10, 'player_y': 1, 'active_script_effect': 'ShowDialogue', ...}
print(st["map_name"], st["player_x"], st["player_y"], st["active_script_effect"])
print([(n["x"], n["y"], n["visible"]) for n in d.npcs()])
```

`workspace/scripts/debug_drive.py` is the minimal client library (also runnable directly as a smoke test: `python3 debug_drive.py --port 9000`).
