//! Integration tests for the generic battle system against the committed
//! fixture project: manifest/table resolution, the scene → battle → scene
//! round trip (win path, flags harvested), and the no-battle-section
//! undefeated-continue fallback.
//!
//! All tests drive the game windowless through the public
//! [`RunnerGame::update`]/[`RunnerGame::draw`] API with synthetic input and a
//! scripted battle rng ([`RunnerOptions::rng_script`]) — no winit, no pixels.
//! Tilesets are generated in code (mirrors `runner_game.rs`).

use crate::alloc_prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use dotzuki_engine::overworld::types::Direction;
use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_runner::battle::BattleSetup;
use dotzuki_runner::{LoadedProject, RunnerGame, RunnerOptions};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// Unique temp directory, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(test: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "dotzuki-runner-battle-{test}-{}-{id}",
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

/// Boot the fixture with a scripted battle rng: every action hits, variance
/// is always 89%, never a crit.
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

fn press_a(game: &mut RunnerGame) {
    frame(game, GbButton::A.bit_mask());
    idle(game, 1);
}

/// Press A until no textbox/choice is on screen (bounded).
fn dismiss_dialogue(game: &mut RunnerGame) {
    for _ in 0..20 {
        if game.dialogue_text().is_none() && game.choice_options().is_none() {
            return;
        }
        press_a(game);
    }
    panic!("dialogue did not close after 20 A presses");
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

// ── manifest / table resolution ─────────────────────────────────────────────

#[test]
fn battle_section_parses_and_resolves_tables() {
    let (_tmp, root) = demo_project("manifest");
    let project = LoadedProject::load(&root).expect("load demo project");

    let section = project.manifest().battle.as_ref().expect("battle section");
    assert_eq!(section.party.as_ref().unwrap().table, "heroes");
    assert_eq!(section.enemies.as_ref().unwrap().table, "monsters");
    let skills = section.skills.as_ref().unwrap();
    assert_eq!(skills.table, "spells");
    assert_eq!(skills.field, "skills");
    assert_eq!(skills.category_field, "type");
    assert_eq!(skills.cost_field, "mpCost");
    let stats = section.stats.as_ref().unwrap();
    assert_eq!((stats.hp.as_str(), stats.attack.as_str()), ("hp", "atk"));
    assert_eq!(
        (stats.defense.as_str(), stats.speed.as_str()),
        ("def", "spd")
    );
    assert_eq!(section.resource.as_deref(), Some("mp"));
    assert_eq!(section.rules.as_deref(), Some("data/rules.ron"));

    // Table ids resolve to record dirs via the data activity.
    assert_eq!(project.table_dir("heroes"), Some(root.join("data/heroes")));
    assert_eq!(
        project.table_dir("monsters"),
        Some(root.join("data/monsters"))
    );
    assert_eq!(project.table_dir("spells"), Some(root.join("data/spells")));
    assert_eq!(project.table_dir("nope"), None);

    // The data-table schema is visible (dotzuki check validates against it).
    let heroes = project.manifest().data_table("heroes").unwrap();
    assert!(heroes.fields.iter().any(|f| f == "atk"));
}

#[test]
fn setup_loads_records_and_chart() {
    let (_tmp, root) = demo_project("setup");
    let project = LoadedProject::load(&root).expect("load demo project");
    let setup = BattleSetup::from_project(&project).expect("battle setup");

    // The player is the FIRST party record; an unknown enemy id falls back to
    // the first enemy record (sorted: "bat" < "slime") with a warning.
    let rng: Box<dyn dotzuki_runner::battle::BattleRng> =
        Box::new(dotzuki_runner::battle::ScriptedRng::new(vec![50, 100, 1]));
    let battle = setup.start("no-such-monster", rng).expect("battle");
    assert_eq!(battle.player().name, "Aria");
    assert_eq!(battle.player().max_hp, 60);
    assert_eq!(battle.player().max_mp, 20);
    assert_eq!(battle.enemy().id, "bat", "unknown id → first enemy record");
    assert_eq!(battle.enemy().max_mp, 3);

    // Aria's move list comes from the skills table, in authored order.
    let names: Vec<&str> = battle
        .player()
        .skills
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(names, ["Slash", "Fire Bolt", "Heal", "Focus"]);
    assert_eq!(battle.player().skills[1].cost, 5);
    assert_eq!(battle.player().skills[1].element.as_deref(), Some("fire"));

    // The bat can't afford Fire Bolt (mp 3 < cost 5) — the AI falls back.
    assert_eq!(battle.enemy().mp, 3);
}

// ── scene integration ───────────────────────────────────────────────────────

#[test]
fn scene_battle_win_path_resumes_scene_and_harvests_flags() {
    let (_tmp, mut game) = boot("win");
    reach_boss(&mut game);

    // Talk to the Boss: the intro line shows…
    press_a(&mut game);
    let page = game.dialogue_text().expect("boss intro text");
    assert!(page.contains("wild Slime"), "page: {page:?}");

    // …then the scene suspends on `await startBattle("slime")`.
    press_a(&mut game);
    assert!(game.battle().is_some(), "battle mode armed");
    assert!(game.battle().unwrap().in_menu());

    // The battle screen renders (panels, bars, menu) without panicking.
    let mut fb = dotzuki_engine::render::FrameBuffer::new(
        dotzuki_engine::render_config::RenderConfig::new(320, 240),
        dotzuki_engine::render::Rgba::BLACK,
    );
    game.draw(&mut fb);
    let colors: std::collections::HashSet<[u8; 4]> = fb
        .data
        .chunks_exact(4)
        .map(|px| [px[0], px[1], px[2], px[3]])
        .collect();
    assert!(colors.len() > 4, "battle screen should not be uniform");

    // The root menu offers Fight/Party/Run (no items block ⇒ no Item); Fight
    // opens the skill menu.
    assert_eq!(
        game.battle().unwrap().menu_items(),
        vec!["Fight".to_string(), "Party".to_string(), "Run".to_string()]
    );
    press_a(&mut game);
    assert_eq!(
        game.battle().unwrap().menu_items(),
        vec![
            "Slash".to_string(),
            "Fire Bolt 5MP".to_string(),
            "Heal 4MP".to_string(),
            "Focus 3MP".to_string()
        ]
    );

    // Auto-A: pick the first skill every round, page through the narration,
    // and collect the battle log as it scrolls by.
    let mut narration: Vec<String> = Vec::new();
    for _ in 0..80 {
        let Some(battle) = game.battle() else { break };
        if let Some(line) = battle.current_line() {
            narration.push(line.to_string());
        }
        press_a(&mut game);
    }
    assert!(
        !narration.is_empty(),
        "the battle should have played (log empty)"
    );
    println!("battle narration: {narration:?}");
    assert_eq!(
        narration,
        vec![
            "Aria used Slash!",
            "53 damage!",
            "Slime used Slash!",
            "28 damage!",
            "Aria used Slash!",
            "53 damage!",
            "Slime fainted!",
            "You won the battle!",
        ]
    );

    // The scene resumed with "win": post-battle text, then the flag lands.
    let page = game.dialogue_text().expect("post-battle text");
    assert!(page.contains("path is clear"), "page: {page:?}");
    dismiss_dialogue(&mut game);
    assert!(
        game.flag("BEAT_SLIME"),
        "scene flags harvested after battle"
    );
    assert!(game.battle().is_none());
}

#[test]
fn scene_battle_without_battle_section_autowins() {
    let (_tmp, root) = demo_project("nobattle");
    // Strip the battle section: startBattle must warn and continue "win".
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(".dotzuki-editor.json")).unwrap())
            .unwrap();
    manifest.as_object_mut().unwrap().remove("battle");
    fs::write(
        root.join(".dotzuki-editor.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let project = LoadedProject::load(&root).expect("load demo project");
    let mut game = RunnerGame::new(project, RunnerOptions::default()).expect("boot game");
    reach_boss(&mut game);

    press_a(&mut game); // boss intro
    press_a(&mut game); // past the intro → startBattle auto-wins
    assert!(
        game.battle().is_none(),
        "no battle without a battle section"
    );
    let page = game.dialogue_text().expect("post-battle text still plays");
    assert!(page.contains("path is clear"), "page: {page:?}");
    dismiss_dialogue(&mut game);
    assert!(game.flag("BEAT_SLIME"));
}

// ── RON effect hooks (v2-a) ─────────────────────────────────────────────────

/// Rewrite the fixture's `data/rules.ron` in a copied project.
fn write_rules(root: &Path, ron: &str) {
    fs::write(root.join("data/rules.ron"), ron).unwrap();
}

#[test]
fn ron_record_overrides_table_power_element_and_cost() {
    let (_tmp, root) = demo_project("override");
    // Slash: table power 40/cost 0/no element → RON power 60, type fire
    // (super-effective vs the grass Slime), cost 7 MP. No hooks ⇒ the v1
    // direct chart application applies with the overridden element.
    write_rules(
        &root,
        r#"Ruleset(
            stats: ["hp", "atk", "def", "spd"],
            types: ["fire", "grass", "water"],
            resources: ["mp"],
            type_chart: [
                (atk: "fire", def: "grass", mult: [2, 1]),
                (atk: "grass", def: "fire", mult: [1, 2]),
                (atk: "water", def: "fire", mult: [2, 1]),
            ],
            effects: [
                Effect(id: "slash", kind: Move, power: 60, type: "fire",
                       cost: [Cost(resource: "mp", amount: 7)]),
            ],
        )"#,
    );
    let project = LoadedProject::load(&root).expect("load demo project");
    let setup = BattleSetup::from_project(&project).expect("battle setup");
    let rng: Box<dyn dotzuki_runner::battle::BattleRng> =
        Box::new(dotzuki_runner::battle::ScriptedRng::new(vec![50, 100, 1]));
    let mut battle = setup.start("slime", rng).expect("battle");

    // The overrides landed on the loaded skill…
    let slash = &battle.player().skills[0];
    assert_eq!(slash.power, 60);
    assert_eq!(slash.element.as_deref(), Some("fire"));
    assert_eq!(slash.cost, 7);
    // …and the Fight submenu shows the overridden cost.
    let mut input = InputState::new();
    input.set_from_bitmask(GbButton::A.bit_mask());
    battle.update(&input);
    assert_eq!(battle.menu_items()[0], "Slash 7MP");

    // …and drive the action: 60×12/8 = 90 → ×89/100 = 80 → ×2 (fire vs
    // grass) = 160 — a one-hit KO, MP 20 → 13.
    for _ in 0..40 {
        if battle.outcome().is_some() {
            break;
        }
        let mut input = InputState::new();
        input.set_from_bitmask(GbButton::A.bit_mask());
        battle.update(&input);
    }
    assert_eq!(battle.player().mp, 13);
    assert_eq!(battle.outcome(), Some(dotzuki_runner::BattleOutcome::Win));
    let log = battle.log();
    assert!(
        log.contains(&"It's super effective!".to_string()),
        "{log:?}"
    );
    assert!(log.contains(&"160 damage!".to_string()), "{log:?}");
}

#[test]
fn project_with_effects_but_unmatched_skill_keeps_v1_behavior() {
    let (_tmp, root) = demo_project("compat");
    // The rules file HAS effects, but none matching "slash" — the built-in
    // category path must run byte-for-byte as v1.
    write_rules(
        &root,
        r#"Ruleset(
            stats: ["hp", "attack", "defense", "speed"],
            types: ["fire", "grass", "water"],
            resources: ["mp"],
            type_chart: [
                (atk: "fire", def: "grass", mult: [2, 1]),
                (atk: "grass", def: "fire", mult: [1, 2]),
                (atk: "water", def: "fire", mult: [2, 1]),
            ],
            effects: [
                Effect(id: "venom-sting", kind: Move, power: 15, hooks: [
                    Hook(on: "DamagingHit", do: [Boost(stat: "attack", stages: 1, target: Source)]),
                ]),
            ],
        )"#,
    );
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    press_a(&mut game); // boss intro
    press_a(&mut game); // → battle

    // One round, exactly the v1 numbers (53 / 28) and lines. (Root → Fight →
    // the first skill.)
    press_a(&mut game);
    press_a(&mut game);
    let mut narration: Vec<String> = Vec::new();
    for _ in 0..4 {
        let Some(battle) = game.battle() else { break };
        if let Some(line) = battle.current_line() {
            narration.push(line.to_string());
        }
        press_a(&mut game);
    }
    assert_eq!(
        narration,
        vec![
            "Aria used Slash!",
            "53 damage!",
            "Slime used Slash!",
            "28 damage!",
        ]
    );
    let battle = game.battle().expect("battle still running");
    assert_eq!(battle.enemy().hp, 37);
    assert_eq!(battle.player().hp, 32);
}

/// The v2-a acceptance: a full scene battle where the Slime's RON-authored
/// `venom-sting` poisons Aria and the `poison` status's `Residual` hook chips
/// her down — every line narrated, no Rust per-game code involved.
#[test]
fn scene_battle_ron_poison_narration_acceptance() {
    let (_tmp, root) = demo_project("poison");
    write_rules(
        &root,
        r#"Ruleset(
            stats: ["hp", "attack", "defense", "speed"],
            types: ["fire", "grass", "water"],
            resources: ["mp"],
            type_chart: [
                (atk: "fire", def: "grass", mult: [2, 1]),
                (atk: "grass", def: "fire", mult: [1, 2]),
                (atk: "water", def: "fire", mult: [2, 1]),
            ],
            effects: [
                Effect(id: "venom-sting", kind: Move, power: 15, accuracy: 100, hooks: [
                    Hook(on: "DamagingHit", chance: [1, 1], do: [
                        InflictStatus(status: "poison", target: Target),
                    ]),
                ]),
                Effect(id: "poison", kind: Status, hooks: [
                    Hook(on: "Residual", do: [
                        DamageFraction(num: 1, den: 8, of: MaxHp, target: Target),
                    ]),
                ]),
            ],
        )"#,
    );
    // The Slime fights with venom-sting only (the AI picks highest power);
    // Aria's Slash is weakened to 5 so the poison gets rounds to work.
    fs::write(
        root.join("data/monsters/slime.json"),
        r#"{
  "id": "slime",
  "name": "Slime",
  "hp": 90,
  "atk": 8,
  "def": 8,
  "spd": 5,
  "mp": 0,
  "element": "grass",
  "skills": ["venom-sting"]
}"#,
    )
    .unwrap();
    fs::write(
        root.join("data/spells/venom-sting.json"),
        r#"{
  "id": "venom-sting",
  "name": "venom-sting",
  "type": "attack",
  "power": 15,
  "accuracy": 100,
  "mpCost": 0
}"#,
    )
    .unwrap();
    let slash: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("data/spells/slash.json")).unwrap())
            .unwrap();
    let mut slash = slash;
    slash["power"] = serde_json::json!(5);
    fs::write(
        root.join("data/spells/slash.json"),
        serde_json::to_string_pretty(&slash).unwrap(),
    )
    .unwrap();

    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    press_a(&mut game); // boss intro
    press_a(&mut game); // → battle

    let mut narration: Vec<String> = Vec::new();
    for _ in 0..120 {
        let Some(battle) = game.battle() else { break };
        if let Some(line) = battle.current_line() {
            narration.push(line.to_string());
        }
        press_a(&mut game);
    }
    println!("poison battle narration: {narration:?}");
    assert!(
        narration.contains(&"Aria was afflicted with poison!".to_string()),
        "{narration:?}"
    );
    assert!(
        narration.contains(&"Aria is hurt by poison!".to_string()),
        "{narration:?}"
    );
    assert!(
        narration.contains(&"Aria fainted!".to_string()),
        "{narration:?}"
    );
    assert_eq!(narration.last(), Some(&"You lost the battle…".to_string()));
}

// ── parties + items + persistence (v2-b) ────────────────────────────────────

/// Add a second hero record (Bryn the Mage) to a copied fixture project.
fn write_bryn(root: &Path) {
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
}

/// Add an items table (schema + one Potion record) and the manifest
/// `battle.items` block to a copied fixture project.
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
                "fields": [ {"id": "name"}, {"id": "healHp"}, {"id": "effect"} ]
            }));
    }
    manifest["battle"]["items"] =
        serde_json::json!({ "table": "items", "healField": "healHp", "starting": { "potion": 3 } });
    fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    fs::create_dir_all(root.join("data/items")).unwrap();
    fs::write(
        root.join("data/items/potion.json"),
        r#"{ "id": "potion", "name": "Potion", "healHp": 50, "effect": "Restores 50 HP" }"#,
    )
    .unwrap();
}

/// Play the armed battle with auto-A (Fight → first skill every round) and
/// collect the narration, then dismiss the post-battle text. Returns the
/// battle log.
fn auto_battle(game: &mut RunnerGame) -> Vec<String> {
    let mut narration: Vec<String> = Vec::new();
    for _ in 0..120 {
        let Some(battle) = game.battle() else { break };
        if let Some(line) = battle.current_line() {
            narration.push(line.to_string());
        }
        press_a(game);
    }
    dismiss_dialogue(game);
    narration
}

/// Walk to the Boss and arm the battle (intro line + startBattle).
fn arm_boss_battle(game: &mut RunnerGame) {
    game.debug_place(1, 2, Direction::Up);
    press_a(game); // boss intro
    press_a(game); // → battle
    assert!(game.battle().is_some(), "battle mode armed");
}

#[test]
fn party_hp_persists_across_two_battles() {
    let (_tmp, root) = demo_project("persist");
    write_bryn(&root);
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);

    // Battle 1 (auto-A Slash): the v1 numbers — Aria takes 28 once, then
    // KOs the Slime. Bryn never fights.
    arm_boss_battle(&mut game);
    assert_eq!(
        game.battle().unwrap().party().len(),
        2,
        "the whole table is the party"
    );
    let narration = auto_battle(&mut game);
    assert_eq!(narration.last(), Some(&"You won the battle!".to_string()));

    // The runner harvested the party state: Aria at 32/60, Bryn untouched.
    let party = game.party_state().expect("party state after battle");
    assert_eq!(party.len(), 2);
    assert_eq!(
        (party[0].id.as_str(), party[0].hp, party[0].mp),
        ("aria", 32, 20)
    );
    assert_eq!(
        (party[1].id.as_str(), party[1].hp, party[1].mp),
        ("bryn", 80, 60)
    );

    // Battle 2 starts from the carried-over state — NOT rebuilt at full HP.
    arm_boss_battle(&mut game);
    let battle = game.battle().unwrap();
    assert_eq!(battle.player().hp, 32, "Aria's damage carried over");
    assert_eq!(battle.party()[1].hp, 80, "Bryn still full");
    let narration = auto_battle(&mut game);
    assert_eq!(narration.last(), Some(&"You won the battle!".to_string()));
    let party = game.party_state().unwrap();
    assert_eq!(party[0].hp, 4, "another 28 damage (32 → 4)");
}

#[test]
fn party_switch_mid_battle_via_runner() {
    let (_tmp, root) = demo_project("switch");
    write_bryn(&root);
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    arm_boss_battle(&mut game);

    // Root → Party → Bryn (cursor starts on him): the switch consumes the
    // turn and the Slime hits BRYN (80 − 23 = 57).
    frame(&mut game, GbButton::Down.bit_mask());
    idle(&mut game, 1);
    press_a(&mut game); // Party
    assert_eq!(
        game.battle().unwrap().menu_items(),
        vec!["× Aria 60/60".to_string(), "Bryn 80/80".to_string()]
    );
    press_a(&mut game); // switch to Bryn
    let narration = auto_battle(&mut game);
    assert!(
        narration.contains(&"Come back, Aria!".to_string()),
        "{narration:?}"
    );
    assert!(
        narration.contains(&"Go, Bryn!".to_string()),
        "{narration:?}"
    );
    let party = game.party_state().unwrap();
    assert_eq!(
        party[1].hp, 57,
        "the Slime's hit landed on Bryn: {narration:?}"
    );
    assert_eq!(party[0].hp, 60, "Aria was never hit");
}

#[test]
fn item_menu_uses_starting_inventory_and_writes_back() {
    let (_tmp, root) = demo_project("items");
    write_items(&root);
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    arm_boss_battle(&mut game);

    // The starting inventory armed the battle; the root menu shows Item.
    assert_eq!(
        game.battle().unwrap().inventory().get("potion"),
        Some(&3),
        "inventory from items.starting"
    );
    assert_eq!(
        game.battle().unwrap().menu_items(),
        vec![
            "Fight".to_string(),
            "Party".to_string(),
            "Item".to_string(),
            "Run".to_string()
        ]
    );

    // Round 1: take a hit first so the heal is visible — auto-A Slash once.
    press_a(&mut game); // Root → Fight
    press_a(&mut game); // Slash
    for _ in 0..8 {
        if game.battle().unwrap().in_menu() {
            break;
        }
        press_a(&mut game);
    }
    assert_eq!(game.battle().unwrap().player().hp, 32);

    // Round 2: Root → Item → Potion. Heals 28 (32 → 60, capped), 3 → 2
    // potions, and the Slime still gets its turn.
    frame(&mut game, GbButton::Down.bit_mask());
    idle(&mut game, 1);
    frame(&mut game, GbButton::Down.bit_mask());
    idle(&mut game, 1);
    press_a(&mut game); // Item
    assert_eq!(
        game.battle().unwrap().menu_items(),
        vec!["Potion ×3".to_string()]
    );
    press_a(&mut game); // use it
    let narration = auto_battle(&mut game);
    assert!(
        narration.contains(&"Aria used Potion!".to_string()),
        "{narration:?}"
    );
    assert!(
        narration.contains(&"Aria recovered 28 HP!".to_string()),
        "{narration:?}"
    );
    assert_eq!(narration.last(), Some(&"You won the battle!".to_string()));

    // The used potion left the persistent inventory; Aria is back near full.
    assert_eq!(game.inventory().unwrap().get("potion"), Some(&2));
    let party = game.party_state().unwrap();
    assert_eq!(
        party[0].hp, 32,
        "healed to 60, then the Slime's 28 (60 → 32)"
    );
}

#[test]
fn party_state_and_inventory_round_trip_through_the_save() {
    let (_tmp, root) = demo_project("savev2");
    write_items(&root);
    let save_file = root.join("test-save.json");
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        save_file: Some(save_file.clone()),
        write_saves: true,
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    arm_boss_battle(&mut game);
    let narration = auto_battle(&mut game);
    assert_eq!(narration.last(), Some(&"You won the battle!".to_string()));
    drop(game);

    // The save (written when the scene finished) carries the party +
    // inventory at the current save version.
    let text = fs::read_to_string(&save_file).expect("save written");
    let raw: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(raw["version"], dotzuki_runner::SAVE_VERSION);
    assert_eq!(raw["party"][0]["hp"], 32);
    assert_eq!(raw["inventory"]["potion"], 3);

    // A fresh boot resumes the party state (and the flags).
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        save_file: Some(save_file),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game from save");
    let party = game.party_state().expect("party state from the save");
    assert_eq!((party[0].id.as_str(), party[0].hp), ("aria", 32));
    assert!(game.flag("BEAT_SLIME"), "flags restored too");

    // …and the next battle really starts from it.
    arm_boss_battle(&mut game);
    assert_eq!(game.battle().unwrap().player().hp, 32);
}

#[test]
fn status_persists_across_two_battles() {
    let (_tmp, root) = demo_project("statuspersist");
    write_rules(
        &root,
        r#"Ruleset(
            stats: ["hp", "attack", "defense", "speed"],
            types: ["fire", "grass", "water"],
            resources: ["mp"],
            type_chart: [],
            effects: [
                Effect(id: "venom-sting", kind: Move, power: 15, accuracy: 100, hooks: [
                    Hook(on: "DamagingHit", chance: [1, 1], do: [
                        InflictStatus(status: "poison", target: Target),
                    ]),
                ]),
                Effect(id: "poison", kind: Status, hooks: [
                    Hook(on: "Residual", do: [
                        DamageFraction(num: 1, den: 8, of: MaxHp, target: Target),
                    ]),
                ]),
            ],
        )"#,
    );
    // The Slime fights with venom-sting only (weak — Aria wins in 2 rounds).
    let slime: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("data/monsters/slime.json")).unwrap())
            .unwrap();
    let mut slime = slime;
    slime["skills"] = serde_json::json!(["venom-sting"]);
    fs::write(
        root.join("data/monsters/slime.json"),
        serde_json::to_string_pretty(&slime).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("data/spells/venom-sting.json"),
        r#"{ "id": "venom-sting", "name": "venom-sting", "type": "attack", "power": 15, "accuracy": 100, "mpCost": 0 }"#,
    )
    .unwrap();

    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);

    // Battle 1: poisoned in round 1, but Aria wins.
    arm_boss_battle(&mut game);
    let narration = auto_battle(&mut game);
    assert!(
        narration.contains(&"Aria was afflicted with poison!".to_string()),
        "{narration:?}"
    );
    assert_eq!(
        narration.last(),
        Some(&"You won the battle!".to_string()),
        "{narration:?}"
    );
    let party = game.party_state().unwrap();
    assert_eq!(
        party[0].status.as_deref(),
        Some("poison"),
        "status harvested"
    );

    // Battle 2: Aria starts poisoned — the chip fires after her first action.
    arm_boss_battle(&mut game);
    assert_eq!(
        game.battle().unwrap().player().status.as_deref(),
        Some("poison")
    );
    let narration = auto_battle(&mut game);
    assert!(
        narration.contains(&"Aria is hurt by poison!".to_string()),
        "the carried-over poison still chips: {narration:?}"
    );
}

// ── EXP & levels (v2-c) ───────────────────────────────────────────────────────

/// Arm the v2-c levels block (the documented defaults) on a copied fixture
/// project, and give the Slime an 8-EXP reward.
fn write_levels(root: &Path) {
    let path = root.join(".dotzuki-editor.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    manifest["battle"]["levels"] = serde_json::json!({
        "expField": "exp",
        "levelField": "level",
        "curve": { "base": 8, "exponent": 3 },
        "growth": 0.05,
        "maxLevel": 100
    });
    fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    let slime_path = root.join("data/monsters/slime.json");
    let mut slime: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&slime_path).unwrap()).unwrap();
    slime["exp"] = serde_json::json!(8);
    fs::write(&slime_path, serde_json::to_string_pretty(&slime).unwrap()).unwrap();
}

#[test]
fn levels_block_parses_with_per_key_defaults() {
    let (_tmp, root) = demo_project("levelsparse");
    // An empty block takes every default…
    let path = root.join(".dotzuki-editor.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    manifest["battle"]["levels"] = serde_json::json!({});
    fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    let project = LoadedProject::load(&root).expect("load demo project");
    let levels = project
        .manifest()
        .battle
        .as_ref()
        .and_then(|b| b.levels.as_ref())
        .expect("levels block");
    assert_eq!(levels.exp_field, "exp");
    assert_eq!(levels.level_field, "level");
    assert_eq!((levels.curve.base, levels.curve.exponent), (8, 3));
    assert_eq!(levels.growth, 0.05);
    assert_eq!(levels.max_level, 100);

    // …and a partial block fills the rest.
    manifest["battle"]["levels"] = serde_json::json!({ "growth": 0.1, "curve": { "base": 4 } });
    fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    let project = LoadedProject::load(&root).expect("load demo project");
    let levels = project
        .manifest()
        .battle
        .as_ref()
        .unwrap()
        .levels
        .as_ref()
        .unwrap();
    assert_eq!(levels.growth, 0.1);
    assert_eq!((levels.curve.base, levels.curve.exponent), (4, 3));
    assert_eq!(levels.exp_field, "exp");
}

#[test]
fn win_awards_exp_level_up_and_it_persists_into_the_next_battle() {
    let (_tmp, root) = demo_project("levelup");
    write_levels(&root);
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);

    // Battle 1: the v1 numbers are UNCHANGED (no level field on the records
    // ⇒ ×1) — 53/28 — then the award narrates after the win text.
    arm_boss_battle(&mut game);
    let narration = auto_battle(&mut game);
    assert!(
        narration.contains(&"53 damage!".to_string()),
        "{narration:?}"
    );
    let win = narration
        .iter()
        .position(|l| l == "You won the battle!")
        .expect("win line");
    assert_eq!(narration[win + 1], "Aria gained 8 EXP!");
    assert_eq!(narration[win + 2], "Aria grew to level 2!");

    // The harvest: level 2, the 8 EXP spent on the level-up, and the
    // heal-the-delta pools (28 taken: 60 → 32; ×1.05 max ⇒ 32 + 3 = 35/63).
    let party = game.party_state().expect("party state after battle");
    assert_eq!((party[0].level, party[0].exp), (2, 0));
    assert_eq!((party[0].hp, party[0].mp), (35, 21));

    // Battle 2 starts from the grown stats.
    arm_boss_battle(&mut game);
    let battle = game.battle().unwrap();
    assert_eq!(battle.player().level, 2);
    assert_eq!((battle.player().hp, battle.player().max_hp), (35, 63));
    assert_eq!(battle.player().max_mp, 21);
    let narration = auto_battle(&mut game);
    // 8 more EXP (needs 64 at level 2) ⇒ no second level-up.
    assert!(
        narration.contains(&"Aria gained 8 EXP!".to_string()),
        "{narration:?}"
    );
    assert!(
        !narration.iter().any(|l| l.contains("grew to level")),
        "{narration:?}"
    );
    let party = game.party_state().unwrap();
    assert_eq!((party[0].level, party[0].exp), (2, 8));
}

#[test]
fn fainted_member_gains_no_exp() {
    let (_tmp, root) = demo_project("faintednoexp");
    write_levels(&root);
    // Bryn at 1 max HP: he faints to the Slime's first answer and stays
    // fainted — a fainted member gains no EXP.
    fs::write(
        root.join("data/heroes/bryn.json"),
        r#"{
  "id": "bryn",
  "name": "Bryn",
  "hp": 1,
  "atk": 14,
  "def": 12,
  "spd": 10,
  "mp": 60,
  "element": "fire",
  "skills": ["slash"]
}"#,
    )
    .unwrap();
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    arm_boss_battle(&mut game);

    // Round 1: switch to Bryn (consumes the turn) — the Slime KOs him;
    // the forced switch brings Aria back for free. Collect the narration
    // from the start (the switch round's lines scroll by before auto_battle).
    let mut narration: Vec<String> = Vec::new();
    frame(&mut game, GbButton::Down.bit_mask());
    idle(&mut game, 1);
    press_a(&mut game); // Party
    press_a(&mut game); // switch to Bryn (cursor starts on him)
    for _ in 0..12 {
        let Some(battle) = game.battle() else { break };
        if let Some(line) = battle.current_line() {
            narration.push(line.to_string());
        }
        // The forced-switch pick list shows "name hp/max" rows.
        if battle.in_menu() && battle.menu_items().iter().any(|i| i.contains('/')) {
            break;
        }
        press_a(&mut game);
    }
    press_a(&mut game); // forced pick: Aria
    narration.extend(auto_battle(&mut game));
    assert!(
        narration.contains(&"Bryn fainted!".to_string()),
        "{narration:?}"
    );
    assert_eq!(
        narration.last(),
        Some(&"Aria grew to level 2!".to_string()),
        "{narration:?}"
    );
    assert!(
        !narration.iter().any(|l| l.contains("Bryn gained")),
        "{narration:?}"
    );
    let party = game.party_state().unwrap();
    assert_eq!(
        (party[0].id.as_str(), party[0].level, party[0].exp),
        ("aria", 2, 0)
    );
    assert_eq!(
        (party[1].id.as_str(), party[1].level, party[1].exp),
        ("bryn", 1, 0)
    );
}

#[test]
fn enemy_level_grows_its_stats() {
    let (_tmp, root) = demo_project("enemylevel");
    write_levels(&root);
    // A level-5 Slime: ×(1 + 0.05 × 4) = ×1.2 — hp 90 → 108, atk 8 → 9,
    // def 8 → 9, spd 5 → 6. Its level ALSO feeds RON battler_level (v1).
    let slime_path = root.join("data/monsters/slime.json");
    let mut slime: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&slime_path).unwrap()).unwrap();
    slime["level"] = serde_json::json!(5);
    fs::write(&slime_path, serde_json::to_string_pretty(&slime).unwrap()).unwrap();

    let project = LoadedProject::load(&root).expect("load demo project");
    let setup = BattleSetup::from_project(&project).expect("battle setup");
    let rng: Box<dyn dotzuki_runner::battle::BattleRng> =
        Box::new(dotzuki_runner::battle::ScriptedRng::new(vec![50, 100, 1]));
    let battle = setup.start("slime", rng).expect("battle");
    assert_eq!(battle.enemy().level, 5);
    assert_eq!(battle.enemy().max_hp, 108);
    assert_eq!(battle.enemy().attack, 9);
    assert_eq!((battle.enemy().defense, battle.enemy().speed), (9, 6));
    // Aria (no level field ⇒ 1) is untouched.
    assert_eq!((battle.player().max_hp, battle.player().attack), (60, 12));
}

#[test]
fn level_and_exp_round_trip_through_the_save() {
    let (_tmp, root) = demo_project("levelsave");
    write_levels(&root);
    let save_file = root.join("test-save.json");
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        save_file: Some(save_file.clone()),
        write_saves: true,
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    arm_boss_battle(&mut game);
    let narration = auto_battle(&mut game);
    assert!(
        narration.contains(&"Aria grew to level 2!".to_string()),
        "{narration:?}"
    );
    drop(game);

    // The save carries level/exp as optional fields (version stays 3; a 0
    // EXP is omitted and defaults back to 0 on load).
    let text = fs::read_to_string(&save_file).expect("save written");
    let raw: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(raw["version"], dotzuki_runner::SAVE_VERSION);
    assert_eq!(raw["party"][0]["level"], 2);
    assert!(
        raw["party"][0].get("exp").is_none(),
        "0 EXP is the omitted default"
    );

    // A fresh boot resumes level/exp; the next battle starts grown.
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        save_file: Some(save_file),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game from save");
    let party = game.party_state().expect("party state from the save");
    assert_eq!((party[0].level, party[0].exp), (2, 0));
    arm_boss_battle(&mut game);
    let battle = game.battle().unwrap();
    assert_eq!(battle.player().level, 2);
    assert_eq!((battle.player().hp, battle.player().max_hp), (35, 63));
}

#[test]
fn party_view_shows_level_and_exp_with_a_levels_block() {
    let (_tmp, root) = demo_project("partylevel");
    write_levels(&root);
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    arm_boss_battle(&mut game);
    let narration = auto_battle(&mut game);
    assert!(
        narration.contains(&"Aria grew to level 2!".to_string()),
        "{narration:?}"
    );

    // Start → Party: the member row carries Lv, an EXP progress line shows.
    frame(&mut game, GbButton::Start.bit_mask());
    idle(&mut game, 1);
    press_a(&mut game); // Party
    let lines = game.menu_lines().expect("party view open");
    assert!(
        lines.contains(&"Aria Lv 2 HP 35/63 MP 21/21".to_string()),
        "{lines:?}"
    );
    assert!(lines.contains(&"EXP 0/64".to_string()), "{lines:?}");
}

// ── encounters, trainer battles & Run (v2-d) ──────────────────────────────────

/// Add an encounters table (schema + records) and the manifest
/// `battle.encounters` + `battle.levels` blocks to a copied fixture project:
/// `gym-leader` (trainer, one Slime, pays 80), `duo` (wild pack, Slime then
/// Bat) and `broken` (references an unknown enemy id).
fn write_encounters(root: &Path) {
    let path = root.join(".dotzuki-editor.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    {
        let activities = manifest["activities"].as_array_mut().unwrap();
        let data = activities
            .iter_mut()
            .find(|a| a["id"] == "data")
            .expect("data activity");
        data["config"]["tables"].as_array_mut().unwrap().push(serde_json::json!({
            "id": "encounters", "label": "Encounters", "dir": "encounters", "icon": "", "idField": "id",
            "fields": [ {"id": "name"}, {"id": "enemies"}, {"id": "trainer"}, {"id": "money"} ]
        }));
    }
    manifest["battle"]["encounters"] = serde_json::json!({ "table": "encounters" });
    manifest["battle"]["levels"] = serde_json::json!({});
    fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    fs::create_dir_all(root.join("data/encounters")).unwrap();
    fs::write(
        root.join("data/encounters/gym-leader.json"),
        r#"{ "id": "gym-leader", "name": "Leader Kai", "enemies": ["slime"], "trainer": true, "money": 80 }"#,
    )
    .unwrap();
    fs::write(
        root.join("data/encounters/duo.json"),
        r#"{ "id": "duo", "name": "Wild Pack", "enemies": ["slime", "bat"] }"#,
    )
    .unwrap();
    fs::write(
        root.join("data/encounters/broken.json"),
        r#"{ "id": "broken", "name": "Broken", "enemies": ["dragon"] }"#,
    )
    .unwrap();
}

/// Patch the Cave's boss_fight storyline to battle `id`, branching three
/// ways on the outcome (win / run / else).
fn write_boss_battle(root: &Path, id: &str) {
    let scene = format!(
        r#"game_scene Cave {{
    @storyline("cave_enter") {{
        @trigger(map = "Cave", on_enter = true)
        @speaker("") {{
            "A cold wind blows through the cave."
        }}
    }}
    @storyline("boss_fight") {{
        @speaker("Boss") {{
            "A wild Slime blocks the path!"
        }}
        @if (true) {{
            result = startBattle("{id}")
            @if (result == "win") {{
                @command("setFlag", "BEAT_SLIME")
                @speaker("Boss") {{
                    "The path is clear!"
                }}
            }} @else {{
                @if (result == "run") {{
                    @command("setFlag", "RAN_AWAY")
                    @speaker("Boss") {{
                        "It let you go…"
                    }}
                }} @else {{
                    @speaker("Boss") {{
                        "The Slime oozes onward..."
                    }}
                }}
            }}
        }}
    }}
}}
"#,
    );
    fs::write(root.join("data/maps/Cave/script.scene"), scene).unwrap();
}

#[test]
fn encounter_resolution_order_and_validation() {
    let (_tmp, root) = demo_project("resolve");
    write_encounters(&root);
    let project = LoadedProject::load(&root).expect("load demo project");
    let setup = BattleSetup::from_project(&project).expect("battle setup");
    let rng = || {
        Box::new(dotzuki_runner::battle::ScriptedRng::new(vec![50, 100, 1]))
            as Box<dyn dotzuki_runner::battle::BattleRng>
    };

    // An encounter id resolves to the enemy party (trainer + money).
    let b = setup.start("gym-leader", rng()).expect("gym-leader");
    assert_eq!(b.enemy().id, "slime");
    assert_eq!(b.enemies_remaining(), 0);
    assert!(b.is_trainer());
    assert_eq!(b.trainer_money(), 80);

    // A wild encounter: a queue, no trainer, no money.
    let b = setup.start("duo", rng()).expect("duo");
    assert_eq!(b.enemy().id, "slime");
    assert_eq!(b.enemies_remaining(), 1);
    assert!(!b.is_trainer());
    assert_eq!(b.trainer_money(), 0);

    // An enemy id still resolves as a single wild enemy (unchanged).
    let b = setup.start("slime", rng()).expect("slime");
    assert_eq!(b.enemy().id, "slime");
    assert_eq!(b.enemies_remaining(), 0);
    assert!(!b.is_trainer());

    // Unknown entirely: the v1 first-enemy fallback ("bat" sorts first).
    let b = setup.start("no-such", rng()).expect("fallback");
    assert_eq!(b.enemy().id, "bat");
    assert!(!b.is_trainer());

    // An encounter referencing an unknown enemy id is a clear error.
    let err = setup
        .start("broken", rng())
        .err()
        .expect("broken must fail");
    let msg = format!("{err:#}");
    assert!(msg.contains("broken"), "{msg}");
    assert!(msg.contains("dragon"), "{msg}");
}

#[test]
fn scene_encounter_queue_send_out_and_exp_sum() {
    let (_tmp, root) = demo_project("queue");
    write_encounters(&root);
    // The duo's second member: a weak Bat (slower than Aria, survives one
    // Slash) with its own EXP reward.
    fs::write(
        root.join("data/monsters/bat.json"),
        r#"{ "id": "bat", "name": "Bat", "hp": 60, "atk": 5, "def": 10, "spd": 5, "mp": 0,
  "element": "fire", "exp": 5, "skills": [] }"#,
    )
    .unwrap();
    let slime: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("data/monsters/slime.json")).unwrap())
            .unwrap();
    let mut slime = slime;
    slime["exp"] = serde_json::json!(8);
    fs::write(
        root.join("data/monsters/slime.json"),
        serde_json::to_string_pretty(&slime).unwrap(),
    )
    .unwrap();
    write_boss_battle(&root, "duo");

    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    arm_boss_battle(&mut game);
    assert_eq!(game.battle().unwrap().enemies_remaining(), 1);
    assert!(!game.battle().unwrap().is_trainer());

    let narration = auto_battle(&mut game);
    println!("queue battle narration: {narration:?}");
    // The Slime faints (round 2), the Bat is sent out and fights back, then
    // faints — the win pays the SUM of both EXP rewards (8 + 5 = 13).
    let faint = narration
        .iter()
        .position(|l| l == "Slime fainted!")
        .expect("slime fainted");
    assert_eq!(narration[faint + 1], "Foe sent out Bat!");
    assert!(
        narration.contains(&"Bat used Attack!".to_string()),
        "{narration:?}"
    );
    let win = narration
        .iter()
        .position(|l| l == "You won the battle!")
        .expect("won");
    assert_eq!(narration[win - 1], "Bat fainted!");
    assert_eq!(narration[win + 1], "Aria gained 13 EXP!");
    assert_eq!(narration[win + 2], "Aria grew to level 2!");
    assert_eq!(game.money(), 100, "a wild win pays no money");
    assert!(game.flag("BEAT_SLIME"), "the queue emptying is a win");
}

#[test]
fn scene_trainer_battle_blocks_run_and_pays_money() {
    let (_tmp, root) = demo_project("trainer");
    write_encounters(&root);
    write_boss_battle(&root, "gym-leader");
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    arm_boss_battle(&mut game);
    assert!(game.battle().unwrap().is_trainer());

    // Root → Run: blocked, the turn NOT consumed (still the same battle,
    // Aria untouched).
    frame(&mut game, GbButton::Down.bit_mask());
    idle(&mut game, 1);
    frame(&mut game, GbButton::Down.bit_mask());
    idle(&mut game, 1);
    press_a(&mut game); // Run
    let battle = game.battle().expect("battle still running");
    assert_eq!(
        battle.current_line(),
        Some("Can't escape from a trainer battle!")
    );
    press_a(&mut game); // dismiss → back at the root menu
    let battle = game.battle().expect("battle still running");
    assert!(battle.in_menu(), "blocked Run returns to the menu");
    assert_eq!(battle.player().hp, 60, "the enemy never acted");
    assert_eq!(game.money(), 100);

    // Fight on and win: the trainer pays 80 G, narrated.
    let narration = auto_battle(&mut game);
    assert_eq!(
        narration.last(),
        Some(&"Got 80 G for winning!".to_string()),
        "{narration:?}"
    );
    assert_eq!(game.money(), 180, "100 start + 80 trainer reward");
    assert!(game.flag("BEAT_SLIME"));
}

#[test]
fn scene_wild_run_reaches_the_scene_as_run() {
    let (_tmp, root) = demo_project("run");
    write_boss_battle(&root, "slime");
    let project = LoadedProject::load(&root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(vec![50, 100, 1]),
        ..RunnerOptions::default()
    };
    let mut game = RunnerGame::new(project, opts).expect("boot game");
    reach_boss(&mut game);
    arm_boss_battle(&mut game);
    assert!(
        !game.battle().unwrap().is_trainer(),
        "no encounters block ⇒ wild"
    );

    // Root → Run: the wild battle ends immediately.
    frame(&mut game, GbButton::Down.bit_mask());
    idle(&mut game, 1);
    frame(&mut game, GbButton::Down.bit_mask());
    idle(&mut game, 1);
    press_a(&mut game); // Run
    assert_eq!(
        game.battle().unwrap().current_line(),
        Some("Got away safely!")
    );
    press_a(&mut game); // dismiss → the scene resumes with "run"
    assert!(game.battle().is_none(), "the battle is over");

    // The scene branched on "run" (NOT win): its run text + flag, no win
    // flag, no money, no whiteout.
    let page = game.dialogue_text().expect("post-run text");
    assert!(page.contains("It let you go"), "page: {page:?}");
    dismiss_dialogue(&mut game);
    assert!(game.flag("RAN_AWAY"));
    assert!(!game.flag("BEAT_SLIME"));
    assert_eq!(game.money(), 100);
    assert!(!game.whiteout_active());
    // The party state still harvested (untouched here).
    assert_eq!(game.party_state().unwrap()[0].hp, 60);
}
