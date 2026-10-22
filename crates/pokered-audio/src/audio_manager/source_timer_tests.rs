//! Controlled canonical sequencer/PCM tests. No Game/IRQ/APU-write timing claim.
use super::*;
use crate::source_clock::{timer_work::TimerFrameWork, AudioAdmissionError};
use alloc::string::String;

fn powered(music: MusicId) -> AudioManager {
    let mut manager = AudioManager::new();
    manager.apu.write_register(0xff26, 0x80);
    manager.play_music(music);
    manager.update_frame();
    manager
}
fn state(manager: &AudioManager) -> String {
    format!("{:?}", manager.sequencer.channels)
}
fn pcm(manager: &mut AudioManager) -> [f32; 1470] {
    let mut buffer = [0.; 1470];
    dotzuki_audio::output::render_apu_stereo(&mut manager.apu, &mut buffer, 44100);
    buffer
}
#[test]
fn retained_instructions_commit_the_same_native_update_and_pcm_exactly_once() {
    for (music, bank) in [
        (MusicId::POKECENTER, 2),
        (MusicId::SILPH_CO, 31),
        (MusicId::DUNGEON3, 31),
    ] {
        for confirmation in [false, true] {
            let mut ordinary = powered(music);
            let mut retained = powered(music);
            let mut supported = 0;
            for frame in 0..96 {
                if confirmation && frame % 29 == 0 {
                    ordinary.play_sfx(SfxId::PressAB);
                    retained.play_sfx(SfxId::PressAB);
                }
                if let Ok(mut work) = retained.begin_source_timer_frame(bank, 0) {
                    supported += 1;
                    let frozen = state(&retained);
                    let mut accepted = 0;
                    loop {
                        let before = work.clone();
                        assert_eq!(
                            retained.step_source_timer_frame(&mut work, |_| Err("busy")),
                            Err(AudioAdmissionError::Admission("busy"))
                        );
                        assert_eq!(work, before);
                        assert_eq!(state(&retained), frozen);
                        work = serde_json::from_str::<TimerFrameWork>(
                            &serde_json::to_string(&work).unwrap(),
                        )
                        .unwrap();
                        let result = retained
                            .step_source_timer_frame(&mut work, |cycles| {
                                accepted += u32::from(cycles);
                                Ok::<(), ()>(())
                            })
                            .unwrap();
                        if let Some(done) = result {
                            assert_eq!(done.cycles, accepted);
                            for _ in 0..3 {
                                work = serde_json::from_str(&serde_json::to_string(&work).unwrap())
                                    .unwrap();
                                assert_eq!(
                                    retained
                                        .step_source_timer_frame(&mut work, |_| -> Result<(), ()> {
                                            panic!("duplicate commit")
                                        })
                                        .unwrap(),
                                    Some(done)
                                );
                            }
                            break;
                        }
                        assert_eq!(state(&retained), frozen);
                    }
                } else {
                    // Next-note paths use the unchanged ordinary owner. They
                    // are not counted as retained-instruction coverage.
                    retained.update_frame();
                }
                ordinary.update_frame();
                assert_eq!(state(&retained), state(&ordinary));
                assert_eq!(
                    retained.sequencer.music_tempo,
                    ordinary.sequencer.music_tempo
                );
                assert_eq!(pcm(&mut retained), pcm(&mut ordinary));
            }
            assert!(
                supported >= 40,
                "music {music:?}, confirm {confirmation}: {supported}"
            );
        }
    }
}

#[test]
fn changed_context_and_unsupported_inputs_fail_before_an_instruction_or_tick() {
    let mut manager = powered(MusicId::POKECENTER);
    let mut work = manager.begin_source_timer_frame(2, 0).unwrap();
    manager
        .step_source_timer_frame(&mut work, |_| Ok::<(), ()>(()))
        .unwrap();
    manager.sequencer.channels[0].delay_counter += 1; // controlled stale-owner fixture
    let frozen = state(&manager);
    let before = work.clone();
    assert!(matches!(
        manager.step_source_timer_frame(&mut work, |_| -> Result<(), ()> {
            panic!("stale source context")
        }),
        Err(AudioAdmissionError::Preflight(_))
    ));
    assert_eq!(state(&manager), frozen);
    assert_eq!(work, before);
    for (bank, mute) in [(0, 0), (8, 0), (31, 0), (2, 1)] {
        assert!(manager.begin_source_timer_frame(bank, mute).is_err());
    }
    assert_eq!(state(&manager), frozen);
    manager.music_after_sfx = Some(MusicId::SILPH_CO);
    assert!(manager.begin_source_timer_frame(2, 0).is_err());
    manager.music_after_sfx = None;
    manager.sequencer.channels[0].delay_counter = 1;
    assert!(manager.begin_source_timer_frame(2, 0).is_err());
}
