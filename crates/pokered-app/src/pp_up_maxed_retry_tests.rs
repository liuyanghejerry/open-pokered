use super::*;
use pokered_core::party_screen::PartyScreenPhase;
use pokered_data::{items::ItemId, moves::MoveId, species::Species};

fn on_game_thread(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("pp-up-menu-regression".to_string())
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn tap(game: &mut PokemonGame, button: GbButton) {
    let mut input = InputState::new();
    input.press(button);
    game.update(&input);
    for _ in 0..32 {
        game.update(&InputState::new());
    }
}

fn at_pp_up_move_menu() -> PokemonGame {
    let mut game = PokemonGame::new_with_options(
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
    game.save_data = SaveData::new();
    let mut mon = pokered_core::pokemon::stats::create_pokemon_with_moves(
        Species::Bulbasaur,
        12,
        [0x9a, 0x78],
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
    )
    .unwrap();
    mon.pp[0] = 1;
    mon.pp_ups[0] = 3;
    game.save_data.party.add(mon).unwrap();
    game.save_data
        .game_data
        .bag
        .add_item(ItemId::PpUp, 2)
        .unwrap();
    game.state.screen = GameScreen::Bag;
    game.bag_screen = BagScreenState::new(vec![(ItemId::PpUp, 2)]);
    // Real bag selection, USE dispatch, party selection, then move selection.
    tap(&mut game, GbButton::A);
    tap(&mut game, GbButton::A);
    assert_eq!(game.state.screen, GameScreen::PartyScreen);
    tap(&mut game, GbButton::A);
    assert_eq!(
        game.party_screen.phase(),
        PartyScreenPhase::ChooseMove { cursor: 0 }
    );
    game
}

fn quantity(game: &PokemonGame) -> u8 {
    game.save_data.game_data.bag.get(0).unwrap().1
}

#[test]
fn capped_pp_up_retry_keeps_the_mon_and_can_apply_to_another_move() {
    on_game_thread(|| {
        for acknowledge in [GbButton::A, GbButton::B] {
            let mut game = at_pp_up_move_menu();
            let original = game.save_data.party.get(0).unwrap().clone();
            tap(&mut game, GbButton::A);
            assert_eq!(game.state.screen, GameScreen::PartyScreen);
            assert_eq!(
                game.party_screen.phase(),
                PartyScreenPhase::MoveChoiceNotice
            );
            assert_eq!(
                game.party_screen.move_choice_notice(),
                Some("TACKLE's PP\nis maxed out.")
            );
            assert_eq!(game.pending_bag_item, Some(ItemId::PpUp));
            assert_eq!(game.save_data.party.get(0).unwrap(), &original);
            assert_eq!(
                quantity(&game),
                2,
                "refusing a capped move does not consume a bottle"
            );

            tap(&mut game, acknowledge);
            assert_eq!(
                game.party_screen.phase(),
                PartyScreenPhase::ChooseMove { cursor: 0 }
            );
            assert_eq!(
                game.party_screen.cursor(),
                0,
                "retry stays on the selected Pokémon"
            );
            tap(&mut game, GbButton::Down);
            tap(&mut game, GbButton::A);
            assert_eq!(game.state.screen, GameScreen::Overworld);
            assert_eq!(game.pending_bag_item, None);
            let mon = game.save_data.party.get(0).unwrap();
            assert_eq!((mon.pp[0], mon.pp_ups[0]), (1, 3));
            assert_eq!((mon.pp[1], mon.pp_ups[1]), (47, 1));
            assert_eq!(
                quantity(&game),
                1,
                "only the successful Growl use consumes a bottle"
            );
        }
    });
}

#[test]
fn capped_pp_up_retry_can_cancel_back_through_party_to_the_bag() {
    on_game_thread(|| {
        let mut game = at_pp_up_move_menu();
        let original = game.save_data.party.get(0).unwrap().clone();
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::B);
        assert_eq!(
            game.party_screen.phase(),
            PartyScreenPhase::ChooseMove { cursor: 0 }
        );
        tap(&mut game, GbButton::B);
        assert_eq!(game.state.screen, GameScreen::PartyScreen);
        assert_eq!(game.party_screen.phase(), PartyScreenPhase::Browsing);
        assert_eq!(game.pending_bag_item, Some(ItemId::PpUp));
        tap(&mut game, GbButton::B);
        assert_eq!(game.state.screen, GameScreen::Bag);
        assert_eq!(game.pending_bag_item, None);
        assert_eq!(quantity(&game), 2);
        assert_eq!(game.save_data.party.get(0).unwrap(), &original);
    });
}

#[test]
fn capped_pp_up_in_a_later_slot_restarts_the_move_cursor_at_the_first_slot() {
    on_game_thread(|| {
        let mut game = at_pp_up_move_menu();
        game.save_data.party.get_mut(0).unwrap().pp_ups[1] = 3;
        let original = game.save_data.party.get(0).unwrap().clone();
        tap(&mut game, GbButton::Down);
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.party_screen.move_choice_notice(),
            Some("GROWL's PP\nis maxed out.")
        );
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.party_screen.phase(),
            PartyScreenPhase::ChooseMove { cursor: 0 }
        );
        assert_eq!(game.save_data.party.get(0).unwrap(), &original);
        assert_eq!(quantity(&game), 2);
    });
}
