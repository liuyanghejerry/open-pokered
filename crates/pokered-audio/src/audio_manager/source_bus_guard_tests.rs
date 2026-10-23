use super::AudioManager;
use crate::apu::Apu;
use crate::music_data::MusicId;
use crate::sfx_data::SfxId;
use crate::source_clock::apu_bus::{ApuAccess, ApuInstruction};
use crate::source_clock::frame_instructions::{FrameStepError, SourceAudioFrame};
use crate::source_clock::{AudioAdmissionError, AudioClockError};
use alloc::{boxed::Box, format, string::String, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};
use dotzuki_audio::apu::ApuPlaybackState;

#[test]
fn new_hook_fade_or_pending_volume_after_progress_is_terminal_in_both_modes() {
    for bus_mode in [false, true] {
        for change in ["hook", "fade", "pending volume"] {
            let mut manager = powered(MusicId::POKECENTER);
            manager.play_sfx(SfxId::PressAB);
            let mut work = if bus_mode {
                manager.begin_source_audio_frame_with_bus(2, 0).unwrap()
            } else {
                manager.begin_source_audio_frame(2, 0).unwrap()
            };
            if bus_mode {
                let mut wrote = false;
                assert!(matches!(
                    manager.resume_source_audio_frame_with_bus(&mut work, |apu, op| {
                        if wrote {
                            return Err(());
                        }
                        let value = apply(apu, op);
                        wrote = matches!(op.access, Some(ApuAccess::Write { .. }));
                        Ok(value)
                    }),
                    Err(AudioAdmissionError::Admission(FrameStepError::Clock(())))
                ));
                assert!(wrote);
            } else {
                let mut accepted = 0;
                assert!(matches!(
                    manager.resume_source_audio_frame(&mut work, |_| {
                        if accepted == 17 {
                            return Err(());
                        }
                        accepted += 1;
                        Ok(())
                    }),
                    Err(AudioAdmissionError::Admission(FrameStepError::Clock(())))
                ));
            }
            assert!(work.cycles() > 0);
            match change {
                "hook" => manager.set_post_frame_hook(Some(Box::new(|_, _| {
                    panic!("unsupported hook must never run")
                }))),
                "fade" => manager.fade_out(3),
                "pending volume" => manager.sequencer.pending_nr50 = Some(0x53),
                _ => unreachable!(),
            }
            let before = snapshot(&manager);
            let mut frozen = work.clone();
            let error = AudioClockError::UnsupportedSoundBank;
            let result = refuse_resume(&mut manager, &mut work, bus_mode);
            assert_eq!(
                result,
                Err(AudioAdmissionError::SourceFailed(error)),
                "{change} bus={bus_mode}"
            );
            frozen.source_failed = Some(error);
            assert_eq!(work, frozen);
            assert_eq!(snapshot(&manager), before);
            for _ in 0..2 {
                work = restored(&work);
                assert_eq!(
                    refuse_resume(&mut manager, &mut work, bus_mode),
                    Err(AudioAdmissionError::SourceFailed(error))
                );
                assert_eq!(work, frozen);
                assert_eq!(snapshot(&manager), before);
            }
        }
    }
}

fn refuse_resume(
    manager: &mut AudioManager,
    work: &mut SourceAudioFrame,
    bus_mode: bool,
) -> Result<crate::source_clock::AudioFrameWork, AudioAdmissionError<FrameStepError<()>>> {
    if bus_mode {
        manager.resume_source_audio_frame_with_bus(work, |_, _| {
            panic!("guard/terminal outcome must not execute")
        })
    } else {
        manager
            .resume_source_audio_frame(work, |_| panic!("guard/terminal outcome must not execute"))
    }
}

#[test]
fn zero_progress_native_mismatch_is_preflight_and_does_not_poison_the_frame() {
    for bus_mode in [false, true] {
        let mut manager = powered(MusicId::POKECENTER);
        let mut work = if bus_mode {
            manager.begin_source_audio_frame_with_bus(2, 0).unwrap()
        } else {
            manager.begin_source_audio_frame(2, 0).unwrap()
        };
        let original = manager.sequencer.channels[2].wave_instrument;
        manager.sequencer.channels[2].wave_instrument ^= 1;
        let before = snapshot(&manager);
        let frozen = work.clone();
        assert_eq!(
            refuse_resume(&mut manager, &mut work, bus_mode),
            Err(AudioAdmissionError::Preflight(
                AudioClockError::NativeStateMismatch(0)
            ))
        );
        assert_eq!(work, frozen);
        assert_eq!(snapshot(&manager), before);
        manager.sequencer.channels[2].wave_instrument = original;
        if bus_mode {
            manager
                .resume_source_audio_frame_with_bus(&mut work, |apu, op| {
                    Ok::<_, ()>(apply(apu, op))
                })
                .unwrap();
        } else {
            manager
                .resume_source_audio_frame(&mut work, |_| Ok::<_, ()>(()))
                .unwrap();
        }
    }
}

#[test]
fn queued_native_volume_is_rejected_without_losing_the_ordinary_output() {
    let mut manager = powered(MusicId::POKECENTER);
    manager.sequencer.pending_nr50 = Some(0x53);
    let before = snapshot(&manager);
    assert_eq!(
        manager.begin_source_audio_frame_with_bus(2, 0),
        Err(AudioClockError::UnsupportedSoundBank)
    );
    assert_eq!(
        manager.begin_source_audio_frame(2, 0),
        Err(AudioClockError::UnsupportedSoundBank)
    );
    assert_eq!(snapshot(&manager), before);
    manager.update_frame();
    assert_eq!(manager.apu.read_register(0xff24), 0x53);
    assert_eq!(manager.sequencer.pending_nr50, None);
}

#[test]
fn ordinary_song_and_press_ab_frame_boundaries_have_no_unapplied_volume() {
    for (music, bank) in [
        (MusicId::POKECENTER, 2),
        (MusicId::SILPH_CO, 31),
        (MusicId::DUNGEON3, 31),
    ] {
        for confirmation in [false, true] {
            let mut manager = powered(music);
            for frame in 0..256 {
                if confirmation && frame % 37 == 0 {
                    manager.play_sfx(SfxId::PressAB);
                }
                assert_eq!(
                    manager.sequencer.pending_nr50, None,
                    "{music:?} confirmation={confirmation} frame={frame}"
                );
                manager.begin_source_audio_frame_with_bus(bank, 0).unwrap();
                manager.update_frame();
                assert_eq!(manager.sequencer.pending_nr50, None);
            }
        }
    }
}

fn powered(music: MusicId) -> AudioManager {
    let mut manager = AudioManager::new();
    manager.apu.write_register(0xff26, 0x80);
    manager.play_music(music);
    manager.update_frame();
    manager
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    sequencer: String,
    apu: ApuPlaybackState,
}

fn snapshot(manager: &AudioManager) -> Snapshot {
    Snapshot {
        sequencer: format!("{:?}", manager.sequencer),
        // Includes oscillator/envelope/length and frame-sequencer phase, so an
        // accidental tick is observable even if all visible registers agree.
        apu: manager.apu.playback_state(),
    }
}

fn apply(apu: &mut Apu, instruction: ApuInstruction) -> Option<u8> {
    match instruction.access {
        Some(ApuAccess::Read { address, at_tcycle }) => {
            apu.tick_n(u32::from(at_tcycle));
            let value = apu.read_register(address);
            apu.tick_n(u32::from(instruction.tcycles - at_tcycle));
            Some(value)
        }
        Some(ApuAccess::Write {
            address,
            value,
            at_tcycle,
        }) => {
            apu.tick_n(u32::from(at_tcycle));
            apu.write_register(address, value);
            apu.tick_n(u32::from(instruction.tcycles - at_tcycle));
            None
        }
        None => {
            apu.tick_n(u32::from(instruction.tcycles));
            None
        }
    }
}

fn restored(work: &SourceAudioFrame) -> SourceAudioFrame {
    serde_json::from_slice(&serde_json::to_vec(work).unwrap()).unwrap()
}

#[test]
fn cpu_and_bus_frames_refuse_the_other_resume_api_before_and_after_partial_progress() {
    for allowance in [0, 17] {
        let mut manager = powered(MusicId::POKECENTER);
        let mut cpu = manager.begin_source_audio_frame(2, 0).unwrap();
        let mut accepted = 0;
        assert_eq!(
            manager.resume_source_audio_frame(&mut cpu, |_| {
                if accepted == allowance {
                    return Err("pause");
                }
                accepted += 1;
                Ok(())
            }),
            Err(AudioAdmissionError::Admission(FrameStepError::Clock(
                "pause"
            )))
        );
        let before = snapshot(&manager);
        let frozen = cpu.clone();
        assert_eq!(
            manager.resume_source_audio_frame_with_bus(&mut cpu, |_, _| -> Result<_, ()> {
                panic!("CPU-only continuation cannot acquire bus output mid-frame")
            }),
            Err(AudioAdmissionError::WrongMode)
        );
        assert_eq!(cpu, frozen);
        assert_eq!(snapshot(&manager), before);
        manager
            .resume_source_audio_frame(&mut cpu, |_| Ok::<_, ()>(()))
            .unwrap();

        let mut manager = powered(MusicId::POKECENTER);
        let mut bus = manager.begin_source_audio_frame_with_bus(2, 0).unwrap();
        accepted = 0;
        assert_eq!(
            manager.resume_source_audio_frame_with_bus(&mut bus, |apu, instruction| {
                if accepted == allowance {
                    return Err("pause");
                }
                accepted += 1;
                Ok(apply(apu, instruction))
            }),
            Err(AudioAdmissionError::Admission(FrameStepError::Clock(
                "pause"
            )))
        );
        let before = snapshot(&manager);
        let frozen = bus.clone();
        assert_eq!(
            manager.resume_source_audio_frame(&mut bus, |_| -> Result<(), ()> {
                panic!("bus continuation cannot replay ordinary batch output")
            }),
            Err(AudioAdmissionError::WrongMode)
        );
        assert_eq!(bus, frozen);
        assert_eq!(snapshot(&manager), before);
        manager
            .resume_source_audio_frame_with_bus(&mut bus, |apu, instruction| {
                Ok::<_, ()>(apply(apu, instruction))
            })
            .unwrap();
    }
}

#[test]
fn installed_post_frame_hook_refuses_begin_without_invoking_the_hook() {
    let mut manager = powered(MusicId::POKECENTER);
    let invocations = Arc::new(AtomicUsize::new(0));
    let hook_invocations = Arc::clone(&invocations);
    manager.set_post_frame_hook(Some(Box::new(move |apu, sequencer| {
        hook_invocations.fetch_add(1, Ordering::SeqCst);
        apu.write_register(0xff25, 0x12);
        sequencer.channels[0].delay_counter = 71;
    })));
    let before = snapshot(&manager);
    assert_eq!(
        manager.begin_source_audio_frame_with_bus(2, 0),
        Err(AudioClockError::UnsupportedSoundBank)
    );
    assert_eq!(snapshot(&manager), before);
    assert_eq!(invocations.load(Ordering::SeqCst), 0);
    // Establish that the installed hook is live and observable on the ordinary
    // API, rather than relying on a callback that could never have run.
    manager.update_frame();
    assert_eq!(invocations.load(Ordering::SeqCst), 1);
    assert_eq!(manager.apu.read_register(0xff25), 0x12);
    assert_eq!(manager.sequencer.channels[0].delay_counter, 71);
}

#[test]
fn changed_wave_instrument_refuses_a_partly_executed_frame_without_clock_or_write() {
    let mut manager = powered(MusicId::POKECENTER);
    manager.play_sfx(SfxId::PressAB);
    let mut work = manager.begin_source_audio_frame_with_bus(2, 0).unwrap();
    let mut wrote = false;
    assert_eq!(
        manager.resume_source_audio_frame_with_bus(&mut work, |apu, instruction| {
            if wrote {
                return Err("after first store");
            }
            let value = apply(apu, instruction);
            wrote = matches!(instruction.access, Some(ApuAccess::Write { .. }));
            Ok(value)
        }),
        Err(AudioAdmissionError::Admission(FrameStepError::Clock(
            "after first store"
        )))
    );
    assert!(wrote);
    assert!(work.cycles() > 0);
    let original = manager.sequencer.channels[2].wave_instrument;
    manager.sequencer.channels[2].wave_instrument ^= 1;
    let before = snapshot(&manager);
    work = restored(&work);
    let mut frozen = work.clone();
    assert_eq!(
        manager.resume_source_audio_frame_with_bus(&mut work, |_, _| -> Result<_, ()> {
            panic!("stale wave source must refuse before touching the APU")
        }),
        Err(AudioAdmissionError::SourceFailed(
            AudioClockError::NativeStateMismatch(0)
        ))
    );
    frozen.source_failed = Some(AudioClockError::NativeStateMismatch(0));
    assert_eq!(work, frozen);
    assert_eq!(snapshot(&manager), before);
    manager.sequencer.channels[2].wave_instrument = original;
    let before = snapshot(&manager);
    work = restored(&work);
    assert_eq!(
        manager.resume_source_audio_frame_with_bus(&mut work, |_, _| -> Result<_, ()> {
            panic!("restoring input cannot replay a terminal source failure")
        }),
        Err(AudioAdmissionError::SourceFailed(
            AudioClockError::NativeStateMismatch(0)
        ))
    );
    assert_eq!(work, frozen);
    assert_eq!(snapshot(&manager), before);
}

#[test]
fn press_ab_modifiers_are_rejected_before_any_frame_work_is_created() {
    for (frequency, tempo) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let mut manager = powered(MusicId::POKECENTER);
        manager.play_sfx(SfxId::PressAB);
        manager.sequencer.frequency_modifier = frequency;
        manager.sequencer.tempo_modifier = tempo;
        let before = snapshot(&manager);
        assert_eq!(
            manager.begin_source_audio_frame_with_bus(2, 0),
            Err(AudioClockError::UnsupportedEffect(4)),
            "frequency={frequency} tempo={tempo}"
        );
        assert_eq!(snapshot(&manager), before);
    }
}

#[test]
fn completed_bus_frame_reuses_cached_result_without_a_second_native_tick_or_apu_access() {
    for (music, bank) in [
        (MusicId::POKECENTER, 2),
        (MusicId::SILPH_CO, 31),
        (MusicId::DUNGEON3, 31),
    ] {
        let mut manager = powered(music);
        manager.play_sfx(SfxId::PressAB);
        let mut work = manager.begin_source_audio_frame_with_bus(bank, 0).unwrap();
        let start = snapshot(&manager);
        let mut cycles = 0;
        let mut writes = 0;
        let result = manager
            .resume_source_audio_frame_with_bus(&mut work, |apu, instruction| {
                cycles += u32::from(instruction.tcycles);
                writes += usize::from(matches!(instruction.access, Some(ApuAccess::Write { .. })));
                Ok::<_, ()>(apply(apu, instruction))
            })
            .unwrap();
        assert!(writes > 0);
        assert_eq!(result.cycles, cycles);
        assert_eq!(work.cycles(), cycles);
        let completed = snapshot(&manager);
        assert_ne!(completed.sequencer, start.sequencer);
        assert_ne!(completed.apu, start.apu);
        for _ in 0..2 {
            work = restored(&work);
            let frozen = work.clone();
            assert_eq!(
                manager.resume_source_audio_frame_with_bus(&mut work, |_, _| -> Result<_, ()> {
                    panic!("completed frame may not clock or write again")
                }),
                Ok(result)
            );
            assert_eq!(work, frozen);
            assert_eq!(snapshot(&manager), completed);
        }
    }
}

#[test]
fn injected_cursor_cycle_corruption_caches_source_failure_after_apu_progress_without_native_commit()
{
    let mut manager = powered(MusicId::POKECENTER);
    manager.play_sfx(SfxId::PressAB);
    let work = manager.begin_source_audio_frame_with_bus(2, 0).unwrap();
    // Controlled test-only snapshot corruption, never a live Game input. This
    // reaches the post-execution accounting failure after real APU effects.
    let mut encoded = serde_json::to_value(&work).unwrap();
    encoded["cycles"] = serde_json::json!(work.cycles() + 4);
    let mut work: SourceAudioFrame = serde_json::from_value(encoded).unwrap();
    let before = snapshot(&manager);
    let mut cycles = 0;
    let mut writes = 0;
    assert_eq!(
        manager.resume_source_audio_frame_with_bus(&mut work, |apu, instruction| {
            cycles += u32::from(instruction.tcycles);
            writes += usize::from(matches!(instruction.access, Some(ApuAccess::Write { .. })));
            Ok::<_, ()>(apply(apu, instruction))
        }),
        Err(AudioAdmissionError::SourceFailed(
            AudioClockError::NativeStateMismatch(0)
        ))
    );
    assert!(work.finished());
    assert!(writes > 0);
    assert!(cycles > 0);
    assert_eq!(work.cycles(), cycles + 4);
    let failed = snapshot(&manager);
    assert_eq!(failed.sequencer, before.sequencer);
    assert_ne!(failed.apu, before.apu);
    for _ in 0..2 {
        work = restored(&work);
        let frozen = work.clone();
        assert_eq!(
            manager.resume_source_audio_frame_with_bus(&mut work, |_, _| -> Result<_, ()> {
                panic!("failed source work must not replay already executed APU effects")
            }),
            Err(AudioAdmissionError::SourceFailed(
                AudioClockError::NativeStateMismatch(0)
            ))
        );
        assert_eq!(work, frozen);
        assert_eq!(snapshot(&manager), failed);
    }
}
