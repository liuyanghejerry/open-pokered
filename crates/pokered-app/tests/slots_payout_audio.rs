//! Controlled payout fixture driven through the shared Game update/audio path.
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::PokemonGame;
use pokered_audio::{music_data::MusicId, output::AudioOutput, sfx_data::SfxId};
use pokered_core::{
    game_state::{GameScreen, Lang},
    slots_screen::{PayoutStage, SlotsPhase, SlotsScreen},
};
use pokered_data::wild_data::GameVersion;
use pokered_renderer::{
    input::{GbButton, InputState},
    FrameBuffer, Rgba,
};

#[test]
fn shared_game_waits_for_sound_freezes_music_and_resumes_after_payout() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
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
            game.audio = Some(AudioOutput::new_pcm());
            game.state.screen = GameScreen::Slots;
            game.state.config.language = Lang::En;
            game.slots_screen = Some(SlotsScreen::new(false, 100, 42));
            game.audio
                .as_ref()
                .unwrap()
                .play_music(MusicId::GAME_CORNER);
            for _ in 0..60 {
                game.update(&InputState::new());
            }
            let slots = game.slots_screen.as_mut().unwrap();
            slots.phase = SlotsPhase::Payout;
            slots.payout_stage = PayoutStage::WaitPress;
            slots.last_payout = 8;
            slots.payout_remaining = 8;
            slots.message = "CHERRY lined up! Scored 8 coins!".into();
            // Purchase is a known17-frame SFX; it is still live at confirmation.
            game.audio.as_ref().unwrap().play_sfx(SfxId::Purchase);
            let mut confirm = InputState::new();
            confirm.press(GbButton::A);
            game.update(&confirm);
            let music = |game: &PokemonGame| {
                format!(
                    "{:?}",
                    &game
                        .audio
                        .as_ref()
                        .unwrap()
                        .manager
                        .lock()
                        .unwrap()
                        .sequencer
                        .channels[..4]
                )
            };
            let frozen = music(&game);
            for _ in 0..8 {
                game.update(&InputState::new());
            }
            if let Ok(dir) = std::env::var("SLOTS_CAPTURE_DIR") {
                std::fs::create_dir_all(&dir).unwrap();
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                game.draw(&mut fb);
                image::RgbaImage::from_fn(160, 144, |x, y| {
                    image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
                })
                .save(format!("{dir}/payout-pending-sfx.png"))
                .unwrap();
            }
            // Use the exact same input/fixture on the base for the before capture.
            if std::env::var_os("SLOTS_BASELINE_CAPTURE").is_some() {
                return;
            }
            assert_eq!(game.slots_screen.as_ref().unwrap().coins, 100);
            assert_eq!(music(&game), frozen);
            let mut complete = false;
            for _ in 0..200 {
                game.update(&InputState::new());
                let slots = game.slots_screen.as_ref().unwrap();
                if slots.phase == SlotsPhase::Result {
                    complete = true;
                    break;
                }
                assert_eq!(music(&game), frozen);
            }
            assert!(complete);
            assert_eq!(game.save_data.game_data.player_coins, 108);
            for _ in 0..60 {
                game.update(&InputState::new());
            }
            assert_ne!(music(&game), frozen);
            assert_eq!(
                game.audio.as_ref().unwrap().last_music_id(),
                Some(MusicId::GAME_CORNER)
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
