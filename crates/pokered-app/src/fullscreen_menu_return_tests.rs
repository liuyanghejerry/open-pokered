//! Full-screen menu cancellation owns the whiteout/LCD/resource continuation.
use super::*;
use pokered_core::foreground_input::{B, DOWN};

fn frame(game: &mut PokemonGame, input: &mut InputState, keys: u8) {
    input.begin_frame();
    for (bit, key) in [(B, GbButton::B), (DOWN, GbButton::Down)] {
        if keys & bit != 0 { input.press(key); } else { input.release(key); }
    }
    game.update(input);
    if let Some(flow) = &game.field_flash { assert!(flow.error.is_none(), "{:?}", flow.error); }
}

#[test]
fn full_screen_returns_restore_map_resources_before_accepting_start_input() {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
        for map in [MapId::ViridianCity, MapId::RedsHouse1F, MapId::UndergroundPathNorthSouth,
            MapId::IndigoPlateauLobby] {
            for screen in [GameScreen::Pokedex, GameScreen::TrainerCard] {
                let mut game = PokemonGame::new(GameVersion::Red);
                game.audio = Some(AudioOutput::new_pcm());
                game.audio.as_ref().unwrap().play_music(pokered_audio::music_data::MusicId::PALLET_TOWN);
                game.overworld = OverworldScreen::new(map, None, PokemonRedData);
                game.overworld.state.player.x = 4;
                game.overworld.state.player.y = 4;
                game.overworld.publish_npc_frame(false);
                game.handle_transition(GameScreen::StartMenu);
                game.handle_transition(screen.clone());
                let mut input = InputState::new();
                for _ in 0..40 { frame(&mut game, &mut input, 0); }
                let pose = (game.overworld.state.player.x, game.overworld.state.player.y,
                    game.overworld.state.player.facing);
                let cursor = game.start_menu.cursor();
                let mut saw_white = false;
                let mut saw_lcd_off = false;
                let mut saw_tileset_copy = false;
                let mut session = crate::render::session::RenderSession::new();
                let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                for tick in 0..100 {
                    frame(&mut game, &mut input, if tick == 0 { B }
                        else if tick == 2 || tick == 3 { DOWN } else { 0 });
                    if tick == 0 { assert!(game.field_flash.is_some()); }
                    if let Some(flow) = &game.field_flash {
                        assert_eq!(game.state.screen, screen);
                        saw_white |= game.field_flash_is_white();
                        saw_lcd_off |= !game.source_display.device().lcd_enabled();
                        saw_tileset_copy |= matches!(flow.work.operation(), Some(RestoreOperation::BulkPatternCopy(
                            pokered_core::overworld::field_restore::PatternCopy {
                                source: pokered_core::overworld::field_restore::PatternSource::Tileset(_), .. })));
                    }
                    session.render(&mut game, &mut cached, &mut |_, _, _, _, _, _| {});
                    game.draw(&mut full);
                    assert_eq!(full.packed(), cached.packed(), "{map:?} {screen:?} frame {tick}");
                    assert_eq!((game.overworld.state.player.x, game.overworld.state.player.y,
                        game.overworld.state.player.facing), pose);
                }
                assert!(saw_white && saw_lcd_off && saw_tileset_copy, "{map:?} {screen:?}");
                assert!(game.field_flash.is_none());
                assert!(game.source_display.device().lcd_enabled());
                assert_eq!(game.state.screen, GameScreen::StartMenu);
                assert!(game.start_menu.input_ready());
                assert_eq!(game.start_menu.cursor(), cursor, "short pulse during restoration");
                frame(&mut game, &mut input, DOWN);
                assert_ne!(game.start_menu.cursor(), cursor);
            }
        }
    }).unwrap().join().unwrap();
}
