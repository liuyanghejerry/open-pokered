//! Integration tests for the `RunnerGame` runtime against the committed
//! fixture project (`tests/fixtures/demo/`): headless boot, the Town `main`
//! auto-play, the Town→Cave warp, and the Hermit's `ShowChoice` storyline.
//!
//! All tests drive the game windowless through the public
//! [`RunnerGame::update`]/[`RunnerGame::draw`] API with synthetic input — no
//! winit, no pixels. Tilesets are generated in code (see `fixture.rs`), so
//! no binary assets are committed.

use crate::alloc_prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use dotzuki_engine::overworld::types::Direction;
use dotzuki_engine::render::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_runner::{run_headless, HeadlessOptions, LoadedProject, RunnerGame, RunnerOptions};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// Unique temp directory, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(test: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "dotzuki-runner-game-{test}-{}-{id}",
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

/// Copy the committed fixture into a temp dir, generate tilesets, boot a game.
fn boot(test: &str) -> (TestDir, RunnerGame) {
    let tmp = TestDir::new(test);
    let root = tmp.path().join("demo");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo");
    copy_dir(&fixture, &root);
    write_tileset(&root.join("data/maps/Town"));
    write_tileset(&root.join("data/maps/Cave"));
    let project = LoadedProject::load(&root).expect("load demo project");
    let game = RunnerGame::new(project, RunnerOptions::default()).expect("boot game");
    (tmp, game)
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

// ── (a) headless boot ───────────────────────────────────────────────────────

#[test]
fn headless_boot_renders_nonuniform_framebuffer() {
    let (_tmp, mut game) = boot("headless");
    let fb = run_headless(&mut game, &HeadlessOptions::default()).expect("headless run");

    assert_eq!(fb.width(), 320);
    assert_eq!(fb.height(), 240);
    let colors: HashSet<[u8; 4]> = fb
        .data
        .chunks_exact(4)
        .map(|px| [px[0], px[1], px[2], px[3]])
        .collect();
    assert!(
        colors.len() > 4,
        "framebuffer should not be uniform (got {} colours)",
        colors.len()
    );
}

// ── (b) Town main storyline auto-plays ──────────────────────────────────────

#[test]
fn town_main_storyline_autoplays_on_boot() {
    let (_tmp, mut game) = boot("main");

    // The boot dispatch plays the map scene's `main` immediately.
    let page = game
        .dialogue_text()
        .expect("main storyline shows a textbox");
    assert!(page.contains("Welcome to Town"), "page: {page:?}");

    // Advancing past it ends the scene and leaves the once-only flag set.
    dismiss_dialogue(&mut game);
    assert!(game.flag("__played_main_Town"));
    assert!(game.dialogue_text().is_none());

    // …and it does not replay on its own afterwards.
    idle(&mut game, 10);
    assert!(game.dialogue_text().is_none());
}

// ── (c) walking onto the warp tile switches maps ────────────────────────────

#[test]
fn stepping_on_warp_tile_loads_cave() {
    let (_tmp, mut game) = boot("warp");
    dismiss_dialogue(&mut game); // Town main
    assert_eq!(game.current_map_id(), Some("Town"));
    // Spawn: map centre scanned outward — (2,1) is the Guide's tile, so the
    // player lands on (1,1), one step right of the (0,1) warp.
    assert_eq!(game.player_tile(), (1, 1));

    // Walk left onto the warp (8 frames/step), then let the fade play out.
    for _ in 0..8 {
        frame(&mut game, GbButton::Left.bit_mask());
    }
    idle(&mut game, 2 * 10 + 2); // fade out + fade in

    assert_eq!(game.current_map_id(), Some("Cave"));
    assert_eq!(game.player_tile(), (3, 4), "warp dest from objects.json");
    // Entering Cave fires its on_enter route.
    let page = game.dialogue_text().expect("cave_enter on_enter plays");
    assert!(page.contains("cold wind"), "page: {page:?}");
}

// ── (d) ShowChoice resolves with the selected index ─────────────────────────

#[test]
fn hermit_choice_sets_flag_for_selected_branch() {
    let (_tmp, mut game) = boot("choice");
    dismiss_dialogue(&mut game); // Town main
    for _ in 0..8 {
        frame(&mut game, GbButton::Left.bit_mask());
    }
    idle(&mut game, 2 * 10 + 2);
    dismiss_dialogue(&mut game); // cave_enter
    assert_eq!(game.current_map_id(), Some("Cave"));

    // Stand below the Hermit (2,2), face up, talk.
    game.debug_place(2, 3, Direction::Up);
    press_a(&mut game);
    let page = game.dialogue_text().expect("hermit speaks");
    assert!(page.contains("torch"), "page: {page:?}");
    press_a(&mut game);

    // Choice menu: two options; pick "Leave it" (index 1) → no flag.
    let options = game.choice_options().expect("choice menu open").to_vec();
    assert_eq!(options, vec!["Take it".to_string(), "Leave it".to_string()]);
    frame(&mut game, GbButton::Down.bit_mask());
    idle(&mut game, 1);
    press_a(&mut game); // confirm
    dismiss_dialogue(&mut game); // "Suit yourself."
    assert!(!game.flag("TOOK_TORCH"), "declining sets no flag");

    // Talk again, pick "Take it" (index 0) → the branch's setFlag lands.
    press_a(&mut game);
    press_a(&mut game); // past the question text → choice menu
    assert!(game.choice_options().is_some());
    press_a(&mut game); // confirm index 0
    dismiss_dialogue(&mut game); // "Wise choice."
    assert!(game.flag("TOOK_TORCH"), "accepting sets TOOK_TORCH");
}

// ── (e) stair tiles move the player between elevation levels ───────────────

#[test]
fn stairs_change_player_elevation() {
    let tmp = TestDir::new("stairs");
    let root = tmp.path().join("demo");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo");
    copy_dir(&fixture, &root);
    write_tileset(&root.join("data/maps/Town"));
    write_tileset(&root.join("data/maps/Cave"));
    // Rewrite Town as a two-level map: an ascend stair (GID 1) at (0,1) and
    // a descend stair (GID 2) at (1,1); no NPCs/warps to interfere.
    fs::write(
        root.join("data/maps/Town/map.tmx.json"),
        r#"{
  "width": 4, "height": 3, "tilewidth": 16, "tileheight": 16,
  "layers": [
    { "name": "ground", "width": 4, "height": 3, "data": [1,1,1,1, 1,1,1,1, 1,1,1,1] },
    { "name": "collision", "width": 4, "height": 3, "data": [1,1,1,1, 0,0,0,1, 1,1,1,1] },
    { "name": "collision1", "width": 4, "height": 3, "data": [1,1,1,1, 0,0,0,1, 1,1,1,1] },
    { "name": "stairs", "width": 4, "height": 3, "data": [0,0,0,0, 1,2,0,0, 0,0,0,0] }
  ],
  "tilesets": [
    { "firstgid": 1, "name": "demo", "tilewidth": 16, "tileheight": 16, "tilecount": 4 }
  ]
}"#,
    )
    .unwrap();
    fs::write(root.join("data/maps/Town/objects.json"), "{}").unwrap();

    let project = LoadedProject::load(&root).expect("load demo project");
    let mut game = RunnerGame::new(project, RunnerOptions::default()).expect("boot game");
    dismiss_dialogue(&mut game); // Town main
    game.debug_place(1, 1, Direction::Down);
    assert_eq!(game.player_elevation(), 0, "spawns on the ground level");

    // Walk left onto the ascend stair → level 1.
    for _ in 0..8 {
        frame(&mut game, GbButton::Left.bit_mask());
    }
    assert_eq!(game.player_tile(), (0, 1));
    assert_eq!(game.player_elevation(), 1, "GID 1 ascends one level");

    // Walk right onto the descend stair → back to the ground.
    for _ in 0..8 {
        frame(&mut game, GbButton::Right.bit_mask());
    }
    assert_eq!(game.player_tile(), (1, 1));
    assert_eq!(game.player_elevation(), 0, "GID 2 descends one level");
}

// ── (f) signs: face + A reads the sign text ─────────────────────────────────

#[test]
fn facing_a_sign_and_pressing_a_reads_it() {
    let (_tmp, mut game) = boot("sign");
    dismiss_dialogue(&mut game); // Town main

    // The Town sign is at (1,1); stand below it, face up, read.
    game.debug_place(1, 2, Direction::Up);
    press_a(&mut game);
    let page = game.dialogue_text().expect("sign text shows");
    assert!(page.contains("You are standing in Town"), "page: {page:?}");
    dismiss_dialogue(&mut game);
    assert!(game.dialogue_text().is_none());

    // Facing a sign-less tile, A does nothing.
    game.debug_place(3, 3, Direction::Up);
    press_a(&mut game);
    assert!(game.dialogue_text().is_none());
}

// ── dialogue-only boot (project without maps) ───────────────────────────────

#[test]
fn mapless_project_boots_dialogue_only() {
    let tmp = TestDir::new("mapless");
    let root = tmp.path().join("demo");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo");
    copy_dir(&fixture, &root);
    // Strip every map: only the story scene remains.
    fs::remove_dir_all(root.join("data/maps")).unwrap();

    let project = LoadedProject::load(&root).expect("load");
    let mut game = RunnerGame::new(project, RunnerOptions::default()).expect("boot");
    assert_eq!(game.current_map_id(), None);

    // The entry scene's main runs, then the game idles without panicking.
    let page = game.dialogue_text().expect("entry scene text");
    assert!(
        page.contains("Welcome to the demo project"),
        "page: {page:?}"
    );
    dismiss_dialogue(&mut game);
    idle(&mut game, 30);

    let mut fb = FrameBuffer::new(RenderConfig::new(320, 240), Rgba::BLACK);
    game.draw(&mut fb); // must not panic without a map

    // Regression: Idle must survive further updates (update() replaces the
    // mode with Overworld while dispatching) — the end card stays on screen.
    idle(&mut game, 30);
    let mut fb = FrameBuffer::new(RenderConfig::new(320, 240), Rgba::BLACK);
    game.draw(&mut fb);
    let colors: HashSet<[u8; 4]> = fb
        .data
        .chunks_exact(4)
        .map(|px| [px[0], px[1], px[2], px[3]])
        .collect();
    assert!(
        colors.len() > 2,
        "end card should render text, got {colors:?}"
    );
}
