//! Integration tests for `dotzuki run` save/load against the committed fixture
//! project (`tests/fixtures/demo/`), driven windowless through the public
//! `RunnerGame` API with tempdir fixture copies.
//!
//! Covered:
//!
//! 1. play → warp → set a flag via the choice scene → a new game instance
//!    resumes into the saved map at the saved tile with flags restored (and
//!    no opening dispatch replay);
//! 2. a corrupt save file boots fresh, no panic;
//! 3. the `fresh` option ignores an existing save;
//! 4. a save-format version mismatch boots fresh;
//! 5. `write_saves: false` (the headless default) never writes a file;
//! 6. a `map` option overrides the save's map.

use crate::alloc_prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use dotzuki_engine::overworld::types::Direction;
use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_runner::{LoadedProject, RunnerGame, RunnerOptions, DEFAULT_SAVE_FILE};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// Unique temp directory, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(test: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "dotzuki-runner-save-{test}-{}-{id}",
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

/// Write a 64×16 `tileset.png` — mirrors fixture.rs.
fn write_tileset(map_dir: &Path) {
    let tile = 16u32;
    let mut img = image::RgbaImage::new(tile * 4, tile);
    for i in 0..4 {
        for y in 0..tile {
            for x in 0..tile {
                img.put_pixel(i * tile + x, y, image::Rgba([0xFF, 0xFF, 0xFF, 0xFF]));
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

/// Boot a game over the project at `root`.
fn boot_game(root: &Path, opts: RunnerOptions) -> RunnerGame {
    let project = LoadedProject::load(root).expect("load demo project");
    RunnerGame::new(project, opts).expect("boot game")
}

/// Boot with saves enabled (the windowed-mode policy).
fn boot_saving(root: &Path) -> RunnerGame {
    boot_game(
        root,
        RunnerOptions {
            write_saves: true,
            ..RunnerOptions::default()
        },
    )
}

fn save_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_SAVE_FILE)
}

/// Update the game one frame with `mask` held.
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

fn dismiss_dialogue(game: &mut RunnerGame) {
    for _ in 0..20 {
        if game.dialogue_text().is_none() && game.choice_options().is_none() {
            return;
        }
        press_a(game);
    }
    panic!("dialogue did not close after 20 A presses");
}

/// Play from boot into the Cave and take the Hermit's torch: Town main →
/// warp → cave_enter → hermit_choice ("Take it"). Ends settled in the
/// overworld at (2,3) facing Up with `TOOK_TORCH` set.
fn play_into_cave(game: &mut RunnerGame) {
    dismiss_dialogue(game); // Town main
    for _ in 0..8 {
        frame(game, GbButton::Left.bit_mask());
    }
    idle(game, 2 * 10 + 2); // fade out + fade in
    assert_eq!(game.current_map_id(), Some("Cave"));
    dismiss_dialogue(game); // cave_enter

    game.debug_place(2, 3, Direction::Up);
    press_a(game); // "Will you take the torch?"
    press_a(game); // → choice menu
    assert!(game.choice_options().is_some());
    press_a(game); // confirm "Take it" (index 0)
    dismiss_dialogue(game); // "Wise choice."
    assert!(game.flag("TOOK_TORCH"));
}

// ── (a) full round trip: play → save → new instance resumes ────────────────

#[test]
fn new_instance_resumes_saved_map_position_and_flags() {
    let (_tmp, root) = demo_project("roundtrip");
    let mut game = boot_saving(&root);
    play_into_cave(&mut game);

    let path = save_path(&root);
    assert!(path.is_file(), "stable-state saves were written");
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.contains(&format!("\"version\": {}", dotzuki_runner::SAVE_VERSION)),
        "save: {text}"
    );
    assert!(text.contains("\"map\": \"Cave\""), "save: {text}");

    // A new instance over the same project resumes instead of booting fresh.
    let mut game2 = boot_saving(&root);
    assert_eq!(
        game2.current_map_id(),
        Some("Cave"),
        "resumed into saved map"
    );
    assert_eq!(game2.player_tile(), (2, 3), "resumed at saved tile");
    assert!(game2.flag("TOOK_TORCH"), "flags restored");
    assert!(
        game2.dialogue_text().is_none(),
        "resume skips the opening dispatch (no cave_enter/main replay)"
    );
    idle(&mut game2, 10);
    assert!(
        game2.dialogue_text().is_none(),
        "nothing fires later either"
    );
}

// ── (b) corrupt save → fresh boot, no panic ─────────────────────────────────

#[test]
fn corrupt_save_boots_fresh() {
    let (_tmp, root) = demo_project("corrupt");
    fs::write(save_path(&root), "{ not valid json").unwrap();

    let game = boot_saving(&root);
    assert_eq!(game.current_map_id(), Some("Town"));
    let page = game.dialogue_text().expect("normal boot plays Town main");
    assert!(page.contains("Welcome to Town"), "page: {page:?}");
}

// ── (c) `fresh` ignores an existing save ────────────────────────────────────

#[test]
fn fresh_option_ignores_save() {
    let (_tmp, root) = demo_project("fresh");
    let mut game = boot_saving(&root);
    play_into_cave(&mut game);
    assert!(save_path(&root).is_file());

    let game2 = boot_game(
        &root,
        RunnerOptions {
            fresh: true,
            write_saves: true,
            ..RunnerOptions::default()
        },
    );
    assert_eq!(
        game2.current_map_id(),
        Some("Town"),
        "fresh boots the entry map"
    );
    assert!(!game2.flag("TOOK_TORCH"), "fresh does not restore flags");
    let page = game2.dialogue_text().expect("fresh boot plays Town main");
    assert!(page.contains("Welcome to Town"), "page: {page:?}");
}

// ── (d) version mismatch → fresh boot ───────────────────────────────────────

#[test]
fn version_mismatch_boots_fresh() {
    let (_tmp, root) = demo_project("version");
    fs::write(
        save_path(&root),
        r#"{
  "version": 99,
  "map": "Cave",
  "player": { "x": 3, "y": 4, "facing": "down" },
  "flags": { "TOOK_TORCH": true }
}"#,
    )
    .unwrap();

    let game = boot_saving(&root);
    assert_eq!(
        game.current_map_id(),
        Some("Town"),
        "incompatible save ignored"
    );
    assert!(!game.flag("TOOK_TORCH"));
}

// ── (d2) a v1 save still resumes (party/inventory default to fresh) ─────────

#[test]
fn v1_save_resumes_with_fresh_party_state() {
    let (_tmp, root) = demo_project("v1compat");
    fs::write(
        save_path(&root),
        r#"{
  "version": 1,
  "map": "Cave",
  "player": { "x": 3, "y": 4, "facing": "down" },
  "flags": { "TOOK_TORCH": true }
}"#,
    )
    .unwrap();

    let game = boot_saving(&root);
    assert_eq!(game.current_map_id(), Some("Cave"), "v1 saves still resume");
    assert_eq!(game.player_tile(), (3, 4));
    assert!(game.flag("TOOK_TORCH"));
    assert!(
        game.party_state().is_none(),
        "v1 ⇒ fresh party at the first battle"
    );
    assert!(game.inventory().is_none(), "v1 ⇒ starting inventory");
}

// ── (e) write_saves off (headless default) never writes ────────────────────

#[test]
fn no_save_written_when_write_saves_off() {
    let (_tmp, root) = demo_project("nowrite");
    // RunnerOptions::default() has write_saves: false — the headless policy.
    let mut game = boot_game(&root, RunnerOptions::default());
    play_into_cave(&mut game);
    idle(&mut game, 30);
    assert!(
        !save_path(&root).exists(),
        "no save file without write_saves"
    );
}

// ── (g) export_save/import_save round trip (the WASM localStorage bridge) ───

#[test]
fn export_import_save_round_trip_without_disk() {
    let (_tmp, root) = demo_project("exportimport");
    // write_saves off: nothing touches the disk; the JSON string is the save.
    let mut game = boot_game(&root, RunnerOptions::default());
    play_into_cave(&mut game);
    assert!(!save_path(&root).exists(), "no disk save in this test");

    let json = game.export_save().expect("stable overworld state exports");
    assert!(json.contains("\"map\": \"Cave\""), "save: {json}");

    // A fresh boot over the same project ignores disk saves anyway; import
    // the exported JSON instead (the WASM shell's localStorage path).
    let mut game2 = boot_game(&root, RunnerOptions::default());
    assert!(game2.import_save(&json), "valid exported save imports");
    assert_eq!(game2.current_map_id(), Some("Cave"));
    assert_eq!(game2.player_tile(), (2, 3));
    assert!(game2.flag("TOOK_TORCH"));

    // Bad input never crashes nor clobbers the current state.
    assert!(!game2.import_save("{ not json"));
    assert!(!game2.import_save(r#"{ "version": 99, "map": "Town", "player": { "x": 0, "y": 0 } }"#));
    assert_eq!(game2.current_map_id(), Some("Cave"));
}

#[test]
fn export_save_is_none_mid_scene() {
    let (_tmp, root) = demo_project("exporttransient");
    let game = boot_game(&root, RunnerOptions::default());
    // Fresh boot plays Town main → a textbox owns the screen: transient.
    assert!(game.dialogue_text().is_some());
    assert!(
        game.export_save().is_none(),
        "mid-scene state cannot round-trip"
    );
}

// ── (f) --map overrides the save's map ─────────────────────────────────────

#[test]
fn map_option_overrides_save() {
    let (_tmp, root) = demo_project("mapoverride");
    let mut game = boot_saving(&root);
    play_into_cave(&mut game);
    assert!(save_path(&root).is_file());

    let game2 = boot_game(
        &root,
        RunnerOptions {
            map: Some("Town".to_string()),
            write_saves: true,
            ..RunnerOptions::default()
        },
    );
    assert_eq!(
        game2.current_map_id(),
        Some("Town"),
        "--map wins over the save"
    );
    assert_eq!(
        game2.player_tile(),
        (1, 1),
        "spawn scan on the override map"
    );
}
