//! Real SAVE/NO/B inputs return through HoldTextDisplayOpen to the field.
use super::*;
use pokered_core::foreground_input::{A, B, RIGHT};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024)
        .spawn(test).unwrap().join().unwrap();
}

fn frame(game: &mut PokemonGame, input: &mut InputState, keys: u8) {
    input.begin_frame();
    input.set_from_bitmask(keys);
    game.update(input);
    if let Some(flow) = &game.field_flash { assert!(flow.error.is_none(), "{:?}", flow.error); }
}

#[test]
fn save_held_a_accepts_soft_reset_without_an_extra_save() {
    run(|| {
        for save in [false, true] {
            let (mut game, mut input, path) = fixture();
            if !save { game.save_menu.cursor = pokered_core::save_menu::YesNoChoice::No; }
            frame(&mut game, &mut input, A);
            for _ in 0..1000 {
                frame(&mut game, &mut input, A);
                if game.field_flash.as_ref().is_some_and(|flow|
                    flow.work.operation() == Some(&RestoreOperation::WaitForARelease)) { break; }
            }
            assert_eq!(game.field_flash.as_ref().unwrap().work.operation(),
                Some(&RestoreOperation::WaitForARelease));
            let saved = std::fs::read(&path).ok();
            assert_eq!(saved.is_some(), save);

            // _Joypad compares the complete raw byte, before any masks.
            // An extra direction must not enter TrySoftReset.
            for _ in 0..64 { frame(&mut game, &mut input, 0x0f | RIGHT); }
            assert_eq!(game.state.screen, GameScreen::SaveMenu);
            for _ in 0..64 {
                frame(&mut game, &mut input, 0x0f);
                if game.state.screen == GameScreen::TitleScreen { break; }
            }
            assert_eq!(game.state.screen, GameScreen::TitleScreen,
                "SAVE's held-A foreground must still execute TrySoftReset");
            assert!(game.field_flash.is_none());
            assert_eq!(std::fs::read(&path).ok(), saved, "reset must not save again");
            if save { std::fs::remove_file(path).unwrap(); }
        }
    });
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
fn save_release_waits_for_the_irq_to_sample_physical_buttons() {
    run(|| {
        let (mut game, mut input, path) = fixture();
        game.save_menu.cursor = pokered_core::save_menu::YesNoChoice::No;
        frame(&mut game, &mut input, A);
        for _ in 0..100 { frame(&mut game, &mut input, A); }
        assert_eq!(game.field_flash.as_ref().unwrap().work.operation(),
            Some(&RestoreOperation::WaitForARelease));
        assert_eq!(game.source_display.source_joy_input(), A);
        assert_eq!(game.overworld.source_foreground().unwrap().raw(), A);
        assert_eq!(game.source_display.remaining(), 0);

        // With the current grant exhausted, no ReadJoypad instruction can
        // sample a newly released button. The held gate must retain the
        // last IRQ sample until the shared clock grants more execution.
        input.begin_frame();
        input.release(GbButton::A);
        game.update_field_flash(&input);
        assert_eq!(game.source_display.source_joy_input(), A);
        assert_eq!(game.overworld.source_foreground().unwrap().raw(), A);
        assert_eq!(game.field_flash.as_ref().unwrap().work.operation(),
            Some(&RestoreOperation::WaitForARelease));

        for _ in 0..100 {
            frame(&mut game, &mut input, 0);
            if game.state.screen == GameScreen::Overworld { break; }
        }
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert_eq!(game.source_display.source_joy_input(), 0);
        assert!(!path.exists());
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
