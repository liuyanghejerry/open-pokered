//! Deterministic party action-menu capture: cargo run -p pokered-app
//! --example party_menu_capture -- <output-directory>
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::{render::draw_party_screen, PokemonGame};
use pokered_core::{
    data::wild_data::GameVersion,
    game_state::Lang,
    party_screen::{PartyScreenInput, PartyScreenState},
    pokemon::stats::create_pokemon,
};
use pokered_data::species::Species;
use pokered_renderer::{FrameBuffer, Rgba};

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).unwrap();
    let mut game = PokemonGame::new_with_options(
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
    let party = vec![
        create_pokemon(Species::Bulbasaur, 7, [0x9a, 0x78]).unwrap(),
        create_pokemon(Species::Charmander, 5, [0x9a, 0x78]).unwrap(),
    ];
    for (lang, name) in [(Lang::En, "en"), (Lang::Zh, "zh")] {
        let mut state = PartyScreenState::new(party.clone());
        state.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_party_screen(&state, game.resources.as_mut(), 10, &mut fb, lang);
        fb.save_png(&out.join(format!("party-menu-{name}.png")))
            .unwrap();
    }
}
