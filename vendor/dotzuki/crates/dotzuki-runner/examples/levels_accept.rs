//! EXP/levels (v2-c) acceptance harness: boots a zero-Rust project headless
//! and drives two consecutive battles, printing as evidence:
//!
//! 1. battle 1's narration — the EXP award + level-up lines after the win
//!    text, and the heal-the-delta pools in the harvested party state;
//! 2. battle 2 starting from the GROWN stats (level 2, higher max HP);
//! 3. the Start menu's Party view showing `Lv` + the EXP progress line;
//! 4. a save round trip — level/exp resume into a fresh boot.
//!
//! Usage:
//!
//! ```sh
//! cargo run -p dotzuki-runner --example levels_accept -- <project-dir> [shot-dir]
//! ```
//!
//! The project should trigger two `startBattle` commands in a row (the
//! acceptance patches StartTown's `script.scene` accordingly) and carry a
//! `battle.levels` block (the scaffolded jrpg template ships one).

use crate::alloc_prelude::*;
use std::path::{Path, PathBuf};

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

fn press_a(game: &mut RunnerGame) {
    frame(game, GbButton::A.bit_mask());
    idle(game, 1);
}

fn print_party(game: &RunnerGame, when: &str) {
    let Some(party) = game.party_state() else {
        return;
    };
    println!("party state {when}:");
    for m in party {
        println!(
            "  {} level {} exp {} hp {} mp {} status {:?}",
            m.id, m.level, m.exp, m.hp, m.mp, m.status
        );
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .expect("usage: levels_accept <project-dir> [shot-dir]");
    let shot_dir = args.next().map(PathBuf::from);

    let save_file = Path::new(&dir).join(".dotzuki-save.json");
    let project = LoadedProject::load(Path::new(&dir)).expect("load project");
    let mut game = RunnerGame::new(
        project,
        RunnerOptions {
            headless: true,
            fresh: true,
            write_saves: true,
            save_file: Some(save_file.clone()),
            // Deterministic battles: always hit, 89% variance, never crit.
            rng_script: Some(vec![50, 100, 1]),
            ..RunnerOptions::default()
        },
    )
    .expect("boot game");

    // Auto-A drives everything: dialogue pages, battle narration, and the
    // Fight → first-skill picks. Back-to-back `startBattle` commands swap
    // Battle→Battle INSIDE one update, so a new battle is detected by the
    // log resetting, not just by a not-in-battle gap.
    let mut battle_no = 0usize;
    let mut completed = 0usize;
    let mut last_log: Vec<String> = Vec::new();
    let mut was_in_battle = false;
    let mut snapped_battle = false;

    for frame_no in 0..6000u32 {
        if frame_no % 4 == 0 {
            frame(&mut game, GbButton::A.bit_mask());
        } else {
            idle(&mut game, 1);
        }

        let in_battle = game.battle().is_some();
        if in_battle {
            let log_len = game.battle().unwrap().log().len();
            let new_battle = !was_in_battle || log_len < last_log.len();
            if new_battle {
                if was_in_battle {
                    // Seamless Battle→Battle swap: close the previous one
                    // out (the party state is already harvested).
                    completed += 1;
                    println!("\nbattle {completed} log:");
                    for line in &last_log {
                        println!("  {line}");
                    }
                    print_party(&game, &format!("after battle {completed}"));
                    println!();
                    if completed == 2 {
                        break;
                    }
                }
                battle_no += 1;
                let battle = game.battle().unwrap();
                println!("battle {battle_no} starts:");
                for c in battle.party() {
                    println!(
                        "  {} level {} exp {} hp {}/{} mp {}/{} atk {}",
                        c.name, c.level, c.exp, c.hp, c.max_hp, c.mp, c.max_mp, c.attack
                    );
                }
                if battle_no == 2 && !snapped_battle {
                    snapped_battle = true;
                    snap(&mut game, &shot_dir, "battle2-start");
                }
            }
            last_log = game.battle().unwrap().log().to_vec();
        } else if was_in_battle {
            completed += 1;
            println!("\nbattle {completed} log:");
            for line in &last_log {
                println!("  {line}");
            }
            print_party(&game, &format!("after battle {completed}"));
            println!();
            last_log.clear();
            if completed == 2 {
                break;
            }
        }
        was_in_battle = in_battle;
    }
    if completed < 2 {
        eprintln!("harness: only {completed} battle(s) completed within the frame cap");
        std::process::exit(1);
    }

    // Dismiss the post-battle text so the scene finishes (the save is
    // written at that stable point).
    for _ in 0..40 {
        if game.dialogue_text().is_none() {
            break;
        }
        press_a(&mut game);
    }
    assert!(game.flag("SLIMES_BEATEN"), "the scene ran to its end");

    // The Party view: Lv + EXP progress on the member rows.
    frame(&mut game, GbButton::Start.bit_mask());
    idle(&mut game, 1);
    press_a(&mut game); // Party
    println!("party view:");
    for line in game.menu_lines().expect("party view open") {
        println!("  {line}");
    }
    snap(&mut game, &shot_dir, "party-view");
    frame(&mut game, GbButton::B.bit_mask());
    idle(&mut game, 1);
    frame(&mut game, GbButton::B.bit_mask());
    idle(&mut game, 1);

    // Save round trip: a fresh boot resumes level/exp from the save.
    println!("\nsave round trip ({}):", save_file.display());
    let project = LoadedProject::load(Path::new(&dir)).expect("reload project");
    let game = RunnerGame::new(
        project,
        RunnerOptions {
            headless: true,
            save_file: Some(save_file),
            rng_script: Some(vec![50, 100, 1]),
            ..RunnerOptions::default()
        },
    )
    .expect("boot from save");
    print_party(&game, "resumed from the save");
    assert!(
        game.party_state().is_some_and(|p| p[0].level > 1),
        "level must survive the save round trip"
    );
    println!("\nacceptance OK");
}
