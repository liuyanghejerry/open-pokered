use super::super::apu_bus::ApuAccess;
use super::*;
use alloc::{string::String, vec::Vec};

#[derive(serde::Deserialize)]
struct InitCase {
    bank: u8,
    input: InitPitchSlideInput,
    trace: Vec<(u16, u8)>,
    result: InitPitchSlideResult,
}
#[derive(serde::Deserialize)]
struct ApplyCase {
    bank: u8,
    channel: u8,
    state: PitchSlideState,
    flags1: u8,
    trace: Vec<(u16, u8)>,
    result: PitchSlideResult,
    accesses: Vec<(usize, u16, String, u16, u8, u8)>,
}
#[derive(serde::Deserialize)]
struct Fixture {
    init: Vec<InitCase>,
    apply: Vec<ApplyCase>,
    onset: Vec<OnsetCase>,
}
#[derive(serde::Deserialize)]
struct OnsetCase {
    bank: u8,
    channel: u8,
    note: u8,
    octave: u8,
    perfect: bool,
    paired: bool,
    note_delay: u8,
    length_modifier: u8,
    target: u16,
    pitch_base: u16,
    state: PitchSlideState,
    flags1: u8,
    saved_low: u8,
    returned_frequency: u16,
    trace: Vec<(u16, u8)>,
    accesses: Vec<(usize, u16, String, u16, u8, u8)>,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("pitch_slide_golden.json")).unwrap()
}
fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> T {
    serde_json::from_slice(&serde_json::to_vec(value).unwrap()).unwrap()
}

#[test]
fn initialization_matches_original_division_and_return_at_every_instruction() {
    let cases = fixture().init;
    assert_eq!(cases.len(), 48);
    for case in cases {
        let mut work = InitPitchSlideInstructions::new(case.input).unwrap();
        for (index, &(pc, t)) in case.trace.iter().enumerate() {
            let before = work;
            assert_eq!(work.try_step(|_| Err("budget")), Err("budget"));
            assert_eq!(work, before);
            work = round_trip(&work);
            let done = work
                .try_step(|actual| {
                    assert_eq!(
                        actual, t,
                        "bank{} input{:?} step{index} pc{pc:04x}",
                        case.bank, case.input
                    );
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(done, index + 1 == case.trace.len());
        }
        assert_eq!(work.result(), Some(case.result));
        assert_eq!(
            work.cycles(),
            case.trace.iter().map(|(_, t)| u32::from(*t)).sum::<u32>()
        );
        assert_eq!(
            work.try_step(|_| -> Result<(), ()> { panic!("replayed RET") }),
            Ok(true)
        );
    }
}

#[test]
fn continuation_matches_original_target_branches_and_apu_access_cycles() {
    let cases = fixture().apply;
    assert_eq!(cases.len(), 72);
    for case in cases {
        let mut work = PitchSlideInstructions::new(case.channel, case.state, case.flags1).unwrap();
        let mut events = Vec::new();
        for (index, &(pc, t)) in case.trace.iter().enumerate() {
            let before = work;
            assert_eq!(work.try_step_with_bus(|_| Err("budget")), Err("budget"));
            assert_eq!(work, before);
            work = round_trip(&work);
            let done = work
                .try_step_with_bus(|instruction| {
                    assert_eq!(
                        instruction.tcycles, t,
                        "bank{} state{:?} flags{} step{index} pc{pc:04x}",
                        case.bank, case.state, case.flags1
                    );
                    match instruction.access {
                        Some(ApuAccess::Write {
                            address,
                            value,
                            at_tcycle,
                        }) => events.push((
                            index,
                            pc,
                            String::from("Write"),
                            address,
                            value,
                            at_tcycle,
                        )),
                        None => {}
                        _ => panic!("unexpected read"),
                    }
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(done, index + 1 == case.trace.len());
        }
        assert_eq!(events, case.accesses);
        assert_eq!(work.result(), Some(case.result));
        assert_eq!(
            work.cycles(),
            case.trace.iter().map(|(_, t)| u32::from(*t)).sum::<u32>()
        );
        assert_eq!(
            work.try_step_with_bus(|_| -> Result<(), ()> { panic!("replayed RET") }),
            Ok(true)
        );
    }
}

#[test]
fn zero_source_divisor_and_non_tonal_contexts_are_refused() {
    let input = InitPitchSlideInput {
        channel: 0,
        current_frequency: 0x500,
        target_frequency: 0x600,
        note_delay: 6,
        length_modifier: 6,
        flags1: 16,
    };
    assert!(InitPitchSlideInstructions::new(input).is_none());
    assert!(InitPitchSlideInstructions::new(InitPitchSlideInput {
        length_modifier: 7,
        ..input
    })
    .is_some());
    assert!(InitPitchSlideInstructions::new(InitPitchSlideInput {
        length_modifier: 0,
        flags1: 0,
        ..input
    })
    .is_none());
    assert!(InitPitchSlideInstructions::new(InitPitchSlideInput {
        length_modifier: 0,
        channel: 3,
        ..input
    })
    .is_none());
    let state = fixture().apply.remove(0).state;
    assert!(PitchSlideInstructions::new(3, state, 16).is_none());
    assert!(PitchSlideInstructions::new(0, state, 0).is_none());
}

#[test]
fn sliding_note_onsets_initialize_before_sfx_gate_and_match_all_original_accesses() {
    use super::super::apu_bus::{BusStepError, NoteBusInput};
    use super::super::note_pitch_instructions::{NotePitchInput, NotePitchInstructions};
    let cases = fixture().onset;
    assert_eq!(cases.len(), 192);
    for case in cases {
        let input = NotePitchInput {
            bank: case.bank,
            channel: case.channel,
            note: case.note << 4,
            octave: case.octave,
            flags1: 16 | u8::from(case.perfect),
            paired_sfx_active: case.paired,
            source_sfx_id: if case.paired && case.channel == 0 {
                144
            } else {
                0
            },
            frequency_modifier: 0,
            pitch_base: Some(case.pitch_base),
            previous_frequency_low: 77,
        };
        let bus = NoteBusInput {
            note_delay: case.note_delay,
            volume: if case.channel == 2 { 0x40 } else { 0x92 },
            duty: 0x80,
            panning: 255,
            wave: Some(super::super::wave_patterns::for_bank(case.bank).unwrap()[0]),
        };
        assert!(NotePitchInstructions::new(input).is_none());
        let mut work = NotePitchInstructions::with_pitch_slide(
            input,
            case.note_delay,
            case.length_modifier,
            case.target,
        )
        .unwrap();
        let mut nr51 = 0;
        let mut accesses = Vec::new();
        for (index, &(pc, t)) in case.trace.iter().enumerate() {
            let before = work.clone();
            assert_eq!(
                work.try_step_with_bus(bus, |_| Err("budget")),
                Err(BusStepError::Execute("budget"))
            );
            assert_eq!(work, before);
            work = round_trip(&work);
            let done = work
                .try_step_with_bus(bus, |instruction| {
                    assert_eq!(
                        instruction.tcycles, t,
                        "bank{} ch{} note{} perfect{} paired{} index{index} pc{pc:04x}",
                        case.bank, case.channel, case.note, case.perfect, case.paired
                    );
                    match instruction.access {
                        Some(ApuAccess::Read { address, at_tcycle }) => {
                            assert_eq!(address, 0xff25);
                            accesses.push((
                                index,
                                pc,
                                String::from("Read"),
                                address,
                                nr51,
                                at_tcycle,
                            ));
                            Ok::<_, ()>(Some(nr51))
                        }
                        Some(ApuAccess::Write {
                            address,
                            value,
                            at_tcycle,
                        }) => {
                            accesses.push((
                                index,
                                pc,
                                String::from("Write"),
                                address,
                                value,
                                at_tcycle,
                            ));
                            if address == 0xff25 {
                                nr51 = value;
                            }
                            Ok(None)
                        }
                        None => Ok(None),
                    }
                })
                .unwrap();
            assert_eq!(done, index + 1 == case.trace.len());
        }
        let result = work.result().unwrap();
        let slide = result
            .pitch_slide
            .expect("paired notes still initialize the slide");
        assert_eq!(slide.state, case.state);
        assert_eq!(slide.flags1, case.flags1);
        assert_eq!(result.saved_frequency_low, case.saved_low);
        if case.paired {
            assert_eq!(slide.returned_frequency, case.returned_frequency);
            assert_eq!(result.wave_return_frequency, None);
        } else {
            assert_eq!(result.wave_return_frequency, Some(case.returned_frequency));
        }
        assert_eq!(accesses, case.accesses);
        assert_eq!(
            work.cycles(),
            case.trace.iter().map(|(_, t)| u32::from(*t)).sum::<u32>()
        );
        assert_eq!(
            work.try_step_with_bus(bus, |_| -> Result<Option<u8>, ()> {
                panic!("replayed onset")
            }),
            Ok(true)
        );
    }
}
