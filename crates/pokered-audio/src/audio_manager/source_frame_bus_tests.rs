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

#[derive(serde::Deserialize)]
struct CpuCase {
    song: SourceSong,
    confirmation: bool,
    frame: usize,
    cycles: u32,
    trace: Vec<(u16, u8)>,
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
    let mut total_accesses = 0;
    for (case_index, case) in bus.cases.iter().enumerate() {
        let source = cpu
            .cases
            .iter()
            .find(|source| {
                (source.song, source.confirmation, source.frame)
                    == (case.song, case.confirmation, case.frame)
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
    assert_eq!(total_accesses, 10101);
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
