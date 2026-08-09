# endbattle_text converter

`convert_endbattle_text.py` populates each map's trainer-NPC `endBattleText`
field in `examples/pokered/crates/pokered-data/maps/*/map.json`, converting the
one-shot victory quips from the original [`pret/pokered`](https://github.com/pret/pokered)
disassembly.

## What it does

For each `scripts/<Map>.asm` in the baseline it reads the `trainer` macro
headers in file order (the 4th macro arg is the `TextEndBattle` label), resolves
`<label>: text_far _X` to the `_X::` text block in `text/<Map>*.asm`, and
assembles the `text`/`line`/`cont`/`para` macros into a string (`\n` for a line
break, `\n\n` for a paragraph). The Nth trainer header maps to the Nth
`isTrainer: true` NPC in that map's `map.json` (an ordinal join — verified
against VermilionGym; class names differ between the baseline and `map.json`,
but object order matches).

Legendary encounters that (ab)use the `trainer` macro (e.g. `EVENT_BEAT_MOLTRES`
in VictoryRoad2F) are not `isTrainer` NPCs, so the extra trailing header is
harmlessly dropped by the join.

## Usage

```bash
# Dry-run: report coverage + the VermilionGym sanity check, write nothing.
python3 tools/endbattle_text/convert_endbattle_text.py

# Write the endBattleText fields into every map.json (idempotent).
python3 tools/endbattle_text/convert_endbattle_text.py --write
```

Set `BASE` at the top of the script to the local `pret/pokered` checkout
(default: `/Users/liuyanghe02/develop/pokered-worktree`). Writes preserve each
file's exact formatting (2-space indent, original trailing-newline state), so
the diff is limited to the added `endBattleText` lines.

The runtime consumes the field via `NpcJson`/`StaticNpcJson.end_battle_text`
→ `PokemonNpcData.end_battle_text` → `PendingTrainerBattle` → `BattleScreen`,
shown in the victory sequence before the prize-money text (see the
`VictoryPhase::EndBattleText` arm in `pokered-core/src/battle/mod.rs`).
