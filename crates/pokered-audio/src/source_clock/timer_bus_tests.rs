use super::*;
use alloc::vec::Vec;

#[derive(serde::Deserialize)]
struct EffectCase {
    bank: u8,
    input: ChannelEffectWorkInput,
    trace: Vec<(u16, u8)>,
    writes: Vec<(usize, u16, u16, u8)>,
}
#[derive(serde::Deserialize)]
struct FrameCase {
    context: TimerFrameContext,
    trace: Vec<(u16, u8)>,
    writes: Vec<(usize, u16, u16, u8)>,
}
#[derive(serde::Deserialize)]
struct Fixture {
    rom_sha256: alloc::string::String,
    pret_revision: alloc::string::String,
    effects: Vec<EffectCase>,
    frames: Vec<FrameCase>,
}
fn fixture() -> Fixture {
    let fixture: Fixture = serde_json::from_str(include_str!("timer_bus_golden.json")).unwrap();
    assert_eq!(
        fixture.rom_sha256,
        "5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b"
    );
    assert_eq!(
        fixture.pret_revision,
        "fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c"
    );
    fixture
}
fn resume<T: serde::Serialize + serde::de::DeserializeOwned>(work: &T) -> T {
    serde_json::from_str(&serde_json::to_string(work).unwrap()).unwrap()
}
fn expected_write(writes: &[(usize, u16, u16, u8)], index: usize, pc: u16) -> Option<ApuWrite> {
    let mut matching = writes.iter().filter(|write| write.0 == index);
    let write = matching.next().map(|&(_, store_pc, address, value)| {
        assert_eq!(store_pc, pc, "fixture store belongs to this instruction");
        ApuWrite { address, value }
    });
    assert!(matching.next().is_none());
    write
}

#[test]
fn effect_stores_match_rom_values_addresses_and_instruction_positions_after_resume() {
    let fixture = fixture();
    assert_eq!(fixture.effects.len(), 492);
    let mut output_addresses = [false; 4];
    for (case_index, case) in fixture.effects.into_iter().enumerate() {
        let mut work = EffectBusInstructions::new(case.bank, case.input).unwrap();
        // The public method on the pre-existing cursor is the integration API;
        // exercise it directly as well as the explicitly banked convenience type.
        let mut direct = EffectInstructionWork::new(case.input).unwrap();
        let mut cycles = 0;
        let mut writes = Vec::new();
        for (index, &(pc, expected_cycles)) in case.trace.iter().enumerate() {
            let expected = expected_write(&case.writes, index, pc);
            let frozen = work;
            let frozen_direct = direct;
            for _ in 0..2 {
                assert_eq!(
                    work.try_step(|actual, write| {
                        assert_eq!((actual, write), (expected_cycles, expected));
                        Err("no clock/write capacity")
                    }),
                    Err("no clock/write capacity")
                );
                assert_eq!(
                    direct.try_step_with_apu(|actual, write| {
                        assert_eq!((actual, write), (expected_cycles, expected));
                        Err("no clock/write capacity")
                    }),
                    Err("no clock/write capacity")
                );
                assert_eq!(work, frozen);
                assert_eq!(direct, frozen_direct);
            }
            work = resume(&work);
            direct = resume(&direct);
            let done = work
                .try_step(|actual, write| {
                    assert_eq!(actual, expected_cycles, "case {case_index} PC {pc:04x}");
                    assert_eq!(write, expected, "case {case_index} PC {pc:04x}");
                    if let Some(write) = write {
                        writes.push((index, pc, write.address, write.value));
                        let address_index = [0xff13, 0xff18, 0xff1d, 0xff22]
                            .iter()
                            .position(|&address| address == write.address)
                            .unwrap();
                        output_addresses[address_index] = true;
                    }
                    cycles += u32::from(actual);
                    Ok::<(), ()>(())
                })
                .unwrap();
            assert_eq!(
                direct
                    .try_step_with_apu(|actual, write| {
                        assert_eq!((actual, write), (expected_cycles, expected));
                        Ok::<(), ()>(())
                    })
                    .unwrap(),
                done
            );
            assert_eq!(done, index + 1 == case.trace.len());
            assert_eq!(work.cycles(), cycles);
            assert_eq!(work.bank(), case.bank);
        }
        assert_eq!(writes, case.writes, "case {case_index}");
        assert!(work.finished());
        assert!(direct.finished());
        assert!(work
            .try_step(|_, _| -> Result<(), ()> { panic!("completed cursor must not execute") })
            .unwrap());
        assert!(direct
            .try_step_with_apu(|_, _| -> Result<(), ()> {
                panic!("completed cursor must not execute")
            })
            .unwrap());
    }
    assert_eq!(output_addresses, [true; 4]);
}

#[test]
fn full_timer_stores_match_rom_and_refusals_retain_clock_and_writes() {
    let fixture = fixture();
    assert_eq!(fixture.frames.len(), 72);
    for (case_index, case) in fixture.frames.into_iter().enumerate() {
        let mut work = TimerFrameBusInstructions::new(case.context).unwrap();
        let mut cycles = 0;
        let mut writes = Vec::new();
        for (index, &(pc, expected_cycles)) in case.trace.iter().enumerate() {
            let expected = expected_write(&case.writes, index, pc);
            let frozen = work.clone();
            for _ in 0..2 {
                assert_eq!(
                    work.try_step(|actual, write| {
                        assert_eq!((actual, write), (expected_cycles, expected));
                        Err("no clock/write capacity")
                    }),
                    Err("no clock/write capacity")
                );
                assert_eq!(work, frozen);
                assert_eq!(work.cycles(), cycles);
            }
            work = resume(&work);
            let done = work
                .try_step(|actual, write| {
                    assert_eq!(actual, expected_cycles, "case {case_index} PC {pc:04x}");
                    assert_eq!(write, expected, "case {case_index} PC {pc:04x}");
                    cycles += u32::from(actual);
                    if let Some(write) = write {
                        writes.push((index, pc, write.address, write.value));
                    }
                    Ok::<(), ()>(())
                })
                .unwrap();
            assert_eq!(done, index + 1 == case.trace.len());
            assert_eq!(work.cycles(), cycles);
            assert_eq!(*work.context(), case.context);
        }
        assert_eq!(writes, case.writes, "case {case_index}");
        assert!(work.finished());
        assert!(work
            .try_step(|_, _| -> Result<(), ()> { panic!("completed cursor must not execute") })
            .unwrap());
    }
}

#[test]
fn missing_inconsistent_and_unsupported_context_is_rejected_before_execution() {
    let fixture = fixture();
    let input = fixture.effects[0].input;
    for bank in [0, 8, 255] {
        assert!(EffectBusInstructions::new(bank, input).is_none());
    }
    for bank in [2, 31] {
        let mut invalid = input;
        invalid.channel = 8;
        assert!(EffectBusInstructions::new(bank, invalid).is_none());
        invalid = input;
        invalid.note_delay = 1;
        assert!(EffectBusInstructions::new(bank, invalid).is_none());
        for flags1 in [16, 64] {
            invalid = input;
            invalid.flags1 = flags1;
            assert!(EffectBusInstructions::new(bank, invalid).is_none());
        }
    }
    let context = fixture
        .frames
        .iter()
        .find(|case| case.context.sound_ids.iter().all(|&id| id != 0))
        .unwrap()
        .context;
    for bank in [0, 8, 255] {
        assert!(TimerFrameBusInstructions::new(TimerFrameContext { bank, ..context }).is_err());
    }
    assert!(TimerFrameBusInstructions::new(TimerFrameContext { mute: 1, ..context }).is_err());
    for channel in 0..8 {
        let mut invalid = context;
        invalid.effects[channel] = None;
        assert!(TimerFrameBusInstructions::new(invalid).is_err());
        invalid = context;
        invalid.sound_ids[channel] = 0;
        assert!(TimerFrameBusInstructions::new(invalid).is_err());
        invalid = context;
        invalid.effects[channel].as_mut().unwrap().channel = (channel as u8 + 1) % 8;
        assert!(TimerFrameBusInstructions::new(invalid).is_err());
        for delay in [0, 1] {
            invalid = context;
            invalid.effects[channel].as_mut().unwrap().note_delay = delay;
            assert!(TimerFrameBusInstructions::new(invalid).is_err());
        }
    }
    for channel in 0..4 {
        let mut invalid = context;
        invalid.effects[channel].as_mut().unwrap().paired_sfx_active = false;
        assert!(TimerFrameBusInstructions::new(invalid).is_err());
        invalid = context;
        invalid.effects[channel + 4] = None;
        invalid.sound_ids[channel + 4] = 0;
        assert!(TimerFrameBusInstructions::new(invalid).is_err());
    }
}
