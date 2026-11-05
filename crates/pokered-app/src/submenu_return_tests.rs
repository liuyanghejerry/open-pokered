//! Bag and in-game Options return through their actual resource wait.
use super::*;
use pokered_core::foreground_input::{A, B, DOWN, RIGHT};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn frame(game: &mut PokemonGame, input: &mut InputState, keys: u8) {
    input.begin_frame();
    for (bit, key) in [
        (A, GbButton::A),
        (B, GbButton::B),
        (DOWN, GbButton::Down),
        (RIGHT, GbButton::Right),
    ] {
        if keys & bit != 0 {
            input.press(key);
        } else {
            input.release(key);
        }
    }
    game.update(input);
    if let Some(flow) = &game.field_flash {
        assert!(flow.error.is_none(), "{:?}", flow.error);
    }
}

fn fixture(map: MapId, screen: GameScreen) -> (PokemonGame, InputState) {
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = Some(AudioOutput::new_pcm());
    game.audio
        .as_ref()
        .unwrap()
        .play_music(pokered_audio::music_data::MusicId::PALLET_TOWN);
    game.overworld = OverworldScreen::new(map, None, PokemonRedData);
    game.overworld.state.player.x = 7;
    game.overworld.state.player.y = 6;
    game.overworld.publish_npc_frame(false);
    game.save_data
        .game_data
        .bag
        .add_item(pokered_data::items::ItemId::Potion, 3)
        .unwrap();
    game.handle_transition(GameScreen::StartMenu);
    game.handle_transition(screen);
    let mut input = InputState::new();
    for _ in 0..32 {
        frame(&mut game, &mut input, 0);
    }
    (game, input)
}

#[test]
fn submenu_cancel_reloads_patterns_and_rejects_input_until_start_returns() {
    run(|| {
        for map in [MapId::ViridianCity, MapId::IndigoPlateauLobby] {
            for screen in [GameScreen::Bag, GameScreen::OptionsMenu] {
                let (mut game, mut input) = fixture(map, screen.clone());
                let player = (
                    game.overworld.state.player.x,
                    game.overworld.state.player.y,
                    game.overworld.state.player.facing,
                );
                let bag = game.save_data.game_data.bag.items().to_vec();
                let cursor = game.start_menu.cursor();
                let mut before = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                game.draw(&mut before);
                frame(&mut game, &mut input, B);
                assert!(game.field_flash.is_some());
                assert_eq!(game.state.screen, screen);
                let mut saw_copy = false;
                for tick in 1..80 {
                    frame(
                        &mut game,
                        &mut input,
                        if tick == 1 || tick == 2 { DOWN } else { 0 },
                    );
                    if let Some(flow) = &game.field_flash {
                        saw_copy |= flow.irq.as_ref().is_some_and(|irq| irq.copy.is_some());
                        let mut during = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                        game.draw(&mut during);
                        assert_eq!(during.packed(), before.packed(),
                            "the old UI remains visible while the textbox transfer is pending");
                    }
                    assert_eq!(
                        (
                            game.overworld.state.player.x,
                            game.overworld.state.player.y,
                            game.overworld.state.player.facing
                        ),
                        player
                    );
                    assert_eq!(game.save_data.game_data.bag.items(), bag);
                    assert!(game.source_display.device().lcd_enabled());
                    assert!(!game.field_flash_white_frame());
                }
                assert!(
                    saw_copy,
                    "must actually service the textbox resource transfer"
                );
                assert!(game.field_flash.is_none());
                assert_eq!(game.state.screen, GameScreen::StartMenu);
                assert!(game.start_menu.input_ready());
                assert_eq!(
                    game.start_menu.cursor(),
                    cursor,
                    "short pulse cannot leak through restoration"
                );
                frame(&mut game, &mut input, DOWN);
                assert_ne!(
                    game.start_menu.cursor(),
                    cursor,
                    "START must accept a later fresh press"
                );
            }
        }
    });
}

#[test]
fn submenu_restoration_cached_frames_match_full_render_and_keep_options() {
    run(|| {
        for screen in [GameScreen::Bag, GameScreen::OptionsMenu] {
            let (mut game, mut input) = fixture(MapId::ViridianCity, screen.clone());
            if screen == GameScreen::OptionsMenu {
                frame(&mut game, &mut input, RIGHT);
                frame(&mut game, &mut input, 0);
            }
            let options = game.options_menu.options;
            let mut session = crate::render::session::RenderSession::new();
            let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            for tick in 0..80 {
                frame(&mut game, &mut input, if tick == 0 { B } else { 0 });
                session.render(&mut game, &mut cached, &mut |_, _, _, _, _, _| {});
                game.draw(&mut full);
                assert_eq!(
                    cached.packed(),
                    full.packed(),
                    "{screen:?} return frame {tick}"
                );
            }
            assert_eq!(game.state.screen, GameScreen::StartMenu);
            if screen == GameScreen::OptionsMenu {
                assert_eq!(game.save_data.game_data.options, options);
            }
        }
    });
}

#[test]
fn title_options_still_returns_to_main_menu_without_field_restoration() {
    run(|| {
        let (mut game, mut input) = fixture(MapId::ViridianCity, GameScreen::OptionsMenu);
        game.main_menu.last_choice = Some(pokered_core::game_state::MainMenuChoice::Option);
        frame(&mut game, &mut input, B);
        assert_eq!(game.state.screen, GameScreen::MainMenu);
        assert!(game.field_flash.is_none());
        let audio = game.audio.as_ref().unwrap().manager.lock().unwrap();
        assert_eq!(audio.sequencer.current_sfx_id, SfxId::PressAB as u8);
        assert!(audio.is_sfx_playing());
    });
}
