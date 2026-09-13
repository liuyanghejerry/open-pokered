//! Deterministic STATS captures: cargo run --release -p pokered-app --example capture_stats -- <directory>

use pokered_app::{
    tools::{apply_lang, capture_screen},
    PokemonGame,
};
use pokered_core::{
    data::wild_data::GameVersion,
    game_state::{GameScreen, Lang},
    pokemon::stats::create_pokemon,
    stats_screen::{StatsPage, StatsScreenState},
};
use pokered_data::species::Species;

fn main() {
    let dir = std::env::args().nth(1).expect("output directory");
    std::fs::create_dir_all(&dir).unwrap();
    for lang in [Lang::Zh, Lang::En] {
        let mut game = PokemonGame::new(GameVersion::Red);
        apply_lang(&mut game, lang);
        let mut mon = create_pokemon(Species::Venusaur, 100, [0xFF, 0xFF]).unwrap();
        mon.ot_id = 65535;
        mon.ot_name = pokered_core::battle::state::encode_name("ABCDEFG");
        for page in [StatsPage::Stats, StatsPage::Moves] {
            game.stats_screen = Some(StatsScreenState {
                pokemon: mon.clone(),
                page,
            });
            let fb = capture_screen(&mut game, GameScreen::PokemonStatsScreen(0), 10);
            fb.save_png(std::path::Path::new(&format!(
                "{dir}/{lang:?}-{page:?}.png"
            )))
            .unwrap();
        }
    }
}
