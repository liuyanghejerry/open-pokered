//! Same deterministic real-turn capture on master and the PR branch.
use dotzuki_app::{GameLoop, InputState};
use pokered_app::PokemonGame;
use pokered_core::{
    battle::{pokered_rules::runtime::StdBattleRng, BattlePhase, BattleScreen},
    data::wild_data::GameVersion,
    game_state::GameScreen,
    pokemon::stats::create_pokemon_with_moves,
};
use pokered_data::{moves::MoveId, species::Species};
use pokered_renderer::{input::GbButton, FrameBuffer, Rgba};
#[test]
#[ignore = "writes matched PR evidence to PR_SCREENSHOTS"]
fn capture_real_turn() {
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = None;
    let make = |species| {
        create_pokemon_with_moves(
            species,
            30,
            [0xff; 2],
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    game.battle = BattleScreen::from_parties(
        true,
        &[make(Species::Charmander)],
        &[make(Species::Bulbasaur)],
        None,
    );
    game.battle.rng = StdBattleRng::from_seed(42);
    let bs = game.battle.battle_state.as_mut().unwrap();
    bs.player.active_mon_mut().speed = 200;
    bs.enemy.active_mon_mut().speed = 10;
    game.battle.phase = BattlePhase::PlayerMenu;
    game.state.screen = GameScreen::Battle;
    let mut fb = FrameBuffer::new(
        dotzuki_engine::render_config::RenderConfig::new(160, 144),
        Rgba::WHITE,
    );
    for frame in 0..160 {
        let mut input = InputState::new();
        if frame % 2 == 0 {
            input.press(GbButton::A);
        }
        game.update(&input);
        game.draw(&mut fb);
        if [10, 30, 50, 90, 130].contains(&frame) {
            image::RgbaImage::from_fn(160, 144, |x, y| {
                image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
            })
            .save(output.join(format!("battle-{frame:03}.png")))
            .unwrap();
        }
    }
}
