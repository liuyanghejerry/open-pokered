//! Overworld idle/movement update profiler: boots straight into the overworld
//! (skip_intro) and runs the production update loop with no renderer, so a
//! host sampler (macOS `sample`) attributes the per-frame simulation cost.
//! Run with:
//!   cargo run --release -p pokered-app --example profile_overworld

use pokered_app::game::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_renderer::input::{GbButton, InputState};

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
    // Give the player a party so menus/encounters behave like mid-game.
    let starter = pokered_core::pokemon::stats::create_pokemon(
        pokered_data::species::Species::Bulbasaur,
        5,
        [0x9A, 0x78],
    )
    .unwrap();
    let _ = game.save_data.party.add(starter);

    let mut state = InputState::new();
    let phase = std::env::args().nth(1).unwrap_or_else(|| "idle".into());
    println!("phase={phase}, sampling window: update loop");
    for frame in 0u32..200_000_000 {
        state.begin_frame();
        state.set_from_bitmask(0);
        if phase == "walk" && frame % 96 < 48 {
            state.press(GbButton::Down);
        }
        game.update(&state);
    }
    println!("done");
}
