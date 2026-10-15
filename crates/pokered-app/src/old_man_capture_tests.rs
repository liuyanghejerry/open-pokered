use super::*;
use pokered_core::battle::{BattlePhase, IntroPhase};
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::species::Species;

#[test]
fn old_man_ball_finishes_before_all_three_jingle_channels_and_field_return() {
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red, None, None, None, false, None, false, true,
        #[cfg(feature = "debug-server")]
        None,
    );
    game.state.config.language = pokered_core::game_state::Lang::En;
    let player = create_pokemon(Species::Bulbasaur, 6, [0x9a, 0x78]).unwrap();
    let enemy = create_pokemon(Species::Weedle, 5, [0x9a, 0x78]).unwrap();
    game.save_data.party.clear();
    game.save_data.party.add(player).unwrap();
    let protected = serde_json::to_value(&game.save_data.party).unwrap();
    game.battle = BattleScreen::from_parties(true, &game.save_data.party.to_vec(), &[enemy], None);
    game.battle.is_old_man = true;
    game.battle.phase = BattlePhase::Intro { phase: IntroPhase::WildReveal, wait_frames: 0 };
    game.state.screen = GameScreen::Battle;
    game.audio = Some(AudioOutput::new_pcm());
    let mut framebuffer = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut seen = false;
    let mut finished = false;
    let mut active_frames = [0usize; 3];
    let mut pcm = vec![0.0f32; 1470];
    let mut audible_jingle = false;
    for frame in 0..2400 {
        let mut input = InputState::new();
        if frame % 8 == 0 { input.press(GbButton::A); }
        game.update(&input);
        game.draw(&mut framebuffer);
        assert!(!matches!(game.battle.phase, BattlePhase::Intro { phase: IntroPhase::PlayerSendOut, .. }));
        let channels = {
            let audio = game.audio.as_ref().unwrap();
            let manager = audio.manager.lock().unwrap();
            core::array::from_fn::<_, 3, _>(|i| {
                let channel = &manager.sequencer.channels[4 + i];
                (channel.active, channel.sound_id)
            })
        };
        game.audio.as_ref().unwrap().render_pcm(&mut pcm);
        if !seen && channels.iter().any(|&(active, id)| active && id == SfxId::CaughtMon as u8) {
            assert!(game.battle_vfx.item_animation_capture_finished());
            assert_eq!(channels, [(true, SfxId::CaughtMon as u8); 3]);
            seen = true;
        }
        if seen && !finished {
            audible_jingle |= pcm.iter().any(|sample| sample.abs() > 0.00001);
            for (i, &(active, id)) in channels.iter().enumerate() {
                if active {
                    assert_eq!(id, SfxId::CaughtMon as u8, "jingle channel {i} was replaced");
                    active_frames[i] += 1;
                }
            }
            if channels.iter().all(|&(active, _)| !active) {
                finished = true;
            } else {
                assert_eq!(game.state.screen, GameScreen::Battle);
                assert!(game.battle.current_message.as_deref().unwrap_or_default().contains("was caught!"));
            }
        }
        if game.state.screen != GameScreen::Battle {
            assert!(seen && finished, "demo returned before the jingle finished");
            break;
        }
    }
    assert!(seen && finished);
    assert!(audible_jingle);
    assert!(active_frames.iter().all(|&frames| frames > 4), "all voices must survive the old four-frame cutoff");
    assert_eq!(serde_json::to_value(&game.save_data.party).unwrap(), protected);
    assert!(game.post_capture_flow.is_none());
    assert!(game.battle.captured_mon.is_none());
}
