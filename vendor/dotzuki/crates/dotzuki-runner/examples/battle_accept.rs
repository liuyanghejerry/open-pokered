//! Battle v2-b acceptance harness: boots a zero-Rust project headless and
//! drives two consecutive battles with a scripted agenda, printing the
//! battle logs, the persistent party state and the inventory as evidence.
//!
//! Usage:
//!
//! ```sh
//! cargo run -p dotzuki-runner --example battle_accept -- <project-dir> [shot-dir]
//! ```
//!
//! The project should trigger two `startBattle` commands in a row (the
//! acceptance patches StartTown's `script.scene` accordingly). Battle 1
//! agenda: **Item** (Potion at full HP — cap + decrement + turn consumed),
//! **Party** (switch to the second member — the enemy hits the NEW member),
//! **Fight**. Battle 2 agenda: **Fight** — it starts from the carried-over
//! party state. Screenshots of the root/party/item menus land in `shot-dir`
//! (default: no screenshots).

use crate::alloc_prelude::*;
use std::path::{Path, PathBuf};

use dotzuki_engine::render::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_runner::headless::save_png;
use dotzuki_runner::{LoadedProject, RunnerGame, RunnerOptions, SCREEN_H, SCREEN_W};

/// One root-menu pick for the scripted agenda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
    Fight,
    Party,
    Item,
}

/// Draw the current frame into a PNG.
fn snap(game: &mut RunnerGame, path: &Path) {
    let mut fb = FrameBuffer::new(
        RenderConfig::new(SCREEN_W as u32, SCREEN_H as u32),
        Rgba::BLACK,
    );
    game.draw(&mut fb);
    save_png(&fb, path).expect("screenshot");
    println!("  [shot] {}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .expect("usage: battle_accept <project-dir> [shot-dir]");
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

    // The agenda per battle (index = battles started so far − 1).
    let agendas: &[&[Pick]] = &[&[Pick::Item, Pick::Party, Pick::Fight], &[Pick::Fight]];
    let mut battles_done = 0usize;
    let mut agenda: Vec<Pick> = Vec::new();
    // Queued button presses; one key every other frame (a held key is not a
    // fresh just-press, so keys alternate with idle frames).
    let mut keys: Vec<GbButton> = Vec::new();
    let mut last_log: Vec<String> = Vec::new();
    let mut snapped: Vec<String> = Vec::new();
    let mut was_in_battle_prev = false;

    let mut input = InputState::new();
    for frame in 0..6000u32 {
        let was_in_battle = was_in_battle_prev;

        // Decide this frame's input.
        let mut mask = 0;
        if frame % 2 == 0 {
            if !keys.is_empty() {
                mask = keys.remove(0).bit_mask();
            } else if let Some(battle) = game.battle() {
                if battle.in_menu() {
                    let items = battle.menu_items();
                    let label = if items.first().is_some_and(|i| i == "Fight") {
                        "root"
                    } else if items.iter().any(|i| i.contains('/')) {
                        "party"
                    } else {
                        "sub"
                    };
                    if let Some(dir) = &shot_dir {
                        let tag = format!("battle{}-{label}", battles_done + 1);
                        if !snapped.contains(&tag) {
                            snapped.push(tag.clone());
                            snap(&mut game, &dir.join(format!("{tag}.png")));
                        }
                    }
                    mask = match label {
                        "root" => {
                            let pick = agenda.first().copied().unwrap_or(Pick::Fight);
                            if !agenda.is_empty() {
                                agenda.remove(0);
                            }
                            match pick {
                                Pick::Fight => GbButton::A.bit_mask(),
                                Pick::Party => {
                                    keys.push(GbButton::A);
                                    GbButton::Down.bit_mask()
                                }
                                Pick::Item => {
                                    keys.push(GbButton::Down);
                                    keys.push(GbButton::A);
                                    GbButton::Down.bit_mask()
                                }
                            }
                        }
                        // Submenus: confirm the entry under the cursor (the
                        // party cursor starts on the first switchable member).
                        _ => GbButton::A.bit_mask(),
                    };
                } else if frame % 4 == 0 {
                    mask = GbButton::A.bit_mask(); // page narration
                }
            } else if frame % 4 == 0 {
                mask = GbButton::A.bit_mask(); // page dialogue
            }
        }
        input.set_from_bitmask(mask);
        game.update(&input);
        input.begin_frame();

        let in_battle = game.battle().is_some();
        // Battle-start edge: print the party it starts with (the carried-over
        // state from battle 2 on) and load its agenda.
        if in_battle && !was_in_battle {
            let n = battles_done + 1;
            let battle = game.battle().unwrap();
            println!("battle {n} starts:");
            for c in battle.party() {
                let status = c.status.as_deref().unwrap_or("-");
                println!(
                    "  {} {}/{} MP {}/{} status {}",
                    c.name, c.hp, c.max_hp, c.mp, c.max_mp, status
                );
            }
            println!("  inventory: {:?}", battle.inventory());
            agenda = agendas[battles_done].to_vec();
        }
        if in_battle {
            last_log = game.battle().unwrap().log().to_vec();
        }
        // Battle-end edge: the runner has harvested the party state.
        if was_in_battle && !in_battle {
            battles_done += 1;
            println!("\nbattle {battles_done} log:");
            for line in &last_log {
                println!("  {line}");
            }
            if let Some(party) = game.party_state() {
                println!("party state after battle {battles_done}:");
                for m in party {
                    println!("  {} hp {} mp {} status {:?}", m.id, m.hp, m.mp, m.status);
                }
            }
            if let Some(inv) = game.inventory() {
                println!("inventory: {inv:?}");
            }
            println!();
            last_log.clear();
            if battles_done == 2 {
                println!("both battles done — stopping");
                break;
            }
        }
        was_in_battle_prev = in_battle;
    }

    if battles_done < 2 {
        eprintln!("harness: only {battles_done} battle(s) completed within the frame cap");
        std::process::exit(1);
    }
}
