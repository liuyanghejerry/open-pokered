//! Controlled one-owner state/PCM checks, not Game/IRQ/APU timestamp evidence.
use super::*;
use crate::source_clock::{
    frame_instructions::{FrameStepError, SourceAudioFrame},
    AudioAdmissionError, AudioClockError,
};
use alloc::string::String;
use alloc::vec::Vec;
fn powered(music: MusicId) -> AudioManager {
    let mut m = AudioManager::new();
    m.apu.write_register(0xff26, 0x80);
    m.play_music(music);
    m.update_frame();
    m
}
fn state(m: &AudioManager) -> String {
    format!("{:?}", m.sequencer.channels)
}
fn pcm(m: &mut AudioManager) -> [f32; 1470] {
    let mut out = [0.; 1470];
    dotzuki_audio::output::render_apu_stereo(&mut m.apu, &mut out, 44100);
    out
}
#[test]
fn complete_frames_pause_resume_and_commit_one_existing_native_tick_with_identical_pcm() {
    for (music, bank) in [
        (MusicId::POKECENTER, 2),
        (MusicId::SILPH_CO, 31),
        (MusicId::DUNGEON3, 31),
    ] {
        for confirmation in [false, true] {
            let mut ordinary = powered(music);
            let mut retained = powered(music);
            let mut total_instructions = 0;
            for frame in 0..256 {
                if confirmation && frame % 37 == 0 {
                    ordinary.play_sfx(SfxId::PressAB);
                    retained.play_sfx(SfxId::PressAB);
                }
                let mut work = retained
                    .begin_source_audio_frame(bank, 0)
                    .unwrap_or_else(|e| {
                        panic!("{music:?} confirm{confirmation} frame{frame}: {e:?}")
                    });
                let frozen = state(&retained);
                let mut cycles = 0;
                let mut turn = 0;
                loop {
                    let before = work.clone();
                    assert_eq!(
                        retained.resume_source_audio_frame(&mut work, |_| Err("blocked")),
                        Err(AudioAdmissionError::Admission(FrameStepError::Clock(
                            "blocked"
                        )))
                    );
                    assert_eq!(work, before);
                    assert_eq!(state(&retained), frozen);
                    work = serde_json::from_str::<SourceAudioFrame>(
                        &serde_json::to_string(&work).unwrap(),
                    )
                    .unwrap();
                    let allowance = [1, 17, 31][turn % 3];
                    turn += 1;
                    let mut accepted = 0;
                    let result = retained.resume_source_audio_frame(&mut work, |t| {
                        if accepted == allowance {
                            Err("yield")
                        } else {
                            accepted += 1;
                            cycles += u32::from(t);
                            Ok(())
                        }
                    });
                    total_instructions += accepted;
                    match result {
                        Ok(done) => {
                            assert_eq!(done.cycles, cycles);
                            for _ in 0..3 {
                                work = serde_json::from_str(&serde_json::to_string(&work).unwrap())
                                    .unwrap();
                                assert_eq!(
                                    retained
                                        .resume_source_audio_frame(
                                            &mut work,
                                            |_| -> Result<(), ()> {
                                                panic!("completed frame updates native audio twice")
                                            }
                                        )
                                        .unwrap(),
                                    done
                                );
                            }
                            break;
                        }
                        Err(AudioAdmissionError::Admission(FrameStepError::Clock("yield"))) => {
                            assert_eq!(state(&retained), frozen)
                        }
                        e => panic!(
                            "{music:?} confirm{confirmation} frame{frame} cycles{cycles}: {e:?}"
                        ),
                    }
                }
                ordinary.update_frame();
                assert_eq!(state(&retained), state(&ordinary));
                assert_eq!(
                    retained.sequencer.music_tempo,
                    ordinary.sequencer.music_tempo
                );
                assert_eq!(retained.sequencer.sfx_tempo, ordinary.sequencer.sfx_tempo);
                assert_eq!(pcm(&mut retained), pcm(&mut ordinary));
            }
            assert!(total_instructions > 10000);
        }
    }
}
#[test]
fn changed_native_program_and_unsupported_source_context_reject_before_clock_or_tick() {
    let mut m = powered(MusicId::POKECENTER);
    let mut work = m.begin_source_audio_frame(2, 0).unwrap();
    let mut accepted = 0;
    assert!(matches!(
        m.resume_source_audio_frame(&mut work, |_| {
            if accepted == 3 {
                Err("pause")
            } else {
                accepted += 1;
                Ok(())
            }
        }),
        Err(AudioAdmissionError::Admission(_))
    ));
    let mut before = work.clone();
    let original = m.sequencer.channels[0].data[0];
    m.sequencer.channels[0].data[0] ^= 1;
    let native = state(&m);
    assert!(matches!(
        m.resume_source_audio_frame(&mut work, |_| -> Result<(), ()> {
            panic!("stale native stream")
        }),
        Err(AudioAdmissionError::SourceFailed(
            AudioClockError::NativeStateMismatch(_)
        ))
    ));
    before.source_failed = Some(AudioClockError::NativeStateMismatch(0));
    assert_eq!(work, before);
    assert_eq!(state(&m), native);
    m.sequencer.channels[0].data[0] = original;
    assert!(matches!(
        m.resume_source_audio_frame(&mut work, |_| -> Result<(), ()> {
            panic!("terminal failure must not resume after restoring the stream")
        }),
        Err(AudioAdmissionError::SourceFailed(
            AudioClockError::NativeStateMismatch(_)
        ))
    ));
    for (bank, mute) in [(0, 0), (8, 0), (31, 0), (2, 1)] {
        assert!(m.begin_source_audio_frame(bank, mute).is_err());
    }
    let mut unprimed = AudioManager::new();
    unprimed.play_music(MusicId::POKECENTER);
    assert!(unprimed.begin_source_audio_frame(2, 0).is_err());
    m.music_after_sfx = Some(MusicId::SILPH_CO);
    assert!(m.begin_source_audio_frame(2, 0).is_err());
}

#[derive(serde::Deserialize)]
struct OriginalFrames {
    cases: Vec<OriginalFrame>,
}
#[derive(serde::Deserialize)]
struct OriginalFrame {
    song: crate::source_clock::frame_instructions::SourceSong,
    confirmation: bool,
    frame: usize,
    cycles: u32,
    trace: Vec<(u16, u8)>,
}
#[test]
fn whole_native_frame_cursor_matches_every_original_rom_instruction() {
    let fixtures: OriginalFrames =
        serde_json::from_str(include_str!("../source_clock/frame_work_golden.json")).unwrap();
    assert_eq!(fixtures.cases.len(), 311);
    let mut instructions = 0;
    for case in fixtures.cases {
        let mut m = powered(case.song.music());
        for frame in 0..=case.frame {
            if case.confirmation && frame % 37 == 0 {
                m.play_sfx(SfxId::PressAB);
            }
            if frame < case.frame {
                m.update_frame();
            }
        }
        let mut work = m.begin_source_audio_frame(case.song.bank(), 0).unwrap();
        for (index, (pc, expected)) in case.trace.iter().enumerate() {
            let before = work.clone();
            assert!(matches!(
                work.try_step(&m.sequencer, |_| Err("denied")),
                Err(FrameStepError::Clock("denied"))
            ));
            assert_eq!(work, before);
            work = serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
            let mut actual = None;
            let finished = work
                .try_step(&m.sequencer, |t| {
                    assert!(actual.replace(t).is_none(), "more than one instruction");
                    Ok::<(), ()>(())
                })
                .unwrap();
            assert_eq!(
                actual,
                Some(*expected),
                "{:?} confirmation{} frame{} instruction{index} PC{pc:04x}",
                case.song,
                case.confirmation,
                case.frame
            );
            assert_eq!(finished, index + 1 == case.trace.len());
        }
        assert_eq!(work.cycles(), case.cycles);
        assert!(work.finished());
        assert!(work
            .try_step(&m.sequencer, |_| -> Result<(), ()> { panic!("second RET") })
            .unwrap());
        instructions += case.trace.len();
    }
    assert_eq!(instructions, 205206);
}
