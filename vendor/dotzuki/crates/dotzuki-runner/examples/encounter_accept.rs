//! Battle v2-d acceptance harness: boots a scaffolded zero-Rust project
//! headless and drives a trainer battle (Run blocked, money on a win), a
//! wild battle (Run succeeds — the `"run"` outcome), and a shop sell,
//! printing the battle logs, money and inventory as evidence.
//!
//! Usage:
//!
//! ```sh
//! cargo run -p dotzuki-runner --example encounter_accept -- <project-dir> [shot-dir]
//! ```
//!
//! The project's entry scene should run `startBattle("bug-catcher")`, then
//! `startBattle("slime")`, then `openShop(["potion"])` (the acceptance
//! patches the jrpg template's StartTown `script.scene` accordingly). Battle
//! 1 agenda: **Run** (blocked — turn not consumed), **Fight**. Battle 2
//! agenda: **Run** (success). Shop agenda: **Sell** one Potion. Screenshots
//! of the blocked-Run / got-away / sold lines land in `shot-dir` (default:
//! no screenshots).

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
    Run,
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
        .expect("usage: encounter_accept <project-dir> [shot-dir]");
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
    println!("money at boot: {}", game.money());

    // The agenda per battle (index = battles started so far − 1): the
    // trainer battle tries Run first (blocked), the wild battle runs.
    // Battle 1's agenda is pre-armed: the entry scene suspends on
    // `startBattle` during boot, so its first menu frame precedes the
    // battle-start edge below (later battles arm on their edge — text plays
    // between them).
    let agendas: &[&[Pick]] = &[&[Pick::Run, Pick::Fight], &[Pick::Run]];
    let mut battles_done = 0usize;
    let mut agenda: Vec<Pick> = agendas[0].to_vec();
    // Queued button presses; one key every other frame (a held key is not a
    // fresh just-press, so keys alternate with idle frames).
    let mut keys: Vec<GbButton> = Vec::new();
    let mut last_log: Vec<String> = Vec::new();
    let mut was_in_battle_prev = false;
    let mut snapped: Vec<String> = Vec::new();
    // Shop agenda: 0 = root (→ Sell), 1 = sell view (→ sell one), 2 = note
    // (→ dismiss), 3 = back to root (→ B), 4 = exit (→ B), 5 = done.
    let mut shop_step = 0usize;
    let mut shop_done = false;

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
                    let is_root = battle.menu_items().first().is_some_and(|i| i == "Fight");
                    mask = if is_root {
                        let pick = agenda.first().copied().unwrap_or(Pick::Fight);
                        if !agenda.is_empty() {
                            agenda.remove(0);
                        }
                        match pick {
                            Pick::Fight => GbButton::A.bit_mask(),
                            Pick::Run => {
                                // Fight/Party/Item/Run → Down ×3, confirm.
                                keys.push(GbButton::Down);
                                keys.push(GbButton::Down);
                                keys.push(GbButton::A);
                                GbButton::Down.bit_mask()
                            }
                        }
                    } else {
                        GbButton::A.bit_mask() // skill submenu
                    };
                } else if frame % 4 == 0 {
                    mask = GbButton::A.bit_mask(); // page narration
                }
            } else if game.shop_lines().is_some() && battles_done == 2 && !shop_done {
                match shop_step {
                    0 => {
                        println!("shop root: {:?}", game.shop_lines().unwrap());
                        keys.push(GbButton::Down); // → Sell
                        keys.push(GbButton::A);
                    }
                    1 => {
                        println!("sell view: {:?}", game.shop_lines().unwrap());
                        keys.push(GbButton::A); // sell one Potion
                    }
                    2 => {
                        let lines = game.shop_lines().unwrap();
                        println!("sell note: {lines:?}");
                        if let Some(dir) = &shot_dir {
                            snap(&mut game, &dir.join("shop-sold.png"));
                        }
                        keys.push(GbButton::A); // dismiss the note
                    }
                    3 => keys.push(GbButton::B), // back to the root
                    _ => keys.push(GbButton::B), // exit the shop
                }
                shop_step += 1;
            } else if frame % 4 == 0 {
                mask = GbButton::A.bit_mask(); // page dialogue
            }
        }
        input.set_from_bitmask(mask);
        game.update(&input);
        input.begin_frame();

        let in_battle = game.battle().is_some();
        // Battle-start edge: print the matchup and load its agenda.
        if in_battle && !was_in_battle {
            let n = battles_done + 1;
            let battle = game.battle().unwrap();
            println!(
                "battle {n} starts: {} vs {} (queued {}, trainer {}, reward {})",
                battle.player().name,
                battle.enemy().name,
                battle.enemies_remaining(),
                battle.is_trainer(),
                battle.trainer_money(),
            );
            if battles_done > 0 {
                agenda = agendas[battles_done].to_vec();
            }
        }
        if in_battle {
            // Screenshot the two v2-d narration lines on their first show.
            // Screenshot the two v2-d narration lines on their first show.
            let line = game.battle().unwrap().current_line().map(str::to_string);
            if let (Some(dir), Some(line)) = (&shot_dir, line) {
                let tag = match line.as_str() {
                    l if l.starts_with("Can't escape") => Some("run-blocked"),
                    l if l.starts_with("Got away") => Some("run-safe"),
                    l if l.starts_with("Got ") && l.ends_with("for winning!") => {
                        Some("trainer-money")
                    }
                    _ => None,
                };
                if let Some(tag) = tag {
                    if !snapped.contains(&tag.to_string()) {
                        snapped.push(tag.to_string());
                        snap(&mut game, &dir.join(format!("{tag}.png")));
                    }
                }
            }
            last_log = game.battle().unwrap().log().to_vec();
        }
        // Battle-end edge: the runner has harvested the state.
        if was_in_battle && !in_battle {
            battles_done += 1;
            println!("\nbattle {battles_done} log:");
            for line in &last_log {
                println!("  {line}");
            }
            if battles_done == 1 {
                assert!(
                    last_log
                        .iter()
                        .any(|l| l == "Can't escape from a trainer battle!"),
                    "the trainer Run attempt must be blocked (and the turn not consumed)"
                );
            }
            println!("money after battle {battles_done}: {}", game.money());
            if let Some(party) = game.party_state() {
                for m in party {
                    println!(
                        "  {} hp {} mp {} level {} exp {}",
                        m.id, m.hp, m.mp, m.level, m.exp
                    );
                }
            }
            println!();
            last_log.clear();
        }
        // Shop-exit edge.
        if shop_step >= 5 && game.shop_lines().is_none() && !shop_done {
            shop_done = true;
            println!(
                "shop closed: money {}, inventory {:?}",
                game.money(),
                game.inventory()
            );
        }
        // Everything done: both battles + the shop, scene finished.
        if battles_done == 2 && shop_done && game.battle().is_none() {
            println!("acceptance complete");
            break;
        }
        was_in_battle_prev = in_battle;
    }

    // Verify the v2-d contract points and exit non-zero on any miss.
    let money = game.money();
    let potions = game.inventory().and_then(|inv| inv.get("potion").copied());
    println!("final: money {money}, potions {potions:?}");
    assert_eq!(battles_done, 2, "both battles must complete");
    assert!(shop_done, "the shop sell must complete");
    assert_eq!(money, 142, "100 start + 32 trainer + 10 sell (20/2)");
    assert_eq!(potions, Some(2), "3 starting − 1 sold");
}
