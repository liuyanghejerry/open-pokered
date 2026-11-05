//! Actual party cancellation through shared resource work and START input.
use super::*;
use pokered_data::{maps::MapId, moves::MoveId, species::Species};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn frame(game: &mut PokemonGame, input: &mut InputState, buttons: u8) {
    input.begin_frame();
    for (bit, key) in [(1, GbButton::A), (2, GbButton::B), (128, GbButton::Down)] {
        if buttons & bit != 0 {
            input.press(key);
        } else {
            input.release(key);
        }
    }
    game.update(input);
    if let Some(flow) = game.field_flash.as_ref() {
        assert!(flow.error.is_none(), "{:?}", flow.error);
    }
}

fn fixture(map: MapId) -> (PokemonGame, InputState) {
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = Some(AudioOutput::new_pcm());
    let audio = game.audio.as_ref().unwrap();
    audio.play_music(pokered_audio::music_data::MusicId::DUNGEON3);
    audio.update_frame();
    game.overworld = OverworldScreen::new(map, None, PokemonRedData);
    game.overworld.publish_npc_frame(false);
    let mon = pokered_core::pokemon::stats::create_pokemon_with_moves(
        Species::Bulbasaur,
        5,
        [0x9a, 0x78],
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    game.save_data.party.add(mon).unwrap();
    game.start_menu.open(false, true, false);
    game.party_screen = PartyScreenState::new(game.save_data.party.to_vec());
    game.state.screen = GameScreen::PartyScreen;
    game.source_display
        .begin_legacy_party_sprite_hide()
        .unwrap();
    game.overworld.player_presentation.suppress_current_image();
    game.overworld.npc_presentation.suppress_current_images();
    let mut input = InputState::new();
    for _ in 0..64 {
        frame(&mut game, &mut input, 0);
    }
    (game, input)
}

#[test]
fn party_cancel_discards_short_restore_pulses_and_keeps_map_actors_stationary() {
    run(|| {
        for map in [MapId::PalletTown, MapId::RedsHouse2F, MapId::RockTunnel1F] {
            let (mut game, mut input) = fixture(map);
            let player = (game.overworld.state.player.x, game.overworld.state.player.y);
            let npcs: Vec<_> = game
                .overworld
                .npc_states
                .iter()
                .map(|n| (n.x, n.y))
                .collect();
            let darkness = game.overworld.dark_cave.map_pal_offset;
            frame(&mut game, &mut input, 2);
            assert!(
                game.field_flash.is_some(),
                "real PartyScreenAction::Cancelled must start restoration"
            );
            let mut returned = false;
            let mut saw_white = false;
            for tick in 0..600 {
                frame(
                    &mut game,
                    &mut input,
                    match tick {
                        10 | 11 => 128,
                        16 | 17 => 1,
                        _ => 0,
                    },
                );
                saw_white |= game.field_flash_white_frame();
                assert_eq!(
                    (game.overworld.state.player.x, game.overworld.state.player.y),
                    player
                );
                assert_eq!(
                    game.overworld
                        .npc_states
                        .iter()
                        .map(|n| (n.x, n.y))
                        .collect::<Vec<_>>(),
                    npcs
                );
                assert_eq!(game.overworld.dark_cave.map_pal_offset, darkness);
                if game.field_flash.is_none() {
                    returned = true;
                    break;
                }
            }
            assert!(returned && saw_white);
            assert_eq!(game.state.screen, GameScreen::StartMenu);
            for _ in 0..8 {
                frame(&mut game, &mut input, 0);
            }
            assert_eq!(game.start_menu.cursor(), 0);
            assert_eq!(game.state.screen, GameScreen::StartMenu);
            frame(&mut game, &mut input, 128);
            for _ in 0..8 {
                frame(&mut game, &mut input, 0);
            }
            assert_eq!(game.start_menu.cursor(), 1);
            frame(&mut game, &mut input, 1);
            assert_eq!(game.state.screen, GameScreen::Bag);
        }
    });
}

#[test]
fn party_return_polls_held_keys_after_restoration_without_repeating_the_cancel_key() {
    run(|| {
        for (held, expected_screen, expected_cursor) in [
            (128, GameScreen::StartMenu, 1),
            (1, GameScreen::PartyScreen, 0),
            (2, GameScreen::StartMenu, 0),
        ] {
            let (mut game, mut input) = fixture(MapId::PalletTown);
            frame(&mut game, &mut input, 2);
            let mut returned = false;
            for _ in 0..600 {
                frame(&mut game, &mut input, held);
                if game.field_flash.is_none() {
                    returned = true;
                    break;
                }
                assert_eq!(game.state.screen, GameScreen::PartyScreen);
            }
            assert!(returned);
            for _ in 0..8 {
                frame(&mut game, &mut input, held);
            }
            assert_eq!(game.state.screen, expected_screen);
            assert_eq!(game.start_menu.cursor(), expected_cursor);
        }
    });
}

#[test]
fn party_return_completes_after_a_recent_party_swap_sound() {
    run(|| {
        let (mut game, mut input) = fixture(MapId::PalletTown);
        game.audio.as_ref().unwrap().play_sfx(SfxId::Swap);
        for _ in 0..8 {
            frame(&mut game, &mut input, 0);
        }
        frame(&mut game, &mut input, 2);
        {
            let manager = game.audio.as_ref().unwrap().manager.lock().unwrap();
            assert_eq!(manager.sequencer.channels[4].sound_id, SfxId::PressAB as u8);
            assert!(manager.sequencer.channels[5].active);
            assert_eq!(manager.sequencer.channels[5].sound_id, SfxId::Swap as u8);
        }
        for _ in 0..600 {
            frame(&mut game, &mut input, 0);
            if game.field_flash.is_none() {
                break;
            }
        }
        assert!(game.field_flash.is_none());
        assert_eq!(game.state.screen, GameScreen::StartMenu);
    });
}
