//! Real Game input across simulated joypad strides and the return to the field.
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

fn game(map: MapId, x: u16, y: u16) -> PokemonGame {
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
    g.overworld.state.player.x = x;
    g.overworld.state.player.y = y;
    g.overworld.state.player.facing = Direction::Up;
    g.audio = Some(pokered_audio::output::AudioOutput::new_pcm());
    for _ in 0..2 {
        g.update(&InputState::new());
    }
    g
}

fn departure() -> PokemonGame {
    let mut g = game(MapId::VermilionDock, 14, 1);
    g.overworld.last_map = Some(MapId::VermilionCity);
    g.overworld.set_flag_live("EVENT_SS_ANNE_LEFT", true);
    g.overworld
        .set_flag_live("EVENT_WALKED_PAST_GUARD_AFTER_SS_ANNE_LEFT", false);
    for _ in 0..1000 {
        if g.overworld.state.current_map == MapId::VermilionCity {
            break;
        }
        g.update(&input(GbButton::Up));
    }
    assert_eq!(g.overworld.state.current_map, MapId::VermilionCity);
    g
}

fn first_stride(g: &mut PokemonGame) {
    for _ in 0..100 {
        if g.overworld.active_script_effect_label().as_deref() == Some("MovePlayerRelative")
            && g.overworld.state.walk_counter == 7
        {
            return;
        }
        g.update(&InputState::new());
    }
    panic!("scene did not start its simulated walk");
}

fn capture(g: &mut PokemonGame, name: &str) {
    let Ok(dir) = std::env::var("SCRIPT_INPUT_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    // Use the actual Game screen, including DisplayTextIDInit and START menu.
    g.draw(&mut fb);
    image::RgbaImage::from_fn(160, 144, |x, y| {
        image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
    })
    .save(format!("{dir}/{name}.png"))
    .unwrap();
}

fn drive_start(g: &mut PokemonGame, name: &str, from: usize, to: usize, menu: bool) {
    first_stride(g);
    let mut seen_menu = false;
    let mut trace = Vec::new();
    // Frame 0 is the first post-update counter-7 sample. Button changes below
    // apply to the following hardware frame, like the original replay.
    for frame in 0..60 {
        seen_menu |= g.state.screen == GameScreen::StartMenu;
        trace.push(
            serde_json::json!({"frame":frame, "screen":format!("{:?}",g.state.screen),
            "walk":g.overworld.state.walk_counter, "x":g.overworld.state.player.x,
            "y":g.overworld.state.player.y, "effect":g.overworld.active_script_effect_label(),
            "owner":g.overworld.foreground_observation()}),
        );
        if frame == 59 {
            capture(g, name);
        }
        g.update(&if (from..to).contains(&frame) {
            input(GbButton::Start)
        } else {
            InputState::new()
        });
    }
    if let Ok(dir) = std::env::var("SCRIPT_INPUT_CAPTURE_DIR") {
        std::fs::write(
            format!("{dir}/{name}.json"),
            serde_json::to_vec_pretty(&trace).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(
        seen_menu, menu,
        "{name}: START already sampled during a simulated stride is not a new press at return"
    );
    if !menu {
        // The ignored held press must not disable the menu: releasing and
        // pressing START anew after the movement still opens it normally.
        g.update(&input(GbButton::Start));
        for _ in 0..40 {
            g.update(&InputState::new());
        }
        assert_eq!(g.state.screen, GameScreen::StartMenu, "{name}: fresh START");
    }
}

#[test]
fn start_held_across_the_second_simulated_stride_is_not_a_new_menu_press() {
    let mut g = departure();
    drive_start(&mut g, "departure-held-start", 0, 46, false);
    assert_eq!(
        (g.overworld.state.player.x, g.overworld.state.player.y),
        (18, 29)
    );
}

#[test]
fn start_pressed_after_the_last_stride_poll_still_opens_the_menu() {
    let mut g = departure();
    drive_start(&mut g, "departure-late-start", 17, 46, true);
}

#[test]
fn a_start_pulse_between_stride_polls_does_not_open_the_menu() {
    let mut g = departure();
    drive_start(&mut g, "departure-start-pulse", 0, 10, false);
}

#[test]
fn a_single_guard_step_does_not_consume_start_pressed_after_its_poll() {
    let mut g = game(MapId::Route22Gate, 4, 3);
    for _ in 0..8 {
        g.update(&input(GbButton::Up));
    }
    for _ in 0..181 {
        g.update(&InputState::new());
    }
    assert!(g.overworld.pending_dialogue.is_some());
    for frame in 0..3000 {
        if g.overworld.active_script_effect_label().as_deref() == Some("MovePlayerRelative") {
            break;
        }
        g.update(&if frame % 8 == 0 {
            input(GbButton::A)
        } else {
            InputState::new()
        });
    }
    drive_start(&mut g, "guard-held-start", 0, 30, true);
    assert_eq!(
        (g.overworld.state.player.x, g.overworld.state.player.y),
        (4, 3)
    );
}
