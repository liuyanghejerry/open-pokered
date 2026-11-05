//! Controlled complete audio calls; no Game boot or complete VBlank claim.
use super::*;
use crate::source_clock::{
    apu_bus::{ApuAccess, ApuInstruction},
    frame_instructions::{FrameStepError, SourceSong},
    AudioAdmissionError,
};
use alloc::{string::String, vec::Vec};

std::thread_local! {
    static WRITES: std::cell::RefCell<Vec<(u16, u8)>> = const { std::cell::RefCell::new(Vec::new()) };
}
fn record_write(address: u16, value: u8) {
    WRITES.with(|writes| writes.borrow_mut().push((address, value)));
}
fn powered(music: MusicId) -> AudioManager {
    let mut manager = AudioManager::new();
    manager.apu.write_register(0xff26, 0x80);
    manager.play_music(music);
    manager.update_frame();
    manager
}
fn native(manager: &AudioManager) -> String {
    format!("{:?}", manager.sequencer)
}
fn execute(apu: &mut crate::apu::Apu, instruction: ApuInstruction) -> Option<u8> {
    let mut value = None;
    let offset = match instruction.access {
        Some(ApuAccess::Read { at_tcycle, .. } | ApuAccess::Write { at_tcycle, .. }) => at_tcycle,
        None => instruction.tcycles,
    };
    apu.tick_n(u32::from(offset));
    match instruction.access {
        Some(ApuAccess::Read { address, .. }) => value = Some(apu.read_register(address)),
        Some(ApuAccess::Write { address, value, .. }) => apu.write_register(address, value),
        None => (),
    }
    apu.tick_n(u32::from(instruction.tcycles - offset));
    value
}

#[test]
fn every_field_song_keeps_source_frames_and_native_channel_updates_in_sync() {
    let mut failures = Vec::new();
    for &song in SourceSong::ALL {
        for confirmation in [false, true] {
            let mut manager = powered(song.music());
            let mut failed = None;
            for frame in 0..4096 {
                if confirmation && frame % 37 == 0 {
                    manager.play_sfx(SfxId::PressAB);
                }
                let mut work = match manager.begin_source_audio_frame_with_bus(song.bank(), 0) {
                    Ok(work) => work,
                    Err(error) => {
                        failed = Some(format!(
                            "{song:?} confirmation{confirmation} frame {frame}: {error:?}"
                        ));
                        break;
                    }
                };
                if let Err(error) = manager
                    .resume_source_audio_frame_with_bus(&mut work, |apu, instruction| {
                        Ok::<_, ()>(execute(apu, instruction))
                    })
                {
                    failed = Some(format!(
                        "{song:?} confirmation{confirmation} frame {frame}: {error:?}"
                    ));
                    break;
                }
            }
            if let Some(failed) = failed {
                failures.push(failed);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "unsupported legal field music: {failures:#?}"
    );
}

#[derive(serde::Deserialize)]
struct CpuCase {
    song: SourceSong,
    confirmation: bool,
    frame: usize,
    cycles: u32,
    trace: Vec<(u16, u8)>,
    #[serde(default)]
    initial_noise_override: Option<u8>,
}
#[derive(serde::Deserialize)]
struct CpuFixture {
    cases: Vec<CpuCase>,
}
#[derive(serde::Deserialize)]
struct BusCase {
    song: SourceSong,
    confirmation: bool,
    frame: usize,
    cycles: u32,
    nr50: u8,
    nr51: u8,
    initial_wave_override: Option<u8>,
    accesses: Vec<(usize, u16, u8, String, u16, u8)>,
    #[serde(default)]
    initial_noise_override: Option<u8>,
}
#[derive(serde::Deserialize)]
struct BusFixture {
    cases: Vec<BusCase>,
}
fn at_frame(case: &BusCase) -> AudioManager {
    let mut manager = powered(case.song.music());
    for frame in 0..=case.frame {
        if case.confirmation && frame % 37 == 0 {
            manager.play_sfx(SfxId::PressAB);
        }
        if frame < case.frame {
            manager.update_frame();
        }
    }
    if let Some(instrument) = case.initial_noise_override {
        let noise = &mut manager.sequencer.channels[7];
        noise.reset();
        noise.data = crate::sfx_data::get_sfx_track(SfxId::from_u8(instrument - 1).unwrap())
            .channels[3]
            .unwrap()
            .to_vec();
        noise.active = true;
        noise.sound_id = instrument;
        noise.note_speed = 1;
        noise.delay_counter = 1;
        noise.flags1.insert(crate::ChannelFlags1::NOISE_OR_SFX);
        manager.sequencer.sfx_playing = true;
    }
    manager.apu.write_register(0xff24, case.nr50);
    manager.apu.write_register(0xff25, case.nr51);
    if let Some(instrument) = case.initial_wave_override {
        manager.sequencer.channels[2].wave_instrument = instrument;
    }
    manager
}

#[test]
fn whole_source_apu_accesses_match_original_and_commit_never_replays_batch_output() {
    let cpu: CpuFixture =
        serde_json::from_str(include_str!("../source_clock/frame_work_golden.json")).unwrap();
    let bus: BusFixture =
        serde_json::from_str(include_str!("../source_clock/frame_bus_golden.json")).unwrap();
    assert_eq!(cpu.cases.len(), 311);
    assert_eq!(bus.cases.len(), 957);
    assert_eq!(check_whole_source_frames(&cpu, &bus), 10101);
}

#[test]
fn percussion_frames_match_original_starts_busy_drops_noise_notes_and_endings() {
    let cpu: CpuFixture = serde_json::from_str(include_str!(
        "../source_clock/percussion_frame_work_golden.json"
    ))
    .unwrap();
    let bus: BusFixture = serde_json::from_str(include_str!(
        "../source_clock/percussion_frame_bus_golden.json"
    ))
    .unwrap();
    assert_eq!(cpu.cases.len(), 106);
    assert_eq!(bus.cases.len(), 318);
    // Four explicit busy-voice fixtures complement 102 ordinary playback
    // states. Every input is replayed against three independently traced ports.
    assert_eq!(
        cpu.cases
            .iter()
            .filter(|c| c.initial_noise_override.is_some())
            .count(),
        4
    );
    assert_eq!(
        bus.cases
            .iter()
            .filter(|c| c.initial_noise_override.is_some())
            .count(),
        12
    );
    let expected_accesses = bus.cases.iter().map(|c| c.accesses.len()).sum::<usize>();
    assert!(expected_accesses > 1000);
    assert_eq!(check_whole_source_frames(&cpu, &bus), expected_accesses);
}

#[test]
fn safari_slide_frames_match_original_with_and_without_paired_sfx() {
    let cpu: CpuFixture =
        serde_json::from_str(include_str!("../source_clock/slide_frame_work_golden.json")).unwrap();
    let bus: BusFixture =
        serde_json::from_str(include_str!("../source_clock/slide_frame_bus_golden.json")).unwrap();
    assert_eq!(cpu.cases.len(), 128);
    assert_eq!(bus.cases.len(), 384);
    assert!(cpu
        .cases
        .iter()
        .all(|case| case.song == SourceSong::SafariZone && case.initial_noise_override.is_none()));
    let expected = bus
        .cases
        .iter()
        .map(|case| case.accesses.len())
        .sum::<usize>();
    assert_eq!(expected, 744);
    assert_eq!(check_whole_source_frames(&cpu, &bus), expected);
}

fn check_whole_source_frames(cpu: &CpuFixture, bus: &BusFixture) -> usize {
    let mut total_accesses = 0;
    for (case_index, case) in bus.cases.iter().enumerate() {
        let source = cpu
            .cases
            .iter()
            .find(|source| {
                (
                    source.song,
                    source.confirmation,
                    source.frame,
                    source.initial_noise_override,
                ) == (
                    case.song,
                    case.confirmation,
                    case.frame,
                    case.initial_noise_override,
                )
            })
            .unwrap();
        assert_eq!(source.cycles, case.cycles);
        let mut manager = at_frame(case);
        let mut ordinary = at_frame(case);
        let frozen = native(&manager);
        let mut expected_apu = manager.apu.clone();
        let mut work = manager
            .begin_source_audio_frame_with_bus(case.song.bank(), 0)
            .unwrap();
        WRITES.with(|writes| writes.borrow_mut().clear());
        manager.apu.set_register_sink(Some(record_write));
        let mut next_access = 0;
        for (index, (pc, cycles)) in source.trace.iter().copied().enumerate() {
            let before = work.clone();
            let apu_before = manager.apu.playback_state();
            assert_eq!(
                work.try_step_with_bus(&manager.sequencer, |_| Err("denied")),
                Err(FrameStepError::Clock("denied"))
            );
            assert_eq!(work, before);
            assert_eq!(manager.apu.playback_state(), apu_before);
            let mut expected = ApuInstruction::plain(cycles);
            let mut expected_read = None;
            if let Some((at, source_pc, offset, kind, address, value)) =
                case.accesses.get(next_access)
            {
                if *at == index {
                    assert_eq!(*source_pc, pc);
                    expected.access = Some(if kind == "read" {
                        expected_read = Some(*value);
                        ApuAccess::Read {
                            address: *address,
                            at_tcycle: *offset,
                        }
                    } else {
                        assert_eq!(kind, "write");
                        ApuAccess::Write {
                            address: *address,
                            value: *value,
                            at_tcycle: *offset,
                        }
                    });
                    next_access += 1;
                }
            }
            // Retained bus operands must survive suspension on both sides of
            // every hardware access. Child cursor tests cover every CPU-only
            // boundary; avoid repeating whole-frame JSON for those here.
            if expected.access.is_some() || index == 0 {
                work = serde_json::from_slice(&serde_json::to_vec(&work).unwrap()).unwrap();
            }
            assert_eq!(execute(&mut expected_apu, expected), expected_read);
            let engine = &mut manager.engine;
            let done = work
                .try_step_with_bus(&engine.sequencer, |actual| {
                    assert_eq!(
                        actual, expected,
                        "case {case_index} instruction {index} PC {pc:04x}"
                    );
                    let receipt = execute(&mut engine.apu, actual);
                    assert_eq!(receipt, expected_read);
                    Ok::<_, ()>(receipt)
                })
                .unwrap();
            assert_eq!(done, index + 1 == source.trace.len());
            if expected.access.is_some() || done {
                work = serde_json::from_slice(&serde_json::to_vec(&work).unwrap()).unwrap();
            }
        }
        assert_eq!(next_access, case.accesses.len());
        assert_eq!(
            native(&manager),
            frozen,
            "source work must not tick a second sequencer"
        );
        assert_eq!(work.cycles(), case.cycles);
        let done = manager
            .resume_source_audio_frame_with_bus(&mut work, |_, _| -> Result<Option<u8>, ()> {
                panic!("the source RET already completed")
            })
            .unwrap();
        ordinary.update_frame();
        assert_eq!(
            native(&manager),
            native(&ordinary),
            "logical commit case {case_index}"
        );
        assert_eq!(
            manager.apu.playback_state(),
            expected_apu.playback_state(),
            "APU phase case {case_index}"
        );
        let expected_writes: Vec<_> = case
            .accesses
            .iter()
            .filter_map(|(_, _, _, kind, address, value)| {
                (kind == "write").then_some((*address, *value))
            })
            .collect();
        WRITES.with(|writes| {
            assert_eq!(
                *writes.borrow(),
                expected_writes,
                "no extra writes at commit"
            )
        });
        for _ in 0..2 {
            work = serde_json::from_slice(&serde_json::to_vec(&work).unwrap()).unwrap();
            assert_eq!(
                manager
                    .resume_source_audio_frame_with_bus(
                        &mut work,
                        |_, _| -> Result<Option<u8>, ()> {
                            panic!("completed frame must neither clock nor commit again")
                        }
                    )
                    .unwrap(),
                done
            );
        }
        assert_eq!(native(&manager), native(&ordinary));
        assert_eq!(manager.apu.playback_state(), expected_apu.playback_state());
        total_accesses += next_access;
    }
    total_accesses
}

#[test]
fn source_frame_runner_matches_single_instructions_through_refusal_and_resume() {
    struct PrefixBus<'a> {
        ops: &'a mut Vec<ApuInstruction>,
        allowance: usize,
        enabled: bool,
        combined: &'a mut usize,
    }
    impl crate::source_clock::apu_bus::SourceAudioFrameBus for PrefixBus<'_> {
        type Error = &'static str;
        fn execute(
            &mut self,
            apu: &mut crate::apu::Apu,
            op: ApuInstruction,
        ) -> Result<Option<u8>, Self::Error> {
            if self.ops.len() == self.allowance {
                return Err("yield");
            }
            self.ops.push(op);
            Ok(execute(apu, op))
        }
        fn execute_plain_run(
            &mut self,
            apu: &mut crate::apu::Apu,
            cycles: &[u8],
        ) -> Result<usize, Self::Error> {
            if !self.enabled {
                return Ok(0);
            }
            let count = cycles.len().min(self.allowance - self.ops.len());
            if count == 0 {
                return Err("yield");
            }
            self.ops
                .extend(cycles[..count].iter().map(|&t| ApuInstruction::plain(t)));
            apu.tick_n(cycles[..count].iter().map(|&t| u32::from(t)).sum());
            *self.combined += 1;
            Ok(count)
        }
    }
    let fixture: BusFixture =
        serde_json::from_str(include_str!("../source_clock/frame_bus_golden.json")).unwrap();
    let mut combined = 0;
    for enabled in [false, true] {
        for case in fixture.cases.iter().step_by(37) {
            for allowance in [1, 17, 31, 128] {
                let manager = at_frame(case);
                let mut actual = manager
                    .begin_source_audio_frame_with_bus(case.song.bank(), 0)
                    .unwrap();
                let mut expected = actual.clone();
                let mut actual_apu = manager.apu.clone();
                let mut expected_apu = actual_apu.clone();
                loop {
                    let frozen = actual.clone();
                    assert_eq!(
                        actual.run_with_bus(
                            &manager.sequencer,
                            &mut actual_apu,
                            &mut |_: &mut crate::apu::Apu, _: ApuInstruction| Err::<Option<u8>, _>(
                                "denied"
                            )
                        ),
                        Err(FrameStepError::Clock("denied")),
                    );
                    assert_eq!(actual, frozen);
                    actual = serde_json::from_slice(&serde_json::to_vec(&actual).unwrap()).unwrap();
                    let mut actual_ops = Vec::new();
                    let mut expected_ops = Vec::new();
                    let result = actual.run_with_bus(
                        &manager.sequencer,
                        &mut actual_apu,
                        &mut PrefixBus {
                            ops: &mut actual_ops,
                            allowance,
                            enabled,
                            combined: &mut combined,
                        },
                    );
                    let reference = loop {
                        match expected.try_step_with_bus(&manager.sequencer, |op| {
                            if expected_ops.len() == allowance {
                                return Err("yield");
                            }
                            expected_ops.push(op);
                            Ok(execute(&mut expected_apu, op))
                        }) {
                            Ok(true) => break Ok(()),
                            Ok(false) => (),
                            Err(error) => break Err(error),
                        }
                    };
                    assert_eq!(result, reference);
                    assert_eq!(actual_ops, expected_ops);
                    assert_eq!(actual, expected);
                    assert_eq!(actual_apu.playback_state(), expected_apu.playback_state());
                    if result.is_ok() {
                        break;
                    }
                }
                actual
                    .run_with_bus(
                        &manager.sequencer,
                        &mut actual_apu,
                        &mut |_: &mut crate::apu::Apu,
                              _: ApuInstruction|
                         -> Result<Option<u8>, ()> {
                            panic!("completed frame must not execute")
                        },
                    )
                    .unwrap();
            }
        }
    }
    assert!(combined > 0);
}

#[test]
fn live_manager_resumes_across_refusals_and_commits_one_logical_tick_per_frame() {
    for (music, bank) in [
        (MusicId::POKECENTER, 2),
        (MusicId::SILPH_CO, 31),
        (MusicId::DUNGEON3, 31),
    ] {
        for confirmation in [false, true] {
            let mut manager = powered(music);
            let mut ordinary = powered(music);
            for frame in 0..256 {
                if confirmation && frame % 37 == 0 {
                    manager.play_sfx(SfxId::PressAB);
                    ordinary.play_sfx(SfxId::PressAB);
                }
                let frozen = native(&manager);
                let mut work = manager.begin_source_audio_frame_with_bus(bank, 0).unwrap();
                let mut turns = 0;
                loop {
                    let before = work.clone();
                    let apu_before = manager.apu.playback_state();
                    assert_eq!(
                        manager.resume_source_audio_frame_with_bus(&mut work, |_, _| Err("denied")),
                        Err(AudioAdmissionError::Admission(FrameStepError::Clock(
                            "denied"
                        )))
                    );
                    assert_eq!(work, before);
                    assert_eq!(manager.apu.playback_state(), apu_before);
                    assert_eq!(native(&manager), frozen);
                    work = serde_json::from_slice(&serde_json::to_vec(&work).unwrap()).unwrap();
                    let allowance = [1, 17, 31][turns % 3];
                    turns += 1;
                    let mut accepted = 0;
                    let result =
                        manager.resume_source_audio_frame_with_bus(&mut work, |apu, op| {
                            if accepted == allowance {
                                return Err("yield");
                            }
                            accepted += 1;
                            Ok(execute(apu, op))
                        });
                    match result {
                        Ok(done) => {
                            assert_eq!(done.cycles, work.cycles());
                            break;
                        }
                        Err(AudioAdmissionError::Admission(FrameStepError::Clock("yield"))) => {
                            assert_eq!(native(&manager), frozen);
                        }
                        other => panic!("{music:?} confirm{confirmation} frame{frame}: {other:?}"),
                    }
                }
                ordinary.update_frame();
                assert_eq!(native(&manager), native(&ordinary));
            }
        }
    }
}

#[test]
fn party_swap_and_confirmation_overlap_finish_through_retained_audio_frames() {
    let mut saw_confirmation_with_second_swap_pulse = false;
    for music in [MusicId::PALLET_TOWN, MusicId::SILPH_CO, MusicId::DUNGEON3] {
        let bank = SourceSong::from_music(music).unwrap().bank();
        for delay in 0..=20 {
            for confirmation in [false, true] {
                let mut manager = powered(music);
                let mut ordinary = powered(music);
                for audio in [&mut manager, &mut ordinary] {
                    audio.play_sfx(SfxId::Swap);
                    for _ in 0..delay {
                        audio.update_frame();
                    }
                    if confirmation {
                        audio.play_sfx(SfxId::PressAB);
                    }
                }
                saw_confirmation_with_second_swap_pulse |= confirmation
                    && manager.sequencer.channels[5].active;
                for frame in 0..40 {
                    let frozen = native(&manager);
                    let mut cursor = None;
                    if frame == 0 {
                        let mut accepted = 0;
                        assert!(matches!(manager.service_source_audio_frame(&mut cursor, bank, 0,
                            |apu: &mut crate::apu::Apu, op: ApuInstruction| {
                                if accepted == 17 { return Err("yield"); }
                                accepted += 1;
                                Ok(execute(apu, op))
                            }), Err(AudioAdmissionError::Admission(FrameStepError::Clock("yield")))));
                        assert_eq!(native(&manager), frozen);
                        cursor = serde_json::from_slice(&serde_json::to_vec(&cursor).unwrap()).unwrap();
                    }
                    let result = manager.service_source_audio_frame(&mut cursor, bank, 0,
                        |apu: &mut crate::apu::Apu, op: ApuInstruction| Ok::<_, ()>(execute(apu, op)));
                    assert!(result.is_ok(), "{music:?} delay{delay} confirm{confirmation} frame{frame}: {result:?}");
                    ordinary.update_frame();
                    assert_eq!(native(&manager), native(&ordinary));
                    let after = native(&manager);
                    assert_ne!(after, frozen);
                    let apu_after = manager.apu.playback_state();
                    let repeated = manager.service_source_audio_frame(&mut cursor, bank, 0,
                        |_: &mut crate::apu::Apu, _: ApuInstruction| -> Result<Option<u8>, ()> { panic!("completed SWAP frame replayed") });
                    assert_eq!(repeated, result);
                    assert_eq!(native(&manager), after);
                    assert_eq!(manager.apu.playback_state(), apu_after);
                }
                assert!(!manager.sequencer.channels[4].active);
                assert!(!manager.sequencer.channels[5].active);
            }
        }
    }
    assert!(saw_confirmation_with_second_swap_pulse);
}

#[test]
fn start_menu_noise_and_confirmation_overlap_survive_source_handoff() {
    let mut saw_overlap = false;
    for music in [MusicId::PALLET_TOWN, MusicId::SILPH_CO, MusicId::DUNGEON3] {
        let bank = SourceSong::from_music(music).unwrap().bank();
        for delay in 0..=12 {
            for confirmation in [false, true] {
                let mut manager = powered(music);
                let mut ordinary = powered(music);
                for audio in [&mut manager, &mut ordinary] {
                    audio.play_sfx(SfxId::StartMenu);
                    for _ in 0..delay { audio.update_frame(); }
                    if confirmation { audio.play_sfx(SfxId::PressAB); }
                }
                saw_overlap |= confirmation && manager.sequencer.channels[7].active;
                for frame in 0..32 {
                    assert!(manager.begin_source_audio_frame(bank, 0).is_ok(),
                        "aggregate admission {music:?} delay{delay} frame{frame}");
                    let frozen = native(&manager);
                    let mut cursor = None;
                    if frame == 0 {
                        let mut accepted = 0;
                        assert!(matches!(manager.service_source_audio_frame(&mut cursor, bank, 0,
                            |apu: &mut crate::apu::Apu, op: ApuInstruction| {
                                if accepted == 17 { return Err("yield"); }
                                accepted += 1;
                                Ok(execute(apu, op))
                            }), Err(AudioAdmissionError::Admission(FrameStepError::Clock("yield")))));
                        assert_eq!(native(&manager), frozen);
                        cursor = serde_json::from_slice(&serde_json::to_vec(&cursor).unwrap()).unwrap();
                    }
                    let result = manager.service_source_audio_frame(&mut cursor, bank, 0,
                        |apu: &mut crate::apu::Apu, op: ApuInstruction| Ok::<_, ()>(execute(apu, op)));
                    assert!(result.is_ok(), "{music:?} delay{delay} confirm{confirmation} frame{frame}: {result:?}");
                    ordinary.update_frame();
                    assert_eq!(native(&manager), native(&ordinary));
                }
                assert!(!manager.sequencer.channels[4].active);
                // Music may already have restarted its own drum. It must no
                // longer be the still-active START noise stream.
                assert!(!manager.sequencer.channels[7].active
                    || manager.sequencer.channels[7].sound_id != SfxId::StartMenu as u8);
            }
        }
    }
    assert!(saw_overlap);
}

#[test]
fn start_noise_handoff_rejects_altered_programs_and_unrelated_ids() {
    use crate::source_clock::AudioClockError;
    for altered_id in [false, true] {
        let mut manager = powered(MusicId::PALLET_TOWN);
        manager.play_sfx(SfxId::StartMenu);
        if altered_id { manager.sequencer.channels[7].sound_id = 143; }
        else { manager.sequencer.channels[7].data[0] ^= 1; }
        let frozen = native(&manager);
        assert!(matches!(manager.begin_source_audio_frame(2, 0), Err(AudioClockError::ActiveSfx(7))));
        assert!(matches!(manager.begin_source_audio_frame_with_bus(2, 0), Err(AudioClockError::ActiveSfx(7))));
        assert_eq!(native(&manager), frozen);
    }
}

#[test]
fn swap_admission_rejects_altered_programs_and_modifiers_on_the_remaining_pulse() {
    use crate::source_clock::AudioClockError;
    for channel in [4, 5] {
        let mut manager = powered(MusicId::PALLET_TOWN);
        manager.play_sfx(SfxId::Swap);
        manager.sequencer.channels[channel].data[0] ^= 1;
        assert!(matches!(manager.begin_source_audio_frame(2, 0), Err(AudioClockError::ActiveSfx(i)) if i == channel as u8));
        assert!(matches!(manager.begin_source_audio_frame_with_bus(2, 0), Err(AudioClockError::ActiveSfx(i)) if i == channel as u8));
    }
    for (frequency, tempo) in [(1, 0), (0, 1)] {
        let mut manager = powered(MusicId::PALLET_TOWN);
        manager.play_sfx(SfxId::Swap);
        manager.sequencer.channels[4].active = false;
        manager.sequencer.frequency_modifier = frequency;
        manager.sequencer.tempo_modifier = tempo;
        let frozen = native(&manager);
        assert!(matches!(manager.begin_source_audio_frame_with_bus(2, 0), Err(AudioClockError::UnsupportedEffect(5))));
        assert_eq!(native(&manager), frozen);
    }
}

#[test]
fn exclusive_source_service_preserves_instructions_refusals_and_once_only_commit() {
    for data in [
        include_str!("../source_clock/frame_bus_golden.json"),
        include_str!("../source_clock/percussion_frame_bus_golden.json"),
        include_str!("../source_clock/slide_frame_bus_golden.json"),
    ] {
        let fixture: BusFixture = serde_json::from_str(data).unwrap();
        for case in fixture.cases.iter().step_by(17) {
            for allowance in [1, 17, usize::MAX] {
                let mut manager = at_frame(case);
                let mut reference = at_frame(case);
                let mut cursor = None;
                let mut expected = reference.begin_source_audio_frame_with_bus(case.song.bank(), 0).unwrap();
                let mut iteration = 0;
                loop {
                    // Refuse the first instruction even on the new-frame path.
                    let limit = if iteration == 0 { 0 } else { allowance };
                    iteration += 1;
                    let mut actual_ops = Vec::new();
                    let mut expected_ops = Vec::new();
                    let actual = manager.service_source_audio_frame(
                        &mut cursor, case.song.bank(), 0,
                        |apu: &mut crate::apu::Apu, op: ApuInstruction| {
                            if actual_ops.len() == limit { return Err("yield"); }
                            actual_ops.push(op);
                            Ok(execute(apu, op))
                        },
                    );
                    let expected_result = reference.resume_source_audio_frame_with_bus(
                        &mut expected, |apu, op| {
                            if expected_ops.len() == limit { return Err("yield"); }
                            expected_ops.push(op);
                            Ok(execute(apu, op))
                        },
                    );
                    assert_eq!(actual, expected_result);
                    assert_eq!(actual_ops, expected_ops);
                    assert_eq!(cursor.as_deref(), Some(&expected));
                    assert_eq!(native(&manager), native(&reference));
                    assert_eq!(manager.apu.playback_state(), reference.apu.playback_state());
                    if actual.is_ok() {
                        let repeated = manager.service_source_audio_frame(
                            &mut cursor, case.song.bank(), 0,
                            |_: &mut crate::apu::Apu, _: ApuInstruction| -> Result<Option<u8>, &str> {
                                panic!("completed frame must not replay")
                            },
                        );
                        assert_eq!(repeated, actual);
                        assert_eq!(native(&manager), native(&reference));
                        break;
                    }
                }
            }
            // Also exercise entry and completion within one exclusive borrow.
            let mut manager = at_frame(case);
            let mut reference = at_frame(case);
            let mut cursor = None;
            let mut expected = reference.begin_source_audio_frame_with_bus(case.song.bank(), 0).unwrap();
            let mut actual_ops = Vec::new();
            let mut expected_ops = Vec::new();
            let actual = manager.service_source_audio_frame(&mut cursor, case.song.bank(), 0,
                |apu: &mut crate::apu::Apu, op: ApuInstruction| {
                    actual_ops.push(op); Ok::<_, ()>(execute(apu, op))
                });
            let expected_result = reference.resume_source_audio_frame_with_bus(&mut expected,
                |apu, op| { expected_ops.push(op); Ok::<_, ()>(execute(apu, op)) });
            assert_eq!(actual, expected_result);
            assert!(actual.is_ok());
            assert_eq!(actual_ops, expected_ops);
            assert_eq!(native(&manager), native(&reference));
            assert_eq!(manager.apu.playback_state(), reference.apu.playback_state());
        }
    }
}

#[test]
fn exclusive_source_service_rejects_unsupported_entry_and_changed_retained_state() {
    let mut manager = powered(MusicId::POKECENTER);
    let mut cursor = None;
    let before = native(&manager);
    assert!(matches!(manager.service_source_audio_frame(&mut cursor, 31, 0,
        |_: &mut crate::apu::Apu, _: ApuInstruction| -> Result<Option<u8>, ()> { panic!("invalid entry") }),
        Err(AudioAdmissionError::Preflight(_))));
    assert!(cursor.is_none());
    assert_eq!(native(&manager), before);
    let mut instructions = 0;
    assert_eq!(manager.service_source_audio_frame(&mut cursor, 2, 0,
        |apu: &mut crate::apu::Apu, op: ApuInstruction| {
            if instructions == 1 { return Err("yield"); }
            instructions += 1; Ok(execute(apu, op))
        }), Err(AudioAdmissionError::Admission(FrameStepError::Clock("yield"))));
    manager.sequencer.channels[0].data[0] ^= 1;
    assert!(matches!(manager.service_source_audio_frame(&mut cursor, 2, 0,
        |_: &mut crate::apu::Apu, _: ApuInstruction| -> Result<Option<u8>, ()> { panic!("changed program") }),
        Err(AudioAdmissionError::SourceFailed(_))));
    assert!(cursor.as_ref().unwrap().cycles() > 0);
}

#[test]
fn both_source_entry_modes_reject_pending_volume_and_native_pause() {
    use crate::source_clock::AudioClockError;
    for music in [MusicId::POKECENTER, MusicId::SILPH_CO, MusicId::DUNGEON3] {
        let mut manager = powered(music);
        for _ in 0..20 {
            manager.update_frame();
        }
        let bank = SourceSong::from_music(music).unwrap().bank();
        assert!(manager.begin_source_audio_frame(bank, 0).is_ok());
        assert!(manager.begin_source_audio_frame_with_bus(bank, 0).is_ok());
        for (pending_volume, paused) in [(true, false), (false, true), (true, true)] {
            manager.sequencer.pending_nr50 = pending_volume.then_some(0x55);
            manager.set_music_paused(paused);
            let before = native(&manager);
            let apu_before = format!("{:?}", manager.apu);
            assert!(matches!(
                manager.begin_source_audio_frame(bank, 0),
                Err(AudioClockError::UnsupportedSoundBank)
            ));
            assert!(matches!(
                manager.begin_source_audio_frame_with_bus(bank, 0),
                Err(AudioClockError::UnsupportedSoundBank)
            ));
            assert_eq!(native(&manager), before);
            assert_eq!(format!("{:?}", manager.apu), apu_before);
        }
        manager.sequencer.pending_nr50 = None;
        manager.set_music_paused(false);
        assert!(manager.begin_source_audio_frame(bank, 0).is_ok());
        assert!(manager.begin_source_audio_frame_with_bus(bank, 0).is_ok());
    }
}
