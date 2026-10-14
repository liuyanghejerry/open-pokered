//! Ordinary bag/party inputs for ItemUseTMHM's LearnMove continuation.
use super::rare_candy::RareCandyPhase as LearnPhase;
use super::*;
use pokered_core::{bag_screen::BagPhase, party_screen::PartyScreenPhase};
use pokered_data::{items::ItemId, moves::MoveId, species::Species};

fn on_game_thread(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
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
fn quantity(game: &PokemonGame) -> u8 {
    game.save_data
        .game_data
        .bag
        .get(0)
        .map_or(0, |(_, qty)| qty)
}
fn fixture(item: ItemId, full: bool) -> PokemonGame {
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
    let mut moves = [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None];
    if full {
        moves[2] = MoveId::LeechSeed;
        moves[3] = if item == ItemId::Hm01 {
            MoveId::VineWhip
        } else {
            MoveId::Cut
        };
    }
    // Already evolution-eligible level16: machines must not level/evolve it.
    let mut mon = pokered_core::pokemon::stats::create_pokemon_with_moves(
        Species::Bulbasaur,
        16,
        [0x9a, 0x78],
        moves,
    )
    .unwrap();
    mon.pp = if full { [7, 4, 3, 1] } else { [7, 4, 0, 0] };
    mon.pp_ups = if full { [1, 2, 3, 1] } else { [1, 2, 0, 0] };
    game.save_data.party.add(mon).unwrap();
    game.save_data
        .party
        .add(
            pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 12, [0x9a, 0x78])
                .unwrap(),
        )
        .unwrap();
    game.save_data.game_data.bag.add_item(item, 2).unwrap();
    game.state.screen = GameScreen::Bag;
    game.bag_screen = BagScreenState::new(vec![(item, 2)]);
    game
}
fn use_machine(game: &mut PokemonGame, item: ItemId) {
    tap(game, GbButton::A); // USE/TOSS
    tap(game, GbButton::A); // USE
    assert_eq!(game.bag_screen.phase(), BagPhase::MachineBoot { item });
    tap(game, GbButton::A);
    assert_eq!(
        game.bag_screen.phase(),
        BagPhase::MachineTeach { item, cursor: 0 }
    );
    tap(game, GbButton::A);
    assert_eq!(game.state.screen, GameScreen::PartyScreen);
    assert_eq!(game.party_screen.phase(), PartyScreenPhase::Browsing);
    tap(game, GbButton::A);
    assert_eq!(game.rare_candy_phase(), None, "a machine is not a candy");
    assert_eq!(quantity(game), 2);
    assert!(game.evolution_anim.is_none());
}
fn full_confirm(game: &mut PokemonGame) {
    assert_eq!(
        game.machine_learning_phase(),
        Some(LearnPhase::TryingToLearn(0))
    );
    tap(game, GbButton::A);
    assert_eq!(
        game.machine_learning_phase(),
        Some(LearnPhase::TryingToLearn(1))
    );
    tap(game, GbButton::A);
    assert_eq!(
        game.machine_learning_phase(),
        Some(LearnPhase::ReplaceConfirm { yes: true })
    );
}
fn ack_learned(game: &mut PokemonGame) {
    for _ in 0..600 {
        if !game
            .audio
            .as_ref()
            .is_some_and(|audio| audio.is_sfx_playing())
        {
            tap(game, GbButton::A);
            return;
        }
        game.update(&InputState::new());
    }
    panic!("learning sound did not finish");
}
fn numeric_state(game: &PokemonGame) -> (u8, u16, u16, u16, u16, u16, u16, u32, [u16; 5]) {
    let m = game.save_data.party.get(0).unwrap();
    (
        m.level,
        m.hp,
        m.max_hp,
        m.attack,
        m.defense,
        m.speed,
        m.special,
        m.total_exp,
        m.stat_exp,
    )
}

#[test]
fn tmhm_full_replacement_waits_for_learned_ack_and_preserves_pp_and_stats() {
    on_game_thread(|| {
        for item in [ItemId::Tm21, ItemId::Hm01] {
            let mut game = fixture(item, true);
            let numbers = numeric_state(&game);
            let partner = *game.save_data.party.get(1).unwrap();
            use_machine(&mut game, item);
            full_confirm(&mut game);
            tap(&mut game, GbButton::A);
            tap(&mut game, GbButton::Down);
            tap(&mut game, GbButton::A);
            assert_eq!(
                game.machine_learning_phase(),
                Some(LearnPhase::Forgot { slot: 1 })
            );
            assert_eq!(game.save_data.party.get(0).unwrap().moves[1], MoveId::Growl);
            tap(&mut game, GbButton::A);
            let new_move = if item == ItemId::Hm01 {
                MoveId::Cut
            } else {
                MoveId::MegaDrain
            };
            assert_eq!(
                game.machine_learning_phase(),
                Some(LearnPhase::Learned(new_move))
            );
            let m = game.save_data.party.get(0).unwrap();
            assert_eq!(m.moves[1], new_move);
            assert_eq!(m.pp, [7, if item == ItemId::Hm01 { 30 } else { 10 }, 3, 1]);
            assert_eq!(m.pp_ups, [1, 0, 3, 1]);
            assert_eq!(quantity(&game), 2, "Learned text still owns the item");
            ack_learned(&mut game);
            assert_eq!(quantity(&game), if item == ItemId::Hm01 { 2 } else { 1 });
            assert_eq!(game.state.screen, GameScreen::Bag);
            assert_eq!(game.machine_learning_phase(), None);
            assert_eq!(numeric_state(&game), numbers);
            assert!(game.evolution_anim.is_none());
            assert_eq!(*game.save_data.party.get(1).unwrap(), partner);
            tap(&mut game, GbButton::A);
            assert_eq!(
                quantity(&game),
                if item == ItemId::Hm01 { 2 } else { 1 },
                "no duplicate finalization"
            );
        }
    });
}

#[test]
fn tmhm_free_slot_learned_notice_also_defers_consumption_and_has_no_stats_box() {
    on_game_thread(|| {
        for item in [ItemId::Tm21, ItemId::Hm01] {
            let mut game = fixture(item, false);
            let numbers = numeric_state(&game);
            use_machine(&mut game, item);
            let learned = if item == ItemId::Hm01 {
                MoveId::Cut
            } else {
                MoveId::MegaDrain
            };
            assert_eq!(
                game.machine_learning_phase(),
                Some(LearnPhase::Learned(learned))
            );
            assert_eq!(game.save_data.party.get(0).unwrap().pp_ups, [1, 2, 0, 0]);
            assert_eq!(quantity(&game), 2);
            let mut shown = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            game.draw(&mut shown);
            let flow = game.rare_candy_flow.take();
            let mut normal_party = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            game.draw(&mut normal_party);
            game.rare_candy_flow = flow;
            let mut ink = 0;
            for y in 0..144 {
                for x in 0..160 {
                    assert_eq!(
                        shown.get_pixel(x, y),
                        normal_party.get_pixel(x, y),
                        "machines use the party notice without Candy's stats box"
                    );
                    ink += usize::from(shown.get_pixel(x, y) != Some(Rgba::WHITE));
                }
            }
            assert!(ink > 100);
            ack_learned(&mut game);
            assert_eq!(quantity(&game), if item == ItemId::Hm01 { 2 } else { 1 });
            assert_eq!(numeric_state(&game), numbers);
            assert!(game.evolution_anim.is_none());
        }
    });
}

#[test]
fn tmhm_decline_back_and_list_cancel_all_ask_abandon_and_consume_nothing() {
    on_game_thread(|| {
        for item in [ItemId::Tm21, ItemId::Hm01] {
            for cancel in 0..4 {
                let mut game = fixture(item, true);
                let before = game.save_data.party.to_vec();
                use_machine(&mut game, item);
                full_confirm(&mut game);
                match cancel {
                    0 => {
                        tap(&mut game, GbButton::Down);
                        tap(&mut game, GbButton::A);
                    }
                    1 => tap(&mut game, GbButton::B),
                    2 => {
                        tap(&mut game, GbButton::A);
                        tap(&mut game, GbButton::B);
                    }
                    _ => {
                        tap(&mut game, GbButton::A);
                        for _ in 0..4 {
                            tap(&mut game, GbButton::Down);
                        }
                        tap(&mut game, GbButton::A);
                    }
                }
                assert_eq!(
                    game.machine_learning_phase(),
                    Some(LearnPhase::AbandonConfirm { yes: true })
                );
                assert_eq!(quantity(&game), 2);
                tap(&mut game, GbButton::A);
                assert_eq!(game.machine_learning_phase(), Some(LearnPhase::DidNotLearn));
                assert!(game
                    .party_screen
                    .item_use_notice()
                    .unwrap()
                    .contains("did not learn"));
                tap(&mut game, GbButton::A);
                assert_eq!(game.machine_learning_phase(), None);
                assert_eq!(game.state.screen, GameScreen::Bag);
                assert_eq!(quantity(&game), 2);
                assert_eq!(game.save_data.party.to_vec(), before);
            }
        }
    });
}

#[test]
fn tmhm_refusing_abandonment_restarts_trying_and_can_still_learn() {
    on_game_thread(|| {
        let mut game = fixture(ItemId::Tm21, true);
        use_machine(&mut game, ItemId::Tm21);
        full_confirm(&mut game);
        tap(&mut game, GbButton::B);
        tap(&mut game, GbButton::B);
        assert_eq!(
            game.machine_learning_phase(),
            Some(LearnPhase::TryingToLearn(0))
        );
        full_confirm(&mut game);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::Down);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        ack_learned(&mut game);
        assert_eq!(quantity(&game), 1);
        assert_eq!(
            game.save_data.party.get(0).unwrap().moves[1],
            MoveId::MegaDrain
        );
    });
}

#[test]
fn tmhm_hm_refusal_resets_to_first_move_and_preserves_the_pending_machine() {
    on_game_thread(|| {
        let mut game = fixture(ItemId::Tm21, true);
        let before = *game.save_data.party.get(0).unwrap();
        use_machine(&mut game, ItemId::Tm21);
        full_confirm(&mut game);
        tap(&mut game, GbButton::A);
        for _ in 0..3 {
            tap(&mut game, GbButton::Down);
        }
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.party_screen.phase(),
            PartyScreenPhase::MoveChoiceNotice
        );
        assert_eq!(*game.save_data.party.get(0).unwrap(), before);
        assert_eq!(quantity(&game), 2);
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.party_screen.phase(),
            PartyScreenPhase::ChooseMove { cursor: 0 }
        );
        tap(&mut game, GbButton::Down);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        ack_learned(&mut game);
        assert_eq!(game.save_data.party.get(0).unwrap().moves[3], MoveId::Cut);
        assert_eq!(quantity(&game), 1);
    });
}

#[test]
fn tmhm_boot_teach_decline_party_cancel_already_known_and_incompatible_keep_quantity() {
    on_game_thread(|| {
        let mut game = fixture(ItemId::Tm21, true);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::B);
        assert_eq!(game.bag_screen.phase(), BagPhase::Browsing);
        assert_eq!(quantity(&game), 2);
        assert_eq!(game.machine_learning_phase(), None);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::B);
        assert_eq!(game.state.screen, GameScreen::Bag);
        assert_eq!(quantity(&game), 2);
        for known in [false, true] {
            let mut game = fixture(ItemId::Tm21, true);
            let m = if known {
                pokered_core::pokemon::stats::create_pokemon_with_moves(
                    Species::Bulbasaur,
                    16,
                    [0x9a, 0x78],
                    [
                        MoveId::MegaDrain,
                        MoveId::Growl,
                        MoveId::LeechSeed,
                        MoveId::Cut,
                    ],
                )
                .unwrap()
            } else {
                pokered_core::pokemon::stats::create_pokemon(Species::Caterpie, 12, [0x9a, 0x78])
                    .unwrap()
            };
            *game.save_data.party.get_mut(0).unwrap() = m;
            game.bag_screen = BagScreenState::new(vec![(ItemId::Tm21, 2)]);
            use_machine(&mut game, ItemId::Tm21);
            assert_eq!(game.machine_learning_phase(), None);
            assert_eq!(
                game.party_screen.phase(),
                PartyScreenPhase::ItemUseNotice { wait_frames: 0 }
            );
            tap(&mut game, GbButton::A);
            assert_eq!(game.party_screen.phase(), PartyScreenPhase::Browsing);
            assert_eq!(quantity(&game), 2);
        }
    });
}

#[test]
fn tmhm_restore_overworld_slot_clears_learning_before_normal_party_reentry() {
    on_game_thread(|| {
        use pokered_core::start_menu::StartMenuItem;
        fn menu(game: &mut PokemonGame, item: StartMenuItem) {
            for _ in 0..game.start_menu.item_count() {
                if game.start_menu.current_item() == item {
                    break;
                }
                tap(game, GbButton::Down);
            }
            assert_eq!(game.start_menu.current_item(), item);
            tap(game, GbButton::A);
        }
        let mut game = fixture(ItemId::Tm21, true);
        game.handle_transition(GameScreen::Overworld);
        let before = game.save_data.party.to_vec();
        let hash = game.agent_save_state_slot(0).unwrap();
        tap(&mut game, GbButton::Start);
        menu(&mut game, StartMenuItem::Item);
        use_machine(&mut game, ItemId::Tm21);
        full_confirm(&mut game);
        assert_eq!(game.agent_restore_state_slot(0).unwrap(), hash);
        assert_eq!(game.machine_learning_phase(), None);
        tap(&mut game, GbButton::Start);
        menu(&mut game, StartMenuItem::Pokemon);
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.party_screen.phase(),
            PartyScreenPhase::ActionMenu { cursor: 0 }
        );
        assert_eq!(game.save_data.party.to_vec(), before);
        assert_eq!(quantity(&game), 2);
    });
}

#[test]
fn tmhm_soft_reset_then_continue_discards_unsaved_learning_and_consumption() {
    on_game_thread(|| {
        use pokered_core::start_menu::StartMenuItem;
        fn menu(game: &mut PokemonGame, item: StartMenuItem) {
            for _ in 0..game.start_menu.item_count() {
                if game.start_menu.current_item() == item {
                    break;
                }
                tap(game, GbButton::Down);
            }
            assert_eq!(game.start_menu.current_item(), item);
            tap(game, GbButton::A);
        }
        let mut game = fixture(ItemId::Tm21, true);
        game.handle_transition(GameScreen::Overworld);
        game.save_data.game_data.position.map_id = MapId::PalletTown as u8;
        game.save_data.game_data.position.x = 5;
        game.save_data.game_data.position.y = 6;
        let before = game.save_data.party.to_vec();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "machine-soft-reset-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("owned.sav");
        let bytes = export_sram(&game.save_data);
        std::fs::write(&path, &bytes).unwrap();
        game.state.save_summary = Some(save_summary_from_data(&game.save_data));
        game.save_path = Some(path.clone());
        game.external_saves = false;
        tap(&mut game, GbButton::Start);
        menu(&mut game, StartMenuItem::Item);
        use_machine(&mut game, ItemId::Tm21);
        full_confirm(&mut game);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::Down);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.machine_learning_phase(),
            Some(LearnPhase::Learned(MoveId::MegaDrain))
        );
        assert_eq!(quantity(&game), 2);
        let mut chord = InputState::new();
        for key in [GbButton::A, GbButton::B, GbButton::Start, GbButton::Select] {
            chord.press(key);
        }
        for _ in 0..SOFT_RESET_HOLD_FRAMES {
            game.update(&chord);
            chord.begin_frame();
        }
        assert_eq!(game.state.screen, GameScreen::TitleScreen);
        assert_eq!(game.machine_learning_phase(), None);
        for _ in 0..200 {
            if game.state.screen == GameScreen::Overworld {
                break;
            }
            tap(&mut game, GbButton::A);
        }
        assert_eq!(
            game.main_menu.last_choice,
            Some(pokered_core::game_state::MainMenuChoice::Continue)
        );
        assert_eq!(game.save_data.party.to_vec(), before);
        tap(&mut game, GbButton::Start);
        menu(&mut game, StartMenuItem::Pokemon);
        tap(&mut game, GbButton::A);
        assert_eq!(game.machine_learning_phase(), None);
        assert_eq!(
            game.party_screen.phase(),
            PartyScreenPhase::ActionMenu { cursor: 0 }
        );
        assert_eq!(quantity(&game), 2);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "unsaved learned state cannot write SRAM"
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    });
}
