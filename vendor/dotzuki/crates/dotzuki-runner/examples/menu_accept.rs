//! Menus/shops/game-over acceptance harness: boots a zero-Rust project
//! headless and drives, as evidence:
//!
//! 1. the **shop flow** — talk to the Shopkeeper, screenshot the shop UI,
//!    buy a Potion (money math + inventory logged), exit, and show the
//!    scene's follow-up text resume;
//! 2. the **game-over whiteout** — lose the Hermit's battle on purpose,
//!    show the scene's post-lose text, screenshot the blackout and the
//!    "collapsed" message, then log the healed party and the respawn at the
//!    entry map's spawn (flags kept).
//!
//! Usage:
//!
//! ```sh
//! cargo run -p dotzuki-runner --example menu_accept -- <project-dir> [shot-dir]
//! ```
//!
//! The project is expected to be a scaffolded `dotzuki` template with the
//! acceptance patch applied (Shopkeeper at (13,10), Hermit at (11,10), an
//! overwhelming Slime — see the feature branch's acceptance notes).

use crate::alloc_prelude::*;
use std::path::{Path, PathBuf};

use dotzuki_engine::overworld::types::Direction;
use dotzuki_engine::render::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_runner::headless::save_png;
use dotzuki_runner::{LoadedProject, RunnerGame, RunnerOptions, SCREEN_H, SCREEN_W};

/// Draw the current frame into a PNG.
fn snap(game: &mut RunnerGame, dir: &Option<PathBuf>, name: &str) {
    let Some(dir) = dir else { return };
    let mut fb = FrameBuffer::new(
        RenderConfig::new(SCREEN_W as u32, SCREEN_H as u32),
        Rgba::BLACK,
    );
    game.draw(&mut fb);
    let path = dir.join(format!("{name}.png"));
    save_png(&fb, &path).expect("screenshot");
    println!("  [shot] {}", path.display());
}

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

/// Press A until no textbox/choice/whiteout message is up (bounded).
fn dismiss(game: &mut RunnerGame) {
    for _ in 0..40 {
        if game.dialogue_text().is_none() && game.choice_options().is_none() {
            return;
        }
        press_a(game);
    }
    panic!("dialogue did not close");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .expect("usage: menu_accept <project-dir> [shot-dir]");
    let shot_dir = args.next().map(PathBuf::from);

    let project = LoadedProject::load(Path::new(&dir)).expect("load project");
    let mut game = RunnerGame::new(
        project,
        RunnerOptions {
            headless: true,
            fresh: true,
            // Deterministic battles: always hit, 89% variance, never crit.
            rng_script: Some(vec![50, 100, 1]),
            ..RunnerOptions::default()
        },
    )
    .expect("boot game");

    dismiss(&mut game); // StartTown main
    println!("== 1. the shop ==");
    println!("money at boot: {} (manifest shop.startMoney)", game.money());

    // Talk to the Shopkeeper at (13,10): the shop opens on the spot.
    game.debug_place(12, 10, Direction::Right);
    press_a(&mut game);
    let rows = game.shop_lines().expect("shop open");
    println!("shop shelf: {rows:?}");
    snap(&mut game, &shot_dir, "shop");

    // Buy one Potion: 100 → 80 G, inventory 3 → 4.
    let before = game.money();
    press_a(&mut game);
    let rows = game.shop_lines().expect("shop still open");
    println!(
        "bought a Potion: money {before} → {} G; inventory {:?}",
        game.money(),
        game.inventory().expect("inventory materialized")
    );
    println!("shop note: {:?}", rows.last());
    snap(&mut game, &shot_dir, "shop-bought");
    assert_eq!(game.money(), 80, "20 G charged");
    assert_eq!(game.inventory().unwrap().get("potion"), Some(&4));
    press_a(&mut game); // dismiss the note

    // B exits; the scene resumes with its follow-up line.
    press(&mut game, GbButton::B);
    let page = game.dialogue_text().expect("scene resumed after the shop");
    println!("scene resumed with: {page:?}");
    assert!(page.contains("Come again!"), "page: {page:?}");
    dismiss(&mut game);

    println!("\n== 2. the whiteout ==");
    // Talk to the Hermit at (11,10) and lose on purpose (overwhelming Slime).
    game.debug_place(12, 10, Direction::Left);
    press_a(&mut game); // "Something stirs…"
    press_a(&mut game); // → battle
    let mut last_log: Vec<String> = Vec::new();
    for _ in 0..120 {
        let Some(battle) = game.battle() else { break };
        last_log = battle.log().to_vec();
        press_a(&mut game);
    }
    println!("lost battle log:");
    for line in &last_log {
        println!("  {line}");
    }
    assert_eq!(
        last_log.last().map(String::as_str),
        Some("You lost the battle…")
    );

    // The scene's post-lose text still plays…
    let page = game.dialogue_text().expect("post-lose text");
    println!("post-lose text: {page:?}");
    assert!(page.contains("no match"), "page: {page:?}");
    press_a(&mut game); // scene finishes → whiteout

    // …then the whiteout: blackout first, then the collapsed line.
    assert!(game.whiteout_active(), "whiteout armed");
    idle(&mut game, 10);
    snap(&mut game, &shot_dir, "whiteout-blackout");
    idle(&mut game, 25);
    let page = game.dialogue_text().expect("whiteout message");
    println!("whiteout message: {page:?}");
    assert!(page.contains("collapsed"), "page: {page:?}");
    snap(&mut game, &shot_dir, "whiteout-message");

    // Landing it: party healed to full, back at the entry spawn, flags kept.
    press_a(&mut game);
    assert!(!game.whiteout_active());
    println!("after the whiteout:");
    println!(
        "  map: {:?} tile: {:?}",
        game.current_map_id(),
        game.player_tile()
    );
    for m in game.party_state().expect("party state") {
        println!("  {} hp {} mp {} status {:?}", m.id, m.hp, m.mp, m.status);
    }
    println!(
        "  __played_main_StartTown flag kept: {}",
        game.flag("__played_main_StartTown")
    );
    assert_eq!(game.current_map_id(), Some("StartTown"));
    assert!(game
        .party_state()
        .unwrap()
        .iter()
        .all(|m| m.hp > 0 && m.status.is_none()));
    snap(&mut game, &shot_dir, "whiteout-after");

    println!("\nacceptance OK");
}
