//! The real FIGHT caller survives MESSAGE_BOX, including full/retained redraw.
use super::*;
use pokered_core::battle::{BattleInput, BattleTextBackdrop};
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::species::Species;

fn game() -> crate::PokemonGame {
    let mut game = crate::PokemonGame::new(pokered_core::data::wild_data::GameVersion::Red);
    game.audio = None;
    let mon = |species, moves| create_pokemon_with_moves(species, 50, [0x9a, 0x78], moves).unwrap();
    game.battle = BattleScreen::from_parties(
        true,
        &[mon(
            Species::Snorlax,
            [
                MoveId::Headbutt,
                MoveId::Amnesia,
                MoveId::Rest,
                MoveId::BodySlam,
            ],
        )],
        &[mon(
            Species::Magikarp,
            [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
        )],
        None,
    );
    game.battle.phase = BattlePhase::PlayerMenu;
    game.state.screen = pokered_core::game_state::GameScreen::Battle;
    game.state.config.language = Lang::En;
    assert!(game.resources.is_some());
    game
}

#[test]
fn fight_refusal_retains_the_selected_upper_pane_and_overlays_only_the_dialog_caller() {
    for disabled in [false, true] {
        let mut game = game();
        if disabled {
            game.battle
                .battle_state
                .as_mut()
                .unwrap()
                .player
                .disabled_move = 1;
        } else {
            game.battle
                .battle_state
                .as_mut()
                .unwrap()
                .player
                .active_mon_mut()
                .pp[0] = 0;
        }
        let a = BattleInput {
            a: true,
            ..BattleInput::none()
        };
        game.battle.update_frame(a);
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
        let mut menu = FrameBuffer::new(config, Rgba::WHITE);
        game.draw(&mut menu);
        let canonical = serde_json::to_value(&game.battle.battle_state).unwrap();
        game.battle.update_frame(a);
        assert!(matches!(
            game.battle.phase,
            BattlePhase::ShowingText {
                backdrop: BattleTextBackdrop::FightMoveNotice { .. },
                ..
            }
        ));
        for ready in [false, true] {
            if ready {
                for _ in 0..10 {
                    game.battle.update_frame(BattleInput::none());
                }
            }
            let mut actual = FrameBuffer::new(config, Rgba::WHITE);
            game.draw(&mut actual);
            let mut expected = menu.clone();
            let mut painter = FrameBufferPainter::new(&mut expected).with_lang(Lang::En);
            menus::battle_text::draw(
                game.battle.current_message.as_deref().unwrap(),
                ready,
                &BATTLE_TEXT_DEFAULT_LAYOUT,
                &mut Ui::new(&mut painter),
                Lang::En,
            );
            assert_eq!(
                actual.packed(),
                expected.packed(),
                "retained pane, disabled={disabled}, ready={ready}"
            );
            assert_eq!(actual.display_palette(), expected.display_palette());
            assert_ne!(
                actual.packed(),
                menu.packed(),
                "a real visible refusal must cover the move rows"
            );
            for y in 0..96 {
                for x in 0..160 {
                    assert_eq!(
                        actual.get_pixel(x, y),
                        menu.get_pixel(x, y),
                        "upper caller {x},{y}"
                    );
                }
            }
            assert_eq!(
                serde_json::to_value(&game.battle.battle_state).unwrap(),
                canonical,
                "draw cannot consume a turn"
            );
        }
        // Context, not the English message string, decides which caller remains.
        if let BattlePhase::ShowingText { backdrop, .. } = &mut game.battle.phase {
            *backdrop = BattleTextBackdrop::Battle;
        }
        let mut ordinary = FrameBuffer::new(config, Rgba::WHITE);
        game.draw(&mut ordinary);
        let mut upper_diff = false;
        for y in 64..96 {
            for x in 0..88 {
                upper_diff |= ordinary.get_pixel(x, y) != menu.get_pixel(x, y);
            }
        }
        assert!(
            upper_diff,
            "generic battle text must not accidentally pass a retained-pane assertion"
        );
    }
}
