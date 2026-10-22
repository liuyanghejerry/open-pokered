//! Controlled shared-manager/PCM checks; not native gameplay or IRQ timing.
use super::*;
use crate::source_clock::{AudioAdmissionError, AudioClockError};
fn powered() -> AudioManager {
    let mut manager = AudioManager::new();
    manager.apu.write_register(0xff26, 0x80);
    manager
}
fn compare(a: &mut AudioManager, b: &mut AudioManager) {
    assert_eq!(a.last_music_id(), b.last_music_id());
    assert_eq!(a.is_fading(), b.is_fading());
    assert_eq!(a.low_health_alarm_active(), b.low_health_alarm_active());
    assert_eq!(a.nr50(), b.nr50());
    assert_eq!(a.sequencer.music_tempo, b.sequencer.music_tempo);
    assert_eq!(a.sequencer.sfx_tempo, b.sequencer.sfx_tempo);
    for channel in 0..8 {
        assert_eq!(
            format!("{:?}", a.sequencer.channels[channel]),
            format!("{:?}", b.sequencer.channels[channel])
        );
    }
    let mut left = [0f32; 1470];
    let mut right = [0f32; 1470];
    dotzuki_audio::output::render_apu_stereo(&mut a.apu, &mut left, 44100);
    dotzuki_audio::output::render_apu_stereo(&mut b.apu, &mut right, 44100);
    assert_eq!(
        left, right,
        "admission must preserve real PCM, not only delay counters"
    );
}
#[test]
fn denied_frames_then_one_admitted_tick_preserve_songs_confirmation_and_pcm() {
    for music in [MusicId::POKECENTER, MusicId::SILPH_CO, MusicId::DUNGEON3] {
        for confirmation in [false, true] {
            let mut ordinary = powered();
            let mut guarded = powered();
            for m in [&mut ordinary, &mut guarded] {
                m.play_music(music);
                m.update_frame();
            }
            for frame in 1..=96 {
                if confirmation && frame % 37 == 0 {
                    ordinary.play_sfx(SfxId::PressAB);
                    guarded.play_sfx(SfxId::PressAB);
                }
                for refusal in 0..2 {
                    let mut calls = 0;
                    let result = guarded.try_update_frame_with_source_work(|work| {
                        calls += 1;
                        assert!(work.cycles > 0);
                        Err::<(), _>(refusal)
                    });
                    assert_eq!(calls, 1);
                    assert_eq!(result, Err(AudioAdmissionError::Admission(refusal)));
                    compare(&mut ordinary, &mut guarded);
                }
                let mut seen = None;
                let actual = guarded
                    .try_update_frame_with_source_work(|work| {
                        assert!(seen.is_none());
                        seen = Some(work);
                        Ok::<(), ()>(())
                    })
                    .unwrap();
                assert_eq!(seen, Some(actual));
                ordinary.update_frame();
                compare(&mut ordinary, &mut guarded);
            }
        }
    }
}
#[test]
fn unsupported_preflight_does_not_call_admission_or_advance_any_native_audio() {
    for case in 0..6 {
        let mut ordinary = powered();
        let mut guarded = powered();
        for m in [&mut ordinary, &mut guarded] {
            m.play_music(if case == 0 {
                MusicId::PALLET_TOWN
            } else {
                MusicId::POKECENTER
            });
            if case != 1 {
                m.update_frame();
            }
            match case {
                2 => m.play_sfx(SfxId::Pokeflute),
                3 => m.fade_out(10),
                4 => m.set_low_health_alarm(true),
                5 => m.play_cry(SfxId::Cry00, 0, 0x80),
                _ => (),
            }
        }
        for _ in 0..3 {
            let result = guarded.try_update_frame_with_source_work(|_| -> Result<(), ()> {
                panic!("unsupported input cannot request admission")
            });
            assert!(matches!(result, Err(AudioAdmissionError::Preflight(_))));
            compare(&mut ordinary, &mut guarded);
        }
        // Later ordinary updates also match, so rejected preflight did not
        // silently consume a fade countdown or deferred sound transition.
        for _ in 0..20 {
            ordinary.update_frame();
            guarded.update_frame();
            compare(&mut ordinary, &mut guarded);
        }
    }
}
#[test]
fn postcondition_error_is_reported_as_advanced_and_the_tick_is_not_rolled_back() {
    let mut ordinary = powered();
    let mut guarded = powered();
    for m in [&mut ordinary, &mut guarded] {
        m.play_music(MusicId::POKECENTER);
        m.update_frame();
        // Controlled pending-resume conflict, deliberately outside supported
        // source state. The ordinary update changes music after its one tick.
        m.music_after_sfx = Some(MusicId::SILPH_CO);
    }
    let mut calls = 0;
    let result = guarded.try_update_frame_with_source_work(|_| {
        calls += 1;
        Ok::<(), ()>(())
    });
    assert!(matches!(
        result,
        Err(AudioAdmissionError::Advanced(
            AudioClockError::NativeStateMismatch(_)
        ))
    ));
    assert_eq!(calls, 1);
    ordinary.update_frame();
    compare(&mut ordinary, &mut guarded);
    assert_eq!(guarded.last_music_id(), Some(MusicId::SILPH_CO));
}
