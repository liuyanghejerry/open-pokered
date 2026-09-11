//! Integration tests for the overworld Start menu (party / bag / save), the
//! `openShop` shop flow, the game-over whiteout, and save v3 (money) — all
//! against the committed fixture project (`tests/fixtures/demo/`).
//!
//! All tests drive the game windowless through the public
//! [`RunnerGame::update`]/[`RunnerGame::draw`] API with synthetic input — no
//! winit, no pixels. Tilesets are generated in code (mirrors `battle.rs`).

use crate::alloc_prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use dotzuki_engine::overworld::types::Direction;
use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_runner::{LoadedProject, RunnerGame, RunnerOptions};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// Unique temp directory, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(test: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "dotzuki-runner-menus-{test}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TestDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// Write a 64×16 `tileset.png` (four 16×16 flat-colour tiles, row-major).
fn write_tileset(map_dir: &Path) {
    let tile = 16u32;
    let mut img = image::RgbaImage::new(tile * 4, tile);
    for (i, &[r, g, b, a]) in [
        [0xFF, 0x00, 0x00, 0xFF],
        [0x00, 0xFF, 0x00, 0xFF],
        [0x00, 0x00, 0xFF, 0xFF],
        [0xFF, 0xFF, 0x00, 0xFF],
    ]
    .iter()
    .enumerate()
    {
        for y in 0..tile {
            for x in 0..tile {
                img.put_pixel(i as u32 * tile + x, y, image::Rgba([r, g, b, a]));
            }
        }
    }
    img.save(map_dir.join("tileset.png")).unwrap();
}

/// Copy the committed fixture into a temp dir and generate its tilesets.
fn demo_project(test: &str) -> (TestDir, PathBuf) {
    let tmp = TestDir::new(test);
    let root = tmp.path().join("demo");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo");
    copy_dir(&fixture, &root);
    write_tileset(&root.join("data/maps/Town"));
    write_tileset(&root.join("data/maps/Cave"));
    (tmp, root)
}

fn boot(test: &str) -> (TestDir, RunnerGame) {
    let (tmp, root) = demo_project(test);
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let game = RunnerGame::new(project, opts).expect("boot game");
    (tmp, game)
}

/// Update the game one frame with `mask` held (fresh InputState ⇒ just-press).
fn frame(game: &mut RunnerGame, mask: u8) {
    let mut input = InputState::new();
    input.set_from_bitmask(mask);
    game.update(&input);
}

fn idle(game: &mut RunnerGame, n: u32) {
    for _ in 0..n {
        frame(game, 0);
    }
}

fn press(game: &mut RunnerGame, button: GbButton) {
    frame(game, button.bit_mask());
    idle(game, 1);
}

fn press_a(game: &mut RunnerGame) {
    press(game, GbButton::A);
}

/// Press A until no textbox/choice/whiteout is on screen (bounded).
fn dismiss_dialogue(game: &mut RunnerGame) {
    for _ in 0..40 {
        if game.dialogue_text().is_none() && game.choice_options().is_none() {
            return;
        }
        press_a(game);
    }
    panic!("dialogue did not close after 40 A presses");
}

/// Add an items table (schema + Potion/Elixir/Tent records) and the manifest
/// `battle.items` block to a copied fixture project. The Potion heals and is
/// cheap, the Elixir heals and is unaffordable on the default 100 G… barely,
/// the Tent does nothing (healHp 0 ⇒ unusable).
fn write_items(root: &Path) {
    let path = root.join(".dotzuki-editor.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    {
        let activities = manifest["activities"].as_array_mut().unwrap();
        let data = activities
            .iter_mut()
            .find(|a| a["id"] == "data")
            .expect("data activity");
        data["config"]["tables"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "items", "label": "Items", "dir": "items", "icon": "", "idField": "id",
                "fields": [ {"id": "name"}, {"id": "healHp"}, {"id": "price"}, {"id": "effect"} ]
            }));
    }
    manifest["battle"]["items"] =
        serde_json::json!({ "table": "items", "healField": "healHp", "starting": { "potion": 3 } });
    fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    fs::create_dir_all(root.join("data/items")).unwrap();
    fs::write(
        root.join("data/items/potion.json"),
        r#"{ "id": "potion", "name": "Potion", "healHp": 50, "price": 20, "effect": "Restores 50 HP" }"#,
    )
    .unwrap();
    fs::write(
        root.join("data/items/elixir.json"),
        r#"{ "id": "elixir", "name": "Elixir", "healHp": 999, "price": 200, "effect": "Restores all HP" }"#,
    )
    .unwrap();
    fs::write(
        root.join("data/items/tent.json"),
        r#"{ "id": "tent", "name": "Tent", "healHp": 0, "price": 50, "effect": "Rest at camp" }"#,
    )
    .unwrap();
}

/// Patch the Town scene with a `shop_test` storyline routed to the Guide:
/// open a shop, then a follow-up line.
fn write_shop_scene(root: &Path) {
    fs::write(
        root.join("data/maps/Town/script.scene"),
        r#"game_scene Town {
    @storylines {
        @speaker("Guide") {
            "Welcome to Town."
        }
    }
    @storyline("shop_test") {
        @trigger(map = "Town", npc = "Guide")
        @command("openShop", ["potion", "elixir"])
        @speaker("Guide") {
            "Come again!"
        }
    }
}
"#,
    )
    .unwrap();
}

/// Walk from boot to facing the Cave's Boss NPC (Town main → warp →
/// cave_enter → stand below the Boss at (1,1)).
fn reach_boss(game: &mut RunnerGame) {
    dismiss_dialogue(game); // Town main
    for _ in 0..8 {
        frame(game, GbButton::Left.bit_mask());
    }
    idle(game, 2 * 10 + 2); // warp fade out + in
    assert_eq!(game.current_map_id(), Some("Cave"));
    dismiss_dialogue(game); // cave_enter
    game.debug_place(1, 2, Direction::Up);
}

/// Play the armed battle with auto-A (Fight → first skill every round) and
/// collect the narration. Does NOT dismiss any post-battle text.
fn auto_battle(game: &mut RunnerGame) -> Vec<String> {
    let mut narration: Vec<String> = Vec::new();
    for _ in 0..120 {
        let Some(battle) = game.battle() else { break };
        if let Some(line) = battle.current_line() {
            narration.push(line.to_string());
        }
        press_a(game);
    }
    narration
}

// ── Start menu ────────────────────────────────────────────────────────────────

#[test]
fn start_menu_opens_and_closes() {
    let (_tmp, mut game) = boot("openclose");
    dismiss_dialogue(&mut game); // Town main
    assert!(game.menu_lines().is_none(), "no menu in the overworld");

    press(&mut game, GbButton::Start);
    assert_eq!(
        game.menu_lines().expect("menu open"),
        vec!["Party", "Bag", "Save", "Close"]
    );

    // B closes; Start reopens; the Close entry closes too.
    press(&mut game, GbButton::B);
    assert!(game.menu_lines().is_none(), "B closed the menu");
    press(&mut game, GbButton::Start);
    assert!(game.menu_lines().is_some(), "Start reopens the menu");
    for _ in 0..3 {
        press(&mut game, GbButton::Down);
    }
    press_a(&mut game); // Close
    assert!(game.menu_lines().is_none(), "Close entry closed the menu");

    // The overworld is live again: talking to the Guide works. (The spawn
    // tile is hemmed in by walls/the warp/the Guide, so dialogue — not
    // walking — is the unfreeze probe here.)
    game.debug_place(1, 1, Direction::Right);
    press_a(&mut game);
    assert!(
        game.dialogue_text().is_some(),
        "menu close unfreezes the overworld"
    );
}

#[test]
fn party_view_shows_record_details() {
    let (_tmp, mut game) = boot("partyview");
    dismiss_dialogue(&mut game);
    press(&mut game, GbButton::Start);
    press_a(&mut game); // Party

    let lines = game.menu_lines().expect("party view open");
    // Base stats from the record, current HP/MP full (no battle yet).
    assert!(
        lines.contains(&"Aria HP 60/60 MP 20/20".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"ATK 12 DEF 10 SPD 15 grass".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"Skills: Slash, Fire Bolt, Heal, Focus".to_string()),
        "{lines:?}"
    );

    // B backs out to the root menu.
    press(&mut game, GbButton::B);
    assert_eq!(
        game.menu_lines().expect("back at root"),
        vec!["Party", "Bag", "Save", "Close"]
    );
}

/// Boot the fixture with a hand-written v3 save: Aria at 32/60, Bryn fainted,
/// 2 potions + 1 tent, 100 G. Returns the game in the overworld (resume
/// skips the opening dispatch, so no textbox is up).
fn boot_from_hurt_save(test: &str) -> (TestDir, RunnerGame) {
    let (tmp, root) = demo_project(test);
    write_items(&root);
    fs::write(
        root.join("data/heroes/bryn.json"),
        r#"{
  "id": "bryn",
  "name": "Bryn",
  "hp": 80,
  "atk": 14,
  "def": 12,
  "spd": 10,
  "mp": 60,
  "element": "fire",
  "skills": ["fire-bolt", "slash"]
}"#,
    )
    .unwrap();
    let save_file = root.join("test-save.json");
    fs::write(
        &save_file,
        r#"{
  "version": 3,
  "map": "Town",
  "player": { "x": 1, "y": 1, "facing": "down" },
  "flags": { "__played_main_Town": true },
  "party": [
    { "id": "aria", "hp": 32, "mp": 20 },
    { "id": "bryn", "hp": 0, "mp": 60 }
  ],
  "inventory": { "potion": 2, "tent": 1 },
  "money": 100
}"#,
    )
    .unwrap();
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        save_file: Some(save_file),
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let game = RunnerGame::new(project, opts).expect("boot from save");
    assert_eq!(game.player_tile(), (1, 1), "resumed at the saved tile");
    (tmp, game)
}

/// Open the Bag and move the cursor onto `row` (0-based bag entry).
fn open_bag_at(game: &mut RunnerGame, row: usize) {
    press(game, GbButton::Start);
    press(game, GbButton::Down); // Bag
    press_a(game);
    for _ in 0..row {
        press(game, GbButton::Down);
    }
}

#[test]
fn bag_heal_caps_at_max_and_decrements() {
    let (_tmp, mut game) = boot_from_hurt_save("bagheal");
    assert_eq!(game.money(), 100, "money from the save");

    // Bag rows come from the inventory (sorted by id), names from records.
    open_bag_at(&mut game, 0);
    assert_eq!(
        game.menu_lines().expect("bag open"),
        vec!["Potion ×2".to_string(), "Tent ×1".to_string()]
    );

    // Use the Potion → the member list marks Bryn (fainted) unselectable.
    press_a(&mut game);
    assert_eq!(
        game.menu_lines().expect("target pick"),
        vec!["Aria HP 32/60".to_string(), "× Bryn HP 0/80".to_string()]
    );

    // Heal Aria: 32 + 50 caps at 60 (28 healed), count 2 → 1.
    press_a(&mut game);
    assert_eq!(
        game.menu_lines().expect("heal note"),
        vec!["Aria recovered 28 HP!".to_string()]
    );
    press_a(&mut game); // dismiss the note → back to the Bag
    assert_eq!(
        game.menu_lines().expect("bag again"),
        vec!["Potion ×1".to_string(), "Tent ×1".to_string()]
    );

    // Close the menu: the heal and the count persist in the runner state.
    press(&mut game, GbButton::B); // Bag → root
    press(&mut game, GbButton::B); // root → closed
    assert!(game.menu_lines().is_none());
    let party = game.party_state().expect("party state");
    assert_eq!(party[0].hp, 60, "healed (capped at max)");
    assert_eq!(party[1].hp, 0, "Bryn untouched");
    assert_eq!(game.inventory().unwrap().get("potion"), Some(&1));
}

#[test]
fn bag_heal_does_not_revive_fainted_member() {
    let (_tmp, mut game) = boot_from_hurt_save("bagfaint");
    open_bag_at(&mut game, 0);
    press_a(&mut game); // Potion → target pick
    press(&mut game, GbButton::Down); // Bryn
    press_a(&mut game);
    assert_eq!(
        game.menu_lines().expect("rejection note"),
        vec!["It won't have any effect.".to_string()]
    );
    press_a(&mut game); // dismiss
    assert_eq!(
        game.inventory().unwrap().get("potion"),
        Some(&2),
        "the rejected heal kept the item"
    );
    assert_eq!(game.party_state().unwrap()[1].hp, 0, "still fainted");
}

#[test]
fn bag_unusable_item_shows_no_effect() {
    let (_tmp, mut game) = boot_from_hurt_save("bagtent");
    open_bag_at(&mut game, 1); // Tent (healHp 0)
    press_a(&mut game);
    assert_eq!(
        game.menu_lines().expect("no-effect note"),
        vec!["It won't have any effect.".to_string()]
    );
}

#[test]
fn save_from_menu_writes_a_resumable_file() {
    let (_tmp, root) = demo_project("menusave");
    let save_file = root.join("menu-save.json");
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        save_file: Some(save_file.clone()),
        // No write_saves: the menu Save must write anyway.
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot");
    dismiss_dialogue(&mut game);

    press(&mut game, GbButton::Start);
    press(&mut game, GbButton::Down);
    press(&mut game, GbButton::Down); // Save
    press_a(&mut game);
    assert_eq!(
        game.menu_lines().expect("save confirmation"),
        vec!["Game saved.".to_string()]
    );

    let text = fs::read_to_string(&save_file).expect("menu save wrote the file");
    let raw: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(raw["version"], 3);
    assert_eq!(raw["map"], "Town");
    assert_eq!(raw["money"], 100, "default money persisted");

    // The file really resumes.
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        save_file: Some(save_file),
        ..RunnerOptions::default()
    };
    let game = RunnerGame::new(project, opts).expect("boot from the menu save");
    assert_eq!(game.current_map_id(), Some("Town"));
    assert!(game.flag("__played_main_Town"));
    assert_eq!(game.money(), 100);
}

// ── shops ─────────────────────────────────────────────────────────────────────

/// Boot with the shop scene + items, dismiss Town main, and talk to the
/// Guide (standing right of the spawn).
fn open_shop(test: &str) -> (TestDir, RunnerGame) {
    let (tmp, root) = demo_project(test);
    write_items(&root);
    write_shop_scene(&root);
    let project = LoadedProject::load(&root).expect("load demo project");
    let game = RunnerGame::new(project, RunnerOptions::default()).expect("boot");
    (tmp, game)
}

fn talk_to_guide(game: &mut RunnerGame) {
    dismiss_dialogue(game); // Town main
    game.debug_place(1, 1, Direction::Right); // face the Guide at (2,1)
    press_a(game); // talk → the shop_test storyline
}

#[test]
fn shop_buy_flow_and_exit_resumes_scene() {
    let (_tmp, mut game) = open_shop("shopbuy");
    talk_to_guide(&mut game);

    // The shop opens on its root: Buy / Sell / Exit.
    let rows = game.shop_lines().expect("shop open");
    assert_eq!(
        rows,
        vec!["Buy".to_string(), "Sell".to_string(), "Exit".to_string()],
        "{rows:?}"
    );
    assert_eq!(game.money(), 100);

    // Buy: the shelf on the default 100 G (no shop section in the fixture);
    // the unaffordable Elixir is marked.
    press_a(&mut game);
    let rows = game.shop_lines().expect("buy view");
    assert_eq!(
        rows,
        vec!["Potion 20 G".to_string(), "× Elixir 200 G".to_string()],
        "{rows:?}"
    );

    // Buy two Potions: 100 → 80 → 60, inventory 3 → 5 (starting counts).
    press_a(&mut game);
    assert_eq!(
        game.shop_lines().unwrap().last(),
        Some(&"Bought a Potion!".to_string())
    );
    press_a(&mut game); // dismiss the note
    press_a(&mut game); // buy again
    press_a(&mut game); // dismiss
    assert_eq!(game.money(), 60);
    assert_eq!(game.inventory().unwrap().get("potion"), Some(&5));

    // The Elixir stays unaffordable: A only shows the no-money note.
    press(&mut game, GbButton::Down);
    press_a(&mut game);
    assert_eq!(
        game.shop_lines().unwrap().last(),
        Some(&"Not enough money…".to_string())
    );
    assert_eq!(game.money(), 60, "blocked buys don't charge");
    assert_eq!(game.inventory().unwrap().get("elixir"), None);
    press_a(&mut game); // dismiss

    // B returns to the root; B again exits: the scene resumes with its
    // follow-up line.
    press(&mut game, GbButton::B);
    assert_eq!(
        game.shop_lines().expect("back at the root"),
        vec!["Buy".to_string(), "Sell".to_string(), "Exit".to_string()]
    );
    press(&mut game, GbButton::B);
    assert!(game.shop_lines().is_none(), "shop closed");
    let page = game.dialogue_text().expect("scene resumed");
    assert!(page.contains("Come again!"), "page: {page:?}");
    dismiss_dialogue(&mut game);
}

#[test]
fn shop_sell_flow_money_math_and_counts() {
    let (_tmp, mut game) = open_shop("shopsell");
    talk_to_guide(&mut game);

    // Root → Sell: the inventory (3 Potions from items.starting) at
    // floor(price / 2) = 10 G each.
    press(&mut game, GbButton::Down);
    press_a(&mut game);
    let rows = game.shop_lines().expect("sell view");
    assert_eq!(rows, vec!["Potion ×3 10 G".to_string()], "{rows:?}");

    // Sell two Potions: money 100 → 110 → 120, count 3 → 1.
    press_a(&mut game);
    assert_eq!(
        game.shop_lines().unwrap().last(),
        Some(&"Sold a Potion!".to_string())
    );
    press_a(&mut game); // dismiss
    assert_eq!(game.money(), 110);
    assert_eq!(game.inventory().unwrap().get("potion"), Some(&2));
    press_a(&mut game); // sell again
    press_a(&mut game); // dismiss
    assert_eq!(game.money(), 120);
    assert_eq!(game.inventory().unwrap().get("potion"), Some(&1));

    // Sell the last one: the stack leaves the inventory, the Sell list is
    // empty, and the Buy side never changed (still 3 potions' worth of
    // nothing bought — the shelf is unaffected).
    press_a(&mut game); // sell the last
    press_a(&mut game); // dismiss
    assert_eq!(game.money(), 130);
    assert_eq!(game.inventory().unwrap().get("potion"), None);
    assert_eq!(
        game.shop_lines().expect("empty sell view"),
        vec!["Nothing to sell.".to_string()]
    );

    // B → root → Exit entry: the scene resumes with its follow-up line.
    press(&mut game, GbButton::B);
    press(&mut game, GbButton::Down);
    press(&mut game, GbButton::Down);
    press_a(&mut game); // Exit
    let page = game.dialogue_text().expect("scene resumed");
    assert!(page.contains("Come again!"), "page: {page:?}");
    dismiss_dialogue(&mut game);
}

#[test]
fn shop_sell_zero_priced_item_sells_for_zero() {
    let (tmp, root) = demo_project("sellzero");
    write_items(&root);
    write_shop_scene(&root);
    // Hand the player a worthless item (price 0 ⇒ sells for 0, allowed).
    fs::write(
        root.join("data/items/pebble.json"),
        r#"{ "id": "pebble", "name": "Pebble", "healHp": 0, "price": 0, "effect": "Just a rock" }"#,
    )
    .unwrap();
    let path = root.join(".dotzuki-editor.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    manifest["battle"]["items"]["starting"] = serde_json::json!({ "pebble": 1, "potion": 1 });
    fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();

    let project = LoadedProject::load(&root).expect("load demo project");
    let mut game = RunnerGame::new(project, RunnerOptions::default()).expect("boot");
    talk_to_guide(&mut game);

    // Root → Sell: Pebble 0 G (0/2), Potion 10 G (20/2) — sorted by id.
    press(&mut game, GbButton::Down);
    press_a(&mut game);
    let rows = game.shop_lines().expect("sell view");
    assert_eq!(
        rows,
        vec!["Pebble ×1 0 G".to_string(), "Potion ×1 10 G".to_string()],
        "{rows:?}"
    );

    // Selling the Pebble adds nothing but decrements the count.
    press_a(&mut game);
    assert_eq!(
        game.shop_lines().unwrap().last(),
        Some(&"Sold a Pebble!".to_string())
    );
    assert_eq!(game.money(), 100, "price 0 sells for 0");
    assert_eq!(game.inventory().unwrap().get("pebble"), None);
    let _ = tmp;
}

#[test]
fn shop_section_seeds_start_money() {
    let (tmp, root) = demo_project("shopsection");
    write_items(&root);
    write_shop_scene(&root);
    // An explicit shop section: 15 G start — the 20 G Potion is unaffordable.
    let path = root.join(".dotzuki-editor.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    manifest["shop"] = serde_json::json!({ "currency": "G", "startMoney": 15 });
    fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();

    let project = LoadedProject::load(&root).expect("load demo project");
    let mut game = RunnerGame::new(project, RunnerOptions::default()).expect("boot");
    assert_eq!(game.money(), 15, "startMoney from the shop section");
    talk_to_guide(&mut game);
    press_a(&mut game); // root → Buy
    assert_eq!(
        game.shop_lines().expect("buy view"),
        vec!["× Potion 20 G".to_string(), "× Elixir 200 G".to_string()]
    );
    press_a(&mut game);
    assert_eq!(
        game.shop_lines().unwrap().last(),
        Some(&"Not enough money…".to_string())
    );
    assert_eq!(game.money(), 15);
    let _ = tmp;
}

// ── game-over whiteout ────────────────────────────────────────────────────────

#[test]
fn lost_battle_triggers_whiteout_and_respawns_at_entry() {
    let (_tmp, root) = demo_project("whiteout");
    // An overwhelming Slime: faster, and its built-in Attack one-shots Aria.
    fs::write(
        root.join("data/monsters/slime.json"),
        r#"{
  "id": "slime",
  "name": "Slime",
  "hp": 200,
  "atk": 60,
  "def": 50,
  "spd": 99,
  "mp": 0,
  "element": "grass",
  "skills": []
}"#,
    )
    .unwrap();
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot");
    reach_boss(&mut game);

    press_a(&mut game); // boss intro
    press_a(&mut game); // → battle
    let narration = auto_battle(&mut game);
    assert_eq!(
        narration.last(),
        Some(&"You lost the battle…".to_string()),
        "{narration:?}"
    );

    // The scene resumes with "lose": its post-lose text still plays…
    let page = game.dialogue_text().expect("post-lose text");
    assert!(page.contains("The Slime oozes onward"), "page: {page:?}");
    assert!(
        !game.whiteout_active(),
        "whiteout waits for the scene to finish"
    );

    // …then the whiteout: blackout first (no text yet), then the line.
    press_a(&mut game); // dismiss the post-lose text → scene finishes
    assert!(game.whiteout_active(), "whiteout armed");
    assert!(
        game.dialogue_text().is_none(),
        "blackout phase shows no text"
    );
    idle(&mut game, 35);
    let page = game.dialogue_text().expect("whiteout message");
    assert!(page.contains("Aria collapsed"), "page: {page:?}");

    // Dismissing the message lands the whiteout: party healed to full
    // (status cleared), player back on the entry map's spawn, flags kept.
    press_a(&mut game);
    assert!(!game.whiteout_active());
    assert_eq!(
        game.current_map_id(),
        Some("Town"),
        "returned to the entry map"
    );
    assert_eq!(game.player_tile(), (1, 1), "the entry spawn");
    let party = game.party_state().expect("party state");
    assert_eq!(party.len(), 1);
    assert_eq!((party[0].hp, party[0].mp), (60, 20), "healed to full");
    assert_eq!(party[0].status, None, "status cleared");
    assert!(game.flag("__played_main_Town"), "flags kept");
}

#[test]
fn whiteout_heals_only_when_project_has_no_maps() {
    let (tmp, root) = demo_project("maplesswo");
    // Dialogue-only boot: strip the maps; the entry scene runs a battle and
    // loses (same overwhelming Slime as above).
    fs::remove_dir_all(root.join("data/maps")).unwrap();
    fs::write(
        root.join("data/monsters/slime.json"),
        r#"{
  "id": "slime",
  "name": "Slime",
  "hp": 200,
  "atk": 60,
  "def": 50,
  "spd": 99,
  "mp": 0,
  "element": "grass",
  "skills": []
}"#,
    )
    .unwrap();
    fs::write(
        root.join("assets/scenes/main.scene"),
        r#"game_scene Main {
    @storylines {
        @command("startBattle", "slime")
        @speaker("Guide") {
            "That went badly."
        }
    }
}
"#,
    )
    .unwrap();
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot");
    assert_eq!(game.current_map_id(), None);

    let narration = auto_battle(&mut game);
    assert_eq!(
        narration.last(),
        Some(&"You lost the battle…".to_string()),
        "{narration:?}"
    );
    // The post-lose text plays, then the whiteout heals in place.
    dismiss_dialogue(&mut game); // "That went badly."
    idle(&mut game, 35);
    let page = game.dialogue_text().expect("whiteout message");
    assert!(page.contains("collapsed"), "page: {page:?}");
    press_a(&mut game);
    assert_eq!(game.current_map_id(), None, "no maps: nowhere to return to");
    let party = game.party_state().expect("party state");
    assert_eq!((party[0].hp, party[0].mp), (60, 20), "healed to full");
    let _ = tmp;
}
