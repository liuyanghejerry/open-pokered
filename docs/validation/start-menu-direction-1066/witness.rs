//! External, ordinary-input witness; never edits PokemonGame runtime state.
//! Link this exact file separately to actual M5 and the candidate App libraries.
//! Usage: witness FIXTURE_SAV BOOT_PREFIX_JSON NEW_OUTPUT_DIR before|after up-a|up-b
//! Requires App built with debug-server (constructor's final None argument).

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use dotzuki_engine::render_config::RenderConfig;
use pokered_app::PokemonGame;
use pokered_core::data::maps::MapId;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::{Direction, MovementState};
use pokered_core::start_menu::StartMenuItem;
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::{FrameBuffer, Rgba};
use serde_json::{json, Value};

fn observation(game: &PokemonGame) -> Value {
    let audio = game.audio.as_ref().map(|audio| {
        let id = audio.manager.lock().unwrap().sequencer.current_sfx_id;
        json!({"sfx_id": id, "sfx_playing": audio.is_sfx_playing()})
    });
    json!({
        "frame": game.frame_count,
        "screen": format!("{:?}", game.state.screen),
        "map": format!("{:?}", game.overworld.state.current_map),
        "position": [game.overworld.state.player.x, game.overworld.state.player.y],
        "facing": format!("{:?}", game.overworld.state.player.facing),
        "movement": format!("{:?}", game.overworld.state.player.movement_state),
        "menu_cursor": game.start_menu.cursor(),
        "menu_item": format!("{:?}", game.start_menu.current_item()),
        "menu_saved_cursor": game.start_menu.saved_cursor(),
        "menu_items": format!("{:?}", game.start_menu.items()),
        "money": game.save_data.game_data.player_money,
        "party": &game.save_data.party,
        "bag": &game.save_data.game_data.bag,
        "flags": game.overworld.script_flags(),
        "script_idle": game.overworld.script_engine_idle(), "audio": audio,
    })
}

fn button(name: &str) -> Result<GbButton, String> {
    match name.to_ascii_uppercase().as_str() {
        "A" => Ok(GbButton::A),
        "B" => Ok(GbButton::B),
        "START" => Ok(GbButton::Start),
        "SELECT" => Ok(GbButton::Select),
        "UP" => Ok(GbButton::Up),
        "DOWN" => Ok(GbButton::Down),
        "LEFT" => Ok(GbButton::Left),
        "RIGHT" => Ok(GbButton::Right),
        _ => Err(format!("unknown physical button {name:?}")),
    }
}

fn line(writer: &mut BufWriter<File>, value: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value).map_err(|e| e.to_string())?;
    writer.write_all(b"\n").map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())
}

fn check(condition: bool, frame: usize, description: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(format!("frame {frame}: {description}"))
    }
}

fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 5 {
        return Err("expected FIXTURE BOOT_PREFIX NEW_OUTPUT before|after up-a|up-b".into());
    }
    let fixture = PathBuf::from(&args[0]);
    let prefix_file = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let candidate = match args[3].as_str() {
        "before" => false,
        "after" => true,
        _ => return Err("expected before|after".into()),
    };
    let control = match args[4].as_str() {
        "up-a" => "A",
        "up-b" => "B",
        _ => return Err("expected up-a|up-b".into()),
    };
    if output.exists() {
        return Err("output must not exist; no overwriting or resuming a prior Game".into());
    }
    let bytes = fs::read(&fixture).map_err(|e| e.to_string())?;
    if bytes.len() != 32768 {
        return Err(format!(
            "physical SRAM must be 32768 bytes, got {}",
            bytes.len()
        ));
    }
    let prefix: Vec<Option<String>> =
        serde_json::from_slice(&fs::read(&prefix_file).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if prefix.len() != 1428 {
        return Err(format!(
            "frozen ordinary boot prefix must be 1428 frames, got {}",
            prefix.len()
        ));
    }
    let mut timeline: Vec<Vec<String>> = prefix
        .into_iter()
        .map(|b| b.into_iter().collect())
        .collect();
    timeline.extend(vec![vec![]; 32]);
    timeline.extend(vec![vec!["START".into()]; 2]);
    timeline.extend(vec![vec![]; 30]);
    // START is already established: one frame is an actual simultaneous
    // poll, followed by release. Do not leak UP into the legacy field.
    timeline.push(vec!["UP".into(), control.into()]);
    timeline.extend(vec![vec![]; 31]);
    // The same fixed B closes the candidate, and is harmless on the legacy
    // field. No fresh A is sent to the adjacent Forest trainer.
    timeline.extend(vec![vec!["B".into()]; 2]);
    timeline.extend(vec![vec![]; 30]);
    if timeline.len() != 1556 {
        return Err("unexpected fixed timeline length".into());
    }
    // Validate every input before starting a Game. No expected-state branch
    // participates in this immutable physical vector.
    for keys in &timeline {
        for key in keys {
            button(key)?;
        }
    }
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let save = output.join("ordinary.sav");
    fs::write(&save, &bytes).map_err(|e| e.to_string())?;
    fs::write(
        output.join("fixed-inputs.json"),
        serde_json::to_vec_pretty(&timeline).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::write(output.join("result.json"), b"{\"complete\":false}\n").map_err(|e| e.to_string())?;
    let mut states =
        BufWriter::new(File::create(output.join("states.jsonl")).map_err(|e| e.to_string())?);
    let mut inputs =
        BufWriter::new(File::create(output.join("inputs.jsonl")).map_err(|e| e.to_string())?);
    let mut captures =
        BufWriter::new(File::create(output.join("captures.jsonl")).map_err(|e| e.to_string())?);
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        Some(save.clone()),
        None,  // no snapshot
        None,  // embedded map scripts
        false, // cold boot, no skip-intro
        None,  // no warp
        false, // no watch
        true,  // actual no-device PCM sequencer
        None,  // no debug queue/handle
    );
    let mut input = InputState::new();
    let mut boot_anchor = None;
    line(&mut states, &observation(&game))?;
    check(
        game.frame_count == 0,
        0,
        "cold constructor must start at frame zero",
    )?;
    for (index, keys) in timeline.iter().enumerate() {
        let frame = index + 1;
        input.begin_frame();
        let next = keys
            .iter()
            .map(|key| button(key))
            .collect::<Result<Vec<_>, _>>()?;
        for physical in [
            GbButton::A,
            GbButton::B,
            GbButton::Start,
            GbButton::Select,
            GbButton::Up,
            GbButton::Down,
            GbButton::Left,
            GbButton::Right,
        ] {
            if next.contains(&physical) {
                if !input.is_held(physical) {
                    input.press(physical);
                }
            } else if input.is_held(physical) {
                input.release(physical);
            }
        }
        line(
            &mut inputs,
            &json!({
                "frame": frame, "buttons": keys,
                "raw_current": input.raw_current(), "raw_previous": input.raw_previous(),
                "fresh_up": input.is_just_pressed(GbButton::Up),
                "fresh_a": input.is_just_pressed(GbButton::A),
                "fresh_b": input.is_just_pressed(GbButton::B),
            }),
        )?;
        game.update(&input);
        let state = observation(&game);
        line(&mut states, &state)?;
        check(
            game.frame_count == frame as u64,
            frame,
            "exactly one real update per physical frame",
        )?;
        if frame == 1428 {
            check(
                game.state.screen == GameScreen::Overworld,
                frame,
                "normal Continue must reach field",
            )?;
            check(
                game.overworld.state.current_map == MapId::ViridianForest,
                frame,
                "fixture map",
            )?;
            check(
                (game.overworld.state.player.x, game.overworld.state.player.y) == (2, 19),
                frame,
                "fixture position",
            )?;
            check(
                game.overworld.state.player.facing == Direction::Down,
                frame,
                "original Continue faces Down",
            )?;
            boot_anchor = Some(state.clone());
        }
        if let Some(anchor) = &boot_anchor {
            for field in ["money", "party", "bag", "flags", "map", "position"] {
                check(
                    state[field] == anchor[field],
                    frame,
                    &format!("menu input must not mutate {field}"),
                )?;
            }
        }
        if frame == 1460 {
            check(
                game.state.screen == GameScreen::Overworld,
                frame,
                "neutral field before START",
            )?;
            check(
                game.overworld.state.player.movement_state == MovementState::Idle,
                frame,
                "field movement idle",
            )?;
            check(
                game.overworld.script_engine_idle(),
                frame,
                "field script idle before START",
            )?;
        }
        if frame == 1492 {
            check(
                game.state.screen == GameScreen::StartMenu,
                frame,
                "normal START opened menu",
            )?;
            check(
                game.start_menu.cursor() == 0 && game.start_menu.saved_cursor() == 0,
                frame,
                "initial and saved row zero",
            )?;
        }
        if matches!(frame, 1493 | 1494 | 1524) {
            check(
                game.state.screen
                    == if candidate {
                        GameScreen::StartMenu
                    } else {
                        GameScreen::Overworld
                    },
                frame,
                "combination follows the declared legacy/candidate branch",
            )?;
            check(
                if candidate || control == "A" {
                    game.start_menu.current_item() == StartMenuItem::Exit
                } else {
                    game.start_menu.cursor() == 0
                },
                frame,
                "candidate wraps; legacy UP+B closes before moving",
            )?;
            check(
                game.start_menu.saved_cursor()
                    == if candidate || control == "B" {
                        0
                    } else {
                        game.start_menu.item_count() - 1
                    },
                frame,
                "direction-only path must not write the saved row",
            )?;
            if frame == 1494 {
                check(
                    input.raw_current() == 0,
                    frame,
                    "combined buttons are explicitly released",
                )?;
            }
        }
        if matches!(frame, 1525 | 1556) {
            check(
                game.state.screen == GameScreen::Overworld,
                frame,
                "fresh button after release closes normally",
            )?;
            check(
                game.start_menu.saved_cursor()
                    == if candidate || control == "A" {
                        game.start_menu.item_count() - 1
                    } else {
                        0
                    },
                frame,
                "fresh B saves candidate EXIT; legacy remembered row is preserved",
            )?;
        }
        if matches!(frame, 1428 | 1460 | 1492 | 1493 | 1494 | 1524 | 1525 | 1556) {
            let before_draw = observation(&game);
            let mut framebuffer = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            game.draw(&mut framebuffer);
            let name = format!("frame-{frame:04}.png");
            framebuffer
                .save_png(&output.join(&name))
                .map_err(|e| e.to_string())?;
            check(
                observation(&game) == before_draw,
                frame,
                "actual drawing must not advance the recorded public state",
            )?;
            line(
                &mut captures,
                &json!({"frame": frame, "path": name, "width": 160, "height": 144, "state": before_draw}),
            )?;
        }
    }
    check(
        fs::read(&save).map_err(|e| e.to_string())? == bytes,
        1556,
        "physical SRAM must remain unchanged",
    )?;
    fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "complete": true, "all_pass": true, "frames": 1556, "states": 1557,
            "case": args[4], "expected_behavior": args[3],
            "fixture_source": fixture, "boot_prefix_source": prefix_file,
            "constructor": "ordinary physical save / cold boot / no_audio PCM",
            "state_setters": 0, "seed_changes": 0, "debug_RPC": 0,
            "hidden_RNG_parity_claim": false,
        }))
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("START menu ordinary-input witness failed: {error}");
        std::process::exit(1);
    }
}
