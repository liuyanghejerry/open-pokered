//! Battle initialization and completion clear the START menu's shared RAM row.
use pokered_app::PokemonGame;
use pokered_core::{
    battle::BattlePhase,
    game_state::{GameScreen, Lang},
    start_menu::{StartMenuInput, StartMenuState},
};
use pokered_data::{species::Species, wild_data::GameVersion};
use pokered_renderer::input::{GbButton, InputState};

#[test]
fn natural_wild_escape_reopens_start_menu_at_first_item() {
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
            g.state.config.language = Lang::En;
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
            g.overworld
                .set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_GOT_POKEDEX);
            g.start_menu = StartMenuState::new(true, true, false);
            for _ in 0..2 {
                g.start_menu.update_frame(StartMenuInput {
                    down: true,
                    ..StartMenuInput::none()
                });
            }
            g.start_menu.update_frame(StartMenuInput {
                b: true,
                ..StartMenuInput::none()
            });
            assert_eq!(g.start_menu.saved_cursor(), 2);
            let mut entered = false;
            let mut escaped = false;
            let mut input = InputState::new();
            let mut held = None;
            let mut key = None;
            for t in 0..2400 {
                if t % 24 == 0 {
                    key = match g.state.screen {
                        GameScreen::Overworld if !entered => Some(if t / 24 % 2 == 0 {
                            GbButton::Down
                        } else {
                            GbButton::Up
                        }),
                        GameScreen::Battle => {
                            entered = true;
                            assert_eq!(g.start_menu.saved_cursor(), 0);
                            match g.battle.phase {
                                BattlePhase::PlayerMenu => {
                                    Some(if g.battle.battle_menu.row() == 0 {
                                        GbButton::Down
                                    } else if g.battle.battle_menu.col() == 0 {
                                        GbButton::Right
                                    } else {
                                        GbButton::A
                                    })
                                }
                                BattlePhase::Intro { .. } | BattlePhase::ShowingText { .. } => {
                                    Some(GbButton::A)
                                }
                                _ => None,
                            }
                        }
                        GameScreen::Overworld => {
                            escaped = true;
                            Some(GbButton::Start)
                        }
                        _ => None,
                    };
                }
                input.begin_frame();
                let pressed = if t % 24 < 2 { key } else { None };
                if pressed != held {
                    if let Some(b) = held {
                        input.release(b);
                    }
                    if let Some(b) = pressed {
                        input.press(b);
                    }
                    held = pressed;
                }
                g.update(&input);
                if g.state.screen == GameScreen::StartMenu {
                    assert!(entered && escaped);
                    assert_eq!(g.start_menu.cursor(), 0);
                    assert_eq!(g.start_menu.saved_cursor(), 0);
                    return;
                }
            }
            panic!("natural encounter/escape/START sequence did not complete");
        })
        .unwrap()
        .join()
        .unwrap();
}
