//! Integration tests for `dotzuki run --watch` hot reload against the committed
//! fixture project (`tests/fixtures/demo/`). The reload *logic* is driven
//! directly through [`RunnerGame::reload_scenes`] /
//! [`RunnerGame::reload_current_map`] after overwriting fixture files in a
//! tempdir copy — no notify timing involved, so the tests are not flaky.
//!
//! Covered:
//!
//! 1. editing `Town/script.scene` text → the next NPC talk shows the new
//!    text, without rebooting the game;
//! 2. a broken scene edit keeps the old scenes running (reload reports
//!    failure, old dialogue still plays);
//! 3. editing the collision layer of the current map changes `is_blocked`
//!    after the reload while the player position is preserved.

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
            "dotzuki-runner-watch-{test}-{}-{id}",
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

/// Write a 64×16 `tileset.png` (four 16×16 flat-colour tiles, row-major) —
/// mirrors fixture.rs.
fn write_tileset(map_dir: &Path) {
    let tile = 16u32;
    let colors = [
        [0xFF, 0x00, 0x00, 0xFF],
        [0x00, 0xFF, 0x00, 0xFF],
        [0x00, 0x00, 0xFF, 0xFF],
        [0xFF, 0xFF, 0x00, 0xFF],
    ];
    let mut img = image::RgbaImage::new(tile * 4, tile);
    for (i, &[r, g, b, a]) in colors.iter().enumerate() {
        for y in 0..tile {
            for x in 0..tile {
                img.put_pixel(i as u32 * tile + x, y, image::Rgba([r, g, b, a]));
            }
        }
    }
    img.save(map_dir.join("tileset.png")).unwrap();
}

/// Copy the committed fixture into a temp dir, generate tilesets, boot a game
/// (watching enabled, though these tests drive the reload path directly).
fn boot(test: &str) -> (TestDir, PathBuf, RunnerGame) {
    let tmp = TestDir::new(test);
    let root = tmp.path().join("demo");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo");
    copy_dir(&fixture, &root);
    write_tileset(&root.join("data/maps/Town"));
    write_tileset(&root.join("data/maps/Cave"));
    let project = LoadedProject::load(&root).expect("load demo project");
    let game = RunnerGame::new(
        project,
        RunnerOptions {
            watch: true,
            ..RunnerOptions::default()
        },
    )
    .expect("boot game");
    (tmp, root, game)
}

/// Update the game one frame with `mask` held (fresh InputState ⇒ every
/// non-zero frame also reads as a just-press).
fn frame(game: &mut RunnerGame, mask: u8) {
    let mut input = InputState::new();
    input.set_from_bitmask(mask);
    game.update(&input);
}

/// Run `n` frames with nothing held.
fn idle(game: &mut RunnerGame, n: u32) {
    for _ in 0..n {
        frame(game, 0);
    }
}

/// Single A press (one frame is enough: a fresh InputState is a just-press).
fn press_a(game: &mut RunnerGame) {
    frame(game, GbButton::A.bit_mask());
    idle(game, 1);
}

/// Press A until no textbox/choice is on screen (bounded, so a broken scene
/// fails loudly instead of hanging the test).
fn dismiss_dialogue(game: &mut RunnerGame) {
    for _ in 0..20 {
        if game.dialogue_text().is_none() && game.choice_options().is_none() {
            return;
        }
        press_a(game);
    }
    panic!("dialogue did not close after 20 A presses");
}

/// Talk to the Guide NPC (2,1): stand at (1,1), face right, press A.
fn talk_to_guide(game: &mut RunnerGame) {
    game.debug_place(1, 1, Direction::Right);
    press_a(game);
}

// ── (a) scene text edit takes effect on next talk, without reboot ───────────

#[test]
fn scene_edit_takes_effect_on_next_talk() {
    let (_tmp, root, mut game) = boot("scene-edit");
    dismiss_dialogue(&mut game); // Town main auto-play

    // Baseline: talking to the Guide runs the Town scene's `main`.
    talk_to_guide(&mut game);
    let page = game.dialogue_text().expect("guide speaks");
    assert!(page.contains("Welcome to Town."), "page: {page:?}");
    dismiss_dialogue(&mut game);

    // Edit the scene on disk and reload — same game instance, no reboot.
    let scene_path = root.join("data/maps/Town/script.scene");
    let source = fs::read_to_string(&scene_path).unwrap();
    fs::write(
        &scene_path,
        source.replace("Welcome to Town.", "Welcome to the NEW Town."),
    )
    .unwrap();
    assert!(game.reload_scenes(), "valid edit should reload cleanly");

    talk_to_guide(&mut game);
    let page = game.dialogue_text().expect("guide speaks the new text");
    assert!(page.contains("Welcome to the NEW Town."), "page: {page:?}");
    dismiss_dialogue(&mut game);
}

// ── (b) a broken scene edit keeps the old scenes running ────────────────────

#[test]
fn broken_scene_edit_keeps_old_scenes() {
    let (_tmp, root, mut game) = boot("broken-edit");
    dismiss_dialogue(&mut game); // Town main auto-play

    let scene_path = root.join("data/maps/Town/script.scene");
    fs::write(
        &scene_path,
        "game_scene Town {\n    @storylines {\n        @@@ not valid dsl @@@\n    }\n}\n",
    )
    .unwrap();
    assert!(
        !game.reload_scenes(),
        "a scene with diagnostics must not swap in"
    );

    // The old compiled scene still plays.
    talk_to_guide(&mut game);
    let page = game.dialogue_text().expect("old scene still runs");
    assert!(page.contains("Welcome to Town."), "page: {page:?}");
    dismiss_dialogue(&mut game);
}

// ── (c) collision edit applies to the current map, position preserved ───────

#[test]
fn collision_edit_applies_to_current_map() {
    let (_tmp, root, mut game) = boot("map-edit");
    dismiss_dialogue(&mut game); // Town main auto-play
    assert_eq!(game.player_tile(), (1, 1));
    assert!(!game.is_blocked(2, 1), "interior starts walkable");

    // Flip collision tile (2,1) to solid: row 1 of the collision layer goes
    // from `0, 0, 0, 1` to `0, 0, 1, 1` (ground data differs, so the string
    // is unique to the collision layer).
    let tmx_path = root.join("data/maps/Town/map.tmx.json");
    let tmx = fs::read_to_string(&tmx_path).unwrap();
    let edited = tmx.replace(
        "[1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 1]",
        "[1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1]",
    );
    assert_ne!(tmx, edited, "collision data found and edited");
    fs::write(&tmx_path, edited).unwrap();

    assert!(game.reload_current_map(), "valid map edit should reload");
    assert!(game.is_blocked(2, 1), "edited tile is now solid");
    assert_eq!(game.player_tile(), (1, 1), "player position preserved");
}

// ── map reload failure keeps the old map ────────────────────────────────────

#[test]
fn broken_map_edit_keeps_old_map() {
    let (_tmp, root, mut game) = boot("broken-map");
    dismiss_dialogue(&mut game); // Town main auto-play
    assert_eq!(game.current_map_id(), Some("Town"));
    assert!(!game.is_blocked(2, 1));

    fs::remove_file(root.join("data/maps/Town/tileset.png")).unwrap();
    assert!(
        !game.reload_current_map(),
        "a map that fails to load must not swap in"
    );
    assert_eq!(game.current_map_id(), Some("Town"), "old map kept");
    assert!(!game.is_blocked(2, 1), "old collision still in effect");
}
