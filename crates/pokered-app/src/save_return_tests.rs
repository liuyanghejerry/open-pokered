//! Real SAVE/NO/B inputs return through HoldTextDisplayOpen to the field.
use super::*;
use pokered_core::foreground_input::{A, B, RIGHT};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024)
        .spawn(test).unwrap().join().unwrap();
}

fn frame(game: &mut PokemonGame, input: &mut InputState, keys: u8) {
    input.begin_frame();
    for (bit, key) in [(A, GbButton::A), (B, GbButton::B), (RIGHT, GbButton::Right)] {
        if keys & bit != 0 { input.press(key); } else { input.release(key); }
    }
    game.update(input);
    if let Some(flow) = &game.field_flash { assert!(flow.error.is_none(), "{:?}", flow.error); }
}

fn fixture() -> (PokemonGame, InputState, std::path::PathBuf) {
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = Some(AudioOutput::new_pcm());
    game.audio.as_ref().unwrap().play_music(pokered_audio::music_data::MusicId::PALLET_TOWN);
    game.overworld = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
    game.overworld.state.player.x = 20;
    game.overworld.state.player.y = 30;
    game.overworld.publish_npc_frame(false);
    game.handle_transition(GameScreen::StartMenu);
    game.handle_transition(GameScreen::SaveMenu);
    let path = std::env::temp_dir().join(format!("pokered-save-return-{}-{}.sav", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    assert!(!path.exists());
    game.save_path = Some(path.clone());
    let mut input = InputState::new();
    for _ in 0..32 { frame(&mut game, &mut input, 0); }
    (game, input, path)
}

#[test]
fn save_cancel_keeps_choice_delay_and_does_not_write_or_return_to_start() {
    run(|| {
        let (mut game, mut input, path) = fixture();
        let facing = game.overworld.state.player.facing;
        frame(&mut game, &mut input, B);
        assert!(game.field_flash.is_some());
        for tick in 1..160 {
            frame(&mut game, &mut input, if tick == 3 || tick == 4 { RIGHT } else { 0 });
            assert!(!path.exists(), "cancel must never save");
            assert_eq!((game.overworld.state.player.x, game.overworld.state.player.y), (20, 30));
            assert_eq!(game.overworld.state.player.facing, facing);
            assert_ne!(game.state.screen, GameScreen::StartMenu);
            if tick < 15 { assert_eq!(game.state.screen, GameScreen::SaveMenu); }
            if game.state.screen == GameScreen::Overworld { break; }
        }
        assert_eq!(game.state.screen, GameScreen::Overworld);
    });
}

#[test]
fn save_no_waits_for_a_release_while_irq_and_music_keep_running() {
    run(|| {
        let (mut game, mut input, path) = fixture();
        game.save_menu.cursor = pokered_core::save_menu::YesNoChoice::No;
        frame(&mut game, &mut input, A);
        for _ in 0..80 { frame(&mut game, &mut input, A); }
        let flow = game.field_flash.as_ref().expect("held A still owns the text display");
        assert_eq!(flow.work.operation(), Some(&RestoreOperation::WaitForARelease));
        let before = flow.irq.as_ref().unwrap().completed;
        for _ in 0..20 { frame(&mut game, &mut input, A); }
        assert!(game.field_flash.as_ref().unwrap().irq.as_ref().unwrap().completed > before);
        assert!(!path.exists());
        for _ in 0..100 {
            frame(&mut game, &mut input, 0);
            if game.state.screen == GameScreen::Overworld { break; }
        }
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert_eq!((game.overworld.state.player.x, game.overworld.state.player.y), (20, 30));
    });
}

#[test]
fn actual_save_writes_before_release_and_finishes_on_the_map() {
    run(|| {
        let (mut game, mut input, path) = fixture();
        frame(&mut game, &mut input, A);
        for _ in 0..1000 {
            frame(&mut game, &mut input, A);
            if game.field_flash.is_some() { break; }
        }
        assert!(path.exists());
        assert_eq!(game.state.screen, GameScreen::SaveMenu);
        for _ in 0..40 { frame(&mut game, &mut input, A); }
        assert_eq!(game.state.screen, GameScreen::SaveMenu);
        let bytes = std::fs::read(&path).unwrap();
        for _ in 0..100 {
            frame(&mut game, &mut input, 0);
            if game.state.screen == GameScreen::Overworld { break; }
        }
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "restoration must not save twice");
        std::fs::remove_file(path).unwrap();
    });
}
