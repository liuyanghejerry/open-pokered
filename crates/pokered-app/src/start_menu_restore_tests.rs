//! Actual START cancellation retains the source input owner through map reload.
use super::*;
use pokered_core::foreground_input::{A, B, RIGHT, START};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024)
        .spawn(test).unwrap().join().unwrap();
}

fn frame(game: &mut PokemonGame, input: &mut InputState, buttons: u8) {
    input.begin_frame();
    for (bit, key) in [(A, GbButton::A), (B, GbButton::B),
        (RIGHT, GbButton::Right), (START, GbButton::Start)] {
        if buttons & bit != 0 { input.press(key); } else { input.release(key); }
    }
    game.update(input);
    if let Some(flow) = &game.field_flash {
        assert!(flow.error.is_none(), "{:?}", flow.error);
    }
}

fn fixture(map: MapId, x: u16, y: u16) -> (PokemonGame, InputState) {
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = Some(AudioOutput::new_pcm());
    game.audio.as_ref().unwrap().play_music(pokered_audio::music_data::MusicId::DUNGEON3);
    game.overworld = OverworldScreen::new(map, None, PokemonRedData);
    game.overworld.state.player.x = x;
    game.overworld.state.player.y = y;
    game.overworld.publish_npc_frame(false);
    game.handle_transition(GameScreen::StartMenu);
    game.overworld.begin_source_field();
    game.start_menu.begin_source_redisplay();
    let mut input = InputState::new();
    for _ in 0..64 { frame(&mut game, &mut input, 0); }
    assert!(game.start_menu.input_ready());
    (game, input)
}

#[test]
fn start_close_owns_resource_waits_and_discards_short_direction_pulses() {
    run(|| {
        for (map, x, y) in [(MapId::ViridianCity, 20, 30),
            (MapId::RedsHouse2F, 3, 6), (MapId::RockTunnel1F, 15, 3),
            (MapId::IndigoPlateauLobby, 7, 6)] {
            let (mut game, mut input) = fixture(map, x, y);
            let facing = game.overworld.state.player.facing;
            let darkness = game.overworld.dark_cave.map_pal_offset;
            let npcs: Vec<_> = game.overworld.npc_states.iter().map(|n| (n.x, n.y)).collect();
            frame(&mut game, &mut input, B);
            assert!(game.field_flash.is_some());
            let mut returned = false;
            for tick in 1..120 {
                frame(&mut game, &mut input, if tick == 3 || tick == 4 { RIGHT } else { 0 });
                assert_eq!((game.overworld.state.player.x, game.overworld.state.player.y), (x, y));
                assert_eq!(game.overworld.state.player.facing, facing);
                assert_eq!(game.overworld.npc_states.iter().map(|n| (n.x, n.y)).collect::<Vec<_>>(), npcs);
                assert_eq!(game.overworld.dark_cave.map_pal_offset, darkness);
                assert!(game.source_display.device().lcd_enabled());
                assert!(!game.field_flash_white_frame(), "START close has no party whiteout");
                if game.field_flash.is_none() { returned = true; break; }
                assert_eq!(game.state.screen, GameScreen::StartMenu);
            }
            assert!(returned);
            assert_eq!(game.state.screen, GameScreen::Overworld);
            for _ in 0..8 { frame(&mut game, &mut input, 0); }
            assert_eq!((game.overworld.state.player.x, game.overworld.state.player.y), (x, y));
            assert_eq!(game.overworld.state.player.facing, facing);
            // A rendered map alone does not prove that the caller returned.
            frame(&mut game, &mut input, START);
            for _ in 0..80 { frame(&mut game, &mut input, 0); }
            assert_eq!(game.state.screen, GameScreen::StartMenu);
            assert!(game.start_menu.input_ready());
        }
    });
}

#[test]
fn start_close_cached_frames_match_full_draw_until_map_control_returns() {
    run(|| {
        let (mut game, mut input) = fixture(MapId::ViridianCity, 20, 30);
        let mut session = crate::render::session::RenderSession::new();
        let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
        for tick in 0..80 {
            if tick != 0 { frame(&mut game, &mut input, if tick == 1 { B } else { 0 }); }
            session.render(&mut game, &mut cached, &mut scroll);
            game.draw(&mut full);
            assert_eq!(cached.packed(), full.packed(), "frame {tick}");
        }
        assert_eq!(game.state.screen, GameScreen::Overworld);
    });
}

#[test]
fn earliest_start_cancel_keeps_unfinished_opening_noise_without_locking() {
    run(|| {
        let (mut game, mut input) = fixture(MapId::ViridianCity, 20, 30);
        frame(&mut game, &mut input, B);
        for _ in 0..120 {
            frame(&mut game, &mut input, 0);
            if game.state.screen == GameScreen::Overworld { break; }
        }
        assert_eq!(game.state.screen, GameScreen::Overworld);
        for _ in 0..8 { frame(&mut game, &mut input, 0); }
        // Actual START, then B on the first ready input frame. Do not wait
        // for the CHAN8 opening sound or inject a replacement sound.
        for _ in 0..4 {
            frame(&mut game, &mut input, START);
            if game.state.screen == GameScreen::StartMenu { break; }
        }
        assert_eq!(game.state.screen, GameScreen::StartMenu);
        for _ in 0..64 {
            if game.start_menu.input_ready() { break; }
            frame(&mut game, &mut input, 0);
        }
        assert!(game.start_menu.input_ready());
        {
            let audio = game.audio.as_ref().unwrap().manager.lock().unwrap();
            assert!(audio.sequencer.channels[7].active);
            assert_eq!(audio.sequencer.channels[7].sound_id, SfxId::StartMenu as u8);
        }
        frame(&mut game, &mut input, B);
        for _ in 0..120 {
            frame(&mut game, &mut input, 0);
            if game.state.screen == GameScreen::Overworld { break; }
        }
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert!(game.field_flash.is_none());
        assert_eq!((game.overworld.state.player.x, game.overworld.state.player.y), (20, 30));
    });
}

#[test]
fn held_start_and_exit_a_finish_restoration_without_reopening_or_waiting_for_release() {
    run(|| {
        for key in [START, A] {
            let (mut game, mut input) = fixture(MapId::ViridianCity, 20, 30);
            if key == A {
                for _ in 0..7 {
                    if game.start_menu.current_item() == pokered_core::start_menu::StartMenuItem::Exit { break; }
                    game.start_menu.update_frame(pokered_core::start_menu::StartMenuInput {
                        down: true, ..pokered_core::start_menu::StartMenuInput::none()
                    });
                }
                assert_eq!(game.start_menu.current_item(), pokered_core::start_menu::StartMenuItem::Exit);
            }
            frame(&mut game, &mut input, key);
            assert!(game.field_flash.is_some());
            for _ in 0..120 {
                frame(&mut game, &mut input, key);
                if game.field_flash.is_none() { break; }
            }
            assert!(game.field_flash.is_none());
            assert_eq!(game.state.screen, GameScreen::Overworld);
            for _ in 0..8 { frame(&mut game, &mut input, key); }
            assert_eq!(game.state.screen, GameScreen::Overworld);
            for _ in 0..8 { frame(&mut game, &mut input, 0); }
            for _ in 0..4 {
                frame(&mut game, &mut input, START);
                if game.state.screen == GameScreen::StartMenu { break; }
            }
            assert_eq!(game.state.screen, GameScreen::StartMenu);
        }
    });
}
