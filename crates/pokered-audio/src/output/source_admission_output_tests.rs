use super::*;
use crate::source_clock::{AudioAdmissionError, AudioClockError};
#[test]
fn hosted_admission_uses_one_existing_manager_and_preserves_pcm() {
    for music in [MusicId::POKECENTER, MusicId::PALLET_TOWN] {
        let ordinary = AudioOutput::new_pcm();
        let guarded = AudioOutput::new_pcm();
        for output in [&ordinary, &guarded] {
            output.play_music(music);
            output.update_frame();
        }
        let mut a = [0f32; 1470];
        let mut b = [0f32; 1470];
        for frame in 0..40 {
            let mut called = false;
            let result = guarded.try_update_frame_with_source_work(|work| {
                called = true;
                assert!(work.cycles > 0);
                Err::<(), _>(frame)
            });
            if music == MusicId::POKECENTER {
                assert!(called);
                assert_eq!(result, Err(AudioAdmissionError::Admission(frame)));
            } else {
                assert!(!called);
                assert_eq!(
                    result,
                    Err(AudioAdmissionError::Preflight(
                        AudioClockError::UnsupportedSoundBank
                    ))
                );
            }
            ordinary.render_pcm(&mut a);
            guarded.render_pcm(&mut b);
            assert_eq!(a, b);
            if music == MusicId::POKECENTER {
                ordinary.update_frame();
                guarded
                    .try_update_frame_with_source_work(|_| Ok::<(), ()>(()))
                    .unwrap();
            }
            ordinary.render_pcm(&mut a);
            guarded.render_pcm(&mut b);
            assert_eq!(a, b);
            let expected = ordinary.manager.lock().unwrap();
            let actual = guarded.manager.lock().unwrap();
            for channel in 0..8 {
                assert_eq!(
                    format!("{:?}", expected.sequencer.channels[channel]),
                    format!("{:?}", actual.sequencer.channels[channel])
                );
            }
        }
    }
}
