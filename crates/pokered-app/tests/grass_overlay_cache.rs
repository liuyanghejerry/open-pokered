//! Grass priority pixels outside the player sprite must not survive scrolling.
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::{render::session::RenderSession, PokemonGame};
use pokered_core::game_state::GameScreen;
use pokered_data::{species::Species, wild_data::GameVersion};
use pokered_renderer::{
    input::{GbButton, InputState},
    FrameBuffer, Rgba,
};

#[test]
fn route1_grass_walk_cached_pixels_match_full_draw_every_frame() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            #[cfg(feature = "debug-server")]
            let mut g = PokemonGame::new_with_options(
                GameVersion::Red,
                None,
                None,
                None,
                false,
                None,
                false,
                true,
                None,
            );
            #[cfg(not(feature = "debug-server"))]
            let mut g = PokemonGame::new_with_options(
                GameVersion::Red,
                None,
                None,
                None,
                false,
                None,
                false,
                true,
            );
            g.audio = None;
            g.set_seed(42);
            g.state.screen = GameScreen::Overworld;
            g.overworld
                .warp_to_map(pokered_data::maps::MapId::Route1, 12, 22);
            for _ in 0..120 {
                g.update(&InputState::new());
            }
            g.save_data.party.clear();
            g.save_data
                .party
                .add(
                    pokered_core::pokemon::stats::create_pokemon(Species::Mew, 50, [0x99; 2])
                        .unwrap(),
                )
                .unwrap();
            g.overworld.state.repel_steps = 200;
            let mut session = RenderSession::new();
            let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            let mut input = InputState::new();
            let mut held = None;
            for frame in 0..480 {
                input.begin_frame();
                let key = if frame % 24 < 2 {
                    Some(if frame / 24 % 2 == 0 {
                        GbButton::Down
                    } else {
                        GbButton::Up
                    })
                } else {
                    None
                };
                if key != held {
                    if let Some(b) = held {
                        input.release(b);
                    }
                    if let Some(b) = key {
                        input.press(b);
                    }
                    held = key;
                }
                g.update(&input);
                assert_eq!(g.state.screen, GameScreen::Overworld);
                let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
                session.render(&mut g, &mut cached, &mut scroll);
                let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut full);
                assert_eq!(cached.packed(), full.packed(), "grass walk frame {frame}");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
