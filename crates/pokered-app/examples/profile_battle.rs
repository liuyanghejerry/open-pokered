//! Battle render profiler: boots straight into a wild battle and renders in a
//! loop with no window, so a host sampler (macOS `sample`) attributes the
//! per-frame render cost. Run with:
//!   cargo run --release -p pokered-app --example profile_battle

use dotzuki_engine::render_config::RenderConfig;
use pokered_app::game::PokemonGame;
use pokered_app::render::session::RenderSession;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::species::Species;
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::{FrameBuffer, Rgba};

fn main() {
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        true,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    if let Some(starter) = create_pokemon(Species::Bulbasaur, 5, [0x9A, 0x78]) {
        let _ = game.save_data.party.add(starter);
    }
    game.debug_start_wild_battle(Species::Pidgey, 5);
    println!("battle started");

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut session = RenderSession::new();
    let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};

    let mut state = InputState::new();
    for frame in 0u32..200_000_000 {
        state.begin_frame();
        state.set_from_bitmask(0);
        // Intro frames advance on their own; A advances the fight once the
        // menus are up (FIGHT -> first move at one press per 64 frames).
        if frame > 600 && frame % 64 < 8 {
            state.press(GbButton::A);
        }
        // Keep the profiler inside battles: re-trigger one whenever the
        // previous fight has settled back to the overworld.
        if frame % 1200 == 900 {
            game.debug_start_wild_battle(Species::Pidgey, 5);
        }
        game.update(&state);
        session.render(&mut game, &mut fb, &mut scroll);
        if frame % 20_000 == 0 {
            println!("frame {frame}");
        }
    }
    println!("done");
}
