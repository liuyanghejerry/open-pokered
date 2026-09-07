use dotzuki_app::InputState;
use dotzuki_renderer::input::GbButton;
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::{GameScreen, MainMenuChoice};
use pokered_core::overworld::{Direction, OverworldScreen};
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::save::SaveData;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId, species::Species};

fn game() -> PokemonGame {
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
    game.save_data = SaveData::new();
    game
}

fn tap(game: &mut PokemonGame, button: GbButton) {
    let mut input = InputState::new();
    input.press(button);
    game.update(&input);
    game.update(&InputState::new());
}

fn approach_ball(game: &mut PokemonGame, x: u16) {
    game.overworld.state.player.x = x;
    game.overworld.state.player.y = 4;
    game.overworld.state.player.facing = Direction::Up;
    tap(game, GbButton::A);
    for _ in 0..300 {
        if game.overworld.pending_choice.is_some() {
            return;
        }
        tap(game, GbButton::A);
    }
    panic!(
        "starter choice did not open: {:?}",
        game.overworld.active_script_effect_value()
    );
}

#[test]
fn declining_bulbasaur_then_accepting_charmander_gives_only_charmander() {
    let mut game = game();
    game.state.screen = GameScreen::Overworld;
    game.overworld = OverworldScreen::new(MapId::OaksLab, None, PokemonRedData);
    game.overworld
        .set_flag_live("EVENT_OAK_ASKED_TO_CHOOSE_MON", true);
    approach_ball(&mut game, 8);
    assert!(
        game.save_data.party.is_empty(),
        "preview must not give a starter"
    );
    tap(&mut game, GbButton::Down);
    tap(&mut game, GbButton::A);
    for _ in 0..10 {
        game.update(&InputState::new());
    }
    assert!(
        game.save_data.party.is_empty(),
        "NO must not give a starter"
    );
    approach_ball(&mut game, 6);
    tap(&mut game, GbButton::A);
    for _ in 0..300 {
        if !game.save_data.party.is_empty() {
            break;
        }
        tap(&mut game, GbButton::A);
    }
    assert_eq!(game.save_data.party.count(), 1);
    assert_eq!(
        game.save_data.party.leader().unwrap().species,
        Species::Charmander
    );
}

#[test]
fn new_game_discards_previous_party_before_starter_selection() {
    let mut game = game();
    game.save_data
        .party
        .add(create_pokemon(Species::Bulbasaur, 7, [0x9a, 0x78]).unwrap())
        .unwrap();
    game.save_data.game_data.player_money = 99999;
    game.main_menu.last_choice = Some(MainMenuChoice::NewGame);
    game.state.screen = GameScreen::MainMenu;
    game.handle_transition(GameScreen::OakSpeech);
    game.handle_transition(GameScreen::Overworld);
    assert!(
        game.save_data.party.is_empty(),
        "NEW GAME must not carry the loaded save's party"
    );
    assert_eq!(game.overworld.party_count, 0);
    assert_eq!(
        game.save_data.game_data.player_money,
        SaveData::new().game_data.player_money
    );
}
