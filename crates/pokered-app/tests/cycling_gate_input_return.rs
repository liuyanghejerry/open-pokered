//! Actual Game input at the two Cycling Road guards, after normal movement.
use dotzuki_app::InputState;
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::GbButton;
use pokered_app::PokemonGame;
use pokered_core::{
    game_state::{GameScreen, Lang},
    overworld::{Direction, OverworldScreen},
    save::SaveData,
};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId, wild_data::GameVersion};
use pokered_renderer::{FrameBuffer, Rgba};

fn input(button: GbButton) -> InputState {
    let mut input = InputState::new();
    input.press(button);
    input
}

fn capture(g: &mut PokemonGame, name: &str) {
    let Ok(dir) = std::env::var("CYCLING_GATE_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    g.draw(&mut fb);
    image::RgbaImage::from_fn(160, 144, |x, y| {
        image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
    })
    .save(format!("{dir}/{name}.png"))
    .unwrap();
}

fn save_trace(name: &str, value: impl serde::Serialize) {
    let Ok(dir) = std::env::var("CYCLING_GATE_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        format!("{dir}/{name}.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}

fn refusal(gate: u8, approach: u16) -> (PokemonGame, Vec<[u16; 3]>) {
    let (map, counter_y) = if gate == 16 {
        (MapId::Route16Gate1F, 7)
    } else {
        (MapId::Route18Gate1F, 3)
    };
    let mut g = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    g.save_data = SaveData::new();
    g.state.screen = GameScreen::Overworld;
    g.state.config.language = Lang::En;
    g.overworld = OverworldScreen::new(map, None, PokemonRedData);
    g.overworld.state.player.x = 5;
    g.overworld.state.player.y = counter_y + approach;
    g.overworld.state.player.facing = Direction::Left;
    g.audio = Some(pokered_audio::output::AudioOutput::new_pcm());
    for _ in 0..2 {
        g.update(&InputState::new());
    }
    for _ in 0..8 {
        g.update(&input(GbButton::Left));
    }
    for _ in 0..181 {
        g.update(&InputState::new());
    }
    assert!(
        g.overworld.pending_dialogue.is_some(),
        "gate {gate} must stop a pedestrian"
    );
    let mut approach_started = false;
    let mut approach_done = false;
    let mut trace = Vec::new();
    for frame in 0..3000 {
        let moving = matches!(
            g.overworld.active_script_effect_label().as_deref(),
            Some("MovePlayer" | "MovePlayerRelative")
        );
        if moving
            && g.overworld.state.walk_counter != 0
            && g.overworld.state.player.facing == Direction::Up
        {
            approach_started = true;
        }
        if approach_started && !approach_done {
            trace.push([
                u16::from(g.overworld.state.walk_counter),
                g.overworld.state.player.x,
                g.overworld.state.player.y,
            ]);
            approach_done =
                g.overworld.state.player.y == counter_y && g.overworld.state.walk_counter == 0;
        }
        if g.overworld.active_script_effect_label().as_deref() == Some("MovePlayerRelative")
            && g.overworld.state.player.facing == Direction::Right
            && g.overworld.state.walk_counter == 7
        {
            save_trace(&format!("gate{gate}-approach{approach}"), &trace);
            return (g, trace);
        }
        g.update(&if frame % 8 == 0 {
            input(GbButton::A)
        } else {
            InputState::new()
        });
    }
    panic!("gate {gate} did not reach its final rightward stride");
}

fn start_case(name: &str, from: usize, to: usize, expected: Option<usize>) {
    let mut results = Vec::new();
    for gate in [16, 18] {
        let (mut g, _) = refusal(gate, 3);
        let mut first_menu = None;
        let mut trace = Vec::new();
        for frame in 0..60 {
            if g.state.screen == GameScreen::StartMenu {
                first_menu.get_or_insert(frame);
            }
            trace.push(serde_json::json!({"frame":frame,"screen":format!("{:?}",g.state.screen),
                "walk":g.overworld.state.walk_counter,"x":g.overworld.state.player.x,"y":g.overworld.state.player.y,
                "effect":g.overworld.active_script_effect_label(),"owner":g.overworld.foreground_observation()}));
            if frame == 59 {
                capture(&mut g, &format!("gate{gate}-{name}"));
            }
            g.update(&if (from..to).contains(&frame) {
                input(GbButton::Start)
            } else {
                InputState::new()
            });
        }
        save_trace(&format!("gate{gate}-{name}"), trace);
        results.push((gate, first_menu));
        if first_menu.is_none() {
            for _ in 0..2 {
                g.update(&input(GbButton::Start));
            }
            for _ in 0..40 {
                g.update(&InputState::new());
            }
            assert_eq!(
                g.state.screen,
                GameScreen::StartMenu,
                "gate {gate}: fresh START remains usable"
            );
        }
    }
    assert_eq!(results, vec![(16, expected), (18, expected)], "{name}");
}

#[test]
fn cycling_guard_completion_accepts_a_single_frame_start() {
    start_case("completion-start", 16, 17, Some(17));
}
#[test]
fn cycling_guard_discards_start_before_the_completion_poll() {
    start_case("early-start", 15, 16, None);
}
#[test]
fn cycling_guard_accepts_start_on_the_next_field_poll() {
    start_case("later-start", 17, 19, Some(19));
}
#[test]
fn cycling_guard_held_start_is_fresh_after_its_only_rightward_poll() {
    start_case("held-start", 0, 30, Some(17));
}
#[test]
fn cycling_guard_does_not_replay_a_pulse_during_the_stride() {
    start_case("walking-start-pulse", 0, 10, None);
}

#[test]
fn cycling_guard_return_preserves_held_directions() {
    let mut results = Vec::new();
    for gate in [16, 18] {
        for (name, button) in [
            ("up", GbButton::Up),
            ("down", GbButton::Down),
            ("left", GbButton::Left),
            ("right", GbButton::Right),
        ] {
            let (mut g, _) = refusal(gate, 3);
            let mut first_step = None;
            let mut trace = Vec::new();
            for frame in 0..50 {
                if frame >= 17 && g.overworld.state.walk_counter != 0 {
                    first_step.get_or_insert(frame);
                }
                trace.push(
                    serde_json::json!({"frame":frame,"walk":g.overworld.state.walk_counter,
                    "x":g.overworld.state.player.x,"y":g.overworld.state.player.y,
                    "facing":format!("{:?}",g.overworld.state.player.facing)}),
                );
                g.update(&if frame < 30 {
                    input(button)
                } else {
                    InputState::new()
                });
            }
            save_trace(&format!("gate{gate}-direction-{name}"), trace);
            results.push((gate, name, first_step));
        }
    }
    assert_eq!(
        results,
        vec![
            (16, "up", None),
            (16, "down", Some(17)),
            (16, "left", Some(17)),
            (16, "right", Some(17)),
            (18, "up", None),
            (18, "down", Some(17)),
            (18, "left", Some(17)),
            (18, "right", Some(17))
        ]
    );
}

