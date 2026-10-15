//! StartMenu_Pokemon.loop through the actual shared runtime and party input.
use super::*;
use pokered_core::party_screen::{PartyScreenMode, PartyScreenPhase};
use pokered_data::{maps::MapId, moves::MoveId, species::Species};

fn on_game_thread(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
fn idle(game: &mut PokemonGame, frames: usize) {
    for _ in 0..frames {
        game.update(&InputState::new());
    }
}
fn tap(game: &mut PokemonGame, button: GbButton) {
    let mut input = InputState::new();
    input.press(button);
    game.update(&input);
    idle(game, 32);
}
fn fixture(move_id: MoveId, badges: u8, map: MapId) -> PokemonGame {
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
    game.audio = None;
    game.save_data = SaveData::new();
    let (species, moves) = match move_id {
        MoveId::Fly => (
            Species::Pidgey,
            [MoveId::Gust, MoveId::Fly, MoveId::None, MoveId::None],
        ),
        MoveId::Surf => (
            Species::Squirtle,
            [MoveId::Tackle, MoveId::Surf, MoveId::None, MoveId::None],
        ),
        MoveId::Dig => (
            Species::Charmander,
            [MoveId::Scratch, MoveId::Dig, MoveId::None, MoveId::None],
        ),
        MoveId::Teleport => (
            Species::Abra,
            [MoveId::Teleport, MoveId::None, MoveId::None, MoveId::None],
        ),
        MoveId::Softboiled => (
            Species::Chansey,
            [
                MoveId::Pound,
                MoveId::Softboiled,
                MoveId::None,
                MoveId::None,
            ],
        ),
        _ => (
            Species::Bulbasaur,
            [MoveId::Tackle, move_id, MoveId::None, MoveId::None],
        ),
    };
    game.save_data
        .party
        .add(
            pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 12, [0x9a, 0x78])
                .unwrap(),
        )
        .unwrap();
    game.save_data
        .party
        .add(
            pokered_core::pokemon::stats::create_pokemon_with_moves(
                species,
                12,
                [0x9a, 0x78],
                moves,
            )
            .unwrap(),
        )
        .unwrap();
    game.save_data.game_data.obtained_badges = badges;
    game.overworld = OverworldScreen::new(map, None, PokemonRedData);
    game.overworld.state.player.x = 3;
    game.overworld.state.player.y = 6;
    game.state.screen = GameScreen::PartyScreen;
    game.party_screen = PartyScreenState::new(game.save_data.party.to_vec());
    game.party_screen.set_cursor(1);
    game
}
fn choose_field_move(game: &mut PokemonGame) {
    tap(game, GbButton::A);
    assert_eq!(
        game.party_screen.phase(),
        PartyScreenPhase::ActionMenu { cursor: 0 }
    );
    tap(game, GbButton::A);
}
fn assert_notice_then_list(game: &mut PokemonGame, acknowledge: GbButton) {
    assert_eq!(game.state.screen, GameScreen::PartyScreen);
    assert!(matches!(
        game.party_screen.phase(),
        PartyScreenPhase::ItemUseNotice { .. }
    ));
    assert!(game.party_screen.item_use_notice().is_some());
    assert!(
        game.overworld.pending_dialogue.is_none(),
        "no second text owner on the field"
    );
    idle(game, 120);
    assert!(
        game.party_screen.item_use_notice().is_some(),
        "time alone cannot dismiss it"
    );
    tap(game, acknowledge);
    assert_eq!(game.state.screen, GameScreen::PartyScreen);
    assert_eq!(game.party_screen.phase(), PartyScreenPhase::Browsing);
    assert_eq!(game.party_screen.mode(), PartyScreenMode::Normal);
    assert_eq!(
        game.party_screen.cursor(),
        1,
        "restore the field-move user's cursor"
    );
    assert!(game.pending_softboiled_user.is_none());
    assert_eq!(game.party_screen.party(), game.save_data.party.to_vec());
}
#[test]
fn field_move_refusals_acknowledge_then_resume_the_same_party_member() {
    on_game_thread(|| {
        let cases = [
            (MoveId::Cut, 0, "BADGE"),
            (MoveId::Fly, 0, "BADGE"),
            (MoveId::Surf, 0, "BADGE"),
            (MoveId::Strength, 0, "BADGE"),
            (MoveId::Flash, 0, "BADGE"),
            (MoveId::Cut, 2, "anything to CUT"),
            (MoveId::Fly, 4, "FLY here"),
            (MoveId::Surf, 16, "No SURFing"),
            (MoveId::Dig, 0, "time to use"),
            (MoveId::Teleport, 0, "TELEPORT"),
            (MoveId::Softboiled, 0, "Not healthy"),
        ];
        for (move_id, badges, text) in cases {
            let mut game = fixture(move_id, badges, MapId::RedsHouse2F);
            if move_id == MoveId::Softboiled {
                let mon = game.save_data.party.get_mut(1).unwrap();
                mon.hp = mon.max_hp / 5;
                game.party_screen
                    .refresh_party(game.save_data.party.to_vec());
            }
            let saved = game.save_data.party.to_vec();
            choose_field_move(&mut game);
            assert!(
                game.party_screen.item_use_notice().unwrap().contains(text),
                "{move_id:?}"
            );
            assert_notice_then_list(&mut game, GbButton::A);
            assert_eq!(
                game.save_data.party.to_vec(),
                saved,
                "{move_id:?} does not spend HP/PP"
            );
            tap(&mut game, GbButton::A);
            assert_eq!(
                game.party_screen.phase(),
                PartyScreenPhase::ActionMenu { cursor: 0 }
            );
            tap(&mut game, GbButton::B);
            tap(&mut game, GbButton::B);
            assert_eq!(game.state.screen, GameScreen::StartMenu);
        }
    });
}
#[test]
fn softboiled_success_keeps_result_until_ack_and_returns_to_the_user() {
    on_game_thread(|| {
        let mut game = fixture(MoveId::Softboiled, 0, MapId::RedsHouse2F);
        game.save_data.party.get_mut(0).unwrap().hp = 1;
        game.party_screen
            .refresh_party(game.save_data.party.to_vec());
        let before = game.save_data.party.to_vec();
        choose_field_move(&mut game);
        assert_eq!(
            game.party_screen.mode(),
            PartyScreenMode::SoftboiledTarget(1)
        );
        tap(&mut game, GbButton::Up);
        tap(&mut game, GbButton::A);
        let cost = before[1].max_hp / 5;
        assert_eq!(game.save_data.party.get(1).unwrap().hp, before[1].hp - cost);
        assert_eq!(game.save_data.party.get(0).unwrap().hp, 1 + cost);
        assert!(game
            .party_screen
            .item_use_notice()
            .unwrap()
            .contains("recovered by"));
        tap(&mut game, GbButton::A); // Before the medicine's 50-frame wait ends.
        assert!(game.party_screen.item_use_notice().is_some());
        assert_eq!(game.save_data.party.get(1).unwrap().hp, before[1].hp - cost);
        assert_notice_then_list(&mut game, GbButton::B);
        for (old, new) in before.iter().zip(game.save_data.party.to_vec()) {
            assert_eq!((old.pp, old.pp_ups), (new.pp, new.pp_ups));
        }
    });
}
#[test]
fn softboiled_invalid_target_ack_returns_to_normal_list_without_spending_hp() {
    on_game_thread(|| {
        for hp in [0, 35] {
            let mut game = fixture(MoveId::Softboiled, 0, MapId::RedsHouse2F);
            game.save_data.party.get_mut(0).unwrap().hp = hp;
            game.party_screen
                .refresh_party(game.save_data.party.to_vec());
            let before = game.save_data.party.to_vec();
            choose_field_move(&mut game);
            tap(&mut game, GbButton::Up);
            tap(&mut game, GbButton::A);
            assert!(game
                .party_screen
                .item_use_notice()
                .unwrap()
                .contains("effect"));
            assert_notice_then_list(&mut game, GbButton::A);
            assert_eq!(game.save_data.party.to_vec(), before);
        }
    });
}
#[test]
fn softboiled_cancel_restores_user_and_does_not_reopen_after_stats() {
    on_game_thread(|| {
        let mut game = fixture(MoveId::Softboiled, 0, MapId::RedsHouse2F);
        let before = game.save_data.party.to_vec();
        choose_field_move(&mut game);
        tap(&mut game, GbButton::Up);
        tap(&mut game, GbButton::B);
        assert_eq!(game.party_screen.cursor(), 1);
        assert_eq!(game.party_screen.mode(), PartyScreenMode::Normal);
        assert!(game.pending_softboiled_user.is_none());
        tap(&mut game, GbButton::A);
        tap(&mut game, GbButton::Down); // STATS follows SOFTBOILED
        tap(&mut game, GbButton::A);
        assert_eq!(game.state.screen, GameScreen::PokemonStatsScreen(1));
        tap(&mut game, GbButton::B);
        assert_eq!(game.party_screen.mode(), PartyScreenMode::Normal);
        assert_eq!(game.party_screen.cursor(), 1);
        assert_eq!(game.save_data.party.to_vec(), before);
    });
}
#[test]
fn fly_cancel_still_returns_to_party_and_successful_strength_still_returns_to_field() {
    on_game_thread(|| {
        let mut game = fixture(MoveId::Fly, 4, MapId::Route1);
        choose_field_move(&mut game);
        assert_eq!(game.state.screen, GameScreen::TownMap);
        tap(&mut game, GbButton::B);
        assert_eq!(game.state.screen, GameScreen::PartyScreen);
        assert_eq!(game.party_screen.mode(), PartyScreenMode::Normal);
        assert_eq!(game.party_screen.cursor(), 1);
        assert!(game.overworld.pending_warp.is_none());
        let mut game = fixture(MoveId::Strength, 8, MapId::RedsHouse2F);
        choose_field_move(&mut game);
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert!(game.overworld.pending_dialogue.is_some());
        assert!(game.party_screen.item_use_notice().is_none());
        assert!(game.overworld.strength_active);
    });
}

#[test]
fn softboiled_snapshot_restore_discards_the_abandoned_target_picker() {
    on_game_thread(|| {
        let mut game = fixture(MoveId::Softboiled, 0, MapId::RedsHouse2F);
        game.handle_transition(GameScreen::Overworld);
        let snapshot = game.agent_save_state().unwrap();
        game.handle_transition(GameScreen::PartyScreen);
        choose_field_move(&mut game);
        assert_eq!(game.pending_softboiled_user, Some(1));
        game.agent_restore_state(&snapshot).unwrap();
        assert_eq!(game.state.screen, GameScreen::Overworld);
        game.handle_transition(GameScreen::PartyScreen);
        assert!(game.pending_softboiled_user.is_none());
        assert_eq!(game.party_screen.mode(), PartyScreenMode::Normal);
        assert_eq!(game.party_screen.party(), game.save_data.party.to_vec());
    });
}

#[test]
fn softboiled_ordinary_load_and_soft_reset_discard_the_abandoned_picker() {
    on_game_thread(|| {
        for reset in [false, true] {
            let mut game = fixture(MoveId::Softboiled, 0, MapId::RedsHouse2F);
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir()
                .join(format!("field-move-reset-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("owned.sav");
            std::fs::write(&path, export_sram(&game.save_data)).unwrap();
            game.save_path = Some(path.clone());
            let before = game.save_data.party.to_vec();
            choose_field_move(&mut game);
            assert_eq!(game.pending_softboiled_user, Some(1));
            if reset {
                let mut chord = InputState::new();
                for key in [GbButton::A, GbButton::B, GbButton::Start, GbButton::Select] {
                    chord.press(key);
                }
                for _ in 0..SOFT_RESET_HOLD_FRAMES {
                    game.update(&chord);
                    chord.begin_frame();
                }
                assert_eq!(game.state.screen, GameScreen::TitleScreen);
            } else {
                game.load_from_path(&path).unwrap();
            }
            // The ordinary-load API delegates overlay reconstruction to its
            // caller; verify the next party entry has no discarded picker.
            game.handle_transition(GameScreen::PartyScreen);
            assert!(game.pending_softboiled_user.is_none());
            assert_eq!(game.party_screen.mode(), PartyScreenMode::Normal);
            assert_eq!(game.save_data.party.to_vec(), before);
            std::fs::remove_dir_all(dir).unwrap();
        }
    });
}
