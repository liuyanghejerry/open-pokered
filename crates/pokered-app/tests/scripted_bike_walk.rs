//! Actual Game bicycle and walking badge refusals across the first redraw.
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

fn capture(g: &mut PokemonGame, name: &str) {
    let Ok(dir) = std::env::var("SCRIPT_INPUT_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    // Capture the actual Game frame at the first nonzero scripted walk counter.
    g.draw(&mut fb);
    image::RgbaImage::from_fn(160, 144, |x, y| {
        image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
    })
    .save(format!("{dir}/{name}.png"))
    .unwrap();
}

fn refusal(bike: bool) -> PokemonGame {
    let mut g = game(MapId::Route23, 7, 137);
    if bike {
        g.overworld.state.player.transport = pokered_core::overworld::TransportMode::Biking;
    }
    for _ in 0..10 {
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
    for _ in 0..100 {
        if g.overworld.active_script_effect_label().as_deref() == Some("MovePlayerRelative")
            && g.overworld.state.walk_counter > 0
        {
            return g;
        }
        g.update(&InputState::new());
    }
    panic!("badge refusal did not start its simulated DOWN");
}

fn compare_stride(bike: bool, expected: &[u8]) {
    let mut g = refusal(bike);
    let name = if bike {
        "bike-first-redraw"
    } else {
        "foot-control"
    };
    capture(&mut g, name);
    let mut records = Vec::new();
    for frame in 0..expected.len() {
        records.push(
            serde_json::json!({"frame": frame, "walk": g.overworld.state.walk_counter,
            "x": g.overworld.state.player.x, "y": g.overworld.state.player.y,
            "transport": format!("{:?}",g.overworld.state.player.transport)}),
        );
        g.update(&InputState::new());
    }
    if let Ok(dir) = std::env::var("SCRIPT_INPUT_CAPTURE_DIR") {
        std::fs::write(
            format!("{dir}/{name}.json"),
            serde_json::to_vec_pretty(&records).unwrap(),
        )
        .unwrap();
    }
    for (row, &counter) in records.iter().zip(expected) {
        assert_eq!(
            row["walk"].as_u64(),
            Some(counter as u64),
            "{name} frame {}",
            row["frame"]
        );
        assert_eq!(row["x"].as_u64(), Some(7));
        assert_eq!(
            row["y"].as_u64(),
            Some(if counter == 0 { 137 } else { 136 })
        );
    }
    assert_eq!(
        g.overworld.state.player.transport,
        if bike {
            pokered_core::overworld::TransportMode::Biking
        } else {
            pokered_core::overworld::TransportMode::Walking
        }
    );
}

#[test]
fn bicycle_scripted_stride_publishes_the_first_two_pixels_before_speedup() {
    // Original physical Route23 badge refusal after using Bicycle from ITEM.
    compare_stride(true, &[7, 6, 6, 4, 4, 2, 2, 0, 0]);
}

#[test]
fn walking_scripted_stride_keeps_its_existing_redraw_wait() {
    compare_stride(false, &[7, 7, 7, 6, 6, 5, 5, 4, 4, 3, 3, 2, 2, 1, 1, 0, 0]);
}
