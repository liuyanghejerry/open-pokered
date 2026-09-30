//! Deterministic production switch capture on the PR base and candidate.
use pokered_app::PokemonGame;
use pokered_core::{
    battle::{menu::PartySubMenuState, BattleInput, BattlePhase, BattleScreen},
    data::wild_data::GameVersion,
    game_state::GameScreen,
    pokemon::stats::create_pokemon_with_moves,
};
use pokered_data::{moves::MoveId, species::Species};
use pokered_renderer::{FrameBuffer, Rgba};

#[test]
#[ignore = "writes matched PR screenshots to PR_SCREENSHOTS"]
fn capture_switch_dialogue() {
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let make = |species| {
        create_pokemon_with_moves(
            species,
            30,
            [0xff, 0xff],
            [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = None;
    game.battle = BattleScreen::from_parties(
        true,
        &[make(Species::Charmander), make(Species::Squirtle)],
        &[make(Species::Snorlax)],
        None,
    );
    game.battle
        .battle_state
        .as_mut()
        .unwrap()
        .enemy
        .active_mon_mut()
        .hp = 1;
    game.battle.party_submenu = Some(PartySubMenuState::new());
    game.battle.phase = BattlePhase::PartySubMenu { selected_index: 1 };
    game.state.screen = GameScreen::Battle;
    game.battle.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    let mut fb = FrameBuffer::new(
        dotzuki_engine::render_config::RenderConfig::new(160, 144),
        Rgba::WHITE,
    );
    game.draw(&mut fb);
    fb.save_png(&output.join("recall.png")).unwrap();
    for frame in 0..120 {
        game.battle.update_frame(BattleInput {
            a: frame % 2 == 0,
            ..BattleInput::none()
        });
        if game
            .battle
            .current_message
            .as_ref()
            .is_some_and(|m| m.contains("SQUIRTLE"))
        {
            game.draw(&mut fb);
            fb.save_png(&output.join("send-out.png")).unwrap();
            return;
        }
    }
    panic!("switch never reached send-out narration");
}
