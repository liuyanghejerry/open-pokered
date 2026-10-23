//! Simulated joypad walks through real map scripts and the shared Game update.
use dotzuki_app::InputState;
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::GbButton;
use pokered_app::{render::draw_overworld, PokemonGame};
use pokered_core::{
    game_state::{GameScreen, Lang},
    overworld::{Direction, OverworldScreen},
    save::SaveData,
};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId, wild_data::GameVersion};
use pokered_renderer::{
    resource::{AssetRoot, ResourceManager},
    FrameBuffer, Rgba,
};

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

fn input(button: GbButton) -> InputState {
    let mut input = InputState::new();
    input.press(button);
    input
}

fn capture(g: &mut PokemonGame, name: &str) {
    let Ok(dir) = std::env::var("RELATIVE_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut resources = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(&mut g.overworld, &mut resources, &mut fb, Lang::En);
    image::RgbaImage::from_fn(160, 144, |x, y| {
        image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
    })
    .save(format!("{dir}/{name}.png"))
    .unwrap();
}

// Original OverworldLoop: first two-pixel advance, redraw wait, then two
// VBlanks between advances. Completion does not consume the next direction.
const STRIDE: [u8; 17] = [7, 7, 7, 6, 6, 5, 5, 4, 4, 3, 3, 2, 2, 1, 1, 0, 0];

fn record_walk(g: &mut PokemonGame, name: &str, y: u16, dy: i16, steps: usize, hold_right: bool) {
    // The caller stops delivering its dialogue/warp input when the movement
    // command begins; all following movement is driven by the real scene.
    for _ in 0..30 {
        if g.overworld.state.walk_counter == 7 {
            break;
        }
        g.update(&InputState::new());
    }
    assert_eq!(g.overworld.state.walk_counter, 7);
    let mut rows = Vec::new();
    let mut actual = Vec::new();
    let start_x = g.overworld.state.player.x;
    for frame in 0..steps * 17 + 26 {
        let row = (g.overworld.state.walk_counter, g.overworld.state.player.y);
        actual.push(row);
        if frame < steps * 17 {
            assert_eq!(
                g.overworld.state.player.x, start_x,
                "{name}: physical input during simulated movement"
            );
        }
        rows.push(
            serde_json::json!({"frame": frame, "walk": row.0, "y": row.1,
            "x": g.overworld.state.player.x, "effect": g.overworld.active_script_effect_label(),
            "idle": g.overworld.script_engine_idle()}),
        );
        if frame == 8 {
            capture(g, name);
        }
        g.update(&if hold_right && frame < steps * 17 + 12 {
            input(GbButton::Right)
        } else {
            InputState::new()
        });
    }
    if let Ok(dir) = std::env::var("RELATIVE_CAPTURE_DIR") {
        std::fs::write(
            format!("{dir}/{name}.json"),
            serde_json::to_vec_pretty(&rows).unwrap(),
        )
        .unwrap();
    }
    let expected: Vec<_> = (0..steps)
        .flat_map(|step| {
            STRIDE.into_iter().map(move |counter| {
                let completed = step as i16 + i16::from(counter == 0);
                (counter, (y as i16 + dy * completed) as u16)
            })
        })
        .collect();
    assert_eq!(&actual[..expected.len()], expected.as_slice(), "{name}");
    assert!(g.overworld.script_engine_idle(), "{name}");
    assert!(g.overworld.active_script_effect_label().is_none(), "{name}");
    assert_eq!(
        g.overworld.state.player.y,
        (y as i16 + dy * steps as i16) as u16
    );
}

#[test]
fn guard_refusal_uses_the_field_clock_in_both_lanes_and_direct_talk() {
    for (x, held_right) in [(4, false), (5, false), (6, false), (4, true)] {
        let mut g = game(MapId::Route22Gate, x, 3);
        let button = if x == 6 { GbButton::A } else { GbButton::Up };
        for _ in 0..if x == 6 { 1 } else { 8 } {
            g.update(&input(button));
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
        assert_eq!(
            g.overworld.active_script_effect_label().as_deref(),
            Some("MovePlayerRelative")
        );
        record_walk(
            &mut g,
            &format!("guard-{x}{}", if held_right { "-right" } else { "" }),
            if x == 6 { 3 } else { 2 },
            1,
            1,
            held_right,
        );
        assert_eq!(g.overworld.state.player.x, x + u16::from(held_right));
    }
}

#[test]
fn dock_departure_keeps_the_boundary_between_two_simulated_steps() {
    for held_right in [false, true] {
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
        for _ in 0..100 {
            if g.overworld.active_script_effect_label().as_deref() == Some("MovePlayerRelative") {
                break;
            }
            g.update(&InputState::new());
        }
        assert_eq!(
            g.overworld.active_script_effect_label().as_deref(),
            Some("MovePlayerRelative")
        );
        record_walk(
            &mut g,
            if held_right {
                "departure-right"
            } else {
                "departure"
            },
            31,
            -1,
            2,
            held_right,
        );
        assert_eq!(g.overworld.state.player.x, 18 + u16::from(held_right));
        assert!(g.overworld.pending_dialogue.is_none());
    }
}
