---
name: pokered-new-trainer
description: Add or extend trainers in the Pokémon Red/Blue reimplementation — add a party roster to an existing trainer class, create a brand-new trainer class, place trainer NPCs on a map (sight-engaged or talk-driven like gym leaders), and wire battle dialogue in the .scene DSL. Use when the user wants to add a trainer, gym battle, rival fight, or edit a trainer's team.
---

# pokered-new-trainer — Adding Trainers

Trainer data lives in `crates/pokered-data/trainers/<Class>.json` — one file per trainer **class** (`Youngster`, `Brock`, `Rocket`, …), holding an ordered list of **party rosters**. A trainer NPC on a map references one roster by class + 1-based index.

## Pick the right case

- **Case A — a new trainer of an existing class** (a new Bug Catcher on Route 2, a harder Brock rematch): edit the class JSON only. No Rust changes.
- **Case B — a brand-new trainer class** (e.g. `Lady`, a custom boss): JSON + a Rust registration checklist.

## Case A: add a roster to an existing class

1. `read_record(table: "trainers", id: "<Class>")` to get the current file.
2. Append a party to `parties` (1–6 Pokémon, each `{ "level": 1-100, "species": "PascalCase" }` — species must exist in `crates/pokered-data/pokemon/`):
   ```json
   {
     "$schema": "../schemas/trainer.schema.json",
     "class": "BugCatcher",
     "constName": "BUG_CATCHER",
     "parties": [ ...existing parties..., { "pokemon": [ { "level": 9, "species": "Weedle" }, { "level": 9, "species": "Kakuna" } ] } ]
   }
   ```
3. `propose_data_edit(table: "trainers", id: "<Class>", content: <COMPLETE JSON>)` — the new roster's battle id is `OPP_<CONSTNAME><N>` where `N` is its **1-based** position in `parties` (e.g. the 3rd Bug Catcher roster → `OPP_BUG_CATCHER3`).
4. Place the trainer on a map (below).

## Case B: a new trainer class

1. `propose_data_edit(table: "trainers", id: "<NewClass>", content: ...)` with a full record: `class` in PascalCase (`^[A-Z][A-Za-z0-9]+$`), `constName` in SCREAMING_SNAKE, `parties` as above. The file lands at `crates/pokered-data/trainers/<NewClass>.json`.
2. **Rust registration checklist** (hand off — the assistant cannot edit `.rs` files):
   - `crates/pokered-data/src/trainer_data.rs`:
     - `TrainerClass` enum — add `<NewClass> = 48` (next value; classes are 0–47 today);
     - `TrainerClass::from_u8` — widen the bound (`value <= TrainerClass::<NewClass> as u8`);
     - `sprite_name()` — map to a `gfx/trainers/<file>.png` sprite (reuse an existing one or add art);
     - `get_base_money()` — prize-money base (payout = base × level of the trainer's last Pokémon; 99 for leaders/elite, 5–90 otherwise);
     - `trainer_class_name()` — the SCREAMING display name shown in battle ("<NAME> wants to battle!");
     - `parse_trainer_id()` — map the SCREAMING name to the new variant so `startBattle("OPP_<NAME>1")` resolves.
   - `crates/pokered-data/build.rs` — append the class to `CLASS_ORDER` **last** (the array order IS the enum order).
   - `crates/pokered-app/src/render/battle_i18n.rs` — add the Chinese name to `trainer_class_zh`.
   - Sprite: add `gfx/trainers/<file>.png` (a battle pic). Until then, point `sprite_name()` at an existing sprite.

## Placing the trainer on a map

Two patterns (choose per trainer; full guidance in `docs/DSL_TRANSLATION_GUIDE.md` §Map-type patterns):

**Sight-engaged (route grunts, gym trainers).** The NPC spots the player and the runtime drives the battle. In `map.json`:
```json
{ "spriteId": 7, "spriteName": "CooltrainerM", "x": 3, "y": 6,
  "movement": "Stationary", "facing": "Right", "range": 0,
  "textId": 2, "isTrainer": true, "trainerClass": "JrTrainerM", "trainerSet": 1,
  "endBattleText": "Darn!\n\nLight years isn't\ntime! It measures\ndistance!" }
```
`trainerSet` is the 1-based roster index. The `.scene` talk handler is **dialogue only** — pre-battle taunt if not beaten, after-battle line if beaten. Do NOT call `startBattle` in it.

**Talk-driven (gym leaders, scripted bosses).** Set `"isTrainer": false` and let the script own the flow (see `maps/PewterGym/script.scene` for the full worked example):
```
result = startBattle("OPP_BROCK1")
@if (result == "win") {
  @speaker("") { @t("...badge speech...", "……徽章台词……") }
  giveBadge("BOULDERBADGE")
  setFlag("EVENT_BEAT_BROCK")
}
```

**Flags.** Track the win with `EVENT_BEAT_*` (`scan_flags` to reuse existing ones; the project scans all `maps/**` for `getFlag`/`setFlag`, so a flag only exists once a script uses it). Gym trainers of a beaten leader are deactivated with a shared flag (e.g. `EVENT_BEAT_PEWTER_GYM_TRAINER_0`).

## Verify

- `check_scene` every scene draft to PASS before proposing.
- Data-only changes (Case A) need no rebuild for the editor Playtest — the runner injects data overrides. For the game binary: `cargo build -p pokered-data && cargo test -p pokered-data`.
- Try the fight headlessly: `cargo run --release --bin pokered-app -- battle --config sample_battle.json -s result.png` (copy `sample_battle.json` and edit the parties), or warp to the map and talk to the NPC.
