//! Integration tests for map-sidecar random encounters: `encounters`
//! parsing (and back-compat with sidecars that lack it), the deterministic
//! per-step roll driven by `RunnerOptions::rng_script`, warp-over-encounter
//! priority, and the sceneless battle's return to the overworld.
//!
//! The committed fixture sidecars are untouched — tests inject an
//! `encounters` block into a temp copy (the Cave map, whose walkable
//! interior gives room to step).

use crate::alloc_prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_runner::{LoadedProject, MapObjects, RunnerGame, RunnerOptions};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// Unique temp directory, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(test: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "dotzuki-runner-encounters-{test}-{}-{id}",
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

/// Flat colours of the four generated tiles (GIDs 1..=4) — mirrors fixture.rs.
const TILE_COLORS: [[u8; 4]; 4] = [
    [0xFF, 0x00, 0x00, 0xFF],
    [0x00, 0xFF, 0x00, 0xFF],
    [0x00, 0x00, 0xFF, 0xFF],
    [0xFF, 0xFF, 0x00, 0xFF],
];

/// Write a 64×16 `tileset.png` (four 16×16 flat-colour tiles, row-major).
fn write_tileset(map_dir: &Path) {
    let tile = 16u32;
    let mut img = image::RgbaImage::new(tile * 4, tile);
    for (i, &[r, g, b, a]) in TILE_COLORS.iter().enumerate() {
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

/// Insert an `encounters` block into the copied Cave's objects.json.
fn inject_cave_encounters(root: &Path, encounters: serde_json::Value) {
    let path = root.join("data/maps/Cave/objects.json");
    let mut objects: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    objects
        .as_object_mut()
        .unwrap()
        .insert("encounters".to_string(), encounters);
    fs::write(&path, serde_json::to_string_pretty(&objects).unwrap()).unwrap();
}

/// Boot the copied project with a scripted rng (drives the encounter roll
/// AND every battle, deterministically).
fn boot(root: &Path, rng_script: Vec<u8>) -> RunnerGame {
    let project = LoadedProject::load(root).expect("load demo project");
    let opts = RunnerOptions {
        rng_script: Some(rng_script),
        ..RunnerOptions::default()
    };
    RunnerGame::new(project, opts).expect("boot game")
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

/// Walk from boot into the Cave (Town main → warp → cave_enter); the player
/// arrives on the warp tile (3,4) — standing on it is not a step, so no
/// encounter can roll before the test walks.
fn reach_cave(game: &mut RunnerGame) {
    dismiss_dialogue(game); // Town main
    for _ in 0..8 {
        frame(game, GbButton::Left.bit_mask());
    }
    idle(game, 2 * 10 + 2); // warp fade out + in
    dismiss_dialogue(game); // cave_enter
    assert_eq!(game.current_map_id(), Some("Cave"));
    assert_eq!(game.player_tile(), (3, 4));
}

/// Play the armed battle with auto-A (Fight → first skill every round) until
/// it resolves; fails loudly when it doesn't.
fn auto_battle(game: &mut RunnerGame) {
    for _ in 0..120 {
        if game.battle().is_none() {
            return;
        }
        press_a(game);
    }
    panic!("battle did not resolve after 120 A presses");
}

// ── parsing / back-compat ───────────────────────────────────────────────────

#[test]
fn encounters_block_deserializes_and_old_sidecars_default_to_none() {
    // The new block: rate + inclusive tile-rectangle zones + weighted table.
    let objects: MapObjects = serde_json::from_str(
        r#"{
            "npcs": [],
            "encounters": {
                "rate": 25,
                "zones": [
                    { "x": 0, "y": 5, "w": 8, "h": 3,
                      "table": [ { "id": "slime", "weight": 70 },
                                 { "id": "bug-catcher", "weight": 30 } ] }
                ]
            }
        }"#,
    )
    .expect("encounters block parses");
    let encounters = objects.encounters.expect("encounters present");
    assert_eq!(encounters.rate, 25);
    assert_eq!(encounters.zones.len(), 1);
    let zone = &encounters.zones[0];
    assert!(zone.contains(0, 5));
    assert!(
        zone.contains(7, 7),
        "rectangle is inclusive of its w×h extent"
    );
    assert!(!zone.contains(8, 7) && !zone.contains(7, 8));
    assert_eq!(zone.table[0].id, "slime");
    assert_eq!(zone.table[0].weight, 70);
    assert_eq!(zone.table[1].id, "bug-catcher");

    // A missing weight defaults to 1.
    let objects: MapObjects = serde_json::from_str(
        r#"{ "encounters": { "rate": 10, "zones": [ { "x": 0, "y": 0, "w": 1, "h": 1,
              "table": [ { "id": "slime" } ] } ] } }"#,
    )
    .unwrap();
    assert_eq!(
        objects.encounters.unwrap().zones[0].table[0].weight,
        1,
        "weight defaults to 1"
    );

    // The committed fixture sidecars (no `encounters` key) load unchanged —
    // `Option` + serde default ⇒ None, and the unknown `music` key in the
    // Town sidecar is still ignored.
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo");
    let town = MapObjects::load(&fixture.join("data/maps/Town")).expect("load Town sidecar");
    assert!(town.encounters.is_none());
    assert_eq!(town.npcs.len(), 1);
    assert_eq!(town.warps.len(), 1);
    assert_eq!(town.signs.len(), 1);
}

// ── the per-step roll ───────────────────────────────────────────────────────

#[test]
fn roll_hit_arms_a_battle_with_the_weighted_pick() {
    let (_tmp, root) = demo_project("hit");
    inject_cave_encounters(
        &root,
        serde_json::json!({
            "rate": 25,
            "zones": [ { "x": 3, "y": 1, "w": 1, "h": 3,
                "table": [ { "id": "slime", "weight": 70 },
                           { "id": "bat", "weight": 30 } ] } ]
        }),
    );
    // Roll byte 0 (< 25) hits; pick byte 80 lands past slime's 70 → bat.
    let mut game = boot(&root, vec![0, 80, 50, 100, 1]);
    reach_cave(&mut game);

    for _ in 0..8 {
        frame(&mut game, GbButton::Up.bit_mask());
    }
    let battle = game.battle().expect("a hit arms a sceneless battle");
    assert_eq!(
        battle.enemy().id,
        "bat",
        "the weighted draw picked past slime"
    );
}

#[test]
fn roll_miss_keeps_the_player_walking() {
    let (_tmp, root) = demo_project("miss");
    inject_cave_encounters(
        &root,
        serde_json::json!({
            "rate": 25,
            "zones": [ { "x": 3, "y": 1, "w": 1, "h": 3,
                "table": [ { "id": "slime", "weight": 1 } ] } ]
        }),
    );
    // Every roll byte is 255 ≥ 25 — never a hit (the script cycles).
    let mut game = boot(&root, vec![255]);
    reach_cave(&mut game);

    // Three steps through the zone: (3,3) → (3,2) → (3,1).
    for _ in 0..24 {
        frame(&mut game, GbButton::Up.bit_mask());
    }
    assert!(game.battle().is_none(), "a missed roll starts no battle");
    assert_eq!(game.player_tile(), (3, 1), "the walk continues unimpeded");
}

#[test]
fn stepping_outside_every_zone_rolls_nothing() {
    let (_tmp, root) = demo_project("outside");
    // Zone sits in the far column the test never enters; rate 255 + byte 0
    // would ALWAYS hit if a roll happened.
    inject_cave_encounters(
        &root,
        serde_json::json!({
            "rate": 255,
            "zones": [ { "x": 1, "y": 4, "w": 1, "h": 1,
                "table": [ { "id": "slime", "weight": 1 } ] } ]
        }),
    );
    let mut game = boot(&root, vec![0]);
    reach_cave(&mut game);

    for _ in 0..24 {
        frame(&mut game, GbButton::Up.bit_mask());
    }
    assert_eq!(game.player_tile(), (3, 1));
    assert!(game.battle().is_none(), "no zone ⇒ no roll");
}

#[test]
fn a_warp_tile_takes_priority_over_the_roll() {
    let (_tmp, root) = demo_project("warp-priority");
    // Zone exactly on the Cave→Town warp tile (3,4), always-hit.
    inject_cave_encounters(
        &root,
        serde_json::json!({
            "rate": 255,
            "zones": [ { "x": 3, "y": 4, "w": 1, "h": 1,
                "table": [ { "id": "slime", "weight": 1 } ] } ]
        }),
    );
    let mut game = boot(&root, vec![0]);
    reach_cave(&mut game);

    // Step off the warp (no zone at (2,4) ⇒ no roll), then back onto it:
    // the warp must fire, not an encounter.
    for _ in 0..8 {
        frame(&mut game, GbButton::Left.bit_mask());
    }
    assert_eq!(game.player_tile(), (2, 4));
    assert!(game.battle().is_none());
    for _ in 0..8 {
        frame(&mut game, GbButton::Right.bit_mask());
    }
    assert!(
        game.battle().is_none(),
        "warp beats the roll on the same tile"
    );
    idle(&mut game, 2 * 10 + 2); // fade out + in
    assert_eq!(game.current_map_id(), Some("Town"));
    // Town's main is once-only (its flag was set at boot) — nothing plays.
    dismiss_dialogue(&mut game);
}

// ── sceneless return flow ───────────────────────────────────────────────────

#[test]
fn winning_a_sceneless_battle_returns_to_the_overworld() {
    let (_tmp, root) = demo_project("win-return");
    inject_cave_encounters(
        &root,
        serde_json::json!({
            "rate": 255,
            "zones": [ { "x": 3, "y": 3, "w": 1, "h": 1,
                "table": [ { "id": "slime", "weight": 1 } ] } ]
        }),
    );
    // Roll byte 50 (< 255) hits; the pick byte is irrelevant (one entry).
    // The battle draws a FRESH copy of the same script ([50,100,1]: always
    // hits, 89% variance, never crits) — the scene-test numbers: Aria wins
    // in two Slashes, taking 28 once.
    let mut game = boot(&root, vec![50, 100, 1]);
    reach_cave(&mut game);

    for _ in 0..8 {
        frame(&mut game, GbButton::Up.bit_mask());
    }
    assert_eq!(game.player_tile(), (3, 3));
    let battle = game.battle().expect("encounter armed the battle");
    assert_eq!(battle.enemy().id, "slime");

    auto_battle(&mut game);
    assert!(game.battle().is_none());

    // The existing settlement path ran (party state harvested: 60 − 28).
    let party = game.party_state().expect("party state after the battle");
    assert_eq!((party[0].id.as_str(), party[0].hp), ("aria", 32));

    // No scene, no whiteout: the player is back in the overworld on the
    // encounter tile and simply walks on (left = out of the zone, no roll).
    assert_eq!(game.player_tile(), (3, 3));
    for _ in 0..8 {
        frame(&mut game, GbButton::Left.bit_mask());
    }
    assert_eq!(game.player_tile(), (2, 3), "walking resumes after the win");
    assert!(game.battle().is_none());
}
