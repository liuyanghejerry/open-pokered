use super::rare_candy::RareCandyPhase;
use super::*;
use pokered_core::party_screen::PartyScreenPhase;
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
    game.save_data.game_data.bag.get(0).unwrap().1
}
fn candy_game(species: Species, level: u8, full: bool) -> PokemonGame {
    candy_game_with_hp(species, level, full, None)
}
fn candy_fixture(species: Species, level: u8, full: bool, hp: Option<u16>) -> PokemonGame {
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
    let mut mon = if species == Species::Bulbasaur {
        pokered_core::pokemon::stats::create_pokemon_with_moves(
            species,
            level,
            [0x9a, 0x78],
            if full {
                [
                    MoveId::Tackle,
                    MoveId::Growl,
                    MoveId::LeechSeed,
                    MoveId::Cut,
                ]
            } else {
                [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None]
            },
        )
        .unwrap()
    } else {
        pokered_core::pokemon::stats::create_pokemon(species, level, [0x9a, 0x78]).unwrap()
    };
    mon.pp = if full { [7, 4, 3, 1] } else { [7, 4, 0, 0] };
    mon.pp_ups = if full { [1, 2, 3, 1] } else { [1, 2, 0, 0] };
    if let Some(hp) = hp {
        mon.hp = hp;
    }
    game.save_data.party.add(mon).unwrap();
    game.save_data
        .party
        .add(
            pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 12, [0x9a, 0x78])
                .unwrap(),
        )
        .unwrap();
    game.save_data
        .game_data
        .bag
        .add_item(ItemId::RareCandy, 2)
        .unwrap();
    game.state.screen = GameScreen::Bag;
    game.bag_screen = BagScreenState::new(vec![(ItemId::RareCandy, 2)]);
    game
}
fn candy_game_with_hp(species: Species, level: u8, full: bool, hp: Option<u16>) -> PokemonGame {
    let mut game = candy_fixture(species, level, full, hp);
    for _ in 0..3 {
        tap(&mut game, GbButton::A);
    }
    assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::LevelAndStats));
    assert_eq!(
        quantity(&game),
        2,
        "RemoveUsedItem is after learning/evolution"
    );
    assert!(game.evolution_anim.is_none());
    game
}
fn acknowledge_learned(game: &mut PokemonGame) {
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
    panic!("learning sound did not complete");
}

fn finish_evolution(game: &mut PokemonGame, cancel: bool) {
    if cancel {
        for _ in 0..1200 {
            if game
                .evolution_anim
                .as_ref()
                .is_some_and(|anim| anim.cancel_window_open())
            {
                break;
            }
            game.update(&InputState::new());
        }
        assert!(game.evolution_anim.as_ref().unwrap().cancel_window_open());
        tap(game, GbButton::B);
    }
    for frame in 0..3000 {
        if game.evolution_anim.is_none() {
            return;
        }
        let mut input = InputState::new();
        if frame % 64 == 0 {
            input.press(GbButton::A);
        }
        game.update(&input);
    }
    panic!("evolution did not return control");
}

fn full_confirm(game: &mut PokemonGame) {
    for expected in [
        RareCandyPhase::TryingToLearn(0),
        RareCandyPhase::TryingToLearn(1),
        RareCandyPhase::ReplaceConfirm { yes: true },
    ] {
        tap(game, GbButton::A);
        assert_eq!(game.rare_candy_phase(), Some(expected));
    }
}

#[test]
fn rare_candy_field_free_slot_shows_stats_then_learned_notice_and_consumes_once() {
    on_game_thread(|| {
        let mut game = candy_game(Species::Bulbasaur, 12, false);
        let partner = game.save_data.party.get(1).unwrap().clone();
        let mon = game.save_data.party.get(0).unwrap();
        assert_eq!((mon.level, mon.hp, mon.max_hp), (13, 37, 37));
        assert_eq!(
            mon.moves,
            [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None]
        );
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.rare_candy_phase(),
            Some(RareCandyPhase::Learned(MoveId::VineWhip))
        );
        assert_eq!(
            game.party_screen.item_use_notice(),
            Some("BULBASAUR learned\nVINE WHIP!")
        );
        let mon = game.save_data.party.get(0).unwrap();
        assert_eq!(
            mon.moves,
            [
                MoveId::Tackle,
                MoveId::Growl,
                MoveId::VineWhip,
                MoveId::None
            ]
        );
        assert_eq!(mon.pp, [7, 4, 10, 0]);
        assert_eq!(mon.pp_ups, [1, 2, 0, 0]);
        assert_eq!(quantity(&game), 2);
        acknowledge_learned(&mut game);
        assert_eq!(game.state.screen, GameScreen::Bag);
        assert_eq!(game.rare_candy_phase(), None);
        assert_eq!(quantity(&game), 1);
        assert_eq!(game.save_data.party.get(1).unwrap(), &partner);
    });
}

#[test]
fn rare_candy_field_full_moves_can_replace_preserving_other_pp_and_ups() {
    on_game_thread(|| {
        let mut game = candy_game(Species::Bulbasaur, 12, true);
        full_confirm(&mut game);
        tap(&mut game, GbButton::A);
        assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::ChooseMove));
        tap(&mut game, GbButton::Down);
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.rare_candy_phase(),
            Some(RareCandyPhase::Forgot { slot: 1 })
        );
        assert_eq!(
            game.save_data.party.get(0).unwrap().moves[1],
            MoveId::Growl,
            "write follows the forgot/And text"
        );
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.rare_candy_phase(),
            Some(RareCandyPhase::Learned(MoveId::VineWhip))
        );
        let mon = game.save_data.party.get(0).unwrap();
        assert_eq!(
            mon.moves,
            [
                MoveId::Tackle,
                MoveId::VineWhip,
                MoveId::LeechSeed,
                MoveId::Cut
            ]
        );
        assert_eq!(mon.pp, [7, 10, 3, 1]);
        assert_eq!(mon.pp_ups, [1, 0, 3, 1]);
        assert_eq!(quantity(&game), 2);
        acknowledge_learned(&mut game);
        assert_eq!(quantity(&game), 1);
        assert_eq!(game.state.screen, GameScreen::Bag);
    });
}

#[test]
fn rare_candy_field_decline_and_forget_cancel_require_abandon_confirmation() {
    on_game_thread(|| {
        for cancel_forget in [false, true] {
            let mut game = candy_game(Species::Bulbasaur, 12, true);
            let before = game.save_data.party.get(0).unwrap().clone();
            full_confirm(&mut game);
            if cancel_forget {
                tap(&mut game, GbButton::A);
                tap(&mut game, GbButton::B);
            } else {
                tap(&mut game, GbButton::Down);
                tap(&mut game, GbButton::A);
            }
            assert_eq!(
                game.rare_candy_phase(),
                Some(RareCandyPhase::AbandonConfirm { yes: true })
            );
            assert_eq!(quantity(&game), 2);
            tap(&mut game, GbButton::A);
            assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::DidNotLearn));
            assert_eq!(game.save_data.party.get(0).unwrap(), &before);
            tap(&mut game, GbButton::A);
            assert_eq!(quantity(&game), 1);
            assert_eq!(game.state.screen, GameScreen::Bag);
        }
    });
}

#[test]
fn rare_candy_field_rejecting_abandonment_restarts_the_learning_prompt() {
    on_game_thread(|| {
        let mut game = candy_game(Species::Bulbasaur, 12, true);
        full_confirm(&mut game);
        tap(&mut game, GbButton::B);
        assert_eq!(
            game.rare_candy_phase(),
            Some(RareCandyPhase::AbandonConfirm { yes: true })
        );
        tap(&mut game, GbButton::B);
        assert_eq!(
            game.rare_candy_phase(),
            Some(RareCandyPhase::TryingToLearn(0))
        );
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::ChooseMove));
        assert_eq!(quantity(&game), 2);
    });
}

#[test]
fn rare_candy_field_hm_guard_keeps_the_pending_move_and_can_retry() {
    on_game_thread(|| {
        let mut game = candy_game(Species::Bulbasaur, 12, true);
        full_confirm(&mut game);
        tap(&mut game, GbButton::A);
        let before = game.save_data.party.get(0).unwrap().clone();
        for _ in 0..3 {
            tap(&mut game, GbButton::Down);
        }
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.party_screen.phase(),
            PartyScreenPhase::MoveChoiceNotice
        );
        assert_eq!(
            game.party_screen.move_choice_notice(),
            Some("HM techniques\ncan't be deleted!")
        );
        assert_eq!(game.save_data.party.get(0).unwrap(), &before);
        assert_eq!(quantity(&game), 2);
        tap(&mut game, GbButton::A);
        assert_eq!(
            game.party_screen.phase(),
            PartyScreenPhase::ChooseMove { cursor: 0 }
        );
        tap(&mut game, GbButton::Down);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        acknowledge_learned(&mut game);
        assert_eq!(quantity(&game), 1);
        assert_eq!(game.save_data.party.get(0).unwrap().moves[3], MoveId::Cut);
    });
}

#[test]
fn rare_candy_field_evolution_waits_for_stats_and_cancel_still_consumes_once() {
    on_game_thread(|| {
        let mut game = candy_game(Species::Charmander, 15, false);
        assert!(game.evolution_anim.is_none());
        tap(&mut game, GbButton::A);
        assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::Evolving));
        assert!(game.evolution_anim.is_some());
        assert_eq!(quantity(&game), 2);
        // Drive the real movie to its B-cancel window and acknowledge its
        // stopped-evolving text; no synthetic outcome or actor removal.
        finish_evolution(&mut game, true);
        assert_eq!(
            game.save_data.party.get(0).unwrap().species,
            Species::Charmander
        );
        assert_eq!(game.save_data.party.get(0).unwrap().level, 16);
        assert_eq!(quantity(&game), 1);
        assert_eq!(game.state.screen, GameScreen::Bag);
        assert!(game.evolution_anim.is_none());
    });
}

#[test]
fn rare_candy_field_evolution_announces_new_species_moves_without_replaying_levelup() {
    on_game_thread(|| {
        let mut game = candy_game(Species::Magikarp, 19, false);
        tap(&mut game, GbButton::A);
        assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::Evolving));
        finish_evolution(&mut game, false);
        assert_eq!(
            game.rare_candy_phase(),
            Some(RareCandyPhase::Learned(MoveId::Bite))
        );
        assert_eq!(
            game.save_data.party.get(0).unwrap().species,
            Species::Gyarados
        );
        assert_eq!(game.save_data.party.get(0).unwrap().level, 20);
        assert_eq!(quantity(&game), 2);
        acknowledge_learned(&mut game);
        assert_eq!(quantity(&game), 1);
        assert!(game.evolution_anim.is_none());
    });
}

#[test]
fn rare_candy_stats_overlay_preserves_the_selected_current_hp_numbers() {
    on_game_thread(|| {
        let mut game = candy_game(Species::Bulbasaur, 12, false);
        let mut shown = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut shown);
        let flow = game.rare_candy_flow.take();
        let mut underlying = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut underlying);
        game.rare_candy_flow = flow;
        let mut ink = 0;
        for y in 16..24 {
            for x in 80..160 {
                assert_eq!(
                    shown.get_pixel(x, y),
                    underlying.get_pixel(x, y),
                    "stats must leave the selected member's current/max HP visible at ({x},{y})"
                );
                if underlying.get_pixel(x, y) != Some(Rgba::WHITE) {
                    ink += 1;
                }
            }
        }
        assert!(ink > 0, "HP-number region must actually contain ink");
    });
}

#[test]
fn rare_candy_field_revives_a_fainted_mon_before_its_pending_learning_prompt() {
    on_game_thread(|| {
        let mut game = candy_game_with_hp(Species::Bulbasaur, 12, true, Some(0));
        let partner = game.save_data.party.get(1).unwrap().clone();
        let mon = game.save_data.party.get(0).unwrap();
        assert_eq!((mon.level, mon.hp, mon.max_hp), (13, 2, 37));
        assert_eq!(
            mon.moves,
            [
                MoveId::Tackle,
                MoveId::Growl,
                MoveId::LeechSeed,
                MoveId::Cut
            ]
        );
        full_confirm(&mut game);
        tap(&mut game, GbButton::B);
        tap(&mut game, GbButton::A);
        assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::DidNotLearn));
        tap(&mut game, GbButton::A);
        assert_eq!(game.state.screen, GameScreen::Bag);
        assert_eq!(quantity(&game), 1);
        assert_eq!(
            (
                game.save_data.party.get(0).unwrap().level,
                game.save_data.party.get(0).unwrap().hp
            ),
            (13, 2)
        );
        assert_eq!(game.save_data.party.get(1).unwrap(), &partner);
    });
}

#[test]
fn rare_candy_restoring_overworld_slot_discards_pending_learning_before_party_reentry() {
    on_game_thread(|| {
        use pokered_core::start_menu::StartMenuItem;
        fn select_menu(game: &mut PokemonGame, item: StartMenuItem) {
            assert_eq!(game.state.screen, GameScreen::StartMenu);
            for _ in 0..game.start_menu.item_count() {
                if game.start_menu.current_item() == item { break; }
                tap(game, GbButton::Down);
            }
            assert_eq!(game.start_menu.current_item(), item);
            tap(game, GbButton::A);
        }

        let mut game = candy_fixture(Species::Bulbasaur, 12, false, None);
        game.handle_transition(GameScreen::Overworld);
        let before = *game.save_data.party.get(0).unwrap();
        let partner = *game.save_data.party.get(1).unwrap();
        let saved_hash = game.agent_save_state_slot(0).unwrap();
        tap(&mut game, GbButton::Start);
        select_menu(&mut game, StartMenuItem::Item);
        for _ in 0..3 { tap(&mut game, GbButton::A); }
        assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::LevelAndStats));
        tap(&mut game, GbButton::A);
        assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::Learned(MoveId::VineWhip)));
        assert_eq!(quantity(&game), 2);
        assert!(matches!(game.agent_save_state_slot(1),
            Err(crate::agent_state::StateError::UnsupportedScreen(_))));

        assert_eq!(game.agent_restore_state_slot(0).unwrap(), saved_hash);
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert_eq!(game.rare_candy_phase(), None);
        assert_eq!(*game.save_data.party.get(0).unwrap(), before);
        assert_eq!(quantity(&game), 2);
        tap(&mut game, GbButton::Start);
        select_menu(&mut game, StartMenuItem::Pokemon);
        tap(&mut game, GbButton::A);
        assert_eq!(game.state.screen, GameScreen::PartyScreen);
        assert!(matches!(game.party_screen.phase(), PartyScreenPhase::ActionMenu { .. }));
        assert_eq!(*game.save_data.party.get(0).unwrap(), before);
        assert_eq!(quantity(&game), 2, "ordinary party A must not finish the abandoned candy");

        tap(&mut game, GbButton::B);
        tap(&mut game, GbButton::B);
        select_menu(&mut game, StartMenuItem::Item);
        for _ in 0..3 { tap(&mut game, GbButton::A); }
        tap(&mut game, GbButton::A);
        assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::Learned(MoveId::VineWhip)));
        acknowledge_learned(&mut game);
        assert_eq!(game.state.screen, GameScreen::Bag);
        assert_eq!(game.rare_candy_phase(), None);
        assert_eq!(game.save_data.party.get(0).unwrap().level, 13);
        assert_eq!(game.save_data.party.get(0).unwrap().moves,
            [MoveId::Tackle, MoveId::Growl, MoveId::VineWhip, MoveId::None]);
        assert_eq!(quantity(&game), 1);
        assert_eq!(*game.save_data.party.get(1).unwrap(), partner);
    });
}

#[test]
fn rare_candy_desktop_soft_reset_discards_pending_learning_before_continue() {
    on_game_thread(|| {
        use pokered_core::start_menu::StartMenuItem;
        fn select_menu(game: &mut PokemonGame, item: StartMenuItem) {
            assert_eq!(game.state.screen, GameScreen::StartMenu);
            for _ in 0..game.start_menu.item_count() {
                if game.start_menu.current_item() == item { break; }
                tap(game, GbButton::Down);
            }
            assert_eq!(game.start_menu.current_item(), item);
            tap(game, GbButton::A);
        }
        fn observe(game: &mut PokemonGame, label: &str) {
            println!("{label}: screen={:?} candy={:?} qty={} level={} moves={:?}",
                game.state.screen, game.rare_candy_phase(), quantity(game),
                game.save_data.party.get(0).unwrap().level,
                game.save_data.party.get(0).unwrap().moves);
            if let Ok(root) = std::env::var("FIDELITY_CANDY_RESET_CAPTURE") {
                let root = std::path::PathBuf::from(root);
                std::fs::create_dir_all(&root).unwrap();
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                game.draw(&mut fb);
                fb.save_png(&root.join(format!("{label}.png"))).unwrap();
                std::fs::write(root.join(format!("{label}.json")), serde_json::to_vec_pretty(
                    &serde_json::json!({"frame":game.frame_count,"screen":format!("{:?}",game.state.screen),
                        "phase":game.rare_candy_phase().map(|p|format!("{p:?}")),"quantity":quantity(game),
                        "party":game.save_data.party})).unwrap()).unwrap();
            }
        }

        let mut game = candy_fixture(Species::Bulbasaur, 12, true, None);
        game.handle_transition(GameScreen::Overworld);
        // A private committed SRAM and summary are fixture construction,
        // before any Candy/soft-reset inputs. Never touch the default save.
        game.save_data.game_data.position.map_id = MapId::PalletTown as u8;
        game.save_data.game_data.position.x = 5;
        game.save_data.game_data.position.y = 6;
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("candy-soft-reset-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("owned.sav");
        let committed = export_sram(&game.save_data);
        std::fs::write(&path, &committed).unwrap();
        game.state.save_summary = Some(save_summary_from_data(&game.save_data));
        game.save_path = Some(path.clone());
        game.external_saves = false;
        let before = *game.save_data.party.get(0).unwrap();
        let partner = *game.save_data.party.get(1).unwrap();
        tap(&mut game, GbButton::Start);
        select_menu(&mut game, StartMenuItem::Item);
        for _ in 0..3 { tap(&mut game, GbButton::A); }
        full_confirm(&mut game);
        assert_eq!(game.rare_candy_phase(), Some(RareCandyPhase::ReplaceConfirm { yes: true }));
        observe(&mut game, "before-chord");
        let mut chord = InputState::new();
        for key in [GbButton::A, GbButton::B, GbButton::Start, GbButton::Select] { chord.press(key); }
        for _ in 0..SOFT_RESET_HOLD_FRAMES {
            game.update(&chord);
            chord.begin_frame(); // Hold the chord without another just-pressed edge.
        }
        assert_eq!(game.state.screen, GameScreen::TitleScreen);
        assert_eq!(game.save_data.party.get(0).unwrap().level, 12);
        assert_eq!(quantity(&game), 2);
        observe(&mut game, "after-chord");
        for _ in 0..100 {
            if game.state.screen == GameScreen::MainMenu { break; }
            tap(&mut game, GbButton::A);
        }
        assert_eq!(game.state.screen, GameScreen::MainMenu);
        // Drive the menu's initial delay and CONTINUE information page with
        // real acknowledgements; never force its phase or target screen.
        for _ in 0..100 {
            if game.state.screen == GameScreen::Overworld { break; }
            tap(&mut game, GbButton::A);
        }
        assert_eq!(game.main_menu.last_choice, Some(pokered_core::game_state::MainMenuChoice::Continue));
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert_eq!(*game.save_data.party.get(0).unwrap(), before);
        observe(&mut game, "continued");
        tap(&mut game, GbButton::Start);
        select_menu(&mut game, StartMenuItem::Pokemon);
        observe(&mut game, "party-entry");
        tap(&mut game, GbButton::A);
        observe(&mut game, "party-a");
        if game.rare_candy_phase() == Some(RareCandyPhase::DidNotLearn) {
            // Complete an observed stale prompt so the old regression also
            // exposes its incorrect consumption, rather than only its UI.
            tap(&mut game, GbButton::A);
            observe(&mut game, "stale-finished");
        }
        assert_eq!(quantity(&game), 2, "CONTINUE/party input must not consume the abandoned Candy");
        assert_eq!(game.rare_candy_phase(), None);
        assert_eq!(game.state.screen, GameScreen::PartyScreen);
        assert!(matches!(game.party_screen.phase(), PartyScreenPhase::ActionMenu { .. }));
        assert_eq!(*game.save_data.party.get(0).unwrap(), before);
        tap(&mut game, GbButton::B);
        tap(&mut game, GbButton::B);
        select_menu(&mut game, StartMenuItem::Item);
        for _ in 0..3 { tap(&mut game, GbButton::A); }
        full_confirm(&mut game);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::Down);
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::A);
        acknowledge_learned(&mut game);
        assert_eq!(game.state.screen, GameScreen::Bag);
        assert_eq!(game.rare_candy_phase(), None);
        assert_eq!(game.save_data.party.get(0).unwrap().level, 13);
        assert_eq!(game.save_data.party.get(0).unwrap().moves[1], MoveId::VineWhip);
        assert_eq!(quantity(&game), 1);
        assert_eq!(*game.save_data.party.get(1).unwrap(), partner);
        assert_eq!(std::fs::read(&path).unwrap(), committed, "unsaved flow must not write SRAM");
        observe(&mut game, "new-candy-finished");
        std::fs::remove_dir_all(dir).unwrap();
    });
}
